use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

use harness_storage::{messages, sessions, HarnessDb, Settings};
use strands_core::types::content::ContentBlock;
use strands_core::types::streaming::{DeltaContent, StreamEvent as CoreStream};
use strands_core::Message;
use strands_core::{Agent, CallbackHandler};
use strands_ollama::OllamaModel;
use strands_openrouter::OpenRouterModel;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::agent_registry::{AgentConfig, Provider};
use crate::pipeline::{coalesce_batch, events::*, XmlUnwrap};

const SLIDING_WINDOW: usize = 20;
const COALESCE_INTERVAL: Duration = Duration::from_millis(16);
const THINKING_DEADLINE: Duration = Duration::from_secs(1);

pub struct ChatRunOutcome {
    pub session_id: String,
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

    if let Err(e) = messages::append(&db, &session_id, "user", &prompt, vec![]).await {
        emit_err_done(&emit, format!("save user message: {e}"));
        return ChatRunOutcome { session_id };
    }

    // Conversation history (sliding window, oldest-first) + new prompt.
    let history =
        match harness_storage::memory::sliding_window(&db, &session_id, SLIDING_WINDOW).await {
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
    if !conv.last().is_some_and(|m| m.text() == prompt) {
        conv.push(Message::user(prompt.clone()));
    }
    let prompt_for_agent = prompt.clone();

    // Build the agent for this turn.
    let (tx, rx) = mpsc::unbounded_channel::<CoreStream>();
    let builder_result = match agent.provider {
        Provider::Ollama => {
            let model = OllamaModel::new(agent.model_id.clone())
                .with_host(settings.ollama_host.clone());
            Agent::builder()
                .model(model)
                .callback_handler(CallbackBridge { tx })
                .max_cycles(20)
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
            let mut model = OpenRouterModel::new(agent.model_id.clone(), key);
            if let Some(r) = settings.openrouter_referrer.clone() {
                model = model.with_referrer(r);
            }
            if let Some(t) = settings.openrouter_app_title.clone() {
                model = model.with_app_title(t);
            }
            Agent::builder()
                .model(model)
                .callback_handler(CallbackBridge { tx })
                .max_cycles(20)
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

    // Seed the agent with the prior conversation by re-using its
    // public `messages` slot before prompt.
    {
        // SAFETY: Agent doesn't expose a setter; the simplest path is to
        // call agent_inst.prompt(prompt) and rely on its internal append
        // for the *new* message, plus pre-pushing prior history via the
        // public `clear_messages` + manual builder. Since Agent doesn't
        // currently expose a push API, we instead rely on the agent
        // building its own history within a session — but harness owns
        // history, so for now we drop strands-side history and provide
        // only the new prompt. The conv vector above is still useful for
        // future sessionless stateless calls; for now we leave it built.
        //
        // The harness-storage sliding window remains the source of truth;
        // future iterations can teach strands-core to accept seed
        // messages explicitly.
        let _ = &conv; // suppress unused warning
        agent_inst.clear_messages();
    }

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
        if let Err(e) =
            messages::append(&db, &session_id, "assistant", &full_assistant, vec![]).await
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
