# northcloud-oculus

Minimal OpenXR + Vulkan VR prototype targeting the Oculus Rift CV1.

Renders a solid color per eye (blue left, red right) to validate the full VR pipeline: OpenXR session, Vulkan rendering, head tracking, and controller pose polling.

## Prerequisites

- **Windows 10/11** (the Oculus PC runtime only runs on Windows)
- **Oculus PC app** installed (provides the OpenXR runtime)
- **Rift CV1** connected (HDMI + 2-3 USB sensors)
- **Vulkan-capable GPU** — NVIDIA GTX 970+ or AMD equivalent
- **Rust stable toolchain** (1.77+) — install via [rustup](https://rustup.rs)
- **CMake** — required by the `shaderc` build dependency for GLSL shader compilation

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

## Troubleshooting

| Problem | Solution |
|---------|----------|
| "Failed to load OpenXR loader" | Install the Oculus PC app |
| "No VR headset found" | Check USB sensors and HDMI, restart Oculus service |
| Vulkan errors | Install/update GPU drivers and Vulkan SDK |
| Black screen in headset | Verify Oculus is the active OpenXR runtime |
| Low framerate | Use `--release` build, check GPU is not thermal throttling |

## Next Steps

1. **Input actions** — Button presses, trigger values, thumbstick axes
2. **Simple 3D scene** — Cube or grid with depth buffer and view-projection matrices
3. **Hand/controller models** — Render geometry at controller poses
4. **Interaction system** — Ray-casting, grab mechanics
5. **SteamVR compatibility** — Test with SteamVR as the active OpenXR runtime
6. **Module decomposition** — Split into xr.rs, renderer.rs, input.rs

## Architecture

```
src/main.rs          — Complete prototype
shaders/
  fullscreen.vert    — Fullscreen triangle from vertex ID (multiview)
  solid.frag         — Solid color per eye
build.rs             — GLSL to SPIR-V compilation at build time
```
