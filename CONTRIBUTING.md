# Contributing to Xiaomi Scale MCP

Thanks for your interest in contributing. Bug reports, documentation improvements, focused fixes, and well-scoped features are welcome.

## Before you start

- Open an issue before starting a large feature or behavior change so the approach can be discussed.
- Keep changes focused. Avoid unrelated refactors, formatting churn, and generated files in the same pull request.
- Do not use a real Xiaomi account, token, profile ID, device ID, or measurement data in committed code, tests, logs, screenshots, or issue reports.

## Development setup

The repository is a Rust workspace. Install a current Rust toolchain, then build it from the repository root:

```bash
cargo build --workspace
```

For local server work, copy the example configuration without committing the result:

```bash
cp config.example.toml config.toml
cargo run -p xiaomi_scale_mcp
```

Set a local `server.authorization_token` in `config.toml`. Xiaomi account authorization is interactive: start the server and enter `auth` in its console. Do not add a Xiaomi token to `config.toml`; the server stores the generated token in the operating-system credential store.

## Code and tests

Follow standard Rust conventions and keep responsibilities separated:

- Keep MCP handlers thin; data access, caching, and Xiaomi domain mapping belong in `xiaomi-scale-mcp/src/dal/`.
- Keep Xiaomi encryption, nonce generation, request signing, and response decoding in `xiaomi-client/src/encryption.rs`.
- Add focused regression tests next to the code they cover, especially for parsing, token handling, encoding, signing, caching, and request construction.
- Keep `xiaomi-client-cli` as manual demonstration code; verify it by compiling the workspace and testing only with your own local account.

Run the full local check suite before opening a pull request:

```bash
cargo fmt --all -- --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
git diff --check
```

When a change affects authentication or Xiaomi network behavior, also describe the manual verification you performed. Never paste captured HTTP traffic or debug output that might contain credentials or device identifiers.

## Security and privacy

- Keep `config.toml`, `.env` files, credentials, and tokens local. Use `config.example.toml` only for placeholders.
- The MCP bearer token in `server.authorization_token` is separate from the Xiaomi account token. Do not expose either one.
- Do not persist Xiaomi passwords, pass tokens, cookies, signed parameters, or encrypted payloads in the cache.
- Treat cached scale profiles and measurements as sensitive health data. Do not attach the `data/` directory to issues or pull requests.
- Report suspected security vulnerabilities privately to the repository owner instead of opening a public issue.

## Pull requests

Use a short, imperative commit subject, for example `Add token login helper`. In the pull request description, include:

1. What changed and why.
2. The validation commands you ran and their results.
3. Any required environment variables, configuration changes, or manual Xiaomi account verification.
4. Compatibility, security, or behavior risks when the change touches authentication, encryption, caching, or network calls.

Be ready to update tests and documentation when review reveals a missing case or unclear behavior.

## Getting help

Open a [GitHub issue](https://github.com/dolczykk/xiaomi-scale-mcp/issues) for bugs, feature requests, or questions that do not contain sensitive data.
