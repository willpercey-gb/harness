---
name: who
description: Look up a person in the Harness knowledge base. Shows who they are, what they work on, and how they're connected. Use when the user asks "who is [name]?".
argument-hint: <person name>
user-invocable: true
---

# Harness Person Lookup

Look up a person by name. The name is `$ARGUMENTS`.

## Instructions

1. Call `get_entity_by_name` with the person's name (any-type search, case-insensitive).
2. If found:
   - Show their **description** and **content** bullets.
   - Call `get_entity_graph` with depth 2 to see their connections (org, projects, tech).
   - Call `search_memories` with their name for additional context from chats / files.
3. If the entity exists but has a weak description or no content, enrich it:
   - If description is generic or missing, call `update_entity` with a better one based on what you found in memories.
   - If content is empty, call `update_entity` with bullet-point facts pulled from the graph and memories.

## If Not Found

1. Tell the user they're not in the graph yet.
2. Ask: "Would you like me to add them? What's their role and what do they work on?"
3. If the user provides info:
   - `add_entity(person, name, description: "...")`
   - `update_entity(name, content: "- fact one\n- fact two")`
   - `add_relationship(...)` to link them to orgs / projects.

## Response Format

```
[Name] — [description]

What we know:
- [content bullets]

Connections:
- Works at: [organisation]
- Works on: [projects]
- Knows about: [technologies]

Related context:
- [key memories, if any]
```
