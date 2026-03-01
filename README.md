# northcloud-oculus

VR observability cockpit targeting the Oculus Rift CV1 and Meta Quest 3 (via Quest Link). Built with **Bevy 0.18** and **bevy_mod_openxr**.

Visualizes infrastructure metrics from Prometheus, Grafana, and Loki in immersive VR with world-space UI panels.

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

- **In headset:** A world-space panel displaying "Northcloud Oculus" with status and uptime information
- **Exit:** Close the window, Ctrl+C in terminal, or remove headset

## How It Works

- **Bevy + bevy_mod_openxr** — Bevy handles rendering (wgpu); bevy_mod_openxr provides the OpenXR session, swapchain, and XR camera/views
- **WorldPanel** — Renders egui UI to textures displayed on 3D quads in world space
- **Interaction** — VR controller raycasting and selection system

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
├── Cargo.toml           — Bevy 0.18, bevy_mod_xr, bevy_mod_openxr, openxr
├── Cargo.lock           — Pinned dependency versions
├── src/
│   ├── main.rs          — Bevy app entry point, demo panel
│   ├── lib.rs           — Module exports
│   ├── world_panel/     — WorldPanel system (egui → texture → 3D quad)
│   ├── interaction/     — VR controller tracking, raycasting, selection
│   ├── data/            — Prometheus, Grafana, Loki data ingestion
│   ├── panels/          — MapPanel, DetailPanel feature panels
│   └── node_marker/     — Health status indicators with animations
├── scripts/
│   └── fetch-openxr-loader.ps1 — Downloads openxr_loader.dll into target\release\
└── docs/
    └── plans/           — Design documents
```

## Dependencies

| Crate | Version | Purpose |
|-------|---------|---------|
| [bevy](https://crates.io/crates/bevy) | 0.18 | Game engine (ECS, rendering via wgpu) |
| [bevy_mod_xr](https://crates.io/crates/bevy_mod_xr) | 0.5 | XR API for Bevy |
| [bevy_mod_openxr](https://crates.io/crates/bevy_mod_openxr) | 0.5 | OpenXR backend for bevy_mod_xr |
| [openxr](https://crates.io/crates/openxr) | 0.21 | OpenXR bindings |
| [bevy_egui](https://crates.io/crates/bevy_egui) | 0.39 | egui integration for Bevy |
| [bevy_xr_utils](https://crates.io/crates/bevy_xr_utils) | 0.5 | VR interaction utilities |
| [reqwest](https://crates.io/crates/reqwest) | 0.12 | HTTP client for data ingestion |
| [serde](https://crates.io/crates/serde) | 1.0 | Serialization for API responses |
