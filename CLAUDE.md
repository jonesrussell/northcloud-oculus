# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

Minimal OpenXR + Vulkan VR prototype targeting the Oculus Rift CV1 and Meta Quest 3 (via Quest Link). Rust (stable 1.77+), single-file implementation in `src/main.rs` (~910 lines). Runs on Windows with an Oculus runtime.

## Build & Run

```bash
cargo build                          # debug build
cargo build --release                # release build
cargo run --release                  # build + run (requires Oculus runtime)
RUST_LOG=debug cargo run --release   # run with per-frame head/controller logging
```

Shaders (GLSL 450) in `shaders/` are compiled to SPIR-V at build time by `build.rs` using shaderc, then embedded via `include_bytes!`. No tests or CI exist yet.

## Architecture

The entire prototype lives in `src/main.rs` as a sequential pipeline:

1. **Init** — OpenXR entry → Vulkan instance/device (created through OpenXR) → session → reference space (STAGE) → action sets (controller grip poses) → swapchain + framebuffers
2. **Frame loop** — poll XR events → session state machine (READY→SYNCHRONIZED→VISIBLE→FOCUSED→STOPPING→EXITING) → frame timing → locate views → sync actions → acquire swapchain image → record command buffer → submit → release → end frame
3. **Cleanup** — strict destruction order: GPU idle → fences → command pool → framebuffers/image views → pipeline/layout/render pass. Swapchain must drop before session; hand spaces before action sets; action sets before reference spaces.

Key types: `Swapchain` (XR swapchain + Vec<Framebuffer>), `Framebuffer` (VkFramebuffer + VkImageView). Constants: `COLOR_FORMAT`, `VIEW_COUNT`, `VIEW_TYPE`, `PIPELINE_DEPTH` (2, double-buffered).

**Rendering:** Multiview stereo (both eyes in one render pass via `GL_EXT_multiview`). Fullscreen triangle generated from vertex ID (no vertex buffer). Fragment shader uses `gl_ViewIndex` for per-eye color.

## Key Design Decisions

- **ash over wgpu** — direct Vulkan bindings preserve OpenXR swapchain integration
- **Single-file monolith** — intentional; module decomposition (xr.rs, renderer.rs, input.rs) is a planned next step
- **Linked OpenXR loader** — compile-time linking for Windows deployment
- **Multiview rendering** — efficient stereo in a single render pass

## Conventions

- **Commits:** conventional commits (`feat:`, `fix:`, `docs:`, `chore:`, `refactor:`)
- **Error handling:** `anyhow::Result` with `.context()` for enrichment, `bail!()` for explicit failures
- **Unsafe:** extensive (Vulkan FFI); add `// SAFETY:` comments explaining invariants for non-trivial unsafe blocks
- **Logging:** `log` facade + `env_logger`; `info!` for milestones, `debug!` for per-frame data, `warn!` for non-fatal issues
- **Resource cleanup ordering is critical** — see cleanup section above; getting this wrong causes segfaults or validation errors
