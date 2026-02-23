# Connecting to production Redis (north-cloud)

The VR app can subscribe to the same Redis Pub/Sub channels used by north-cloud in production. In the headset, a **Redis status bar** (quad above the UML diagram) shows red when disconnected, yellow when connecting, and green when connected. Production runs at `jones@northcloud.biz:/opt/north-cloud`; Redis runs on the host and is not exposed to the internet. Use SSH local port forwarding so the app connects to `127.0.0.1:6379` on your machine, which is forwarded to Redis on the server.

## 1. Start the SSH tunnel

From your machine, in a dedicated terminal (leave it running):

```bash
ssh -L 6379:localhost:6379 -N jones@northcloud.biz
```

`-N` means no remote command—only the port forward. Keep this session open while using the VR app or redis-cli.

Alternatively, run the helper script:

```powershell
.\scripts\tunnel-production-redis.ps1
```

## 2. Get the production Redis password

Use the same `REDIS_PASSWORD` (or equivalent) used by north-cloud on the server—e.g. from `/opt/north-cloud` env or your secrets. **Do not commit this.** Set it in your shell or in a local `.env` that is gitignored.

## 3. Run the VR app

In another terminal, from the northcloud-oculus repo:

```powershell
$env:REDIS_ADDR = "127.0.0.1:6379"
$env:REDIS_PASSWORD = "<production-redis-password>"
$env:REDIS_CHANNELS = "streetcode:crime_feed"   # or comma-separated list from publisher channels
cargo run --release
```

Or use `task run:prod` if you have a `.env` with `REDIS_ADDR`, `REDIS_PASSWORD`, and `REDIS_CHANNELS`.

Channel names come from the publisher’s `channels.redis_channel` (e.g. `streetcode:crime_feed`). Use a comma-separated list to subscribe to multiple channels. If you omit `REDIS_CHANNELS`, the app still connects using the default channel `test` and the status bar will show green when the tunnel is up.

## 4. Optional: redis-cli

With the same tunnel running:

```bash
redis-cli -h 127.0.0.1 -p 6379 -a <password> PING
```

Or set `REDIS_PASSWORD` in your environment and use `-a $REDIS_PASSWORD` (Unix) / `-a $env:REDIS_PASSWORD` (PowerShell) to debug or inspect channels.
