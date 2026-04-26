# Multi-agent context window

## What

Each `chat_send` runs three model calls in sequence:

1. **Anchor agent** — emits `<anchor>` + `<priority>…` + `<aside>…` cards summarising the session goal.
2. **Intent classifier** — labels the new message `expand` | `revise` | `redirect` | `aside`. Skipped when the user picks a value from the composer dropdown.
3. **Main agent** — runs the normal ReAct loop with the cards + intent prepended to its system prompt.

Cards live on `chat_session` in SurrealDB and render in the right sidebar. The user can edit, add, or delete any card; edits flag `edited_by_user: true` and the anchor agent preserves them verbatim on the next turn.

## How

### Per-turn flow

```
load ConversationContext for session
   │
   ├── stage 1: anchor_agent::extract_or_refine
   │     in:  prior context, recent history, new prompt
   │     out: ConversationContext { anchor, priorities[], asides[] }
   │     persist: chat_session.context_*
   │     stream: ContextStarted → ContextAnchor → ContextPriority… →
   │             ContextAside… → ContextDone
   │
   ├── stage 2: intent_agent::classify  (skipped if intent_override = Some)
   │     in:  context (XML), new prompt
   │     out: Intent
   │     stream: IntentClassified { intent, source: "auto"|"manual" }
   │
   └── stage 3: main agent
         system prompt prepended with:
           <context>...</context>
           <intent source="...">expand|revise|redirect|aside</intent>
         existing Agent::prompt ReAct loop runs unchanged
```

One `CancellationToken` covers all three stages.

### XML envelope

```xml
<context>
  <anchor>plan a 4-day Lisbon trip</anchor>
  <priority id="9c2…">4-day duration</priority>
  <priority id="b18…" edited="true">budget under £500</priority>
  <aside id="a7e…">note: timezone is GMT+0</aside>
</context>
<intent source="auto">expand</intent>
```

`xml_envelope(ctx, intent)` renders it; `parse_envelope(text)` and `parse_intent(text)` read it back. Both readers are tolerant — they skip prose preamble and unknown tags.

### Storage

```sql
DEFINE FIELD context_anchor      ON chat_session TYPE option<string>;
DEFINE FIELD context_priorities  ON chat_session FLEXIBLE TYPE array DEFAULT [];
DEFINE FIELD context_asides      ON chat_session FLEXIBLE TYPE array DEFAULT [];
DEFINE FIELD context_updated_at  ON chat_session TYPE option<datetime>;
```

Each card: `{ id: uuid, text: string, edited_by_user: bool }`.

### Streaming events (Rust → frontend)

```
ContextStarted
ContextAnchor { text }
ContextPriority { id, text, edited_by_user }*
ContextAside    { id, text, edited_by_user }*
ContextDone
IntentClassified { intent, source }
SessionStarted   { session_id }       (first turn only)
TextDelta / ReasoningDelta / ToolUse / ToolResult …
Done             { stop_reason, usage }
```

`Cancelled` may replace `Done` at any point.

### Frontend wiring

- `useContextStore` (Pinia) holds the live `ConversationContext` + last `Intent`.
- `applyStreamEvent(ev)` accumulates `context_*` events into a draft and swaps to live state on `context_done`.
- `RightSidebar.vue` renders `AnchorCard` + two `CardList`s + `IntentBadge`.
- `Chat.vue::onStreamEvent` calls `context.applyStreamEvent(ev)` for every event.
- `IntentDropdown` on the composer (`Auto` | `Expand` | `Revise` | `Redirect` | `Aside`); resets to `Auto` after each send.
- Card edits call `updateContext(sessionId, ctx)` synchronously.

### Tauri commands

| Command | Args | Returns |
|---|---|---|
| `get_context` | `session_id` | `ConversationContext` |
| `update_context` | `session_id, context` | `()` |
| `chat_send` | `agent, prompt, session_id?, intent_override?, on_event` | `session_id` |

### File map

**Rust**

| File | Role |
|---|---|
| `crates/harness-chat/src/context.rs` | `Intent`, `IntentSource`, `xml_envelope`, `parse_envelope`, `parse_intent` |
| `crates/harness-chat/src/anchor_agent.rs` | Stage 1 |
| `crates/harness-chat/src/intent_agent.rs` | Stage 2 |
| `crates/harness-chat/src/service.rs` | Pipeline orchestration in `run_chat` |
| `crates/harness-storage/src/context_store.rs` | `ConversationContext`, `ContextCard`, `load`, `save` |
| `crates/harness-storage/src/schema.rs` | Schema additions |
| `src-tauri/src/commands/context.rs` | Tauri commands |

**Frontend**

| File | Role |
|---|---|
| `src/types/context.types.ts` | TS mirrors |
| `src/services/context.ts` | IPC adapter |
| `src/stores/context.ts` | Pinia store |
| `src/components/context/AnchorCard.vue` | Anchor editor |
| `src/components/context/CardList.vue` | Priorities / asides list |
| `src/components/context/IntentBadge.vue` | Intent pill |
| `src/components/composer/IntentDropdown.vue` | Composer override |
| `src/components/RightSidebar.vue` | Card host |
