# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

UML diagram VR viewer targeting the Oculus Rift CV1 and Meta Quest 3 (via Quest Link). Built with **Bevy 0.18** and **bevy_mod_openxr** (OpenXR backend). Rust (stable 1.77+). Runs on Windows with an Oculus runtime. **Redis**: app always tries to connect to Redis (default `127.0.0.1:6379`); if `REDIS_CHANNELS` is unset it subscribes to channel `test` so the **Redis status panel** (3D quad above the diagram) shows real connection state in VR (red / yellow / green). Set `REDIS_CHANNELS` for live article feed. Connection is retried on failure. Run via `task` or `cargo run --release`.

## Build & Run

```bash
cargo build                          # debug build
cargo build --release                # release build (recommended for VR)
cargo run --release                 # build + run (requires Oculus runtime + openxr_loader.dll)
```

**First-time / after `cargo clean`:** Run `.\scripts\fetch-openxr-loader.ps1` so `openxr_loader.dll` is in `target\release\` (Vulkan SDK does not include it).

## Architecture

- **Bevy + OpenXR:** The app uses `bevy_mod_xr` and `bevy_mod_openxr`; `add_xr_plugins(DefaultPlugins.build().disable::<PipelinedRenderingPlugin>())` wires the XR session and rendering. The plugin owns the XR camera/views; we spawn world-space entities (diagram nodes, edges, Redis status quad, optional debug cube, light).
- **Diagram:** One `setup_diagram` Startup system builds the scene: `sample_diagram()` returns nodes and edges; we spawn nodes as quads (`Rectangle`), edges as thin `Cuboid`s, and an optional debug cube. Marker components `DiagramNode`, `DiagramEdge`, `DebugCube` identify diagram entities for future interaction/cleanup.
- **Redis status panel:** A 3D quad (`RedisStatusQuad`) above the diagram shows connection state by material color. **Text2d is not rendered in the XR view** (only on the desktop window), so status in VR is conveyed via the colored quad; `RedisStatusMaterials` resource holds the four material handles; `update_redis_status_panel` updates both the Text2d and the quad’s `MeshMaterial3d<StandardMaterial>`.
- **Live feed:** `src/redis_feed.rs` — `RedisFeedConfig::from_env()` (default channel `test` when `REDIS_CHANNELS` unset), `spawn_subscriber()` (retries connection), `LiveFeedBuffer` (resource), `RedisConnectionStatus`. Articles are shown as `Text2d` on the desktop window via `LiveFeedPanel`; the buffer is drained in `drain_redis_feed`.
- **No custom Vulkan/shaders:** Rendering is via Bevy/wgpu; no build-time shader compilation.

## Key Design Decisions

- **Bevy over raw OpenXR+Vulkan** — Engine handles rendering and lifecycle; bevy_mod_openxr integrates the OpenXR swapchain and session.
- **Runtime-loaded OpenXR loader** — Ensure `openxr_loader.dll` is on PATH or in `target\release\`; use `scripts/fetch-openxr-loader.ps1` if needed.
- **Single setup system** — All diagram content is spawned in one system to keep "diagram as a scene" and make swapping in a file-based loader straightforward.

## Conventions

- **Commits:** conventional commits (`feat:`, `fix:`, `docs:`, `chore:`, `refactor:`)
- **Error handling:** Bevy’s normal patterns; no `anyhow` in the Bevy app.
- **Logging:** Bevy’s `log` / `tracing` as needed.
