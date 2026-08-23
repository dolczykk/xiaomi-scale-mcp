# Install Xiaomi Scale MCP

Use this guide to run Xiaomi Scale MCP from a GitHub Release binary or the published Docker image. For source builds, see the [README](../README.md#quick-start).

## Requirements

- A Xiaomi account with a compatible scale visible in Xiaomi Home. The current discovery flow supports devices whose model starts with `yunmai.scales.`.
- A working operating-system credential store. On Linux, this commonly means a Secret Service-compatible keyring.
- For Docker, a Linux host with Docker and access to the host D-Bus session.

## Configure the server

Both installation methods require a local `config.toml` with a strong MCP bearer token:

```toml
[server]
bind_address = "127.0.0.1:8080"
# Add the LAN IP or DNS name n8n uses when it connects from another machine.
allowed_hosts = ["localhost", "127.0.0.1", "::1"]
authorization_token = "replace-with-a-long-random-token"

[xiaomi]
sid = "xiaomiio"
region = "de"
```

`authorization_token` protects the MCP endpoint and is separate from the Xiaomi account token. `allowed_hosts` defaults to loopback hosts and protects against DNS-rebinding requests; add the LAN IP or hostname used by a trusted remote client such as n8n. `sid` and `region` are optional, non-secret Xiaomi settings. For available Xiaomi region values, see openHAB's [country server list](https://www.openhab.org/addons/bindings/miio/#country-servers).

Never commit `config.toml` or add a Xiaomi account token to it.

## Run a GitHub Release binary

Release tags publish prebuilt archives to [GitHub Releases](https://github.com/dolczykk/xiaomi-scale-mcp/releases). Download the archive for your platform, then verify and extract it:

Edit `config.toml` using the example above, then start the server:

```bash
./xiaomi_scale_mcp
```

The archive contains the binary and example config file which is called `config.example.toml`

## Run the published Docker image

GitHub Container Registry publishes `ghcr.io/dolczykk/xiaomi-scale-mcp` for Linux amd64 and arm64. Use a release tag such as `v0.1.0` for a pinned version, or `latest` for the current `main` build.

Create `config.toml` in the current directory using the configuration above, then run:

```bash
docker pull ghcr.io/dolczykk/xiaomi-scale-mcp:v0.1.0

docker run --rm -it \
  --network host \
  --user "$(id -u):$(id -g)" \
  -e DBUS_SESSION_BUS_ADDRESS="unix:path=/run/user/$(id -u)/bus" \
  -v "$PWD/config.toml:/app/config.toml:ro" \
  -v xiaomi-scale-mcp-data:/app/data \
  -v "/run/user/$(id -u)/bus:/run/user/$(id -u)/bus" \
  ghcr.io/dolczykk/xiaomi-scale-mcp:v0.1.0
```

Replace `v0.1.0` with the release tag you want to run. This command is Linux-specific because it forwards the host D-Bus credential-store socket. Use a release binary on macOS or adapt the credential-store integration for your environment.

### Use Docker Compose instead

Save this as `docker-compose.yml` beside your `config.toml`. It pulls the published image, persists the cache in a named volume, and forwards the Linux D-Bus session for the credential store:

```yaml
volumes:
  data:
services:
  xiaomi-scale-mcp:
    image: ghcr.io/dolczykk/xiaomi-scale-mcp:v0.1.0
    user: "${HOST_UID}:${HOST_GID}"
    restart: unless-stopped
    network_mode: host
    tty: true
    stdin_open: true
    environment:
      DBUS_SESSION_BUS_ADDRESS: "unix:path=/run/user/${HOST_UID}/bus"
    volumes:
      - ./config.toml:/app/config.toml:ro
      - data:/app/data
      - /run/user/${HOST_UID}/bus:/run/user/${HOST_UID}/bus
```

Start it with:

```bash
HOST_UID=$(id -u) HOST_GID=$(id -g) docker compose up
```

Replace `v0.1.0` with the release tag you want to run. Keep the terminal attached so you can enter `auth` in the server console.

## Authorize Xiaomi Home

The server opens an interactive console when it starts. Enter `auth` and complete any password, captcha, or verification-code prompts. The generated Xiaomi token is stored in the operating-system credential store.

Use these console commands afterward:

```text
status  Check whether a Xiaomi credential is stored
logout  Delete the stored Xiaomi credential
help    Show the command list
```

Once authorization succeeds, the MCP endpoint is available at `http://127.0.0.1:8080/mcp`. Configure an agent with the bearer token from `server.authorization_token`; see the [MCP configuration guide](mcp-configuration.md).
