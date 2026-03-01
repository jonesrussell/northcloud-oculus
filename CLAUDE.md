# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

VR observability cockpit targeting the Oculus Rift CV1 and Meta Quest 3 (via Quest Link). Built with **Bevy 0.18** and **bevy_mod_openxr** (OpenXR backend). Rust (stable 1.77+). Runs on Windows with an Oculus runtime.

## Build & Run

```bash
cargo build                          # debug build
cargo build --release                # release build (recommended for VR)
cargo run --release                 # build + run (requires Oculus runtime + openxr_loader.dll)
```

**First-time / after `cargo clean`:** Run `.\scripts\fetch-openxr-loader.ps1` so `openxr_loader.dll` is in `target\release\` (Vulkan SDK does not include it).

## Architecture

- **Bevy + OpenXR:** The app uses `bevy_mod_xr` and `bevy_mod_openxr`; `add_xr_plugins(DefaultPlugins.build().disable::<PipelinedRenderingPlugin>())` wires the XR session and rendering. The plugin owns the XR camera/views; we spawn world-space entities.
- **WorldPanel:** Renders egui UI to textures displayed on 3D quads in world space. A UI camera renders to a texture, which is applied to a quad mesh. The demo panel in `main.rs` is a prototype.
- **Panels:** `MapPanel` displays NodeMarkers on a world map; `DetailPanel` shows node details when selected.
- **NodeMarker:** Color-coded status indicators (healthy/warning/critical) with pulse animations.
- **Data Ingestion:** Async polling from Prometheus, Grafana, and Loki APIs to fetch node status and metrics.
- **Interaction:** VR controller tracking via `bevy_xr_utils`, raycasting against `RaycastTarget` entities, hover/selection state management.
- **No custom Vulkan/shaders:** Rendering is via Bevy/wgpu; no build-time shader compilation.

## Key Design Decisions

- **Bevy over raw OpenXR+Vulkan** — Engine handles rendering and lifecycle; bevy_mod_openxr integrates the OpenXR swapchain and session.
- **Runtime-loaded OpenXR loader** — Ensure `openxr_loader.dll` is on PATH or in `target\release\`; use `scripts/fetch-openxr-loader.ps1` if needed.
- **WorldPanel system** — Decoupled UI rendering to texture allows any egui content to be displayed in 3D space.
- **Async data ingestion** — Uses Bevy's `AsyncComputeTaskPool` to poll data sources without blocking the render loop.

## Conventions

- **Commits:** conventional commits (`feat:`, `fix:`, `docs:`, `chore:`, `refactor:`)
- **Error handling:** Bevy's normal patterns; no `anyhow` in the Bevy app.
- **Logging:** Bevy's `log` / `tracing` as needed.
