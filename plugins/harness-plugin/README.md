# Harness Claude Code Plugin

Personal knowledge graph plugin for Claude Code. Search and write to your Harness desktop app's knowledge base from any Claude session.

## Skills

| Command | Description |
|---|---|
| `/harness:query <search>` | Hybrid search across memories + entities |
| `/harness:who <name>` | Look up a person, see who they're connected to |
| `/harness:remember <info>` | Extract entities + relationships from a statement and store them |
| `/harness:graph <entity>` | Walk the graph N hops from an entity |
| `/harness:review` | Review and tidy entities (dedup, enrich descriptions) |

## Requirements

The Harness desktop app must be running — the MCP server connects to it via a local TCP bridge on port **19851** (Memex uses 19850; both can coexist).

## Installation

```bash
claude --plugin-dir /path/to/harness/plugins/harness-plugin
```

Or symlink into `~/.claude/plugins/` if you want it always available.

The plugin's MCP server is the `harness-mcp` binary. Build it once:

```bash
cd /path/to/harness
cargo build -p harness-mcp --release
```

…then point your `~/.claude/mcp.json` (or per-project `.mcp.json`) at the resulting binary:

```json
{
  "mcpServers": {
    "harness": {
      "command": "/path/to/harness/target/release/harness-mcp"
    }
  }
}
```
