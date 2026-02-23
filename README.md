# northcloud-oculus

UML diagram VR viewer targeting the Oculus Rift CV1 and Meta Quest 3 (via Quest Link). Built with **Bevy 0.18** and **bevy_mod_openxr**.

Renders a static diagram (class boxes + edges) in VR. Optional **Redis live feed**: set `REDIS_ADDR` and `REDIS_CHANNELS` to subscribe to north-cloud Redis Pub/Sub; a **Redis status panel** (3D quad above the diagram) shows connection state in the headset (red = disconnected, yellow = connecting, green = connected). Optional debug cube at (0, 0, -2); set `NORTHCLOUD_DEBUG_CUBE=0` or `false` to disable.

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

Use `--release` for VR performance (debug builds are too slow in-headset).

## What You Should See

- **In headset:** UML diagram (three colored boxes and two gray edges) in 3D; a **Redis status bar** (flat quad above the diagram) showing red when disconnected/disabled, yellow when connecting, green when connected; optional green debug cube at (0, 0, -2).
- **On desktop window:** If Redis is configured, live feed text and "Redis: …" status text appear on the window (2D text does not render in the XR view).
- **Exit:** Close the window, Ctrl+C in terminal, or remove headset.

## Optional: Redis live feed

To subscribe to north-cloud Redis Pub/Sub (e.g. article feeds):

- **Required:** `REDIS_CHANNELS` — comma-separated channel names (e.g. `articles:crime,articles:mining`). If unset, the live feed is disabled and the status bar shows disconnected.
- **Optional:** `REDIS_ADDR` (default `127.0.0.1:6379`), `REDIS_PASSWORD`, `REDIS_MAX_ITEMS` (default 20).

See [docs/PRODUCTION_REDIS.md](docs/PRODUCTION_REDIS.md) for connecting to production Redis via SSH tunnel.

## How It Works

- **Bevy + bevy_mod_openxr** — Bevy handles rendering (wgpu); bevy_mod_openxr provides the OpenXR session, swapchain, and XR camera/views. We spawn world-space entities (diagram nodes, edges, light, Redis status quad, optional debug cube).
- **Diagram** — One Startup system spawns nodes as quads, edges as thin cuboids, and an optional debug cube. Marker components (`DiagramNode`, `DiagramEdge`, `DebugCube`) identify diagram entities for future interaction.
- **Redis status panel** — A 3D quad above the diagram shows connection state by color (red / yellow / green). Bevy’s `Text2d` is not rendered into the XR view, so the status is conveyed with the colored quad; the same status text is shown on the desktop window when present.
- **Live feed** — Optional `redis_feed` module subscribes to Redis Pub/Sub; received articles are shown as text on the desktop window and drive the status (connecting / connected / disconnected).

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
│   ├── main.rs          — Bevy app: add_xr_plugins, setup_diagram, setup_feed_panel, setup_redis_status_panel
│   └── redis_feed.rs    — Redis Pub/Sub subscriber, LiveFeedBuffer, connection status
├── assets/
│   └── fonts/           — FiraSans for feed/status text (desktop window)
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
