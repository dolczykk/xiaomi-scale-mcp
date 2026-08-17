# MCP configuration

This guide connects Xiaomi Scale MCP to AI agents without placing credentials in source control. It covers Codex CLI, Claude Code, and Visual Studio Code; other clients can use the shared connection details below.

## Before you connect

1. Start the server in a separate interactive terminal:

   ```bash
   cargo run -p xiaomi_scale_mcp
   ```

2. Enter `auth` in the server console and complete the Xiaomi sign-in flow. Run `status` to confirm that the Xiaomi credential is stored.
3. Copy the value of `server.authorization_token` from your local `config.toml`. This MCP bearer token is separate from the Xiaomi account token.

Never commit `config.toml`, a client configuration containing a literal token, or personal health data. The examples below use the environment variable `XIAOMI_SCALE_MCP_TOKEN` so the agent configuration contains a reference rather than the token itself.

For a macOS or Linux shell, set the token only in the terminal session that starts your agent:

```bash
export XIAOMI_SCALE_MCP_TOKEN='replace-with-server.authorization_token'
```

## Shared connection details

| Setting | Value |
| --- | --- |
| Transport | Streamable HTTP |
| URL | `http://127.0.0.1:8080/mcp` |
| Authentication | `Authorization: Bearer <server.authorization_token>` |

The default address is local to the same machine. If you deliberately expose the server on another trusted host, use that host’s HTTPS MCP URL instead and keep the bearer token secret.

## Codex CLI

Codex supports remote Streamable HTTP MCP servers and can read the bearer token from an environment variable. In the terminal where `XIAOMI_SCALE_MCP_TOKEN` is set, run:

```bash
codex mcp add xiaomi-scale \
  --url http://127.0.0.1:8080/mcp \
  --bearer-token-env-var XIAOMI_SCALE_MCP_TOKEN
```

Verify the saved server definition, then start Codex from the same environment:

```bash
codex mcp get xiaomi-scale
codex
```

Ask Codex to use `xiaomi-scale` only when you intend to share the requested profile or measurement data with the agent. See the [official OpenAI documentation](https://developers.openai.com/) for Codex updates.

## Claude Code

Claude Code supports remote HTTP MCP servers and expands environment variables in header values. Add a user-scope configuration so it remains private to your account:

```bash
claude mcp add-json --scope user xiaomi-scale \
  '{"type":"http","url":"http://127.0.0.1:8080/mcp","headers":{"Authorization":"Bearer ${XIAOMI_SCALE_MCP_TOKEN}"}}'
```

Check the configuration and open Claude Code from the same environment:

```bash
claude mcp get xiaomi-scale
claude
```

Use `/mcp` in Claude Code to inspect the server and its tools. Do not use `--scope project` for this server because a project configuration can be committed or shared. For current Claude Code behavior, see [Connect Claude Code to tools via MCP](https://code.claude.com/docs/en/mcp).

## Visual Studio Code

Use a user-profile MCP configuration so the token is stored in VS Code’s secret storage rather than in a workspace file:

1. Run **MCP: Open User Configuration** from the Command Palette.
2. Add this configuration, preserving any existing `servers` or `inputs` entries:

   ```json
   {
     "inputs": [
       {
         "type": "promptString",
         "id": "xiaomi-scale-mcp-token",
         "description": "Xiaomi Scale MCP bearer token",
         "password": true
       }
     ],
     "servers": {
       "xiaomi-scale": {
         "type": "http",
         "url": "http://127.0.0.1:8080/mcp",
         "headers": {
           "Authorization": "Bearer ${input:xiaomi-scale-mcp-token}"
         }
       }
     }
   }
   ```

3. Start or enable `xiaomi-scale` when prompted, enter the bearer token, and trust the server only after reviewing its configuration.
4. In Chat, use **Configure Tools** to confirm that the Xiaomi Scale MCP tools are available.

VS Code supports password input variables for sensitive MCP settings. Refer to the [VS Code MCP configuration reference](https://code.visualstudio.com/docs/agents/reference/mcp-configuration) for current configuration options.

## Other MCP-compatible agents

Use a client that supports remote Streamable HTTP MCP servers with custom headers. Configure it with the shared URL and bearer header above. This server uses static bearer authentication, not an OAuth browser flow.

Keep the client configuration private whenever it contains a token. Prefer the client’s secret storage or environment-variable interpolation; otherwise, avoid committing its configuration file. After connecting, confirm that the client discovers `get_users`, `get_weight`, and `get_historical_weights` before requesting measurements.
