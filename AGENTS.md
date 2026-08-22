# Repository Guidelines

## Project Overview

This repository is a Rust workspace for interacting with Xiaomi Home scale data.

The minimum supported Rust version is 1.90.0. All workspace crates use Rust Edition 2024.

The workspace contains:

* `xiaomi-scale-mcp`: authenticated MCP server with Xiaomi authentication, credential-backed sessions, and SurrealDB-backed response caching.
* `xiaomi-client`: reusable Xiaomi Home client library.
* `xiaomi-client-cli`: interactive demonstration client for Xiaomi Home scale flows.
* `xiaomi-encryption-cli`: helper CLI for Xiaomi request encryption and response decryption.

Keep responsibilities within their existing crate and module boundaries. Avoid unrelated refactors when making targeted changes.

## Project Structure & Module Organization

* `Cargo.toml` is the workspace manifest.
* `Cargo.lock` pins the workspace dependency graph.

### `xiaomi-scale-mcp`

* `xiaomi-scale-mcp/src/main.rs` loads configuration and starts the authenticated streamable HTTP MCP server.
* `xiaomi-scale-mcp/src/config.rs` parses and validates server authorization settings and optional non-secret Xiaomi `sid` and `region` settings from TOML.
* `xiaomi-scale-mcp/src/auth.rs` authenticates `/mcp` requests using the configured bearer token.
* `xiaomi-scale-mcp/src/console.rs` runs the interactive Xiaomi authentication command loop alongside the MCP server.
* `xiaomi-scale-mcp/src/credentials.rs` stores the generated Xiaomi token in the operating system credential store.
* `xiaomi-scale-mcp/src/state.rs` initializes shared cache storage and manages the resettable, credential-backed repository.
* `xiaomi-scale-mcp/src/tools.rs` contains structured Xiaomi weight MCP tools and delegates data access to the repository.
* `xiaomi-scale-mcp/src/models.rs` contains MCP request and response schemas.
* `xiaomi-scale-mcp/src/utils.rs` contains shared profile ID, timestamp, and parsing helpers.

### `xiaomi-scale-mcp/src/dal`

The `dal/` module owns persistent cache and repository behavior.

* `cache.rs` contains `CacheDal`, SurrealDB engine setup, schema initialization, cache persistence operations, and cache storage tests.
* `consts.rs` centralizes database, table, schema, TTL, and retention constants.
* `repositories.rs` owns cache refresh policy, lazy Xiaomi authentication, scale/profile discovery, request construction, and domain mapping.
* `utils.rs` contains shared timestamp, cache-key, and deterministic record-ID helpers.

Keep SurrealDB engine and query details isolated in this module.

### `xiaomi-client`

* `xiaomi-client/src/lib.rs` defines the crate API surface and re-exports shared client types.
* `xiaomi-client/src/base/mod.rs` owns `Client`, `Result`, shared service constants, client configuration, and default Xiaomi Home headers and cookies.
* `xiaomi-client/src/auth/` contains login response types and parsing in `mod.rs`, login flows in `login.rs`, and authentication cookie parsing in `cookies.rs`.
* `xiaomi-client/src/home/` contains typed Xiaomi Home requests and responses grouped by API concern.
* `xiaomi-client/src/encryption.rs` is the canonical location for Xiaomi RC4 handling, nonce generation, signing, encrypted parameter generation, and response decoding.
* Supporting modules such as `errors.rs` and `utils.rs` remain at the crate root.

Inline unit tests live next to the code they exercise, primarily in:

* `xiaomi-client/src/auth/`
* `xiaomi-client/src/encryption.rs`
* `xiaomi-client/src/utils.rs`
* `xiaomi-client/src/home/`

### CLI Crates

* `xiaomi-encryption-cli/src/main.rs` contains the Xiaomi encryption/decryption command flow.
* `xiaomi-encryption-cli/src/parser.rs` contains clap argument structs, interactive prompts, and input validation helpers.
* `xiaomi-client-cli/src/main.rs` contains the interactive Xiaomi Home demonstration entry point.
* `xiaomi-client-cli/src/app.rs` orchestrates login challenges, scale/account selection, and the guided Xiaomi Home API flow.
* `xiaomi-client-cli/src/prompt.rs` contains interactive terminal input helpers.

## Build & Validation

Use the following commands from the repository root:

```bash
cargo build --workspace
cargo test --workspace
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

Before finishing a change, run the relevant validation commands. Prefer running the full workspace test suite for changes that affect shared code, authentication, networking, encryption, persistence, or MCP behavior.

Do not weaken, remove, or bypass tests merely to make a change pass.

## Running the Applications

Start the MCP server:

```bash
cargo run -p xiaomi_scale_mcp
```

Start the Xiaomi encryption helper:

```bash
cargo run -p xiaomi-encryption-cli
```

Show its command help:

```bash
cargo run -p xiaomi-encryption-cli -- --help
cargo run -p xiaomi-encryption-cli -- encrypt --help
cargo run -p xiaomi-encryption-cli -- decrypt --help
```

Start the interactive Xiaomi Home demonstration:

```bash
cargo run -p xiaomi-client-cli
```

Show its command metadata without starting the interactive flow:

```bash
cargo run -p xiaomi-client-cli -- --help
```

## Runtime & Configuration

* The MCP server loads `config.toml` by default.
* `MCP_CONFIG_PATH` may be used to select a configuration file outside the repository root.
* `config.example.toml` documents supported configuration fields.
* `server.authorization_token` is required.
* Every MCP request must include:

```text
Authorization: Bearer <server.authorization_token>
```

* `[xiaomi].sid` and `[xiaomi].region` are optional and non-secret.
* The Xiaomi client generates its own device ID.
* The server console supports `auth`, `status`, `logout`, and `help`.
* `auth` handles Xiaomi password, captcha, and verification challenges.
* The MCP response cache uses embedded SurrealKV at `./data/xiaomi-scale-mcp`.
* If disk-backed initialization fails, the cache falls back to an in-memory SurrealDB engine.

## Coding Style & Module Boundaries

Follow standard Rust conventions:

* 4-space indentation.
* `rustfmt` defaults.
* `snake_case` for functions and modules.
* `PascalCase` for types.
* `SCREAMING_SNAKE_CASE` for constants.

Additional repository rules:

* Prefer small, focused changes over broad refactors.
* Do not refactor unrelated code while addressing a targeted task.
* Keep module names short and descriptive.
* Prefer shared helpers over duplicated parsing, encoding, cryptographic, or request-building logic.
* Keep MCP handlers thin.
* Xiaomi calls, cache behavior, refresh policy, and domain mapping belong in `xiaomi-scale-mcp/src/dal/repositories.rs`.
* Keep SurrealDB engine and query details inside `xiaomi-scale-mcp/src/dal/`.
* Keep Xiaomi encryption and request-signing logic centralized in `xiaomi-client/src/encryption.rs`.
* Do not duplicate Xiaomi cryptographic logic in CLI crates.
* Use clap derive structs in `xiaomi-encryption-cli/src/parser.rs` for CLI arguments.
* Use explicit `XiaomiError` variants instead of stringly typed failures where practical.

Preserve existing public APIs, MCP tool names and schemas, authentication behavior, configuration compatibility, and cache semantics unless the task explicitly requires a breaking change.

## Testing Guidelines

* Prefer focused unit tests beside the implementation they cover.
* Name tests after the behavior being verified rather than implementation details.
* Add regression tests for bugs involving parsing, encoding, request signing, authentication, or token handling.
* Test repository persistence and cache behavior using SurrealDB's in-memory engine.
* Run `cargo test --workspace` before opening a PR.
* Keep `xiaomi-client-cli` as demonstration code without unit tests unless its role changes.
* Verify `xiaomi-client-cli` through workspace compilation and documented manual Xiaomi account testing when applicable.

Changes involving network calls, authentication, encryption, caching, or Xiaomi response mapping should include targeted regression coverage whenever feasible.

## Security Requirements

Never commit:

* credentials
* tokens
* `config.toml`
* `.env` files
* captured authenticated responses containing sensitive account or device data

Use `config.example.toml` for documented placeholders.

Additional security rules:

* Xiaomi passwords, captcha answers, and verification codes are transient.
* Only the generated Xiaomi token should be stored in the operating system credential store.
* Never persist Xiaomi pass tokens, cookies, signed parameters, or encrypted request payloads in the MCP response cache.
* Treat Xiaomi endpoints, authentication cookies, request signing, and encrypted payload logic as sensitive code.
* Prefer small, reviewable changes in `xiaomi-client/src/` when modifying authentication or cryptographic behavior.
* Do not log secrets or add debug output containing authentication material.
* The Xiaomi Home demo may print debug responses containing device tokens and identifiers. Never commit, publish, or include captured output in issues or PRs without sanitizing it first.

## Agent Change Guidelines

When modifying this repository:

* Make the smallest change that satisfies the task.
* Follow existing architectural boundaries.
* Avoid introducing new abstractions unless they clearly reduce duplication or complexity.
* Do not change public behavior incidentally while refactoring internals.
* Do not silently broaden configuration formats or accept deprecated fields.
* Do not duplicate Xiaomi request signing, encryption, authentication, or persistence logic.
* Preserve error context and prefer typed errors.
* Add or update tests for behavioral changes.
* Run formatting, linting, compilation, and tests appropriate to the scope of the change.

If a requested change conflicts with a security rule or an established architectural boundary, prefer preserving the security property and call out the conflict explicitly.

## Commit & Pull Request Guidelines

For the full contributor workflow, see [CONTRIBUTING.md](CONTRIBUTING.md).

Use Conventional Commits with short, imperative subjects, for example:

```text
feat(auth): add token login helper
```

Pull requests should include:

* a clear summary of the change
* relevant validation commands that were run
* required environment variables or configuration changes
* manual verification steps when applicable

If a change affects network calls, authentication, encryption, Xiaomi account behavior, or persisted data, document the expected Xiaomi account setup and any risks to existing behavior.
