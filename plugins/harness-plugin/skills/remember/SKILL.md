---
name: remember
description: "ALWAYS USE THIS SKILL when the user's message contains ANY information about people, teams, projects, technologies, companies, decisions, or relationships — even if they didn't explicitly ask you to remember it. This includes casual mentions like 'I'm working with Sarah on the API rewrite' or 'we decided to use Postgres'. If a user prompt contains facts that could be mapped to the knowledge graph, invoke this skill. The only time you should NOT use it is for pure questions with no new information."
argument-hint: <information to remember>
user-invocable: true
---

# Harness Remember

Extract and store knowledge from the user's statement into the Harness knowledge graph. The input is `$ARGUMENTS`.

## When to Use

Use this skill whenever the user's message contains mappable information — you do NOT need to wait for the user to say "remember". Examples of triggers:

- Mentions a person by name ("talked to Sarah", "Dom is leading…")
- Names a project, product, or initiative ("the API rewrite", "Project Atlas")
- References a technology choice ("we're using Kubernetes", "migrating to Postgres")
- States a relationship ("she works at Anthropic", "he's on the platform team")
- Records a decision or fact ("we decided to…", "the deadline is…")

## Instructions

### Step 1 — Parse

Identify:
- **Entities**: people, organisations, projects, technologies, topics, locations.
- **Relationships**: who works where, who works on what, what uses what.
- **Facts**: decisions, preferences, deadlines, context.

### Step 2 — Look up each entity first

For every entity, call `get_entity_by_name`.

- **Exists**: Read its description + content. If description is missing or vague, `update_entity` with a better one. If you learned new facts, `update_entity` with appended content bullets.
- **Doesn't exist**: `add_entity` with a description (one-line explainer of WHAT it is). Then `update_entity` with content bullets for facts.

### Step 3 — Create relationships

For each relationship, call `add_relationship`. Both entities must exist first.

### Step 4 — Store the raw statement

Call `add_memory` with the original statement (`source_type: "manual"`). This keeps the full context searchable even if entity extraction misses nuance.

### Step 5 — Confirm

List what you stored: entities created/updated, relationships added.

## Entity Content Guidelines

- **description** = WHAT it is (short, factual): "Backend engineer on the platform team", "Internal CLI tool for deployment".
- **content** = WHAT we know (bullet list, appended over time):
  ```
  - Leading the Kubernetes migration (Jan 2026)
  - Previously worked on the auth service
  ```

When updating, append new bullets — don't repeat existing ones.

## Deduplication

CRITICAL: Before creating any entity, ALWAYS call `get_entity_by_name` first.

- "Sarah Smith" and "Sarah" might be the same person — check context.
- "K8s" and "Kubernetes" are the same technology — use aliases.

## Example

> Remember that Dom is leading the platform migration to Kubernetes.

1. `get_entity_by_name("Dom")` — check.
2. `add_entity(person, "Dom", description: "Leading platform migration")`.
3. `update_entity("Dom", content: "- Leading the platform migration to Kubernetes")`.
4. `get_entity_by_name("Platform Migration")` — check.
5. `add_entity(project, "Platform Migration", description: "Infrastructure migration to Kubernetes")`.
6. `update_entity("Platform Migration", content: "- Led by Dom\n- Target: Kubernetes")`.
7. `add_entity(technology, "Kubernetes", description: "Container orchestration platform")`.
8. `add_relationship(Dom works_on Platform Migration)`.
9. `add_relationship(Platform Migration uses_tech Kubernetes)`.
10. `add_memory("Dom is leading the platform migration to Kubernetes")`.
