# Harness — Tauri chat playground (Rust port of ai-playground chat module)

**Status:** design approved (2026-04-26), pending spec self-review and implementation plan.
**Repos touched:** `harness/` (new), `../strands-rs/` (add `strands-openrouter` crate).

## 1. Goal

Build a single-user desktop chat application that lets the user converse with LLM agents via Ollama and OpenRouter. The app reuses the chat UI from `../portfolio` (Vue 3 + Vite + TS) inside a Tauri v2 webview, with a Rust core built on `../strands-rs` for agent orchestration and streaming.

## 2. Scope

### In scope (v1)
- Chat with a single agent at a time (the `agent` ReAct pattern from strands-rs).
- Two model providers: Ollama (local) and OpenRouter (cloud, OpenAI-compatible).
- Streaming responses with token-level updates, tool-use chips, reasoning blocks, and a working cancel button.
- Persistent chat sessions and message history, stored in SurrealDB on disk.
- Built-in tools: `get_time`, `calculator`, `http_fetch` (allowlisted hosts), `read_file` (sandboxed).
- MCP client: connect to user-configured MCP servers (stdio or HTTP) and expose their tools to the agent.
- Settings UI: OpenRouter API key, default agent, MCP server list, http_fetch allowlist, read_file sandbox root.
- Dark mode toggle and theming carried over from portfolio.
- Five agent-type filter chips (`agent`, `swarm`, `graph`, `a2a`, `distributed`); only `agent` returns results in v1, others render as `(0)` placeholders for phase 2.

### Out of scope (v1)
- Knowledge graph (entities, categories, semantic search) — dropped entirely.
- AWS Bedrock, OpenAI direct, Google Vertex providers.
- Async worker / Redis job queue.
- FastAPI HTTP layer or any embedded HTTP server.
- CLI tooling.
- Stream safeguards (`gpt-oss-120b-safeguard` intervention pipeline) — untested in source, dropped.
- PDF / docx / spaCy ingestion pipeline.
- Multi-user, auth, sessions sync across devices.
- Summarising conversation manager (sliding window only in v1).

### Phase 2 (documented, not built)
- `swarm`, `graph`, `a2a`, `distributed` agent types.
- Summarising conversation manager.
- OS-keychain for API keys.
- Cross-query with Memex (`../world/desktop/src-tauri/memex-core`) — both apps use the same SurrealDB engine, so this is a future possibility, not a v1 commitment.

## 3. Architecture overview

```
┌─────────────────────────────────────────────────────────────────────┐
│  Vue 3 + Vite + TS frontend   (ported from ../portfolio/src)        │
│   ChatPage.vue   reuses portfolio's Chat.vue, sidebar, styles       │
│   chat.ts        REWRITTEN: invoke + Tauri Channel<StreamEvent>     │
└─────────────────────────────────────────────────────────────────────┘
                          │  Tauri IPC
                          │  (invoke + Channel<T>)
                          ▼
┌─────────────────────────────────────────────────────────────────────┐
│  src-tauri  (the Tauri binary at project root)                      │
│   commands: chat_send, chat_cancel, list_agents, list_sessions,     │
│             get_history, delete_session, settings_get, settings_set │
│   AppState { chat: Arc<ChatService>, store: Arc<HarnessDb> }        │
└─────────────────────────────────────────────────────────────────────┘
        │              │                │
        ▼              ▼                ▼
   harness-chat   harness-storage   harness-tools
        │              │                │
        └──────┬───────┴────────────────┘
               ▼
   ../strands-rs/  (workspace dependency, path = "../strands-rs/...")
     strands-core         (Agent, Model, Tool, StreamEvent)
     strands-macros       (#[tool])
     strands-ollama       (existing)
     strands-openrouter   (NEW — added by us)
```

**Key properties:**
- **Two-repo change.** Provider implementations live in strands-rs; app logic lives in harness.
- **No HTTP server.** Streaming uses `tauri::ipc::Channel<StreamEvent>` (output-only); cancel uses `invoke("chat_cancel", { channelId })`.
- **SurrealDB at `~/.harness/db`** (RocksDB engine, ns=`harness`, db=`chat`), mirroring the Memex pattern in `../world/desktop/src-tauri/memex-core/src/db.rs`.
- **Tauri v2** required for `Channel<T>` and the v2 `tauri-plugin-store` API.

## 4. Repository layout

`src-tauri/` stays at the root (Tauri CLI default location, no config relocation needed). The pure Rust crates live in `crates/` alongside it. The workspace `Cargo.toml` at the root pulls in both.

```
harness/
├── Cargo.toml                 # workspace: members = ["src-tauri", "crates/*"]
├── package.json               # vue/vite frontend
├── vite.config.ts
├── index.html
├── tsconfig.json
├── tsconfig.node.json
├── docs/superpowers/          # specs and plans
├── public/
├── src/                       # FRONTEND (Vue 3 + Vite + TS)
│   ├── main.ts
│   ├── App.vue                # routes: /, /chat, /settings
│   ├── pages/
│   │   ├── Chat.vue           # ported from portfolio, simplified for IPC
│   │   └── Settings.vue       # NEW
│   ├── components/Layout/     # Header.vue, Sidebar.vue (ported)
│   ├── services/
│   │   ├── chat.ts            # REWRITTEN — Tauri IPC adapter
│   │   └── settings.ts        # NEW — store-plugin wrapper
│   ├── types/chat.types.ts    # ported, extended for StreamEvent
│   └── scss/                  # ported
├── src-tauri/                 # the Tauri binary (unchanged location)
│   ├── Cargo.toml             # workspace member; depends on harness-chat etc.
│   ├── tauri.conf.json
│   ├── build.rs
│   ├── capabilities/
│   ├── icons/
│   └── src/
│       ├── main.rs            # tauri::Builder, plugins, AppState
│       ├── commands/
│       │   ├── chat.rs
│       │   ├── agents.rs
│       │   ├── sessions.rs
│       │   └── settings.rs
│       └── state.rs
└── crates/
    ├── harness-chat/          # PURE — no tauri deps
    │   ├── agent_registry.rs
    │   ├── service.rs
    │   ├── pipeline/
    │   │   ├── mod.rs
    │   │   ├── xml_unwrap.rs
    │   │   ├── coalesce.rs
    │   │   └── events.rs
    │   └── cancel.rs
    │
    ├── harness-storage/       # PURE — surrealdb only
    │   ├── db.rs
    │   ├── schema.rs
    │   ├── sessions.rs
    │   ├── messages.rs
    │   └── memory.rs
    │
    └── harness-tools/         # PURE — no tauri deps
        ├── builtins/
        │   ├── get_time.rs
        │   ├── http_fetch.rs
        │   ├── read_file.rs
        │   └── calculator.rs
        ├── mcp/
        │   ├── client.rs
        │   ├── adapter.rs
        │   └── registry.rs
        └── lib.rs
```

**Note on existing scaffold:** `harness/` was created by `create-tauri-app` (Vue + TS, Tauri v2). The existing `src-tauri/` keeps its location. We add a workspace `Cargo.toml` at the project root, add `crates/harness-chat`, `crates/harness-storage`, `crates/harness-tools`, and update `src-tauri/Cargo.toml` to depend on those sibling crates by path.

**Dependency direction (strict):**

```
src-tauri (the Tauri binary) ──► harness-chat ──► harness-storage
                             ╲                ╲──► harness-tools
                              ╲                ╲
                               ╲────────────────╲──► strands-{core,ollama,openrouter,macros}
```

Pure crates (`harness-chat`, `harness-storage`, `harness-tools`) **do not import `tauri`**. This is the test boundary — all three are unit-testable without a Tauri runtime.

## 5. strands-rs changes — `strands-openrouter`

New crate at `../strands-rs/strands-openrouter/`. Cargo.toml adds it to the workspace.

**Public API:**

```rust
pub struct OpenRouterModel {
    client: reqwest::Client,
    api_key: String,
    model: String,
    base_url: String,                   // default "https://openrouter.ai/api/v1"
    referrer: Option<String>,           // OpenRouter "HTTP-Referer" header
    app_title: Option<String>,          // OpenRouter "X-Title" header
}

impl OpenRouterModel {
    pub fn new(model: impl Into<String>, api_key: impl Into<String>) -> Self;
    pub fn with_base_url(mut self, url: impl Into<String>) -> Self;
    pub fn with_referrer(mut self, r: impl Into<String>) -> Self;
    pub fn with_app_title(mut self, t: impl Into<String>) -> Self;
}

#[async_trait::async_trait]
impl strands_core::Model for OpenRouterModel { /* SSE-parsing stream impl */ }
```

**Implementation notes:**
- OpenRouter speaks the OpenAI-compatible `/chat/completions` endpoint with SSE streaming (`stream: true`).
- Conversion: `strands_core::Message` ↔ OpenAI message JSON; `strands_core::ToolSpec` ↔ OpenAI `tools[]`; OpenAI delta chunks → `StreamEvent::ContentBlockDelta`; OpenAI `finish_reason` → `StopReason`.
- Tool-use deltas arrive as JSON-string fragments under `tool_calls[].function.arguments`. Accumulated per `tool_calls[].index` and emitted as `ToolInputDelta`.
- Dependencies: `strands-core` (path), `reqwest` (rustls + stream), `tokio`, `async-trait`, `serde`, `serde_json`, `eventsource-stream`, `futures`, `thiserror`.

**Test:** integration test fed by a recorded SSE fixture file. No live network in CI.

## 6. Streaming pipeline (in `harness-chat/pipeline/`)

```
strands_core::StreamEvent  ──► xml_unwrap ──► coalesce ──► IPC Channel ──► Vue
   (Model adapter emits         (detects        (≈16 ms tick;
    text deltas + tool          <tool_use>,     batches small
    deltas + stop reasons)      <reasoning>,    deltas to keep
                                <thinking>;     UI smooth without
                                emits typed     flooding IPC)
                                events)
```

**Public event enum** (defined in `harness-chat::pipeline::events`, serde-derived, mirrored as TS in `src/types/chat.types.ts`):

```rust
#[derive(Serialize, Clone, Debug)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum StreamEvent {
    SessionStarted { session_id: String },
    TextDelta      { text: String },
    ReasoningDelta { text: String },
    ToolUse        { name: String, id: String },
    ToolResult     { name: String, status: ToolStatus, id: String },
    Thinking       { active: bool },
    Error          { message: String },
    Done           { stop_reason: StopReason, usage: Usage },
    Cancelled,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "snake_case")]
pub enum ToolStatus { Success, Error }
```

**Why typed events instead of XML-in-text:** the portfolio renderer regex-matches custom XML sentinels in the text stream and replaces them with markdown placeholders. We move that detection to Rust at source. The Vue renderer maps `kind` to a Vue component (`<ToolUseChip>`, `<ReasoningBlock>`) instead of regex-replacing strings. Cleaner, faster, no escaping bugs. The markdown body (assistant prose) still renders via markdown-it + highlight.js.

**Cancellation slot:**

```rust
loop {
    tokio::select! {
        evt = stream.next() => match evt { ... },
        _ = cancel.cancelled() => {
            channel.send(StreamEvent::Cancelled).ok();
            break;
        }
    }
}
```

**Coalescer:** if N text-delta events arrive within a 16 ms tick, emit one `TextDelta` with the concatenation. Keeps IPC traffic bounded (a few events/sec rather than ~50/sec).

## 7. Storage (SurrealDB, mirrors Memex)

- **Engine:** `surrealdb` v2 with `kv-rocksdb` feature.
- **Path:** `~/.harness/db` (override via env `HARNESS_DB_PATH` for tests).
- **Namespace / database:** `harness` / `chat`.
- **Init:** `harness-storage::db::init_db(path)` mirrors `../world/desktop/src-tauri/memex-core/src/db.rs::init_db` exactly — create dir, open Surreal, `use_ns/use_db`, run SDL idempotently.

**Schema (`harness-storage::schema::SCHEMA`):**

```sql
DEFINE TABLE chat_session SCHEMAFULL;
DEFINE FIELD title          ON chat_session TYPE string;
DEFINE FIELD agent_id       ON chat_session TYPE string;
DEFINE FIELD created_at     ON chat_session TYPE datetime DEFAULT time::now();
DEFINE FIELD last_msg_at    ON chat_session TYPE datetime DEFAULT time::now();
DEFINE FIELD message_count  ON chat_session TYPE int      DEFAULT 0;
DEFINE FIELD memory         ON chat_session TYPE object   DEFAULT {};
DEFINE FIELD deleted_at     ON chat_session TYPE option<datetime>;
DEFINE INDEX idx_agent_active ON chat_session FIELDS agent_id, deleted_at;

DEFINE TABLE chat_message SCHEMAFULL;
DEFINE FIELD session        ON chat_message TYPE record<chat_session>;
DEFINE FIELD role           ON chat_message TYPE string ASSERT $value INSIDE ['user', 'assistant', 'system'];
DEFINE FIELD content        ON chat_message TYPE string;
DEFINE FIELD content_blocks ON chat_message TYPE array DEFAULT [];
DEFINE FIELD created_at     ON chat_message TYPE datetime DEFAULT time::now();
DEFINE INDEX idx_session_time ON chat_message FIELDS session, created_at;
```

**Memory:** `SlidingWindowMemory` keeps last `N = 20` messages in `chat_session.memory`, snapshotted after each turn. Load = single record fetch; save = single UPDATE.

## 8. Tools

### Built-ins (`harness-tools/builtins/`)

| Tool | Purpose | Sandboxing |
|---|---|---|
| `get_time` | local + UTC time | none |
| `calculator` | arithmetic via `evalexpr` crate | none |
| `http_fetch` | GET URL → text | host allowlist in settings; default empty → tool refuses |
| `read_file` | read text file under sandbox root | path canonicalised, must be inside settings sandbox root; default unset → tool refuses |

Defined with `#[tool]` from `strands-macros`. Each tool is opt-in per agent in the registry; default agents start with `get_time` + `calculator` only (no I/O surface).

### MCP (`harness-tools/mcp/`)

- Client: `rmcp` (official Rust MCP SDK).
- Servers configured in Settings UI: stdio command + args, or HTTP URL.
- On connect, MCP tools are discovered and wrapped in an adapter that implements `strands_core::Tool`. The adapter forwards `invoke(input, ctx)` to MCP's `tools/call`.
- Tool list is merged into the agent's tool list at `chat_send` time — agent always sees a flat list of `Tool` impls, doesn't care about origin.
- UI prefixes MCP tools with `[MCP:server-name]` on the chip.

## 9. Agents and registry

`harness-chat/agent_registry.rs` exposes:

```rust
pub struct AgentConfig {
    pub id: String,
    pub agent_type: AgentType,        // agent | swarm | graph | a2a | distributed
    pub name: String,
    pub description: String,
    pub provider: Provider,           // Ollama | OpenRouter
    pub model_id: String,
    pub parameters: Option<u32>,      // billions, e.g. 70
    pub architecture: Option<Architecture>, // moe | dense | etc.
    pub cost: CostTier,
    pub supports_tools: bool,
    pub default_tools: Vec<&'static str>,  // built-in tool names
    pub disabled: bool,
    pub disabled_message: Option<String>,
}

pub enum Provider { Ollama, OpenRouter }
pub enum AgentType { Agent, Swarm, Graph, A2A, Distributed }
```

Sources of registry entries:

1. **Static OpenRouter list** — curated picks (a few free, a few paid; tagged with cost tier).
2. **Dynamic Ollama discovery** — at app start, GET `{ollama_url}/api/tags`. Each pulled model becomes an agent. If Ollama is unreachable, no Ollama agents appear (no error toast — just absence).

Phase-2 agent types (`swarm`, `graph`, `a2a`, `distributed`) appear in the type filter chips with `(0)` counts.

## 10. Tauri commands (in `src-tauri/src/commands/`)

```rust
#[tauri::command] async fn list_agents(state: State<AppState>) -> Vec<AgentDto>;
#[tauri::command] async fn list_sessions(agent: String, limit: u32, offset: u32, state: State<AppState>) -> SessionsPage;
#[tauri::command] async fn get_history(session_id: String, limit: u32, offset: u32, state: State<AppState>) -> HistoryPage;
#[tauri::command] async fn delete_session(session_id: String, state: State<AppState>) -> Result<(), String>;
#[tauri::command] async fn chat_send(
    agent: String,
    prompt: String,
    session_id: Option<String>,
    on_event: Channel<StreamEvent>,
    state: State<AppState>,
) -> Result<String, String>;          // returns session_id
#[tauri::command] async fn chat_cancel(channel_id: u32, state: State<AppState>) -> Result<(), String>;
#[tauri::command] async fn settings_get(state: State<AppState>) -> Settings;
#[tauri::command] async fn settings_set(settings: Settings, state: State<AppState>) -> Result<(), String>;
```

**Cancellation registry:** `AppState` holds `cancellations: Arc<Mutex<HashMap<u32, CancellationToken>>>`. `chat_send` inserts a token keyed by `on_event.id()` before spawning the streaming task; the task removes its own entry on completion. `chat_cancel` looks up by id and calls `.cancel()`.

## 11. Settings

Stored via `tauri-plugin-store` in `~/Library/Application Support/com.harness.app/settings.json` (or platform equivalent).

```rust
#[derive(Serialize, Deserialize, Default, Clone)]
pub struct Settings {
    pub openrouter_api_key: Option<String>,
    pub openrouter_referrer: Option<String>,
    pub openrouter_app_title: Option<String>,
    pub ollama_url: String,                 // default "http://localhost:11434"
    pub default_agent_id: Option<String>,
    pub mcp_servers: Vec<McpServerSpec>,
    pub http_fetch_allowlist: Vec<String>,
    pub read_file_sandbox_root: Option<PathBuf>,
}

pub enum McpServerSpec {
    Stdio { name: String, command: String, args: Vec<String> },
    Http  { name: String, url: String },
}
```

Settings UI is a single Vue route (`/settings`) with form fields and a "Save" button calling `settings_set`. API key field is a password input; never logged.

## 12. Frontend changes from portfolio

Files **ported as-is**:
- `src/components/Layout/Header.vue` — keep the secret dash link to `/chat`, drop the version-fetch from `app.willpercey.com`.
- `src/components/Layout/Sidebar.vue` (if it exists and is used).
- `src/scss/` — all stylesheets.
- `src/types/chat.types.ts` — extended with `StreamEvent` types.
- `src/pages/Chat.vue` — UI markup and styles preserved verbatim where possible.

Files **rewritten**:
- `src/services/chat.ts`:
  - `getAgents()` → `invoke('list_agents')`.
  - `getSessions()` → `invoke('list_sessions', ...)`.
  - `getChatHistory()` → `invoke('get_history', ...)`.
  - `deleteSession()` → `invoke('delete_session', ...)`.
  - `streamChat()` → creates `Channel<StreamEvent>`, calls `invoke('chat_send', ...)` returning a `{ stream, sessionId, cancel }` triple. The `stream` is an async iterator of `StreamEvent`s (not raw text); `cancel` invokes `chat_cancel` with the channel id.
- `src/pages/Chat.vue::sendMessage()`:
  - Buffer/coalesce logic stays but consumes `StreamEvent`s. Text deltas append to a `markdown` string; reasoning deltas append to a `reasoning` string per turn; tool events push entries into a structured `turnBlocks` array rendered by component children.
  - `renderMarkdown()` is simplified — no XML sentinel substitution; tool/reasoning are rendered by Vue components, not regex-injected HTML.
  - Cancel button added next to "Sending..." indicator.

Files **new**:
- `src/pages/Settings.vue`.
- `src/services/settings.ts`.
- `src/components/chat/ToolUseChip.vue`, `ToolResultChip.vue`, `ReasoningBlock.vue`, `StreamingIndicator.vue`.

## 13. Error handling

- All Tauri commands return `Result<T, String>`. Internal errors are `anyhow::Error`; converted to `String` at the command boundary via `map_err(|e| e.to_string())`.
- Streaming errors: emitted as `StreamEvent::Error { message }` then `StreamEvent::Done { stop_reason: StopReason::Error, usage }`.
- Provider-specific errors (rate limit, auth, network) are normalised in the provider adapter to friendly messages; the raw error is logged via `tracing`.
- No `panic!` in command paths. `unwrap` only allowed in tests.
- Frontend shows errors in the existing `error-message` banner; `StreamEvent::Error` populates the same banner mid-stream.

## 14. Testing strategy

| Crate | Test type | Notes |
|---|---|---|
| `harness-storage` | unit | in-memory Surreal: `Surreal::new::<Mem>()` |
| `harness-chat::pipeline` | unit | hand-built `Stream<StreamEvent>` → assert output sequence after xml_unwrap + coalesce |
| `harness-chat::cancel` | unit | spawn long stream, cancel, assert `Cancelled` arrives within deadline |
| `strands-openrouter` | integration | recorded SSE fixture, no live network |
| `harness-tools::builtins` | unit | each tool tested independently; sandbox/allowlist enforcement verified |
| `harness-tools::mcp` | integration | mock MCP server (stdio) launched in test |
| `src-tauri` | smoke | does it boot, do commands wire up |
| Frontend | manual | dev server + Tauri `cargo tauri dev` |

## 15. Build and dev experience

- `cargo tauri dev` from `harness/` runs Vite dev server + builds Tauri app. Hot-reload works for Vue. Rust changes recompile + relaunch the window.
- `cargo test --workspace` runs all Rust tests. `cargo test -p strands-openrouter` from `../strands-rs/`.
- `cargo build --release` followed by `cargo tauri build` produces a signed/unsigned bundle (signing left for later).
- No CI in v1.

## 16. Open questions / deferred decisions

- **Agent metadata source for OpenRouter.** Hard-coded curated list in v1. Phase 2: pull from OpenRouter's `/models` endpoint and merge.
- **Token usage cost calculation.** Original ai-playground tracked per-user costs; v1 does not. Phase 2: log token counts to a `token_usage` Surreal table.
- **Conversation summarisation.** Not in v1 (sliding window only). Phase 2 toggle.
- **Cross-platform packaging.** macOS only confirmed in v1; Windows/Linux untested.
- **OS keychain for API keys.** v1 uses tauri-plugin-store (plaintext on disk under app-data perms). Phase 2: optional keychain backend.

## 17. Build sequence (proposed for the implementation plan)

This is a sketch; the writing-plans skill produces the canonical sequenced plan.

1. Workspace skeleton: add a root `Cargo.toml` declaring `members = ["src-tauri", "crates/*"]`; create empty `crates/harness-chat`, `crates/harness-storage`, `crates/harness-tools`. Update `src-tauri/Cargo.toml` to depend on the new sibling crates by path. Verify `cargo tauri dev` still launches the unmodified scaffold UI.
2. `harness-storage`: schema + `init_db` + `chat_session`/`chat_message` CRUD + memory snapshot. Tests against in-memory Surreal.
3. `strands-openrouter` crate in `../strands-rs/`: SSE plumbing + Model impl + fixture-based integration test. Wire into `../strands-rs/Cargo.toml` workspace.
4. `harness-chat`: `agent_registry` (static OpenRouter + dynamic Ollama discovery), `service::ChatService`, `pipeline::events::StreamEvent` + `xml_unwrap` + `coalesce`, `cancel::CancellationRegistry`. Tests for pipeline and cancel.
5. `harness-tools/builtins/`: four built-ins with `#[tool]` macro and sandboxing tests.
6. `src-tauri/src/commands/`: wire `list_agents`, `list_sessions`, `get_history`, `delete_session`, `chat_send`, `chat_cancel`, `settings_get`, `settings_set`. AppState assembly.
7. Frontend port: copy `Header.vue`, scss, types from `../portfolio`. Rewrite `chat.ts`. Build new `Chat.vue` with structured event consumption + cancel button + streaming components.
8. `harness-tools/mcp/`: rmcp client, adapter, registry. Settings UI for adding/removing servers. Tools merged into agent at `chat_send`.
9. `Settings.vue` page: API key input, allowlist editors, MCP server list.
10. End-to-end manual test: chat with an Ollama model, chat with an OpenRouter model, run a built-in tool, cancel mid-stream, restart app and verify history persistence.
