# Harness — Phase 2: OpenRouter, Settings, Built-in Tools

## Context

Phase 1 (committed `682f879..8032644`) shipped a working v1: Ollama chat with streaming, persistence, and cancel, all through Tauri IPC channels. Direct `strands_core::Model::stream` is used; tools are not yet wired in; OpenRouter shows as a disabled placeholder; there is no Settings UI.

Phase 2 extends that to a chat app that **runs cloud models with secrets** and **calls tools mid-stream**. It deliberately **defers MCP to phase 3** — MCP is a separate concern with its own client library, transport choices, and lifecycle management, and pulling it in alongside the Agent-loop migration would muddy a single plan.

After phase 2 the user can:

- Set an OpenRouter API key in a Settings page; cloud agents become selectable.
- Pick from a curated set of OpenRouter models (free + paid tiers).
- Configure an `http_fetch` host allowlist and a `read_file` sandbox root.
- Watch tool-use chips appear inline as the model invokes built-ins, with cancel still working mid-tool-loop.

## Deferred to phase 3

- MCP client (`rmcp`) + stdio/HTTP transports + Settings UI for managing MCP servers.
- Phase-2-style agent types beyond `agent` (swarm/graph/a2a/distributed).

## Approach

There's one structural change up-front: the chat service moves from calling `strands_core::Model::stream` directly to driving `strands_core::Agent`'s ReAct loop. That's the only sensible way to handle tool-use cycles. To preserve cancellation, we land a tiny upstream change to strands-rs that exposes the agent's internal cancel flag.

Order of work — each step is independently testable and ends in a commit:

1. **strands-rs upstream: `Agent::cancel_handle()`** — exposes `Arc<AtomicBool>` so the harness can flip it from a Tauri command without locking the agent. Single-line accessor + a unit test.
2. **strands-rs upstream: `strands-openrouter` crate** — `OpenRouterModel` implementing `strands_core::Model` against the OpenAI-compatible `/chat/completions` SSE endpoint. Tested with a recorded SSE fixture.
3. **`harness-storage::settings`** — new module persisting a `Settings` struct as a single SurrealDB record (`settings:singleton`). Loaded once at app start, written by the frontend.
4. **Tauri commands `settings_get` / `settings_set`** + frontend `services/settings.ts` + `src/pages/Settings.vue`.
5. **`harness-chat::service` switched to `Agent` ReAct loop** — using `cancel_handle()` for cancellation, a `CallbackHandler` adapter that pushes core `StreamEvent`s through the existing `XmlUnwrap`/`coalesce` pipeline. No tools registered yet — pipeline observability test still passes.
6. **`harness-tools` crate** — `get_time`, `calculator` (via `evalexpr`), `http_fetch` (allowlisted), `read_file` (sandboxed root). Each annotated with `#[tool]` from `strands-macros`. Unit tests per tool, including refusal paths.
7. **Agent registry: OpenRouter agents from settings** — when an API key is set, the registry surfaces a curated list of OpenRouter models (mix of free + paid tiers) as `disabled: false`; otherwise they remain disabled with a "configure API key in Settings" message.
8. **Tool registration in `ChatService`** — the chat service builds the `Agent` with the configured tools per request, reading the allowlist/sandbox root from settings. Tool-use chips already render in the UI; this just lights them up.
9. **End-to-end smoke** — chat with an OpenRouter model, ask it to use `get_time` and `calculator`, watch the chips arrive, cancel a long generation mid-tool-loop.

## Files

**Modified — strands-rs (`../strands-rs/`):**
- `strands-core/src/agent/mod.rs` — add `pub fn cancel_handle(&self) -> Arc<AtomicBool>`.
- `strands-core/src/agent/mod.rs` — single test confirming `cancel_handle().store(true)` causes the next loop iteration to exit early.
- `Cargo.toml` (workspace) — add `strands-openrouter` member.

**New — strands-rs:**
- `strands-openrouter/Cargo.toml`
- `strands-openrouter/src/lib.rs`
- `strands-openrouter/src/client.rs` — `OpenRouterModel` struct, builder methods.
- `strands-openrouter/src/stream.rs` — SSE parser → `StreamEvent` adapter.
- `strands-openrouter/src/types.rs` — OpenAI-compatible request/response wire types.
- `strands-openrouter/tests/sse_fixture.rs` + `strands-openrouter/tests/fixtures/chat_completion_stream.txt`.

**Modified — harness:**
- `Cargo.toml` (workspace) — add `strands-openrouter` workspace dep, add `harness-tools` member.
- `crates/harness-storage/src/lib.rs` — re-export `settings`.
- `crates/harness-storage/src/schema.rs` — add `settings` SCHEMAFULL table.
- `crates/harness-chat/src/service.rs` — replace direct `Model::stream` call with `Agent::builder().model(...).tools(...).callback_handler(...).build()` then `agent.prompt(input)`. Cancel via `cancel_handle().store(true)` triggered from the existing select! arm.
- `crates/harness-chat/src/agent_registry.rs` — `discover_openrouter(api_key)` returns curated list when key is set; placeholders kept when not.
- `src-tauri/src/state.rs` — load `Settings` at boot, expose `settings()` accessor.
- `src-tauri/src/commands/mod.rs` — add `settings` module.
- `src-tauri/src/lib.rs` — register settings commands and the new tools commands.
- `src/services/chat.ts` — no shape change (events unchanged).
- `src/pages/Chat.vue` — small change to surface OpenRouter "configure API key" hint when the only OpenRouter agent is disabled.
- `src/router.ts` — add `/settings` route.
- `src/components/Layout/Header.vue` — add a settings cog link in the right side.

**New — harness:**
- `crates/harness-storage/src/settings.rs` — `Settings` struct, `load`, `save`.
- `src-tauri/src/commands/settings.rs` — `settings_get` / `settings_set` Tauri commands.
- `crates/harness-tools/Cargo.toml`
- `crates/harness-tools/src/lib.rs`
- `crates/harness-tools/src/builtins/mod.rs`
- `crates/harness-tools/src/builtins/get_time.rs`
- `crates/harness-tools/src/builtins/calculator.rs`
- `crates/harness-tools/src/builtins/http_fetch.rs`
- `crates/harness-tools/src/builtins/read_file.rs`
- `src/pages/Settings.vue`
- `src/services/settings.ts`

## Reusable references

- **strands-rs upstream pattern**: `../strands-rs/strands-ollama/src/client.rs` is the canonical template for adding a Model adapter (struct + builder + `Model` impl with SSE-style streaming). Mirror its shape exactly for `strands-openrouter`.
- **Memex Settings analogue**: not directly applicable (Memex stores config differently), but the SurrealDB record-as-singleton pattern is straightforward — `db.upsert(("settings", "singleton"))`.
- **Existing AppState assembly**: `src-tauri/src/state.rs::AppState::build` is where settings load gets injected. `current_agents()` is the natural place to call `discover_openrouter` based on settings.
- **strands-macros `#[tool]`**: see `../strands-rs/docs/` for the macro contract — async fn with doc-commented args, returning `Result<T, StrandsError>` where `T: Serialize`.

## Settings shape

```rust
// crates/harness-storage/src/settings.rs
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Settings {
    pub openrouter_api_key: Option<String>,
    pub openrouter_referrer: Option<String>,
    pub openrouter_app_title: Option<String>,
    pub ollama_host: String,                  // default: "http://localhost:11434"
    pub default_agent_id: Option<String>,
    pub http_fetch_allowlist: Vec<String>,    // host strings, no scheme
    pub read_file_sandbox_root: Option<PathBuf>,
}
```

Stored as one record at `settings:singleton`. The Rust side never logs the API key. The frontend's password-input field never round-trips the existing key in plain text — the form re-fetches the masked indicator on save.

## Curated OpenRouter agents

A static list of ~6 picks, hand-chosen for diversity:
- 2 free tier (e.g. `meta-llama/llama-3.3-70b-instruct:free`, `qwen/qwen-2.5-72b-instruct:free`)
- 2 cheap-and-good (e.g. `anthropic/claude-haiku-4-5`, `openai/gpt-4o-mini`)
- 2 premium (e.g. `anthropic/claude-opus-4-7`, `openai/o3-mini`)

Each entry is an `AgentConfig` with appropriate `cost`, `parameters`, `architecture`. When `Settings::openrouter_api_key.is_none()`, all six render with `disabled: true` and `disabled_message: Some("Configure OpenRouter API key in Settings")`.

## Tool sandboxing

| Tool | When tool refuses | Where the refusal renders |
|---|---|---|
| `get_time` | never | n/a |
| `calculator` | never | n/a |
| `http_fetch` | host not in `http_fetch_allowlist` | `tool_result` chip with `status: error`, message: "host not allowed: <h>" |
| `read_file` | path resolves outside `read_file_sandbox_root`, or root unset | chip with `status: error`, message: "path outside sandbox" or "no sandbox root configured" |

Each tool returns `ToolOutput::error(...)` for refusal so the chip status drives the UI; `Result::Err` is reserved for unexpected failures (network down, file IO error).

## ChatService migration to Agent ReAct loop

The select! loop becomes thinner — `Agent::prompt` is awaited as a single future, and a `CallbackHandler` impl pushes raw `StreamEvent`s into a `tokio::sync::mpsc::UnboundedSender<CoreStream>`. The select! has three arms:

1. `cancel.cancelled()` — flips `agent.cancel_handle().store(true, Ordering::Relaxed)`, breaks.
2. `tick.tick()` — drains the mpsc receiver, runs the existing `XmlUnwrap`/`coalesce` pipeline, emits to the channel callback, manages thinking indicator.
3. `prompt_future` — when the agent finishes, drain whatever is left and emit terminal `Done`.

The mpsc indirection keeps the Tauri channel out of the agent's hot path and lets the coalescer batch as it does today. Conversation history is rebuilt on each turn from `harness_storage::memory::sliding_window` exactly as it is now — no agent-side persistence.

## Verification

After step 9:

1. `cargo test --workspace` — all tests pass (existing 26 + ~10 new across settings/tools/openrouter).
2. `yarn tauri dev` — app boots, Settings link in header.
3. Settings page accepts an OpenRouter key, saves it.
4. Sidebar OpenRouter chips activate (no longer greyed); Ollama chips still work.
5. Pick `claude-haiku-4-5` (or whatever cheap pick), ask "what time is it?" — `get_time` chip appears, then assistant prose.
6. Ask "fetch https://example.com" without configuring the allowlist — error chip appears with "host not allowed".
7. Add `example.com` to the allowlist, retry — success chip + content.
8. Cancel during a tool-using generation — cancellation arrives within ~200ms; the partial assistant turn persists in history without orphaned tool calls.
9. Restart app — sessions still load; settings still loaded.
