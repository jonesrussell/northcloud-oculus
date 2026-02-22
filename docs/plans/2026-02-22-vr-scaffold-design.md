# VR Scaffold Design: northcloud-oculus

**Date:** 2026-02-22
**Status:** Approved
**Target:** Oculus Rift CV1 via OpenXR + Vulkan (ash)

## Overview

A minimal, single-file Rust prototype that initializes an OpenXR session with Vulkan rendering on the Oculus Rift CV1. The goal is to prove the full VR pipeline: instance creation, session management, swapchain rendering, head tracking, and controller pose polling.

## Decision: ash (Vulkan) over wgpu

wgpu's surface abstraction doesn't fit OpenXR's swapchain ownership model. Integrating wgpu requires reaching into the unsafe `wgpu-hal` Vulkan backend, which is fragile and poorly documented. The `openxr` crate (v0.21) ships a canonical Vulkan example using `ash`, which is the proven path. We use ash (v0.38) directly.

## Decision: Single-file monolith

For a prototype, everything lives in `src/main.rs` (~600-800 lines). No premature abstraction. Once the pipeline is validated, the code can be decomposed into modules.

## Repository Layout

```
northcloud-oculus/
├── Cargo.toml
├── build.rs              # GLSL → SPIR-V compilation via shaderc
├── src/
│   └── main.rs           # Complete prototype
├── shaders/
│   ├── fullscreen.vert   # Fullscreen triangle from vertex ID
│   └── solid.frag        # Solid color, eye-distinguishing
└── docs/
    └── plans/
        └── 2026-02-22-vr-scaffold-design.md
```

## Dependencies

| Crate       | Version | Purpose                                |
|-------------|---------|----------------------------------------|
| openxr      | 0.21    | OpenXR bindings (safe high-level API)  |
| ash         | 0.38    | Raw Vulkan bindings                    |
| glam        | 0.32    | Math (vectors, quaternions, matrices)  |
| anyhow      | 1       | Error handling                         |
| log         | 0.4     | Logging facade                         |
| env_logger  | 0.11    | Logging backend                        |
| ctrlc       | 3       | Graceful Ctrl+C shutdown               |
| shaderc     | 0.8     | GLSL-to-SPIR-V (build dependency)     |

## OpenXR Initialization Sequence

1. **Load runtime** — `openxr::Entry::linked()` loads the Oculus OpenXR runtime DLL
2. **Create Instance** — Request `XR_KHR_vulkan_enable2` extension
3. **Get System** — `FormFactor::HEAD_MOUNTED_DISPLAY` selects the Rift CV1
4. **Check Vulkan requirements** — Runtime specifies required Vulkan extensions/versions
5. **Create Vulkan Instance & Device** — Via ash, honoring runtime requirements. Enable `VK_KHR_multiview`.
6. **Create OpenXR Session** — Bind Vulkan resources via `GraphicsBindingVulkan`
7. **Create Reference Space** — `STAGE` (room-scale origin on the floor)
8. **Create Action Sets** — Controller grip poses for left/right hand
9. **Create Swapchain** — `R8G8B8A8_SRGB`, stereo array (2 layers), runtime-recommended resolution
10. **Create Vulkan render pass + framebuffers** — One per swapchain image

## Rift CV1 Tracking

The CV1 uses external USB sensors (Constellation IR tracking). Through OpenXR, this is fully abstracted — the Oculus runtime handles all sensor fusion internally. Calling `locate_views()` returns calibrated stereo eye poses. The CV1 reports ~1080x1200 per eye at 90Hz.

## Frame Loop

```
loop {
    poll_xr_events()           // session state changes, quit
    if session.is_running():
        frame_wait()           // predicted display time
        frame_begin()
        locate_views()         // left/right eye poses + FOVs
        sync_actions()         // controller state
        acquire_image()        // single swapchain with array layers
        wait_image()
        record_and_submit_vulkan_commands()  // multiview renders both eyes
        release_image()
        frame_end()            // submit layers to compositor
    if session.ended(): break
```

## Rendering

- **Swapchain:** Single swapchain with 2 array layers (stereo)
- **Shaders:** Fullscreen triangle from `gl_VertexIndex`, solid color distinguished by `gl_ViewIndex` (multiview)
- **Visual output:** Left eye = dark blue, right eye = dark red (confirms stereo)
- **Sync:** Fence per in-flight frame (pipeline depth = 2)

## Controller Tracking

- Grip pose actions bound to `/user/hand/left/input/grip/pose` and `/user/hand/right/input/grip/pose`
- Positions/orientations logged to console each frame
- Gracefully handles untracked (powered off) controllers

## Session State Machine

`IDLE → READY → SYNCHRONIZED → VISIBLE → FOCUSED → ... → STOPPING → IDLE`

- Render only in `VISIBLE` or `FOCUSED`
- `request_exit()` on `STOPPING`
- Break on `EXITING` or `LOSS_PENDING`

## Build & Run

### Prerequisites
- Windows 10/11
- Oculus PC app installed
- Rift CV1 connected (HDMI + USB sensors)
- Vulkan GPU (NVIDIA GTX 970+ / AMD equivalent)
- Rust stable toolchain
- Vulkan SDK (optional, for validation layers)

### Active Runtime
Registry: `HKLM\SOFTWARE\Khronos\OpenXR\1\ActiveRuntime` → `C:\Program Files\Oculus\Support\oculus-runtime\oculus_openxr_64.json`

### Commands
```bash
cargo build --release
cargo run --release
```

### Expected Result
- Left eye: dark blue, right eye: dark red
- Controller poses logged to console
- Ctrl+C or headset removal to exit

## Next Steps (Post-Prototype)

1. **Input actions** — Button presses, trigger values, thumbstick axes
2. **Simple 3D scene** — Cube or grid with depth buffer, world-locked geometry
3. **Hand/controller models** — Render simple geometry at controller poses
4. **Interaction system** — Ray-casting, grab mechanics
5. **SteamVR compatibility** — Test with SteamVR as the active OpenXR runtime
6. **Module decomposition** — Split into xr.rs, renderer.rs, input.rs
