# VR Scaffold Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Create a complete, compilable Rust prototype that renders stereo color to an Oculus Rift CV1 via OpenXR + Vulkan, with head and controller tracking.

**Architecture:** Single-file monolith (`src/main.rs`) using `openxr` (v0.21) for XR runtime access and `ash` (v0.38) for Vulkan rendering. Shaders are GLSL compiled to SPIR-V at build time via `shaderc`. The frame loop uses multiview rendering (both eyes in a single render pass) with pipeline depth 2 for double-buffering.

**Tech Stack:** Rust (stable), openxr 0.21, ash 0.38, glam 0.32, shaderc 0.8, anyhow 1, log/env_logger

**Reference:** The official openxrs Vulkan example at `github.com/Ralith/openxrs/blob/master/openxr/examples/vulkan.rs`

---

### Task 1: Create project scaffold

**Files:**
- Create: `Cargo.toml`
- Create: `src/main.rs` (stub)
- Create: `.gitignore`

**Step 1: Create Cargo.toml**

```toml
[package]
name = "northcloud-oculus"
version = "0.1.0"
edition = "2021"

[dependencies]
openxr = { version = "0.21", features = ["loaded"] }
ash = { version = "0.38", default-features = false, features = ["loaded"] }
glam = "0.32"
anyhow = "1"
log = "0.4"
env_logger = "0.11"

[build-dependencies]
shaderc = "0.8"
```

**Step 2: Create stub main.rs**

```rust
fn main() {
    env_logger::init();
    log::info!("northcloud-oculus starting");
    println!("Scaffold OK");
}
```

**Step 3: Create .gitignore**

```
/target
*.spv
```

**Step 4: Verify it compiles**

Run: `cargo build`
Expected: Compiles successfully, prints "Scaffold OK" on `cargo run`

**Step 5: Commit**

```bash
git add Cargo.toml src/main.rs .gitignore
git commit -m "feat: initial project scaffold with dependencies"
```

---

### Task 2: Create GLSL shaders

**Files:**
- Create: `shaders/fullscreen.vert`
- Create: `shaders/solid.frag`

**Step 1: Create vertex shader**

This shader generates a fullscreen triangle from just the vertex index (0, 1, 2). No vertex buffer needed. The `gl_ViewIndex` built-in gives the eye index in multiview mode.

```glsl
#version 450
#extension GL_EXT_multiview : require

// Fullscreen triangle from vertex ID — no vertex buffer needed.
// Vertex 0: (-1, -1), Vertex 1: (3, -1), Vertex 2: (-1, 3)
// This covers the entire clip space with a single oversized triangle.

layout(location = 0) out flat uint out_view_index;

void main() {
    // Generate fullscreen triangle positions from vertex index
    vec2 pos = vec2(
        float((gl_VertexIndex << 1) & 2) * 2.0 - 1.0,
        float(gl_VertexIndex & 2) * 2.0 - 1.0
    );
    gl_Position = vec4(pos, 0.0, 1.0);
    out_view_index = gl_ViewIndex;
}
```

**Step 2: Create fragment shader**

Outputs a different solid color per eye so you can verify stereo rendering by closing one eye at a time in the headset.

```glsl
#version 450

// Renders a distinct solid color per eye:
//   Left eye (view 0) = dark blue
//   Right eye (view 1) = dark red
// This immediately confirms stereo rendering works.

layout(location = 0) in flat uint in_view_index;
layout(location = 0) out vec4 out_color;

void main() {
    if (in_view_index == 0u) {
        out_color = vec4(0.05, 0.05, 0.3, 1.0); // Left eye: dark blue
    } else {
        out_color = vec4(0.3, 0.05, 0.05, 1.0); // Right eye: dark red
    }
}
```

**Step 3: Commit**

```bash
git add shaders/
git commit -m "feat: add fullscreen vertex and solid color fragment shaders"
```

---

### Task 3: Create build script for shader compilation

**Files:**
- Create: `build.rs`

**Step 1: Write build.rs**

Compiles GLSL shaders to SPIR-V at build time using `shaderc`. The compiled `.spv` files are placed in `OUT_DIR` so `include_bytes!` can find them.

```rust
use shaderc;
use std::fs;
use std::path::Path;

fn main() {
    let compiler = shaderc::Compiler::new().expect("Failed to create shaderc compiler");
    let mut options = shaderc::CompileOptions::new().expect("Failed to create compile options");
    options.set_target_env(
        shaderc::TargetEnv::Vulkan,
        shaderc::EnvVersion::Vulkan1_1 as u32,
    );
    options.set_source_language(shaderc::SourceLanguage::GLSL);

    let out_dir = std::env::var("OUT_DIR").unwrap();

    let shaders = [
        ("shaders/fullscreen.vert", shaderc::ShaderKind::Vertex),
        ("shaders/solid.frag", shaderc::ShaderKind::Fragment),
    ];

    for (path, kind) in &shaders {
        println!("cargo:rerun-if-changed={}", path);

        let source = fs::read_to_string(path)
            .unwrap_or_else(|e| panic!("Failed to read shader {}: {}", path, e));

        let artifact = compiler
            .compile_into_spirv(&source, *kind, path, "main", Some(&options))
            .unwrap_or_else(|e| panic!("Failed to compile shader {}: {}", path, e));

        if artifact.get_num_warnings() > 0 {
            eprintln!("Warnings in {}: {}", path, artifact.get_warning_messages());
        }

        let file_name = Path::new(path)
            .file_name()
            .unwrap()
            .to_str()
            .unwrap();
        let out_path = Path::new(&out_dir).join(format!("{}.spv", file_name));

        fs::write(&out_path, artifact.as_binary_u8())
            .unwrap_or_else(|e| panic!("Failed to write SPIR-V {}: {}", out_path.display(), e));
    }
}
```

**Step 2: Verify shaders compile**

Run: `cargo build`
Expected: Build succeeds. SPIR-V files appear in `target/debug/build/northcloud-oculus-*/out/`.

**Step 3: Commit**

```bash
git add build.rs
git commit -m "feat: add build.rs for GLSL to SPIR-V shader compilation"
```

---

### Task 4: Write complete main.rs — OpenXR + Vulkan prototype

This is the core task. The entire prototype lives in `src/main.rs`. Because OpenXR and Vulkan code is deeply intertwined (each step depends on the previous), this is written as one complete file rather than incrementally.

**Files:**
- Modify: `src/main.rs` (replace stub with complete prototype)

**Step 1: Write the complete main.rs**

The file is structured in these sequential sections:
1. Imports and constants
2. Helper structs (Swapchain, Framebuffer)
3. `main()` function:
   a. Logging init
   b. OpenXR entry + instance
   c. System selection (HMD)
   d. Vulkan requirements check
   e. Vulkan instance creation (via OpenXR)
   f. Physical device selection (via OpenXR)
   g. Queue family selection
   h. Vulkan logical device creation (via OpenXR, with multiview)
   i. OpenXR session creation
   j. Reference spaces (STAGE)
   k. Action set + controller pose actions
   l. Interaction profile bindings
   m. Session action set attachment
   n. Action spaces for controllers
   o. View configuration query
   p. Render pass creation (with multiview)
   q. Shader loading + pipeline creation
   r. Command pool + command buffers
   s. Fences
   t. Swapchain creation + framebuffers
   u. Ctrl+C handler
   v. Main frame loop
   w. Cleanup

Here is the complete code:

```rust
//! northcloud-oculus — Minimal OpenXR + Vulkan VR prototype
//!
//! Renders a solid color per eye (blue left, red right) to an Oculus Rift CV1
//! via the Oculus OpenXR runtime. Logs head and controller poses each frame.
//!
//! ## How it works
//!
//! The Rift CV1 uses external USB infrared sensors (Constellation tracking).
//! Through OpenXR, tracking is fully abstracted — the Oculus runtime handles
//! all sensor fusion internally. We just call `locate_views()` to get
//! calibrated stereo eye poses. The CV1 renders at ~1080x1200 per eye, 90Hz.
//!
//! ## Running on Windows with the Oculus runtime
//!
//! 1. Install the Oculus PC app (provides the OpenXR runtime DLL).
//! 2. Ensure the Oculus runtime is the active OpenXR runtime:
//!    Registry: HKLM\SOFTWARE\Khronos\OpenXR\1\ActiveRuntime
//!    → C:\Program Files\Oculus\Support\oculus-runtime\oculus_openxr_64.json
//! 3. Connect Rift CV1 (HDMI + USB sensors).
//! 4. `cargo run --release`
//! 5. Left eye = dark blue, right eye = dark red. Controller poses in console.
//! 6. Ctrl+C or remove headset to exit.
//!
//! ## GPU requirements
//!
//! Vulkan 1.1 capable GPU with multiview support.
//! NVIDIA GTX 970+ or AMD equivalent.

use std::{
    io::Cursor,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};

use anyhow::{bail, Context, Result};
use ash::{util::read_spv, vk::{self, Handle}};
use openxr as xr;

// --- Constants ---

/// Vulkan color format for swapchain images.
const COLOR_FORMAT: vk::Format = vk::Format::R8G8B8A8_SRGB;

/// Number of views (eyes) for stereo rendering.
const VIEW_COUNT: u32 = 2;

/// OpenXR view configuration type — primary stereo (one view per eye).
const VIEW_TYPE: xr::ViewConfigurationType = xr::ViewConfigurationType::PRIMARY_STEREO;

/// Number of frames that can be in-flight simultaneously.
/// 2 = double-buffering: one frame rendering while the previous is displayed.
const PIPELINE_DEPTH: u32 = 2;

// --- Helper structs ---

/// Wraps the OpenXR swapchain and its per-image Vulkan framebuffers.
struct Swapchain {
    handle: xr::Swapchain<xr::Vulkan>,
    buffers: Vec<Framebuffer>,
    resolution: vk::Extent2D,
}

/// A single swapchain image's Vulkan resources.
struct Framebuffer {
    framebuffer: vk::Framebuffer,
    color: vk::ImageView,
}

fn main() -> Result<()> {
    // =========================================================================
    // 1. LOGGING
    // =========================================================================
    env_logger::init();
    log::info!("northcloud-oculus starting");

    // =========================================================================
    // 2. OPENXR ENTRY + INSTANCE
    // =========================================================================
    // Load the OpenXR loader (tries PATH, VULKAN_SDK\\Bin, C:\\VulkanSDK, OPENXR_LOADER_PATH).
    let xr_entry = load_openxr_entry()?;

    // Check that the Vulkan extension is available.
    let available_extensions = xr_entry.enumerate_extensions()?;
    if !available_extensions.khr_vulkan_enable2 {
        bail!("OpenXR runtime does not support the KHR_vulkan_enable2 extension");
    }

    // Create the OpenXR instance with Vulkan support.
    let mut enabled_extensions = xr::ExtensionSet::default();
    enabled_extensions.khr_vulkan_enable2 = true;

    let xr_instance = xr_entry.create_instance(
        &xr::ApplicationInfo {
            application_name: "northcloud-oculus",
            application_version: 0,
            engine_name: "northcloud",
            engine_version: 0,
            api_version: xr::Version::new(1, 0, 0),
        },
        &enabled_extensions,
        &[],
    )?;

    let instance_props = xr_instance.properties()?;
    log::info!(
        "OpenXR runtime: {} {}",
        instance_props.runtime_name,
        instance_props.runtime_version
    );

    // =========================================================================
    // 3. SYSTEM SELECTION (HMD)
    // =========================================================================
    // This selects the connected HMD (Rift CV1). Fails if no headset is found.
    let system = xr_instance
        .system(xr::FormFactor::HEAD_MOUNTED_DISPLAY)
        .context("No VR headset found. Is your Rift CV1 connected and the Oculus app running?")?;

    // Query the environment blend mode (typically OPAQUE for VR headsets).
    let environment_blend_mode =
        xr_instance.enumerate_environment_blend_modes(system, VIEW_TYPE)?[0];
    log::info!("Environment blend mode: {:?}", environment_blend_mode);

    // =========================================================================
    // 4. VULKAN REQUIREMENTS CHECK
    // =========================================================================
    // The OpenXR runtime tells us which Vulkan version it needs.
    let vk_target_version = vk::make_api_version(0, 1, 1, 0);
    let vk_target_version_xr = xr::Version::new(1, 1, 0);

    let reqs = xr_instance.graphics_requirements::<xr::Vulkan>(system)?;
    if vk_target_version_xr < reqs.min_api_version_supported
        || vk_target_version_xr.major() > reqs.max_api_version_supported.major()
    {
        bail!(
            "OpenXR runtime requires Vulkan version > {}, < {}.0.0",
            reqs.min_api_version_supported,
            reqs.max_api_version_supported.major() + 1
        );
    }

    // =========================================================================
    // 5. VULKAN INSTANCE CREATION (via OpenXR)
    // =========================================================================
    // OpenXR creates the Vulkan instance for us so it can inject any extensions
    // it requires. We provide the application info and desired API version.
    //
    // SAFETY: We're passing valid Vulkan create info to OpenXR's Vulkan
    // instance creation function. The transmute converts ash's function pointer
    // type to what OpenXR expects.
    let vk_entry = unsafe { ash::Entry::load()? };

    let vk_app_info = vk::ApplicationInfo::default()
        .application_name(c"northcloud-oculus")
        .application_version(0)
        .engine_name(c"northcloud")
        .engine_version(0)
        .api_version(vk_target_version);

    let vk_instance = unsafe {
        let vk_instance = xr_instance
            .create_vulkan_instance(
                system,
                std::mem::transmute(vk_entry.static_fn().get_instance_proc_addr),
                &vk::InstanceCreateInfo::default().application_info(&vk_app_info)
                    as *const _ as *const _,
            )
            .context("OpenXR failed to create Vulkan instance")?
            .map_err(vk::Result::from_raw)
            .context("Vulkan error creating instance")?;

        ash::Instance::load(
            vk_entry.static_fn(),
            vk::Instance::from_raw(vk_instance as _),
        )
    };

    // =========================================================================
    // 6. PHYSICAL DEVICE SELECTION (via OpenXR)
    // =========================================================================
    // OpenXR chooses the physical device (GPU) that matches the HMD.
    let vk_physical_device = vk::PhysicalDevice::from_raw(unsafe {
        xr_instance
            .vulkan_graphics_device(system, vk_instance.handle().as_raw() as _)
            .context("OpenXR failed to select Vulkan physical device")? as _
    });

    let vk_device_properties =
        unsafe { vk_instance.get_physical_device_properties(vk_physical_device) };
    if vk_device_properties.api_version < vk_target_version {
        bail!("GPU does not support Vulkan 1.1");
    }
    log::info!(
        "Vulkan device: {:?}",
        unsafe { std::ffi::CStr::from_ptr(vk_device_properties.device_name.as_ptr()) }
    );

    // =========================================================================
    // 7. QUEUE FAMILY SELECTION
    // =========================================================================
    let queue_family_index = unsafe {
        vk_instance
            .get_physical_device_queue_family_properties(vk_physical_device)
            .into_iter()
            .enumerate()
            .find_map(|(i, info)| {
                if info.queue_flags.contains(vk::QueueFlags::GRAPHICS) {
                    Some(i as u32)
                } else {
                    None
                }
            })
            .context("No graphics queue family found")?
    };

    // =========================================================================
    // 8. VULKAN LOGICAL DEVICE CREATION (via OpenXR, with multiview)
    // =========================================================================
    // OpenXR creates the logical device so it can inject required extensions.
    // We enable multiview for rendering both eyes in a single render pass.
    //
    // SAFETY: Same pattern as instance creation — valid create info, OpenXR
    // manages the actual vkCreateDevice call.
    let vk_device = unsafe {
        let mut multiview_features = vk::PhysicalDeviceMultiviewFeatures {
            multiview: vk::TRUE,
            ..Default::default()
        };

        let vk_device = xr_instance
            .create_vulkan_device(
                system,
                std::mem::transmute(vk_entry.static_fn().get_instance_proc_addr),
                vk_physical_device.as_raw() as _,
                &vk::DeviceCreateInfo::default()
                    .queue_create_infos(&[vk::DeviceQueueCreateInfo::default()
                        .queue_family_index(queue_family_index)
                        .queue_priorities(&[1.0])])
                    .push_next(&mut multiview_features)
                    as *const _ as *const _,
            )
            .context("OpenXR failed to create Vulkan device")?
            .map_err(vk::Result::from_raw)
            .context("Vulkan error creating device")?;

        ash::Device::load(
            vk_instance.fp_v1_0(),
            vk::Device::from_raw(vk_device as _),
        )
    };

    let queue = unsafe { vk_device.get_device_queue(queue_family_index, 0) };

    // =========================================================================
    // 9. OPENXR SESSION CREATION
    // =========================================================================
    // Bind our Vulkan instance/device/queue to an OpenXR session.
    // The session manages the connection between our app and the HMD.
    let (session, mut frame_wait, mut frame_stream) = unsafe {
        xr_instance.create_session::<xr::Vulkan>(
            system,
            &xr::vulkan::SessionCreateInfo {
                instance: vk_instance.handle().as_raw() as _,
                physical_device: vk_physical_device.as_raw() as _,
                device: vk_device.handle().as_raw() as _,
                queue_family_index,
                queue_index: 0,
            },
        )?
    };

    // =========================================================================
    // 10. REFERENCE SPACES
    // =========================================================================
    // STAGE space: room-scale coordinate system with origin on the floor.
    // All tracking data (head, controllers) is relative to this space.
    let stage = session
        .create_reference_space(xr::ReferenceSpaceType::STAGE, xr::Posef::IDENTITY)?;

    // =========================================================================
    // 11. ACTION SET + CONTROLLER POSE ACTIONS
    // =========================================================================
    // Actions are how OpenXR exposes input. We create pose actions for each
    // controller's grip (the position/orientation of the controller body).
    let action_set = xr_instance.create_action_set("input", "Input", 0)?;

    let left_hand_action = action_set
        .create_action::<xr::Posef>("left_hand", "Left Hand Controller", &[])?;
    let right_hand_action = action_set
        .create_action::<xr::Posef>("right_hand", "Right Hand Controller", &[])?;

    // =========================================================================
    // 12. INTERACTION PROFILE BINDINGS
    // =========================================================================
    // Bind our pose actions to the physical controller paths.
    // We use the Oculus Touch profile for Rift CV1 Touch controllers.
    // Falls back to khr/simple_controller if Touch profile isn't available.
    xr_instance.suggest_interaction_profile_bindings(
        xr_instance.string_to_path("/interaction_profiles/oculus/touch_controller")?,
        &[
            xr::Binding::new(
                &left_hand_action,
                xr_instance.string_to_path("/user/hand/left/input/grip/pose")?,
            ),
            xr::Binding::new(
                &right_hand_action,
                xr_instance.string_to_path("/user/hand/right/input/grip/pose")?,
            ),
        ],
    )?;

    // Also suggest for the simple controller profile as a fallback.
    xr_instance.suggest_interaction_profile_bindings(
        xr_instance.string_to_path("/interaction_profiles/khr/simple_controller")?,
        &[
            xr::Binding::new(
                &left_hand_action,
                xr_instance.string_to_path("/user/hand/left/input/grip/pose")?,
            ),
            xr::Binding::new(
                &right_hand_action,
                xr_instance.string_to_path("/user/hand/right/input/grip/pose")?,
            ),
        ],
    )?;

    // =========================================================================
    // 13. ATTACH ACTION SETS TO SESSION
    // =========================================================================
    session.attach_action_sets(&[&action_set])?;

    // =========================================================================
    // 14. ACTION SPACES FOR CONTROLLERS
    // =========================================================================
    // Action spaces let us query controller poses relative to the stage space.
    let left_hand_space =
        left_hand_action.create_space(session.clone(), xr::Path::NULL, xr::Posef::IDENTITY)?;
    let right_hand_space =
        right_hand_action.create_space(session.clone(), xr::Path::NULL, xr::Posef::IDENTITY)?;

    // =========================================================================
    // 15. VIEW CONFIGURATION
    // =========================================================================
    // Query the recommended rendering resolution for each eye.
    let views = xr_instance.enumerate_view_configuration_views(system, VIEW_TYPE)?;
    assert_eq!(views.len(), VIEW_COUNT as usize);

    let resolution = vk::Extent2D {
        width: views[0].recommended_image_rect_width,
        height: views[0].recommended_image_rect_height,
    };
    log::info!("Render resolution per eye: {}x{}", resolution.width, resolution.height);

    // =========================================================================
    // 16. VULKAN RENDER PASS (with multiview)
    // =========================================================================
    // Multiview renders both eyes in a single render pass. The GPU duplicates
    // draw calls across views, with gl_ViewIndex distinguishing left/right.
    let view_mask = !(!0u32 << VIEW_COUNT); // 0b11 for 2 views

    let render_pass = unsafe {
        vk_device.create_render_pass(
            &vk::RenderPassCreateInfo::default()
                .attachments(&[vk::AttachmentDescription {
                    format: COLOR_FORMAT,
                    samples: vk::SampleCountFlags::TYPE_1,
                    load_op: vk::AttachmentLoadOp::CLEAR,
                    store_op: vk::AttachmentStoreOp::STORE,
                    initial_layout: vk::ImageLayout::UNDEFINED,
                    final_layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                    ..Default::default()
                }])
                .subpasses(&[vk::SubpassDescription::default()
                    .color_attachments(&[vk::AttachmentReference {
                        attachment: 0,
                        layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                    }])
                    .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)])
                .dependencies(&[vk::SubpassDependency {
                    src_subpass: vk::SUBPASS_EXTERNAL,
                    dst_subpass: 0,
                    src_stage_mask: vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
                    dst_stage_mask: vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
                    dst_access_mask: vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
                    ..Default::default()
                }])
                .push_next(
                    &mut vk::RenderPassMultiviewCreateInfo::default()
                        .view_masks(&[view_mask])
                        .correlation_masks(&[view_mask]),
                ),
            None,
        )?
    };

    // =========================================================================
    // 17. SHADER LOADING + GRAPHICS PIPELINE
    // =========================================================================
    // Load SPIR-V shaders compiled by build.rs.
    let vert_spv = read_spv(&mut Cursor::new(
        &include_bytes!(concat!(env!("OUT_DIR"), "/fullscreen.vert.spv"))[..],
    ))?;
    let frag_spv = read_spv(&mut Cursor::new(
        &include_bytes!(concat!(env!("OUT_DIR"), "/solid.frag.spv"))[..],
    ))?;

    let pipeline_layout;
    let pipeline;

    unsafe {
        let vert_module =
            vk_device.create_shader_module(&vk::ShaderModuleCreateInfo::default().code(&vert_spv), None)?;
        let frag_module =
            vk_device.create_shader_module(&vk::ShaderModuleCreateInfo::default().code(&frag_spv), None)?;

        pipeline_layout = vk_device
            .create_pipeline_layout(&vk::PipelineLayoutCreateInfo::default(), None)?;

        let noop_stencil = vk::StencilOpState {
            fail_op: vk::StencilOp::KEEP,
            pass_op: vk::StencilOp::KEEP,
            depth_fail_op: vk::StencilOp::KEEP,
            compare_op: vk::CompareOp::ALWAYS,
            ..Default::default()
        };

        pipeline = vk_device
            .create_graphics_pipelines(
                vk::PipelineCache::null(),
                &[vk::GraphicsPipelineCreateInfo::default()
                    .stages(&[
                        vk::PipelineShaderStageCreateInfo::default()
                            .stage(vk::ShaderStageFlags::VERTEX)
                            .module(vert_module)
                            .name(c"main"),
                        vk::PipelineShaderStageCreateInfo::default()
                            .stage(vk::ShaderStageFlags::FRAGMENT)
                            .module(frag_module)
                            .name(c"main"),
                    ])
                    .vertex_input_state(&vk::PipelineVertexInputStateCreateInfo::default())
                    .input_assembly_state(
                        &vk::PipelineInputAssemblyStateCreateInfo::default()
                            .topology(vk::PrimitiveTopology::TRIANGLE_LIST),
                    )
                    .viewport_state(
                        &vk::PipelineViewportStateCreateInfo::default()
                            .scissor_count(1)
                            .viewport_count(1),
                    )
                    .rasterization_state(
                        &vk::PipelineRasterizationStateCreateInfo::default()
                            .cull_mode(vk::CullModeFlags::NONE)
                            .polygon_mode(vk::PolygonMode::FILL)
                            .line_width(1.0),
                    )
                    .multisample_state(
                        &vk::PipelineMultisampleStateCreateInfo::default()
                            .rasterization_samples(vk::SampleCountFlags::TYPE_1),
                    )
                    .depth_stencil_state(
                        &vk::PipelineDepthStencilStateCreateInfo::default()
                            .depth_test_enable(false)
                            .depth_write_enable(false)
                            .front(noop_stencil)
                            .back(noop_stencil),
                    )
                    .color_blend_state(
                        &vk::PipelineColorBlendStateCreateInfo::default()
                            .attachments(&[vk::PipelineColorBlendAttachmentState {
                                blend_enable: vk::TRUE,
                                src_color_blend_factor: vk::BlendFactor::ONE,
                                dst_color_blend_factor: vk::BlendFactor::ZERO,
                                color_blend_op: vk::BlendOp::ADD,
                                color_write_mask: vk::ColorComponentFlags::RGBA,
                                ..Default::default()
                            }]),
                    )
                    .dynamic_state(
                        &vk::PipelineDynamicStateCreateInfo::default()
                            .dynamic_states(&[
                                vk::DynamicState::VIEWPORT,
                                vk::DynamicState::SCISSOR,
                            ]),
                    )
                    .layout(pipeline_layout)
                    .render_pass(render_pass)
                    .subpass(0)],
                None,
            )
            .map_err(|(_pipelines, err)| err)?[0];

        // Shader modules can be destroyed after pipeline creation.
        vk_device.destroy_shader_module(vert_module, None);
        vk_device.destroy_shader_module(frag_module, None);
    }

    // =========================================================================
    // 18. COMMAND POOL + COMMAND BUFFERS
    // =========================================================================
    let cmd_pool = unsafe {
        vk_device.create_command_pool(
            &vk::CommandPoolCreateInfo::default()
                .queue_family_index(queue_family_index)
                .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER),
            None,
        )?
    };

    let cmds = unsafe {
        vk_device.allocate_command_buffers(
            &vk::CommandBufferAllocateInfo::default()
                .command_pool(cmd_pool)
                .command_buffer_count(PIPELINE_DEPTH)
                .level(vk::CommandBufferLevel::PRIMARY),
        )?
    };

    // =========================================================================
    // 19. FENCES (one per in-flight frame)
    // =========================================================================
    // Start signaled so the first frame doesn't block waiting for a fence.
    let fences: Vec<vk::Fence> = (0..PIPELINE_DEPTH)
        .map(|_| unsafe {
            vk_device
                .create_fence(
                    &vk::FenceCreateInfo::default().flags(vk::FenceCreateFlags::SIGNALED),
                    None,
                )
                .unwrap()
        })
        .collect();

    // =========================================================================
    // 20. SWAPCHAIN + FRAMEBUFFERS
    // =========================================================================
    // OpenXR owns the swapchain. We get VkImage handles from it and wrap them
    // in VkImageViews and VkFramebuffers for rendering.
    let swapchain = {
        let handle = session.create_swapchain(&xr::SwapchainCreateInfo {
            create_flags: xr::SwapchainCreateFlags::EMPTY,
            usage_flags: xr::SwapchainUsageFlags::COLOR_ATTACHMENT
                | xr::SwapchainUsageFlags::SAMPLED,
            format: COLOR_FORMAT.as_raw() as _,
            sample_count: 1,
            width: resolution.width,
            height: resolution.height,
            face_count: 1,
            array_size: VIEW_COUNT,
            mip_count: 1,
        })?;

        let images = handle.enumerate_images()?;
        log::info!("Swapchain created with {} images", images.len());

        let buffers: Vec<Framebuffer> = images
            .into_iter()
            .map(|color_image| {
                let color_image = vk::Image::from_raw(color_image);
                let color = unsafe {
                    vk_device
                        .create_image_view(
                            &vk::ImageViewCreateInfo::default()
                                .image(color_image)
                                .view_type(vk::ImageViewType::TYPE_2D_ARRAY)
                                .format(COLOR_FORMAT)
                                .subresource_range(vk::ImageSubresourceRange {
                                    aspect_mask: vk::ImageAspectFlags::COLOR,
                                    base_mip_level: 0,
                                    level_count: 1,
                                    base_array_layer: 0,
                                    layer_count: VIEW_COUNT,
                                }),
                            None,
                        )
                        .unwrap()
                };
                let framebuffer = unsafe {
                    vk_device
                        .create_framebuffer(
                            &vk::FramebufferCreateInfo::default()
                                .render_pass(render_pass)
                                .width(resolution.width)
                                .height(resolution.height)
                                .attachments(&[color])
                                .layers(1), // Multiview handles layers
                            None,
                        )
                        .unwrap()
                };
                Framebuffer { framebuffer, color }
            })
            .collect();

        Swapchain {
            handle,
            buffers,
            resolution,
        }
    };

    // =========================================================================
    // 21. CTRL+C HANDLER
    // =========================================================================
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    ctrlc::set_handler(move || {
        r.store(false, Ordering::Relaxed);
    })
    .expect("Failed to set Ctrl+C handler");

    // =========================================================================
    // 22. MAIN FRAME LOOP
    // =========================================================================
    let mut event_storage = xr::EventDataBuffer::new();
    let mut session_running = false;
    let mut frame = 0usize;

    'main_loop: loop {
        // --- Handle exit request ---
        if !running.load(Ordering::Relaxed) {
            log::info!("Exit requested");
            match session.request_exit() {
                Ok(()) => {}
                Err(xr::sys::Result::ERROR_SESSION_NOT_RUNNING) => break,
                Err(e) => return Err(e.into()),
            }
        }

        // --- Poll OpenXR events ---
        while let Some(event) = xr_instance.poll_event(&mut event_storage)? {
            use xr::Event::*;
            match event {
                SessionStateChanged(e) => {
                    log::info!("Session state changed to {:?}", e.state());
                    match e.state() {
                        xr::SessionState::READY => {
                            // The runtime is ready — begin the session.
                            session
                                .begin(VIEW_TYPE)
                                .context("Failed to begin session")?;
                            session_running = true;
                        }
                        xr::SessionState::STOPPING => {
                            // The runtime wants us to stop — end the session.
                            session.end().context("Failed to end session")?;
                            session_running = false;
                        }
                        xr::SessionState::EXITING | xr::SessionState::LOSS_PENDING => {
                            break 'main_loop;
                        }
                        _ => {}
                    }
                }
                InstanceLossPending(_) => {
                    log::warn!("OpenXR instance loss pending");
                    break 'main_loop;
                }
                EventsLost(e) => {
                    log::warn!("Lost {} OpenXR events", e.lost_event_count());
                }
                _ => {}
            }
        }

        // Don't render if the session isn't running yet.
        if !session_running {
            std::thread::sleep(Duration::from_millis(100));
            continue;
        }

        // --- Frame timing ---
        // wait_frame() blocks until the runtime is ready for the next frame
        // and gives us the predicted display time.
        let xr_frame_state = frame_wait.wait()?;
        frame_stream.begin()?;

        // If the runtime says not to render (e.g., headset removed), submit
        // an empty frame and continue.
        if !xr_frame_state.should_render {
            frame_stream.end(
                xr_frame_state.predicted_display_time,
                environment_blend_mode,
                &[],
            )?;
            continue;
        }

        // --- Locate head views (eye poses) ---
        let (_, views) = session.locate_views(
            VIEW_TYPE,
            xr_frame_state.predicted_display_time,
            &stage,
        )?;

        // Log head position (average of both eyes ≈ head center)
        let head_pos = views[0].pose.position;
        log::debug!(
            "Head: ({:.3}, {:.3}, {:.3})",
            head_pos.x,
            head_pos.y,
            head_pos.z
        );

        // --- Sync actions (controller tracking) ---
        session.sync_actions(&[(&action_set).into()])?;

        // Poll left controller
        if left_hand_action
            .is_active(&session, xr::Path::NULL)
            .unwrap_or(false)
        {
            let location = left_hand_space.locate(&stage, xr_frame_state.predicted_display_time)?;
            if location
                .location_flags
                .contains(xr::SpaceLocationFlags::POSITION_VALID)
            {
                let p = location.pose.position;
                log::debug!("Left hand:  ({:.3}, {:.3}, {:.3})", p.x, p.y, p.z);
            }
        }

        // Poll right controller
        if right_hand_action
            .is_active(&session, xr::Path::NULL)
            .unwrap_or(false)
        {
            let location =
                right_hand_space.locate(&stage, xr_frame_state.predicted_display_time)?;
            if location
                .location_flags
                .contains(xr::SpaceLocationFlags::POSITION_VALID)
            {
                let p = location.pose.position;
                log::debug!("Right hand: ({:.3}, {:.3}, {:.3})", p.x, p.y, p.z);
            }
        }

        // --- Acquire swapchain image ---
        let image_index = swapchain.handle.acquire_image()?;

        // --- Wait for GPU fence from previous use of this frame slot ---
        unsafe {
            vk_device.wait_for_fences(&[fences[frame]], true, u64::MAX)?;
            vk_device.reset_fences(&[fences[frame]])?;
        }

        // Wait for the swapchain image to be available.
        swapchain.handle.wait_image(xr::Duration::INFINITE)?;

        // --- Record Vulkan command buffer ---
        let cmd = cmds[frame];
        unsafe {
            vk_device.begin_command_buffer(
                cmd,
                &vk::CommandBufferBeginInfo::default()
                    .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT),
            )?;

            vk_device.cmd_begin_render_pass(
                cmd,
                &vk::RenderPassBeginInfo::default()
                    .render_pass(render_pass)
                    .framebuffer(swapchain.buffers[image_index as usize].framebuffer)
                    .render_area(vk::Rect2D {
                        offset: vk::Offset2D::default(),
                        extent: swapchain.resolution,
                    })
                    .clear_values(&[vk::ClearValue {
                        color: vk::ClearColorValue {
                            float32: [0.0, 0.0, 0.0, 1.0],
                        },
                    }]),
                vk::SubpassContents::INLINE,
            );

            vk_device.cmd_set_viewport(
                cmd,
                0,
                &[vk::Viewport {
                    x: 0.0,
                    y: 0.0,
                    width: swapchain.resolution.width as f32,
                    height: swapchain.resolution.height as f32,
                    min_depth: 0.0,
                    max_depth: 1.0,
                }],
            );

            vk_device.cmd_set_scissor(
                cmd,
                0,
                &[vk::Rect2D {
                    offset: vk::Offset2D { x: 0, y: 0 },
                    extent: swapchain.resolution,
                }],
            );

            vk_device.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, pipeline);

            // Draw fullscreen triangle (3 vertices, no vertex buffer).
            vk_device.cmd_draw(cmd, 3, 1, 0, 0);

            vk_device.cmd_end_render_pass(cmd);
            vk_device.end_command_buffer(cmd)?;
        }

        // --- Submit to GPU ---
        unsafe {
            vk_device.queue_submit(
                queue,
                &[vk::SubmitInfo::default().command_buffers(&[cmd])],
                fences[frame],
            )?;
        }

        // --- Release swapchain image ---
        swapchain.handle.release_image()?;

        // --- End frame: submit composition layers ---
        let rect = xr::Rect2Di {
            offset: xr::Offset2Di { x: 0, y: 0 },
            extent: xr::Extent2Di {
                width: swapchain.resolution.width as _,
                height: swapchain.resolution.height as _,
            },
        };

        frame_stream.end(
            xr_frame_state.predicted_display_time,
            environment_blend_mode,
            &[&xr::CompositionLayerProjection::new().space(&stage).views(&[
                xr::CompositionLayerProjectionView::new()
                    .pose(views[0].pose)
                    .fov(views[0].fov)
                    .sub_image(
                        xr::SwapchainSubImage::new()
                            .swapchain(&swapchain.handle)
                            .image_array_index(0)
                            .image_rect(rect),
                    ),
                xr::CompositionLayerProjectionView::new()
                    .pose(views[1].pose)
                    .fov(views[1].fov)
                    .sub_image(
                        xr::SwapchainSubImage::new()
                            .swapchain(&swapchain.handle)
                            .image_array_index(1)
                            .image_rect(rect),
                    ),
            ])],
        )?;

        frame = (frame + 1) % PIPELINE_DEPTH as usize;
    }

    // =========================================================================
    // 23. CLEANUP
    // =========================================================================
    log::info!("Shutting down");

    // Wait for all GPU work to finish before destroying resources.
    unsafe {
        vk_device.device_wait_idle()?;

        for fence in &fences {
            vk_device.destroy_fence(*fence, None);
        }
        vk_device.destroy_command_pool(cmd_pool, None);

        for buf in &swapchain.buffers {
            vk_device.destroy_framebuffer(buf.framebuffer, None);
            vk_device.destroy_image_view(buf.color, None);
        }
        // swapchain.handle is dropped automatically by OpenXR

        vk_device.destroy_pipeline(pipeline, None);
        vk_device.destroy_pipeline_layout(pipeline_layout, None);
        vk_device.destroy_render_pass(render_pass, None);

        // Device and instance are dropped by OpenXR, but we should
        // avoid dropping ash wrappers that would call vkDestroy*.
        // ash::Device and ash::Instance don't call destroy on drop,
        // so this is safe.
    }

    log::info!("Clean shutdown complete");
    Ok(())
}
```

**Step 2: Add ctrlc dependency to Cargo.toml**

The main.rs uses `ctrlc` for graceful exit handling. Add it:

```toml
[dependencies]
# ... existing deps ...
ctrlc = "3"
```

**Step 3: Verify it compiles**

Run: `cargo build`
Expected: Compiles successfully. Will fail at runtime without an OpenXR runtime, but should compile cleanly on any platform.

Note: On Linux/WSL without an OpenXR runtime, the program will fail at `Entry::load()` with a clear error message. That's expected — the binary must be run on Windows with the Oculus runtime.

**Step 4: Commit**

```bash
git add src/main.rs Cargo.toml
git commit -m "feat: complete OpenXR + Vulkan prototype with stereo rendering and controller tracking"
```

---

### Task 5: Verify build and create README with run instructions

**Files:**
- Create: `README.md`

**Step 1: Do a full release build**

Run: `cargo build --release`
Expected: Compiles successfully in release mode.

**Step 2: Write README.md**

```markdown
# northcloud-oculus

Minimal OpenXR + Vulkan VR prototype targeting the Oculus Rift CV1.

Renders a solid color per eye (blue left, red right) to validate the full VR pipeline: OpenXR session, Vulkan rendering, head tracking, and controller pose polling.

## Prerequisites

- **Windows 10/11** (the Oculus PC runtime only runs on Windows)
- **Oculus PC app** installed (provides the OpenXR runtime)
- **Rift CV1** connected (HDMI + 2-3 USB sensors)
- **Vulkan-capable GPU** — NVIDIA GTX 970+ or AMD equivalent
- **Rust stable toolchain** — install via [rustup](https://rustup.rs)
- **Vulkan SDK** (optional, for validation layers during development)

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
src/main.rs          — Complete prototype (~400 lines)
shaders/
  fullscreen.vert    — Fullscreen triangle from vertex ID (multiview)
  solid.frag         — Solid color per eye
build.rs             — GLSL → SPIR-V compilation at build time
```
```

**Step 3: Commit**

```bash
git add README.md
git commit -m "docs: add README with build/run instructions"
```

---

### Task Summary

| Task | Description | Estimated Scope |
|------|-------------|-----------------|
| 1 | Project scaffold (Cargo.toml, stub main, gitignore) | 3 files |
| 2 | GLSL shaders (vertex + fragment) | 2 files |
| 3 | Build script for shader compilation | 1 file |
| 4 | Complete main.rs prototype | 1 file (~450 lines) |
| 5 | README with build/run instructions | 1 file |

Total: 8 files, all from scratch (greenfield project).
