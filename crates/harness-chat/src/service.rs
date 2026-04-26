use std::sync::Arc;
use std::time::Duration;

use futures::StreamExt;
use harness_storage::{messages, sessions, HarnessDb};
use strands_core::{
    types::content::ContentBlock,
    types::streaming::{DeltaContent, StreamEvent as CoreStream},
    Message,
};
use strands_core::model::Model;
use strands_ollama::OllamaModel;
use tokio_util::sync::CancellationToken;

use crate::agent_registry::{AgentConfig, Provider};
use crate::pipeline::{coalesce_batch, events::*, XmlUnwrap};

const SLIDING_WINDOW: usize = 20;
const COALESCE_INTERVAL: Duration = Duration::from_millis(16);
const THINKING_DEADLINE: Duration = Duration::from_secs(1);

pub struct ChatRunOutcome {
    pub session_id: String,
}

/// Drive one chat turn against the model and persist the user/assistant
/// pair. Streams `StreamEvent`s into `emit` as they're produced.
pub async fn run_chat(
    db: Arc<HarnessDb>,
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

    // Resolve or create the session.
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

    // Open the model stream.
    let stream_result = match agent.provider {
        Provider::Ollama => {
            let model = OllamaModel::new(agent.model_id.clone());
            model.stream(&conv, None, &[]).await
        }
        _ => {
            emit_err_done(
                &emit,
                format!("provider {:?} not yet supported", agent.provider),
            );
            return ChatRunOutcome { session_id };
        }
    };
    let mut stream = match stream_result {
        Ok(s) => s,
        Err(e) => {
            emit_err_done(&emit, format!("model open: {e}"));
            return ChatRunOutcome { session_id };
        }
    };

    let mut unwrap = XmlUnwrap::new();
    let mut buffer: Vec<StreamEvent> = Vec::new();
    let mut full_assistant = String::new();
    let mut last_token = tokio::time::Instant::now();
    let mut thinking = false;
    let mut tick = tokio::time::interval(COALESCE_INTERVAL);
    let mut stop_reason = StopReason::EndTurn;
    let mut usage = Usage::default();
    let mut cancelled = false;

    loop {
        tokio::select! {
            biased;
            _ = cancel.cancelled() => {
                cancelled = true;
                break;
            }
            _ = tick.tick() => {
                if !buffer.is_empty() {
                    let merged = coalesce_batch(std::mem::take(&mut buffer));
                    for e in merged {
                        emit(e);
                    }
                }
                let idle = tokio::time::Instant::now().duration_since(last_token);
                if !thinking && idle >= THINKING_DEADLINE {
                    thinking = true;
                    emit(StreamEvent::Thinking { active: true });
                }
            }
            next = stream.next() => {
                match next {
                    Some(Ok(evt)) => match evt {
                        CoreStream::ContentBlockDelta {
                            delta: DeltaContent::TextDelta(t),
                            ..
                        } => {
                            full_assistant.push_str(&t);
                            unwrap.push(&t, &mut buffer);
                            last_token = tokio::time::Instant::now();
                            if thinking {
                                thinking = false;
                                emit(StreamEvent::Thinking { active: false });
                            }
                        }
                        CoreStream::ContentBlockDelta {
                            delta: DeltaContent::ToolInputDelta(_),
                            ..
                        } => {
                            // Tools come in a later phase.
                        }
                        CoreStream::MessageStop { stop_reason: sr } => {
                            stop_reason = sr.into();
                        }
                        CoreStream::Metadata { usage: u } => {
                            usage = u.into();
                        }
                        _ => {}
                    },
                    Some(Err(e)) => {
                        emit(StreamEvent::Error { message: format!("stream: {e}") });
                        stop_reason = StopReason::Error;
                        break;
                    }
                    None => break,
                }
            }
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
        emit(StreamEvent::Done {
            stop_reason,
            usage,
        });
    }

    ChatRunOutcome { session_id }
}

fn make_title(prompt: &str) -> String {
    let trimmed: String = prompt.chars().take(40).collect();
    if trimmed.chars().count() < prompt.chars().count() {
        format!("{trimmed}…")
    } else {
        trimmed
    }
}
