# Xiaomi Scale MCP

[![CI](https://github.com/dolczykk/xiaomi-scale-mcp/actions/workflows/ci.yml/badge.svg)](https://github.com/dolczykk/xiaomi-scale-mcp/actions/workflows/ci.yml)
[![Docker build](https://github.com/dolczykk/xiaomi-scale-mcp/actions/workflows/build.yml/badge.svg)](https://github.com/dolczykk/xiaomi-scale-mcp/actions/workflows/build.yml)
[![Rust](https://img.shields.io/badge/Rust-2024-ed7e1f?logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![Model Context Protocol](https://img.shields.io/badge/MCP-Streamable_HTTP-6b46c1)](https://modelcontextprotocol.io/)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)

An authenticated [Model Context Protocol](https://modelcontextprotocol.io/) server for reading Xiaomi scale profiles and body-composition measurements from Xiaomi Home. It exposes a Streamable HTTP endpoint, keeps Xiaomi credentials in the operating-system credential store, and caches measurements locally to reduce upstream requests.

> This is an independent project and is not affiliated with, endorsed by, or supported by Xiaomi.

## Features

- Connect an MCP client to a bearer-token-protected Streamable HTTP endpoint.
- Discover Xiaomi scale profiles and retrieve their latest or historical measurements.
- Return weight, BMI, body-fat percentage, heart rate, muscle mass, and other metrics when Xiaomi provides them.
- Authenticate interactively, including captcha and verification-code challenges.
- Store only the generated Xiaomi account token in the system credential store; never in `config.toml`.
- Cache profile and measurement responses locally with a five-minute freshness window.
- Include a reusable Xiaomi Home client library plus demonstration and encryption helper CLIs.

```mermaid
flowchart LR
    Client[MCP client] -->|Bearer token| Server[Xiaomi Scale MCP\n/mcp]
    Server --> Tools[get_users\nget_weight\nget_historical_weights]
    Tools --> Cache[Local SurrealDB cache]
    Tools --> Xiaomi[Xiaomi Home API]
    Server --> Keyring[OS credential store]
```

## Requirements

- A current Rust toolchain with Cargo when building from source.
- A Xiaomi account with a compatible scale visible in Xiaomi Home. The current discovery flow supports devices whose model starts with `yunmai.scales.`.
- A working operating-system credential store. On Linux, this commonly means a Secret Service-compatible keyring.

## Install a published release

For prebuilt GitHub Release binaries or the published multi-architecture Docker image, follow the [setup guide](docs/setup.md).

## Quick start

Clone the repository, create a local configuration file, and set a strong MCP bearer token:

```bash
git clone https://github.com/dolczykk/xiaomi-scale-mcp.git
cd xiaomi-scale-mcp
cp config.example.toml config.toml
```

Edit `config.toml`:

```toml
[server]
bind_address = "127.0.0.1:8080"
authorization_token = "random-token"

[xiaomi]
sid = "xiaomiio"
region = "de"
```

`authorization_token` is required. `sid` and `region` are optional, non-secret Xiaomi client settings. For the available Xiaomi region values, see openHAB's [country server list](https://www.openhab.org/addons/bindings/miio/#country-servers).

Build and start the server:

```bash
cargo run -p xiaomi_scale_mcp
```

The server starts at `http://127.0.0.1:8080/mcp` and opens an interactive console in the same terminal. Enter `auth` to sign in to Xiaomi Home. The console supports:

```text
auth    Authorize a Xiaomi account
status  Check whether a Xiaomi credential is stored
logout  Delete the stored Xiaomi credential
help    Show the command list
```

After authentication succeeds, connect an MCP client that supports Streamable HTTP and custom headers using equivalent settings to:

```json
{
  "url": "http://127.0.0.1:8080/mcp",
  "headers": {
    "Authorization": "Bearer replace-with-a-long-random-token"
  }
}
```

Keep the server bound to localhost unless you explicitly understand the network and access-control implications of exposing health data.

## Configure an AI agent

Connect Codex, Claude Code, VS Code, or another compatible MCP client using the [MCP configuration guide](docs/mcp-configuration.md). It provides token-safe setup steps for each client and the shared Streamable HTTP connection details.

## MCP tools

Call `get_users` first and use the returned `profile_id` in the measurement tools.

| Tool | Input | Returns |
| --- | --- | --- |
| `get_users` | None | Available profiles, scale name and model, optional height and target weight, and last-update timestamp. |
| `get_weight` | `profile_id` | The most recent measurement for a profile. |
| `get_historical_weights` | `profile_id`, optional `before`, optional `page_size` | A page of measurements before a Unix timestamp in milliseconds. `page_size` defaults to 20 and must be from 1 to 100. |

Measurements include `weight_kg` and `measured_at_seconds`, plus optional values such as `bmi`, `body_fat_percent`, `heart_rate_bpm`, body-water percentage, muscle mass, bone mass, visceral fat, protein percentage, basal metabolic rate, metabolic age, and body score.

## Cache and privacy

The server caches profile and measurement responses in `./data/xiaomi-scale-mcp`. Cached data is considered fresh for five minutes. Records older than seven days are removed when the cache initializes. If the on-disk SurrealKV cache cannot be opened, the server logs a warning and continues with an in-memory cache.

The cache contains retrieved profile and health data, so protect the host and the `data/` directory accordingly. Xiaomi passwords, pass tokens, cookies, signed parameters, and encrypted request payloads are not stored in the cache. The generated Xiaomi token is stored only through the operating-system credential-store integration.

## Docker on Linux

The included Compose setup runs the server with host networking and forwards the host D-Bus session so the container can use a Linux Secret Service credential store. Create `config.toml` as above, then run:

```bash
HOST_UID=$(id -u) HOST_GID=$(id -g) docker compose up --build
```

Use the interactive container console to enter `auth`. This Compose configuration is designed for Linux hosts with a compatible desktop keyring; use the native Rust command on macOS or Windows, or adapt the credential-store integration for your environment.

## Workspace tools

This workspace also contains supporting binaries:

| Command | Purpose |
| --- | --- |
| `cargo run -p xiaomi-client-cli` | Guided Xiaomi Home demonstration flow for account, device, scale, and measurement discovery. It may print sensitive identifiers, so do not share captured output. |
| `cargo run -p xiaomi-encryption-cli -- --help` | Interactive or flag-driven helper for Xiaomi request encryption and response decryption. |
| `cargo run -p xiaomi-encryption-cli -- encrypt --help` | Show encryption options. |
| `cargo run -p xiaomi-encryption-cli -- decrypt --help` | Show decryption options. |

## Development

Run these checks from the workspace root before opening a pull request:

```bash
cargo fmt --all -- --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

Tests exercise parsing, authentication-token handling, encryption, MCP schemas, repository behavior, and cache persistence using an in-memory SurrealDB engine. The Xiaomi Home demonstration CLI is intended for manual account testing.

## Troubleshooting

See the [troubleshooting guide](docs/troubleshooting.md) for authentication, connectivity, scale-discovery, cache, and Docker/keyring issues.

## Contributing and support

Bug reports, documentation improvements, and focused pull requests are welcome. See [CONTRIBUTING.md](CONTRIBUTING.md) for development, testing, security, and pull-request guidance. Please open an issue before large changes, keep secrets and personal health data out of reports, and include the development checks above with your pull request.

For help, open a [GitHub issue](https://github.com/dolczykk/xiaomi-scale-mcp/issues) with the server version, operating system, non-sensitive logs, and steps to reproduce the problem.

## Project status

This is an early `0.1.x` project. Xiaomi Home endpoints and the MCP-facing interface may evolve, so pin revisions when integrating it into an automated workflow.

## Acknowledgements

- [SmartScaleConnect](https://github.com/AlexxIT/SmartScaleConnect/tree/master) for insights into the initial authentication flow.
- [Xiaomi Cloud Tokens Extractor](https://github.com/PiotrMachowski/Xiaomi-cloud-tokens-extractor/tree/master) for request encryption and response decryption references.
- And ny own reverse engineering of the Xiaomi Home API ;)

## License

Licensed under the [MIT License](LICENSE).
