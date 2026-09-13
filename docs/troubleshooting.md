# Troubleshooting

This guide covers the most common setup and runtime problems. Do not include Xiaomi passwords, account tokens, cookies, signed parameters, encrypted payloads, profile IDs, or health measurements when sharing logs or opening an issue.

## Authentication

| Problem | What to check |
| --- | --- |
| `Xiaomi account is not authorized` | Start the server in an interactive terminal and enter `auth`. Run `status` afterward to confirm that the credential was stored. |
| Xiaomi login challenge | Complete the captcha or verification-code prompt in the server console. Captcha images are written to a temporary file and the path is displayed. |
| Credential-store error | Verify that the operating system credential store is available. On Linux, run a Secret Service-compatible keyring in the user session. |

## MCP connection

| Problem | What to check |
| --- | --- |
| `401 Unauthorized` from `/mcp` | Ensure the client sends `Authorization: Bearer <server.authorization_token>` and that the configured token is non-empty. |
| Connection refused | Confirm the server is running, the configured `bind_address` is correct, and the client uses the `/mcp` path. The default endpoint is `http://127.0.0.1:8080/mcp`. |
| Client cannot connect remotely | The example configuration intentionally binds to localhost. Only expose the service on a trusted network with an appropriately strong bearer token. |

## Scale and measurement data

| Problem | What to check |
| --- | --- |
| No compatible scale or profile | Confirm that the Xiaomi account can see a `yunmai.scales.*` device in Xiaomi Home and review the `[xiaomi]` region settings. |
| A profile has no measurements | Xiaomi Home may not have returned data for that profile. Call `get_users` first, then pass one returned `profile_id` to the measurement tools. |
| Measurements appear stale | Responses are cached for five minutes. Wait for the cache window to expire, then retry the tool call. |

## Logging

| Problem | What to check |
| --- | --- |
| Application logs clutter the console | Set `logging.output = "file"`, which is the default, or set `logging.enabled = false`. Interactive prompts and command results remain visible. |
| Log files are missing | Confirm logging is enabled, `output` is `file` or `both`, and the configured `directory` is writable. The default location is `./data/logs`. |
| The log directory keeps growing | Daily log files are retained indefinitely. Remove older files manually or manage them with an external retention policy. |
| Unexpected log level | Check `logging.level` and whether `RUST_LOG` overrides it in the server environment. |
| Server fails while initializing logging | Check the configured log-directory path and permissions. Requested file logging does not fall back to console output. |

## Cache and Docker

| Problem | What to check |
| --- | --- |
| Disk cache cannot be opened | Check permissions for `./data/xiaomi-scale-mcp` and whether another process owns the cache lock. The server falls back to in-memory caching when disk initialization fails. |
| Docker authentication cannot access the keyring | The provided Compose configuration is for Linux hosts: set `HOST_UID` and `HOST_GID`, ensure the user D-Bus socket exists, and run a Secret Service-compatible keyring. Use the native Rust command on macOS or Windows. |
