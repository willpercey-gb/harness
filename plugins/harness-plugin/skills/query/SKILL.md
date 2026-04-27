---
name: query
description: Search the user's Harness knowledge base for information about people, projects, technologies, decisions, or anything they've stored. Use when the user wants to find something in their personal knowledge graph.
argument-hint: <search query>
user-invocable: true
---

# Harness Query

Search the user's personal knowledge base. The query is `$ARGUMENTS`.

## Instructions

1. Call `query_knowledge` with the query — hybrid search across memories, entities, and relationships.
2. If the query mentions a specific name (person, project, technology), call `get_entity_by_name` to pull its full profile (description + content bullets).
3. For entities found, also call `get_entity_graph` (depth 2) to see relationships and connections.
4. Run `search_memories` to catch context that isn't linked to entities.
5. Combine into a clear, concise answer.

## After Answering

If the query reveals new information that isn't in the knowledge base, proactively update entities:

- If an entity's description is missing or outdated, call `update_entity` with a better one.
- If you learned new facts about an entity, call `update_entity` to append content bullets.

## Response Format

- Lead with the most relevant finding.
- For entity lookups, show: description, key content bullets, relationships.
- Include source attribution (chat, markdown, manual, etc.).
- Keep concise — the user can ask follow-ups.
- If nothing found, say so clearly and offer to record new information via `/harness:remember`.
