use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

use harness_storage::{context_store, messages, sessions, HarnessDb, Settings};
use strands_core::conversation::SummarizingConversationManager;
use strands_core::model::Model;
use strands_core::types::content::ContentBlock;
use strands_core::types::streaming::{DeltaContent, StreamEvent as CoreStream};
use strands_core::Message;
use strands_core::{Agent, CallbackHandler};
use strands_claude_cli::ClaudeCliModel;
use strands_ollama::OllamaModel;
use strands_openrouter::OpenRouterModel;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::agent_registry::{AgentConfig, Provider};
use crate::context::{xml_envelope, Intent, IntentSource};
use crate::context_agent::{self, ContextRequest};
use crate::pipeline::{coalesce_batch, events::*, XmlUnwrap};
use harness_tools::{Calculator, GetTime, HttpFetch, ReadFile};

/// How many prior messages we load from harness-storage to seed the
/// agent. Beyond ~`SUMMARIZE_TRIGGER`, the strands
/// `SummarizingConversationManager` collapses older turns into a
/// summary message; below that, the agent simply sees them verbatim.
/// Refresh the context cards (and re-classify intent) after this many
/// consecutive non-refresh turns. The drift check exists because most
/// turns are `expand` and don't shift the goal — re-running the
/// context agent every turn is wasted spend.
const DRIFT_INTERVAL: u32 = 5;
const HISTORY_LOAD: usize = 200;
/// Total message count that triggers summarization on the agent side.
const SUMMARIZE_TRIGGER: usize = 40;
/// How many recent messages always stay verbatim.
const PRESERVE_RECENT: usize = 12;
/// Fraction of older messages to summarize when triggered.
const SUMMARY_RATIO: f32 = 0.4;
const COALESCE_INTERVAL: Duration = Duration::from_millis(16);
const THINKING_DEADLINE: Duration = Duration::from_secs(1);

pub struct ChatRunOutcome {
    pub session_id: String,
}

/// Build a fresh `Arc<dyn Model>` for the given provider + settings.
/// Used at every stage of the per-turn pipeline (anchor agent, intent
/// agent, summariser, primary agent) — strands models are stateless
/// thin wrappers, so creating multiple instances is cheap.
fn build_model(
    agent: &AgentConfig,
    settings: &Settings,
) -> Result<Arc<dyn Model>, String> {
    match agent.provider {
        Provider::Ollama => {
            let m = OllamaModel::new(agent.model_id.clone())
                .with_host(settings.ollama_host.clone());
            Ok(Arc::new(m))
        }
        Provider::OpenRouter => {
            let key = settings
                .openrouter_api_key
                .clone()
                .filter(|k| !k.trim().is_empty())
                .ok_or_else(|| "OpenRouter API key not configured".to_string())?;
            let mut m = OpenRouterModel::new(agent.model_id.clone(), key);
            if let Some(r) = settings.openrouter_referrer.clone() {
                m = m.with_referrer(r);
            }
            if let Some(t) = settings.openrouter_app_title.clone() {
                m = m.with_app_title(t);
            }
            Ok(Arc::new(m))
        }
        Provider::ClaudeCli => {
            let cwd = dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
            let mut m = ClaudeCliModel::new(agent.model_id.clone())
                .with_cwd(cwd)
                .with_dangerously_skip_permissions(true);
            if let Some(path) = &settings.claude_cli_path {
                m = m.with_command(path.to_string_lossy().to_string());
            }
            Ok(Arc::new(m))
        }
        other => Err(format!("provider {other:?} not yet supported")),
    }
}

/// Bridges strands-core's CallbackHandler to a tokio mpsc channel — the
/// agent ReAct loop fires `on_stream_event` synchronously from inside
/// the model adapter, but we want to consume those events on the same
/// task as the cancel/tick select! loop. The channel is the lowest-
/// friction way to cross that boundary without holding the agent's
/// internal locks.
struct CallbackBridge {
    tx: mpsc::UnboundedSender<CoreStream>,
}

impl CallbackHandler for CallbackBridge {
    fn on_stream_event(&self, event: &CoreStream) {
        let _ = self.tx.send(event.clone());
    }
}

/// Drive one chat turn against the agent's ReAct loop, persist user +
/// assistant messages, and stream typed `StreamEvent`s into `emit` as
/// they arrive.
pub async fn run_chat(
    db: Arc<HarnessDb>,
    settings: Settings,
    agent: AgentConfig,
    prompt: String,
    session_id: Option<String>,
    intent_override: Option<Intent>,
    cancel: CancellationToken,
    emit: impl Fn(StreamEvent) + Send + Sync + 'static,
) -> ChatRunOutcome {
    let emit_err_done = |emit: &dyn Fn(StreamEvent), msg: String| {
        emit(StreamEvent::Error { message: msg });
        emit(StreamEvent::Done {
            stop_reason: StopReason::Error,
            usage: Usage::default(),
        });
    };

    // Resolve / create session.
    let session_id = match session_id {
        Some(id) => id,
        None => match sessions::create(&db, &make_title(&prompt), &agent.id).await {
            Ok(s) => {
                let id = s.id.id.to_string();
                emit(StreamEvent::SessionStarted {
                    session_id: id.clone(),
                });
                id
            }
            Err(e) => {
                emit_err_done(&emit, format!("create session: {e}"));
                return ChatRunOutcome {
                    session_id: String::new(),
                };
            }
        },
    };

    if let Err(e) =
        messages::append(&db, &session_id, "user", &prompt, vec![], Some(&agent.id)).await
    {
        emit_err_done(&emit, format!("save user message: {e}"));
        return ChatRunOutcome { session_id };
    }

    // Conversation history (sliding window, oldest-first) + new prompt.
    // Sliding window includes the user message we just persisted.
    // Drop the tail (it'll be re-appended by Agent::prompt) so we
    // don't double-send the latest user turn.
    let history =
        match harness_storage::memory::sliding_window(&db, &session_id, HISTORY_LOAD).await {
            Ok(h) => h,
            Err(e) => {
                emit_err_done(&emit, format!("load history: {e}"));
                return ChatRunOutcome { session_id };
            }
        };
    let mut conv: Vec<Message> = history
        .iter()
        .map(|m| match m.role.as_str() {
            "assistant" => Message::assistant(vec![ContentBlock::Text {
                text: m.content.clone(),
            }]),
            _ => Message::user(m.content.clone()),
        })
        .collect();
    if matches!(
        conv.last(),
        Some(m) if matches!(m.role, strands_core::types::message::Role::User) && m.text() == prompt
    ) {
        conv.pop();
    }
    let prompt_for_agent = prompt.clone();

    // ---- Lazy context refresh + intent ----
    //
    // The combined context_agent emits both the refreshed cards AND
    // the intent classification in a single model call. We only run
    // it when:
    //   - this session has no prior context yet, OR
    //   - the user picked Redirect from the composer dropdown, OR
    //   - DRIFT_INTERVAL turns have passed since the last refresh.
    //
    // Steady-state turns reuse the existing cards and default the
    // intent to whatever the override is (or Expand). One model call
    // per turn instead of three.
    let prior_context = context_store::load(&db, &session_id)
        .await
        .unwrap_or_default();

    let needs_refresh = prior_context.is_empty()
        || matches!(intent_override, Some(Intent::Redirect))
        || prior_context.turns_since_refresh >= DRIFT_INTERVAL;

    let (mut conv_context, agent_intent) = if needs_refresh {
        emit(StreamEvent::ContextStarted);
        let outcome = match build_model(&agent, &settings) {
            Ok(small_model) => {
                let req = ContextRequest {
                    model: small_model,
                    prior: &prior_context,
                    history: &history,
                    user_prompt: &prompt,
                    cancel: cancel.clone(),
                };
                context_agent::refresh(req).await
            }
            Err(e) => {
                tracing::warn!("context agent skipped: {e}");
                crate::context_agent::ContextOutcome {
                    context: prior_context.clone(),
                    intent: Intent::Expand,
                }
            }
        };

        if cancel.is_cancelled() {
            emit(StreamEvent::Cancelled);
            return ChatRunOutcome { session_id };
        }

        let mut new_ctx = outcome.context;
        new_ctx.turns_since_refresh = 0;
        if let Err(e) = context_store::save(&db, &session_id, &new_ctx).await {
            tracing::warn!("save context: {e}");
        }

        if let Some(anchor) = &new_ctx.anchor {
            emit(StreamEvent::ContextAnchor {
                text: anchor.clone(),
            });
        }
        for p in &new_ctx.priorities {
            emit(StreamEvent::ContextPriority {
                id: p.id.clone(),
                text: p.text.clone(),
                edited_by_user: p.edited_by_user,
            });
        }
        for a in &new_ctx.asides {
            emit(StreamEvent::ContextAside {
                id: a.id.clone(),
                text: a.text.clone(),
                edited_by_user: a.edited_by_user,
            });
        }
        emit(StreamEvent::ContextDone);

        (new_ctx, Some(outcome.intent))
    } else {
        // Steady-state turn: bump the counter and reuse cards as-is.
        let mut new_ctx = prior_context.clone();
        new_ctx.turns_since_refresh = new_ctx.turns_since_refresh.saturating_add(1);
        if let Err(e) = context_store::save(&db, &session_id, &new_ctx).await {
            tracing::warn!("bump turns counter: {e}");
        }
        (new_ctx, None)
    };

    let (intent, intent_source) = match intent_override {
        Some(i) => (i, IntentSource::Manual),
        None => (
            agent_intent.unwrap_or(Intent::Expand),
            IntentSource::Auto,
        ),
    };
    emit(StreamEvent::IntentClassified {
        intent: intent.as_str().to_string(),
        source: match intent_source {
            IntentSource::Auto => "auto".into(),
            IntentSource::Manual => "manual".into(),
        },
    });

    // The context counter increment write was the only mutation on
    // steady-state turns; the conv_context value is otherwise reused.
    let _ = &mut conv_context;
    let context_envelope = xml_envelope(&conv_context, Some((intent, intent_source)));

    // Build the agent for this turn, with tools constructed from
    // current Settings so allowlist / sandbox-root changes take effect
    // on the next message without a restart. The system prompt is
    // prefixed with the multi-agent <context>...</context> + <intent>
    // envelope so the main model knows the user's anchor / priorities /
    // asides and how to weight the latest message.
    let (tx, rx) = mpsc::unbounded_channel::<CoreStream>();

    fn with_tools(
        b: strands_core::AgentBuilder,
        settings: &Settings,
        system: String,
    ) -> strands_core::AgentBuilder {
        b.system_prompt(system)
            .tool(GetTime)
            .tool(Calculator)
            .tool(HttpFetch::new(settings.http_fetch_allowlist.clone()))
            .tool(ReadFile::new(settings.read_file_sandbox_root.clone()))
    }

    fn summarizer(model: Arc<dyn Model>) -> SummarizingConversationManager {
        SummarizingConversationManager::new(model)
            .with_window_size(SUMMARIZE_TRIGGER)
            .with_preserve_recent(PRESERVE_RECENT)
            .with_summary_ratio(SUMMARY_RATIO)
    }

    let builder_result = match agent.provider {
        Provider::Ollama => {
            // Two instances: one boxed inside the agent for chat
            // streaming, one Arc'd for the conversation manager's
            // summary calls. Both are stateless wrappers over reqwest.
            let primary = OllamaModel::new(agent.model_id.clone())
                .with_host(settings.ollama_host.clone());
            let summary_model: Arc<dyn Model> = Arc::new(
                OllamaModel::new(agent.model_id.clone())
                    .with_host(settings.ollama_host.clone()),
            );
            with_tools(
                Agent::builder()
                    .model(primary)
                    .callback_handler(CallbackBridge { tx })
                    .conversation_manager(summarizer(summary_model))
                    .max_cycles(20),
                &settings,
                context_envelope.clone(),
            )
            .build()
        }
        Provider::OpenRouter => {
            let key = match settings.openrouter_api_key.clone() {
                Some(k) if !k.trim().is_empty() => k,
                _ => {
                    emit_err_done(&emit, "OpenRouter API key not configured".into());
                    return ChatRunOutcome { session_id };
                }
            };
            let build_or = |k: String| {
                let mut m = OpenRouterModel::new(agent.model_id.clone(), k);
                if let Some(r) = settings.openrouter_referrer.clone() {
                    m = m.with_referrer(r);
                }
                if let Some(t) = settings.openrouter_app_title.clone() {
                    m = m.with_app_title(t);
                }
                m
            };
            let primary = build_or(key.clone());
            let summary_model: Arc<dyn Model> = Arc::new(build_or(key));
            with_tools(
                Agent::builder()
                    .model(primary)
                    .callback_handler(CallbackBridge { tx })
                    .conversation_manager(summarizer(summary_model))
                    .max_cycles(20),
                &settings,
                context_envelope.clone(),
            )
            .build()
        }
        Provider::ClaudeCli => {
            // The CLI subprocess inherits cwd from harness's Tauri
            // process; for now we pin it to the user's home so the CLI
            // doesn't try to discover CLAUDE.md / plugins from
            // wherever harness happens to be run from.
            //
            // `--dangerously-skip-permissions` is on so the spawned
            // CLI doesn't block waiting for permission prompts that
            // can't be answered through harness's Tauri UI.
            let cwd = dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
            let cli_path = settings
                .claude_cli_path
                .clone()
                .map(|p| p.to_string_lossy().to_string());
            let mut primary = ClaudeCliModel::new(agent.model_id.clone())
                .with_cwd(cwd.clone())
                .with_dangerously_skip_permissions(true);
            if let Some(cmd) = &cli_path {
                primary = primary.with_command(cmd.clone());
            }
            let mut summary_built = ClaudeCliModel::new(agent.model_id.clone())
                .with_cwd(cwd)
                .with_dangerously_skip_permissions(true);
            if let Some(cmd) = &cli_path {
                summary_built = summary_built.with_command(cmd.clone());
            }
            let summary_model: Arc<dyn Model> = Arc::new(summary_built);
            with_tools(
                Agent::builder()
                    .model(primary)
                    .callback_handler(CallbackBridge { tx })
                    .conversation_manager(summarizer(summary_model))
                    .max_cycles(20),
                &settings,
                context_envelope.clone(),
            )
            .build()
        }
        _ => {
            emit_err_done(
                &emit,
                format!("provider {:?} not yet supported", agent.provider),
            );
            return ChatRunOutcome { session_id };
        }
    };

    let mut agent_inst = match builder_result {
        Ok(a) => a,
        Err(e) => {
            emit_err_done(&emit, format!("build agent: {e}"));
            return ChatRunOutcome { session_id };
        }
    };
    let cancel_handle = agent_inst.cancel_handle();
    let _ = builder_result; // keep the moved-out variable name out of warnings

    // Seed the agent with prior history from harness-storage before
    // prompting. Agent::prompt will append the new user turn on top of
    // these seed messages.
    agent_inst.set_messages(conv);

    // Spawn the agent on its own task so we can race cancel + tick
    // against the prompt completion future.
    let prompt_future = tokio::spawn(async move {
        let res = agent_inst.prompt(&prompt_for_agent).await;
        (agent_inst, res)
    });

    let mut unwrap = XmlUnwrap::new();
    let mut buffer: Vec<StreamEvent> = Vec::new();
    let mut full_assistant = String::new();
    let mut last_token = tokio::time::Instant::now();
    let mut thinking = false;
    let mut tick = tokio::time::interval(COALESCE_INTERVAL);
    let mut stop_reason = StopReason::EndTurn;
    let mut usage = Usage::default();
    let mut cancelled = false;
    let mut rx = rx;
    let mut prompt_handle = prompt_future;

    enum LoopExit {
        Cancelled,
        Done,
    }
    let exit = loop {
        tokio::select! {
            biased;
            _ = cancel.cancelled() => {
                cancelled = true;
                cancel_handle.store(true, Ordering::Relaxed);
                break LoopExit::Cancelled;
            }
            _ = tick.tick() => {
                if !buffer.is_empty() {
                    let merged = coalesce_batch(std::mem::take(&mut buffer));
                    for e in merged { emit(e); }
                }
                let idle = tokio::time::Instant::now().duration_since(last_token);
                if !thinking && idle >= THINKING_DEADLINE {
                    thinking = true;
                    emit(StreamEvent::Thinking { active: true });
                }
            }
            recv = rx.recv() => {
                match recv {
                    Some(CoreStream::ContentBlockDelta {
                        delta: DeltaContent::TextDelta(t),
                        ..
                    }) => {
                        full_assistant.push_str(&t);
                        unwrap.push(&t, &mut buffer);
                        last_token = tokio::time::Instant::now();
                        if thinking {
                            thinking = false;
                            emit(StreamEvent::Thinking { active: false });
                        }
                    }
                    Some(CoreStream::ContentBlockStart {
                        content_type:
                            strands_core::types::streaming::ContentBlockType::ToolUse {
                                tool_use_id, name,
                            },
                        ..
                    }) => {
                        buffer.push(StreamEvent::ToolUse { name, id: tool_use_id });
                    }
                    Some(CoreStream::Metadata { usage: u }) => {
                        usage = u.into();
                    }
                    Some(CoreStream::MessageStop { stop_reason: sr }) => {
                        stop_reason = sr.into();
                    }
                    Some(_) => {}
                    None => {
                        // Channel closed — keep selecting; the prompt
                        // future arm will fire next.
                    }
                }
            }
            done = (&mut prompt_handle) => {
                match done {
                    Ok((_agent, Ok(result))) => {
                        stop_reason = result.stop_reason.into();
                        usage = result.usage.into();
                    }
                    Ok((_agent, Err(e))) => {
                        emit(StreamEvent::Error { message: format!("agent: {e}") });
                        stop_reason = StopReason::Error;
                    }
                    Err(e) => {
                        emit(StreamEvent::Error { message: format!("task panic: {e}") });
                        stop_reason = StopReason::Error;
                    }
                }
                while let Ok(evt) = rx.try_recv() {
                    handle_residual(evt, &mut buffer, &mut unwrap, &mut full_assistant);
                }
                break LoopExit::Done;
            }
        }
    };

    if matches!(exit, LoopExit::Cancelled) {
        // Let the prompt task drain so it won't leak / panic on its own.
        match prompt_handle.await {
            Ok(_) => {}
            Err(e) => tracing::warn!("prompt task on cancel: {e}"),
        }
        while let Ok(evt) = rx.try_recv() {
            handle_residual(evt, &mut buffer, &mut unwrap, &mut full_assistant);
        }
    }

    unwrap.flush(&mut buffer);
    if !buffer.is_empty() {
        let merged = coalesce_batch(std::mem::take(&mut buffer));
        for e in merged {
            emit(e);
        }
    }
    if thinking {
        emit(StreamEvent::Thinking { active: false });
    }

    if !full_assistant.is_empty() {
        if let Err(e) = messages::append(
            &db,
            &session_id,
            "assistant",
            &full_assistant,
            vec![],
            Some(&agent.id),
        )
        .await
        {
            tracing::warn!("save assistant message: {e}");
        }
        let new_count = messages::count_for_session(&db, &session_id)
            .await
            .unwrap_or(0);
        let _ = sessions::touch(&db, &session_id, new_count).await;
    }

    if cancelled {
        emit(StreamEvent::Cancelled);
    } else {
        emit(StreamEvent::Done { stop_reason, usage });
    }

    ChatRunOutcome { session_id }
}

fn handle_residual(
    evt: CoreStream,
    buffer: &mut Vec<StreamEvent>,
    unwrap: &mut XmlUnwrap,
    full: &mut String,
) {
    if let CoreStream::ContentBlockDelta {
        delta: DeltaContent::TextDelta(t),
        ..
    } = evt
    {
        full.push_str(&t);
        unwrap.push(&t, buffer);
    }
}

fn make_title(prompt: &str) -> String {
    let trimmed: String = prompt.chars().take(40).collect();
    if trimmed.chars().count() < prompt.chars().count() {
        format!("{trimmed}…")
    } else {
        trimmed
    }
}
