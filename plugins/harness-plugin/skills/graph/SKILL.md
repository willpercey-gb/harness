---
name: graph
description: Explore the Harness knowledge graph around a specific entity. Shows all connections within N hops — people, projects, technologies, organisations. Use when the user wants to see how things are connected.
argument-hint: <entity name> [depth]
user-invocable: true
---

# Harness Graph Explorer

Walk the knowledge graph around an entity. The entity name is `$ARGUMENTS`.

## Instructions

1. Find the entity across all types using `get_entity_by_name`.
2. Once found, call `get_entity_graph` with `depth: 2` (or user-specified).
3. Present the graph as a structured summary:
   - Center entity with description.
   - Direct connections grouped by relationship type.
   - Second-hop connections if notable.

## Response Format

```
[Entity Name] ([type])
[description]

Connections:
- works_at → [orgs]
- works_on → [projects]
- uses_tech → [technologies]
- knows_about → [topics]
- related_to → [other entities]
```

If the entity has many connections, prioritise the most meaningful and mention the total count.
