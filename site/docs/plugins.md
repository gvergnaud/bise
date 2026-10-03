---
title: plugins and MCP
description: a plugin is a folder with skills and MCP servers. bise loads it in every agent, local stdio servers and remote ones, with their logins.
---

bise loads [Agent Plugins 1.0](https://agent-plugins.org/specification): a folder with a `plugin.json`, an optional `skills/` folder and an optional `mcp.json`. the same plugin works in every agent.

## where plugins live

| folder | for |
|---|---|
| `~/.agents/plugins/<name>/` | you, in every repo |
| `<repo>/.agents/plugins/<name>/` | this repo only; it wins over a plugin of the same name in your folder |

a plugin folder:

```text
my-plugin/
  plugin.json
  skills/
    review/SKILL.md
  mcp.json
```

```json title="plugin.json"
{
  "$schema": "https://agent-plugins.org/schemas/1.0.0/plugin.schema.json",
  "name": "my-plugin",
  "version": "0.1.0",
  "description": "what it gives the agents"
}
```

its skills show as `my-plugin:review`, and its MCP tools as `tools.my_plugin.<tool>`.

## see and change them

```sh
bise plugins                    # every plugin, its state, and what's wrong with it
bise plugins disable my-plugin
bise plugins enable my-plugin
```

in bise, `/plugins` does the same. the open agents pick up a change at their next idle.

## MCP servers

`mcp.json` lists the servers. it takes the same shape as Claude Code's, Cursor's and Vibe's `.mcp.json`, so a file copied from them works as it is.

### local servers (stdio)

```json title="mcp.json"
{
  "mcpServers": {
    "files": {
      "command": "npx",
      "args": ["-y", "@modelcontextprotocol/server-filesystem", "${PLUGIN_ROOT}/data"],
      "env": { "LOG_LEVEL": "warn" }
    }
  }
}
```

- `command` is a program on your `PATH`, or `./x` inside the plugin.
- `${PLUGIN_ROOT}` is the plugin's folder; `${PLUGIN_DATA}` a folder of its own that survives updates, `~/.bend-harness/plugin-data/<name>/`.
- a server gets your environment, plus `PLUGIN_ROOT`, `PLUGIN_DATA` and its `env`.

### remote servers

```json title="mcp.json"
{
  "mcpServers": {
    "linear": { "type": "http", "url": "https://mcp.linear.app/mcp" },
    "internal": {
      "type": "http",
      "url": "https://mcp.example.com/mcp",
      "headers": { "Authorization": "Bearer ${INTERNAL_MCP_TOKEN}" }
    },
    "legacy": { "type": "sse", "url": "https://example.com/sse" }
  }
}
```

`http` (or `streamable-http`) is Streamable HTTP; `sse` the older HTTP+SSE. `${VAR}` and `${VAR:-default}` come from your environment when the server connects. a variable that isn't set leaves that server out, and `bise plugins` says which.

### logging in (OAuth)

a remote server with no `Authorization` header can log you in with your browser:

```sh
bise plugins login            # the servers that can log in
bise plugins login linear
bise plugins logout linear    # forget its tokens
```

in bise: `/plugins login`. the server's tools reach your agents as soon as you're logged in, no restart. the tokens stay in `~/.bise/secrets/mcp-oauth/`, readable only by you; they are never printed or sent to a model.

with bise on another machine (SSH), log in in your local browser. the last page won't load: copy its address, then paste it into `bise plugins login`, or run `/plugins login linear <address>`.

for a server with no dynamic registration (GitHub, Slack), give the client in `mcp.json`:

```json title="mcp.json"
{
  "mcpServers": {
    "github": {
      "type": "http",
      "url": "https://api.githubcopilot.com/mcp/",
      "oauth": { "clientId": "your-client-id", "callbackPort": 8765 }
    }
  }
}
```

### server options

these keys work on any server, local or remote (Codex's names):

| key | what it does | default |
|---|---|---|
| `startup_timeout_sec` | start, handshake and first tool list | 30 |
| `tool_timeout_sec` | one call | 300 |
| `enabled_tools` | only these tools | all |
| `disabled_tools` | never these tools | none |
| `enabled` | `false` leaves the server out | true |

## bring your servers from Claude Code or Codex

```sh
bise plugins import-mcp my-servers --dry-run < servers.json
bise plugins import-mcp my-servers < servers.json
```

it turns a list of MCP servers (Claude Code's or Codex's) into one plugin named `my-servers`. it keeps remote servers, their headers and their login clients, and prints host and header names, never a value.

## how agents use the tools

MCP tools are not all put in the model's prompt. an agent searches for the tool it needs, reads its description, and calls it from a small TypeScript program. so many servers cost few tokens until they are used.

## not supported yet

hooks, agents, knowledge, views, `connectors.json` and `libraries.json` of the spec are listed by `bise plugins` as not supported; the rest of the plugin still loads.
