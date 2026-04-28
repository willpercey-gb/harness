# Harness

A local-first desktop chat playground for talking to LLMs across providers, with a built-in long-term memory: a per-user knowledge graph + vector store the agent can read from and write to as it chats.

Tauri v2 + Vue 3 frontend, Rust workspace backend, single binary on macOS / Windows / Linux. Everything is local — chat history lives on your machine, the embedding model runs in-process, and graph + memories are stored in an embedded SurrealDB.

---

## What it does

At its simplest, harness is a chat UI with provider switching and persistent sessions. Layered on top of that:

- **Multiple providers in one window**: Ollama (local models), OpenRouter (cloud), and the Claude / Codex / Gemini CLIs (each runs the user's logged-in local CLI as a subprocess — no API keys held by harness).
- **Multi-agent context cards**: every turn runs an *anchor agent* and an *intent classifier* before the main agent. The anchor extracts a few priorities and asides from the conversation, which render as editable cards in the right sidebar and ride along in the main agent's system prompt. Detailed in `docs/multi-agent-context.md`.
- **Persistent knowledge layer**: the main agent has tools to save free-form memories, look up named entities, and draw typed relationships. Everything it writes is searchable in later sessions via hybrid retrieval.
- **Markdown ingestion**: point harness at a folder (or just `~/`) and it'll chunk every `.md` file under it into the memory store, deduplicated by content hash. The agent can then recall from your notes.
- **Knowledge page**: a graph explorer + memory timeline UI for browsing what the agent (or you) has captured. Lives at `/knowledge` in the app.
- **MCP bridge**: a TCP server on `127.0.0.1:19851` exposing the same memory + entity API to external MCP clients (the bundled Claude Code plugin uses it). Lets your other Claude Code workspaces share the same harness brain.

---

## Architecture

```
harness/
├── src/                      # Vue 3 frontend (Vite, TS, Pinia, vue-router)
│   ├── pages/                #   Chat, Knowledge, Settings
│   ├── components/           #   chat bubbles, tool chips, graph, memory timeline
│   ├── services/             #   typed Tauri-IPC adapters
│   └── types/
├── src-tauri/                # Tauri v2 binary — the host process
│   └── src/
│       ├── commands/         #   Tauri command handlers (chat_send, list_sessions, …)
│       ├── state.rs          #   AppState: HarnessDb + MemexDb + embedder + cancellations
│       └── bridge.rs         #   TCP server for external MCP clients
├── crates/                   # Rust workspace
│   ├── harness-storage/      #   chat sessions, messages, settings (SurrealDB)
│   ├── harness-chat/         #   agent registry, ChatService, multi-agent pipeline
│   ├── harness-tools/        #   built-in agent tools (calculator, memex, http_fetch, …)
│   └── harness-mcp/          #   stdio MCP proxy (used by the Claude plugin)
├── memex-core/               # git submodule → github.com/willpercey-gb/memex-core
├── strands-rs/               # git submodule → github.com/willpercey-gb/strands-rs
└── plugins/harness-plugin/   # Claude Code plugin (skills + manifest)
```

Two embedded SurrealDBs (RocksDB engine) so the single-writer lock isn't contested:

| Path | Owner | Holds |
|---|---|---|
| `~/.harness/db` | `harness-storage` | chat sessions, messages, settings, context cards |
| `~/.harness/memex-db` | `memex-core` | entities, relationships, memory chunks, embeddings |

The frontend never sees either database directly — it talks to the backend exclusively through Tauri IPC commands and `Channel<StreamEvent>` for the hot streaming path.

---

## The knowledge layer (memex-core)

`memex-core` is the heart of long-term memory. It's a self-contained Rust library — no Tauri dependency — that pairs SurrealDB v2 (RocksDB) with in-process embeddings via `fastembed` (BAAI/bge-small-en-v1.5, 384-dim). Everything is local, no network calls at runtime.

### Three things live in the database

**1. Entities — typed graph nodes.** Seven categories, each its own SurrealDB table:

| Type | What it is |
|---|---|
| `person` | Real people the user knows or works with |
| `organization` | Companies, institutions, named teams |
| `project` | Standalone products / apps / initiatives |
| `technology` | Languages, frameworks, databases, platforms |
| `topic` | Domains of expertise (machine learning, DDD, observability, …) |
| `location` | Cities, countries, offices |
| `component` | Sub-products / features / modules inside a parent project |

Every entity has: `name`, `aliases[]`, `description`, `content` (rich-text body that grows over time), `embedding` (384-dim vector indexed via HNSW for ANN search), plus access counters and timestamps. The `name` field has a BM25 full-text index using a Snowball English analyzer.

**2. Relationships — typed graph edges.** Seven relation tables, one per type:

`works_at`, `part_of`, `works_on`, `uses_tech`, `knows_about`, `related_to`, `mentions`

Each edge has `from_id`, `to_id`, `relation_type`, optional `metadata`, and a timestamp. SurrealDB stores them as separate tables (rather than a single edges table) so you can query and filter by relation type cheaply.

**3. Memories — free-form notes with vectors.** A single `memory_chunk` table:

| Field | Meaning |
|---|---|
| `content` | The actual text |
| `summary` | Optional short version |
| `source_type` | `chat`, `manual`, `file`, `screenshot`, … |
| `source_id` | Session id or file hash |
| `source_path` | For ingested files |
| `timestamp` | When written |
| `embedding` | 384-dim vector, HNSW-indexed |
| `metadata` | Free-form JSON |

`memory_chunk` deduplicates on insertion by content hash — re-ingesting the same paragraph twice is a no-op. The `content` field has the same BM25 index as entity names.

### Hybrid query

When the agent asks "what do I know about X?", the `hybrid_query` function blends four retrieval strategies in one call (`memex-core/src/query.rs`):

1. **Vector similarity** — embed the query, do KNN over `memory_chunk.embedding` via SurrealDB's HNSW index.
2. **Full-text BM25** — score the same query against `memory_chunk.content`.
3. **Entity name match** — for each entity table (or a filtered subset), check whether the query string appears in `name` or `aliases[]`.
4. **Recency boost** — newer chunks get a small lift.

Results are deduplicated and merged into a single ranked list, with each result carrying its `linked_entities[]` (any entities mentioned by name in that memory). The agent gets back not just a paragraph but who/what/where it's about.

### Embeddings

`fastembed` runs the BAAI/bge-small-en-v1.5 ONNX model in-process via the `ort` crate. First run downloads ~50 MB to `~/.memex/models/`; thereafter it loads from cache. Embeddings happen on the same Tokio runtime as everything else (offloaded with `spawn_blocking` so they don't stall the executor).

384 dimensions is small enough that storing an embedding per memory chunk and per entity name is cheap on disk.

---

## Tools the agent can call

The main agent (and only the main agent — anchor + intent agents stay text-in/text-out) is given a tool list that includes both generic builtins and memex-backed tools.

### Memex tools

Five tools wrap memex-core. Each is a thin `strands_core::Tool` impl living at `crates/harness-tools/src/builtins/memex.rs`.

| Tool | What it does |
|---|---|
| `remember` | Save a free-form memory chunk. Inputs: `content` (required), `summary` (optional). Tagged with `source_type="chat"` and the current `session_id`. |
| `recall` | Hybrid query for memories + linked entities. Inputs: `query` (required), `limit` (default 5). |
| `note_entity` | Upsert an entity. Inputs: `entity_type` (one of the 7), `name`, `description?`, `content?`. Names match case-insensitively against existing aliases — no duplicates. |
| `lookup_entity` | Find an entity by name across all 7 types. Returns id, type, description. |
| `link_entities` | Create a typed relationship. Inputs: `from`, `to`, `relation` (one of the 7), `metadata?`. Resolves names to ids. |

`remember` and `recall` are the everyday workhorses; the entity tools support structured note-taking when the agent decides it's useful.

### Generic builtins

Also at `crates/harness-tools/src/builtins/`:

- `calculator` — `evalexpr`-backed arithmetic.
- `get_time` — current ISO-8601 time, with optional timezone.
- `http_fetch` — guarded HTTP GET (URL allowlist, size cap, redirect limit).
- `read_file` — guarded file read (path-traversal rejection, max bytes).

---

## Ingesting markdown

The Knowledge page has a folder-icon button that triggers `ingest_markdown_folder`. It defaults to `~/`. The walker (`crates/harness-tools/src/ingest.rs`):

1. Recurses depth-first, skipping `.*`, `node_modules`, `target`, `dist`, `build`.
2. For each `.md` file, splits the body at paragraph boundaries into chunks of ~2 KB with 200-byte overlap.
3. Embeds each chunk and inserts it as a `memory_chunk` with `source_type="file"`, `source_path` set, and `source_id` set to the file's content hash. Duplicates short-circuit.
4. Streams a progress dict back: `{ files_seen, files_ingested, chunks_inserted, errors }`.

Same code path runs from the agent (via the bridge) and from the UI button.

---

## Plugin / MCP integration

Two pieces:

- **`crates/harness-mcp`** — a stdio MCP server binary. It speaks the Anthropic MCP protocol over stdin/stdout but its actual storage lives in the running harness app: it's a thin proxy that opens a TCP connection to harness's bridge (`127.0.0.1:19851`) and forwards each tool call as a JSON-RPC request. This way the Claude Code plugin (and any other MCP client) can read/write the same Memex DB without contending for the RocksDB single-writer lock.
- **`plugins/harness-plugin/`** — a Claude Code plugin manifest with four skills (`query`, `who`, `remember`, `graph`) plus a `knowledge-builder` agent. Drop it into Claude Code's plugin directory and a parallel Claude session can recall what the harness agent learned, or contribute back.

---

## Provider support

| Provider | Crate | What it is |
|---|---|---|
| Ollama | `strands-ollama` | Local Ollama daemon at `http://localhost:11434`. Auto-discovers `/api/tags` on boot; renders one agent per pulled model. |
| OpenRouter | `strands-openrouter` | SSE streaming against `openrouter.ai/api/v1/chat/completions`. API key in Settings. |
| Claude CLI | `strands-claude-cli` | Spawns `claude -p` as a subprocess; uses your logged-in Claude Code session. No key managed by harness. |
| Codex CLI | `strands-codex-cli` | Same pattern via `codex exec --json`. |
| Gemini CLI | `strands-gemini-cli` | Same pattern via `gemini -p --output-format stream-json`. |

The CLI providers are discovered at boot by walking well-known install dirs — Tauri-spawned subprocesses don't inherit your shell's PATH, so we look explicitly. Failure is logged but non-fatal: the agent appears in the list and surfaces a friendly error if you try to use it without the binary installed.

---

## Where things live on disk

| Path | What |
|---|---|
| `~/.harness/db/` | Chat sessions, messages, settings (SurrealDB + RocksDB) |
| `~/.harness/memex-db/` | Entities, relationships, memories (SurrealDB + RocksDB) |
| `~/.memex/models/` | BAAI/bge-small-en-v1.5 ONNX cache |

Both `.harness/` databases are exclusively harness-owned. The `~/.memex/` directory is shared with any other tool that uses memex-core, and only contains the embedding model weights — not user data.
