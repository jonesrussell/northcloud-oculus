# northcloud-oculus

Minimal OpenXR + Vulkan VR prototype targeting the Oculus Rift CV1 and Meta Quest 3 (via Quest Link).

Renders a solid color per eye (blue left, red right) to validate the full VR pipeline: OpenXR session, Vulkan rendering, head tracking, and controller pose polling.

## Prerequisites

- **Windows 10/11** (the Oculus PC runtime only runs on Windows)
- **Oculus PC app** installed (provides the OpenXR runtime)
- **Rift CV1** connected (HDMI + 2-3 USB sensors), **or Meta Quest 3** connected via Quest Link (USB or Air Link)
- **Vulkan-capable GPU** — NVIDIA GTX 970+ or AMD equivalent
- **Rust stable toolchain** (1.77+) — install via [rustup](https://rustup.rs)
- **CMake** — required by the `shaderc` build dependency for GLSL shader compilation
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

Set `RUST_LOG=debug` to see head and controller positions:
```bash
RUST_LOG=debug cargo run --release
```

## What You Should See

- **Left eye:** Dark blue
- **Right eye:** Dark red
- **Console:** Head position and controller positions (with `RUST_LOG=debug`)
- **Exit:** Ctrl+C in terminal or remove headset

## How It Works

The prototype uses:
- **OpenXR** for VR runtime access (session, swapchain, tracking)
- **Vulkan** (via `ash`) for GPU rendering
- **Multiview** rendering — both eyes in a single render pass
- **gl_ViewIndex** in the shader to distinguish left/right eye

The Rift CV1's Constellation tracking (external USB IR sensors) is fully abstracted by OpenXR. The Oculus runtime handles all sensor fusion internally.
The Quest 3's inside-out tracking is similarly abstracted — no code changes needed between headsets.

## Troubleshooting

| Problem | Solution |
|---------|----------|
| "Failed to load OpenXR loader" | Run `.\scripts\fetch-openxr-loader.ps1` to download and place `openxr_loader.dll` next to the exe. Ensure the Oculus PC app is installed for the runtime. |
| "No VR headset found" | Check USB sensors and HDMI, restart Oculus service |
| Vulkan errors | Install/update GPU drivers and Vulkan SDK |
| Black screen in headset | Verify Oculus is the active OpenXR runtime |
| Low framerate | Use `--release` build, check GPU is not thermal throttling |
| Quest 3 not detected via Link | Ensure Meta Quest Link app is running and set as active OpenXR runtime |

## Next Steps

1. **Input actions** — Button presses, trigger values, thumbstick axes
2. **Simple 3D scene** — Cube or grid with depth buffer and view-projection matrices
3. **Hand/controller models** — Render geometry at controller poses
4. **Interaction system** — Ray-casting, grab mechanics
5. **SteamVR compatibility** — Test with SteamVR as the active OpenXR runtime
6. **Module decomposition** — Split into xr.rs, renderer.rs, input.rs

## Architecture

```
northcloud-oculus/
├── Cargo.toml           — Dependencies: openxr 0.21, ash 0.38, glam 0.32, anyhow, log
├── Cargo.lock           — Pinned dependency versions
├── build.rs             — GLSL → SPIR-V compilation at build time (shaderc)
├── src/
│   └── main.rs          — Complete prototype (~910 lines)
├── shaders/
│   ├── fullscreen.vert  — Fullscreen triangle from vertex ID (multiview)
│   └── solid.frag       — Solid color per eye (blue left, red right)
├── scripts/
│   └── fetch-openxr-loader.ps1 — Downloads openxr_loader.dll into target\release\
├── docs/
│   └── plans/           — Design documents
└── .gitignore
```

## Dependencies

| Crate | Version | Purpose |
|-------|---------|---------|
| [openxr](https://crates.io/crates/openxr) | 0.21 | OpenXR bindings (loader loaded at runtime) |
| [ash](https://crates.io/crates/ash) | 0.38 | Raw Vulkan bindings (runtime loaded) |
| [glam](https://crates.io/crates/glam) | 0.32 | Math (vectors, quaternions, matrices) |
| [anyhow](https://crates.io/crates/anyhow) | 1 | Error handling |
| [log](https://crates.io/crates/log) / [env_logger](https://crates.io/crates/env_logger) | 0.4 / 0.11 | Logging |
| [ctrlc](https://crates.io/crates/ctrlc) | 3 | Graceful Ctrl+C shutdown |
| [shaderc](https://crates.io/crates/shaderc) | 0.8 | GLSL → SPIR-V (build-time only) |
