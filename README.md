# northcloud-oculus

UML diagram VR viewer targeting the Oculus Rift CV1 and Meta Quest 3 (via Quest Link). Built with **Bevy 0.18** and **bevy_mod_openxr**.

Renders a static diagram (class boxes + edges) in VR. **Redis status panel** (3D quad above the diagram) shows connection state in the headset: red = disconnected, yellow = connecting, green = connected. The app always tries to connect to Redis at `127.0.0.1:6379` (or `REDIS_ADDR`); if `REDIS_CHANNELS` is unset it subscribes to channel `test` so the bar reflects real connection state. Optional **live feed** of articles when `REDIS_CHANNELS` is set to your channels. Optional debug cube at (0, 0, -2); set `NORTHCLOUD_DEBUG_CUBE=0` or `false` to disable.

## Prerequisites

- **Windows 10/11** (the Oculus PC runtime only runs on Windows)
- **Oculus PC app** installed (provides the OpenXR runtime)
- **Rift CV1** connected (HDMI + 2-3 USB sensors), **or Meta Quest 3** connected via Quest Link (USB or Air Link)
- **Vulkan-capable GPU** — NVIDIA GTX 970+ or AMD equivalent
- **Rust stable toolchain** (1.77+) — install via [rustup](https://rustup.rs)
- **OpenXR loader** — the app loads `openxr_loader.dll` at runtime. The Vulkan SDK does *not* include it. Run once: `.\scripts\fetch-openxr-loader.ps1` (downloads the Khronos loader and copies it to `target\release\`). Re-run after `cargo clean`.

## Setting the Active OpenXR Runtime

The Oculus PC app registers itself as an OpenXR runtime during installation. If SteamVR is also installed, verify the active runtime:

**Registry check:**
```
HKLM\SOFTWARE\Khronos\OpenXR\1\ActiveRuntime
```
Should point to:
```
C:\Program Files\Oculus\Support\oculus-runtime\oculus_openxr_64.json
```

Or use the [OpenXR Developer Tools](https://store.steampowered.com/app/1854710/OpenXR_Developer_Tools/) to switch runtimes.

## Build & Run

```bash
cargo build --release
cargo run --release
```

Or use [Task](https://taskfile.dev): `task` (default) or `task run` to run; `task run:prod` to run with `REDIS_*` from `.env`. Use `--release` for VR performance (debug builds are too slow in-headset).

## What You Should See

- **In headset:** UML diagram (three colored boxes and two gray edges) in 3D; a **Redis status bar** (flat quad above the diagram) showing red when disconnected/disabled, yellow when connecting, green when connected; optional green debug cube at (0, 0, -2).
- **On desktop window:** If Redis is configured, live feed text and "Redis: …" status text appear on the window (2D text does not render in the XR view).
- **Exit:** Close the window, Ctrl+C in terminal, or remove headset.

## Redis connection and live feed

The app **always** attempts a Redis connection so the status bar reflects real state (green when Redis is reachable). If Redis is not running, the bar stays red; start Redis locally (e.g. `redis-server` or Choco `redis`) to see green.

- **REDIS_CHANNELS** — Comma-separated channel names for the live article feed (e.g. `articles:crime,articles:mining`). If **unset**, the app subscribes to channel `test` so the bar still shows connection state; no article feed until you set real channels.
- **REDIS_ADDR** — Default `127.0.0.1:6379`.
- **REDIS_PASSWORD** — Optional; required for production Redis.
- **REDIS_MAX_ITEMS** — Default 20 (max articles in the feed).

Connection is retried a few times at startup if Redis is not ready. See [docs/PRODUCTION_REDIS.md](docs/PRODUCTION_REDIS.md) for connecting to production Redis via SSH tunnel.

### Development: Simulating the message bus

To test the live feed and **animated message cards in VR** without a real north-cloud backend, run a dev publisher that pushes random JSON to Redis:

```bash
# Terminal 1: start Redis (if not already running), then the VR app
cargo run --release

# Terminal 2: publish random articles to channel "test" every 3 seconds
cargo run --bin redis_dev_publisher
```

With both running, the app (subscribing to `test` by default when `REDIS_CHANNELS` is unset) receives each message; new articles appear as **flying cards** that move from the right toward the feed panel, then disappear once they land. Dev publisher env: **REDIS_ADDR** (default `127.0.0.1:6379`), **REDIS_PASSWORD** (optional), **REDIS_CHANNEL** (default `test`), **PUBLISH_INTERVAL_SECS** (default `3`).

## How It Works

- **Bevy + bevy_mod_openxr** — Bevy handles rendering (wgpu); bevy_mod_openxr provides the OpenXR session, swapchain, and XR camera/views. We spawn world-space entities (diagram nodes, edges, light, Redis status quad, optional debug cube).
- **Diagram** — One Startup system spawns nodes as quads, edges as thin cuboids, and an optional debug cube. Marker components (`DiagramNode`, `DiagramEdge`, `DebugCube`) identify diagram entities for future interaction.
- **Redis status panel** — A 3D quad above the diagram shows connection state by color (red / yellow / green). Bevy’s `Text2d` is not rendered into the XR view, so the status is conveyed with the colored quad; the same status text is shown on the desktop window when present.
- **Live feed** — `redis_feed` module always subscribes to Redis (default channel `test` if `REDIS_CHANNELS` unset); connection is retried on failure. Received articles are shown as text on the desktop window; status bar shows connecting / connected / disconnected.

The Rift CV1's Constellation tracking and the Quest 3's inside-out tracking are fully abstracted by OpenXR; no code changes needed between headsets.

## Troubleshooting

| Problem | Solution |
|---------|----------|
| "Failed to load OpenXR loader" | Run `.\scripts\fetch-openxr-loader.ps1` to download and place `openxr_loader.dll` next to the exe. Ensure the Oculus PC app is installed for the runtime. |
| "No VR headset found" | Check USB sensors and HDMI, restart Oculus service |
| Vulkan/GPU errors | Install/update GPU drivers and Vulkan SDK |
| Black screen in headset | Verify Oculus is the active OpenXR runtime |
| Low framerate | Use `--release` build, check GPU is not thermal throttling |
| Quest 3 not detected via Link | Ensure Meta Quest Link app is running and set as active OpenXR runtime |

## Architecture

```
northcloud-oculus/
├── Cargo.toml           — Bevy 0.18, bevy_mod_xr, bevy_mod_openxr, openxr, redis, serde
├── Cargo.lock           — Pinned dependency versions
├── src/
│   ├── main.rs          — Bevy app: diagram, feed panel, Redis status, VR text quads, animated message cards
│   ├── redis_feed.rs    — Redis Pub/Sub subscriber, LiveFeedBuffer, recently_received for animation
│   ├── text_texture.rs  — Rasterize text to RGBA for VR quads
│   └── bin/
│       └── redis_dev_publisher.rs — Dev: publishes random articles to Redis for testing
├── assets/
│   └── fonts/           — FiraSans for feed/status text (desktop window)
├── Taskfile.yml        — task run, run:prod (with .env), fetch-openxr, tunnel
├── scripts/
│   └── fetch-openxr-loader.ps1 — Downloads openxr_loader.dll into target\release\
├── docs/
│   ├── PRODUCTION_REDIS.md — SSH tunnel + env for production Redis
│   └── plans/           — Design documents
└── .gitignore
```

## Dependencies

| Crate | Version | Purpose |
|-------|---------|---------|
| [bevy](https://crates.io/crates/bevy) | 0.18 | Game engine (ECS, rendering via wgpu) |
| [bevy_mod_xr](https://crates.io/crates/bevy_mod_xr) | 0.5 | XR API for Bevy |
| [bevy_mod_openxr](https://crates.io/crates/bevy_mod_openxr) | 0.5 | OpenXR backend for bevy_mod_xr |
| [openxr](https://crates.io/crates/openxr) | 0.21 | OpenXR bindings |
| [redis](https://crates.io/crates/redis) | 0.27 | Redis Pub/Sub for live feed |
| [serde](https://crates.io/crates/serde) / serde_json | 1.0 | JSON parsing of feed messages |
