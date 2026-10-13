# Self-Hosting the Signaling Server

Relay clients use `ws://localhost:8080` by default. To use a self-hosted server, set the Signaling Server URL in the client Settings. For a public server, put a TLS-terminating reverse proxy in front of the Go server and configure clients with its `wss://` URL.

## Build and run

From the repository root:

```bash
cd server
go build -o relay-server .
./relay-server
```

The server listens on `:8080` by default. Its WebSocket route is `/ws/{code}` and its health endpoint is `GET /health`.

## Configuration

The server reads these environment variables. Command-line flags override the environment values.

| Environment variable | Default | Command-line flag | Purpose |
| --- | --- | --- | --- |
| `RELAY_ADDR` | `:8080` | `--addr` | HTTP listen address |
| `RELAY_MAX_SESSIONS` | `1000` | `--max-sessions` | Maximum concurrent sessions |
| `RELAY_SESSION_TTL` | `10m` | `--session-ttl` | Session lifetime, as a Go duration |
| `RELAY_RATE_LIMIT` | `10485760` | `--relay-rate-limit` | Relay bandwidth limit in bytes per second |

Example:

```bash
RELAY_ADDR=127.0.0.1:8080 \
RELAY_MAX_SESSIONS=1000 \
RELAY_SESSION_TTL=10m \
RELAY_RATE_LIMIT=10485760 \
./relay-server
```

The server does not terminate TLS itself. For public use, configure a reverse proxy to forward HTTP and WebSocket upgrades to the server, and restrict direct access to its listener as appropriate for your deployment.

## Health check

```bash
curl -fsS http://localhost:8080/health
```

The response reports server health and the current active-session count.

## Operational notes

- Sessions are held in memory and are not shared across server instances.
- Configure the client URL with `wss://` when the reverse proxy provides TLS.
- The signaling server forwards peer signaling and relayed ciphertext. It can see participant network addresses and transfer metadata; it does not decrypt file contents.
- Choose session and relay limits for the capacity of the host. The defaults are configuration values, not a capacity guarantee.
