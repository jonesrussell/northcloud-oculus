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
    // Load the OpenXR loader. On Windows with Oculus, this finds the Oculus
    // OpenXR runtime DLL via the registry.
    // The "linked" feature on the openxr dependency means the OpenXR loader
    // is linked at compile time. Entry::linked() is available because of this.
    let xr_entry = xr::Entry::linked();

    // Check that the Vulkan extension is available.
    let available_extensions = xr_entry.enumerate_extensions()?;
    if !available_extensions.khr_vulkan_enable2 {
        bail!("OpenXR runtime does not support the KHR_vulkan_enable2 extension");
    }

    // Create the OpenXR instance with Vulkan support.
    let mut enabled_extensions = xr::ExtensionSet::default();
    enabled_extensions.khr_vulkan_enable2 = true;

    let has_touch_plus = available_extensions.meta_touch_controller_plus;
    if has_touch_plus {
        enabled_extensions.meta_touch_controller_plus = true;
        log::info!("Quest 3 Touch Plus controller extension available");
    }

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
    // Suggest bindings for Quest 3 Touch Plus controllers (requires the extension).
    if has_touch_plus {
        xr_instance.suggest_interaction_profile_bindings(
            xr_instance
                .string_to_path("/interaction_profiles/meta/touch_controller_plus")?,
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
    }

    // Oculus Touch profile — Rift CV1 / Quest 2 Touch controllers.
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
        left_hand_action.create_space(&session, xr::Path::NULL, xr::Posef::IDENTITY)?;
    let right_hand_space =
        right_hand_action.create_space(&session, xr::Path::NULL, xr::Posef::IDENTITY)?;

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
    let mut swapchain = {
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
    let mut exit_requested = false;
    let mut frame = 0usize;

    'main_loop: loop {
        // --- Handle exit request (only call request_exit once) ---
        if !running.load(Ordering::Relaxed) && !exit_requested {
            log::info!("Exit requested");
            exit_requested = true;
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
                InteractionProfileChanged(_) => {
                    if session_running {
                        let left_path =
                            xr_instance.string_to_path("/user/hand/left")?;
                        let right_path =
                            xr_instance.string_to_path("/user/hand/right")?;
                        let left_profile = session
                            .current_interaction_profile(left_path)
                            .context("Failed to query left hand interaction profile")?;
                        let right_profile = session
                            .current_interaction_profile(right_path)
                            .context("Failed to query right hand interaction profile")?;
                        if left_profile != xr::Path::NULL {
                            log::info!(
                                "Left hand profile: {}",
                                xr_instance.path_to_string(left_profile)
                                    .context("Failed to convert left profile path to string")?
                            );
                        }
                        if right_profile != xr::Path::NULL {
                            log::info!(
                                "Right hand profile: {}",
                                xr_instance.path_to_string(right_profile)
                                    .context("Failed to convert right profile path to string")?
                            );
                        }
                    }
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

    // Wait for all GPU work to finish before destroying any resources.
    unsafe { vk_device.device_wait_idle()?; }

    // Destroy Vulkan resources that reference swapchain images.
    unsafe {
        for fence in &fences {
            vk_device.destroy_fence(*fence, None);
        }
        vk_device.destroy_command_pool(cmd_pool, None);

        for buf in &swapchain.buffers {
            vk_device.destroy_framebuffer(buf.framebuffer, None);
            vk_device.destroy_image_view(buf.color, None);
        }
    }

    // Drop OpenXR handles in reverse creation order.
    // Swapchain must be dropped before session (it belongs to the session).
    drop(swapchain);
    drop(left_hand_space);
    drop(right_hand_space);
    drop(left_hand_action);
    drop(right_hand_action);
    drop(action_set);
    drop(stage);
    drop(frame_wait);
    drop(frame_stream);
    drop(session);

    // Destroy remaining Vulkan objects and the device/instance.
    unsafe {
        vk_device.destroy_pipeline(pipeline, None);
        vk_device.destroy_pipeline_layout(pipeline_layout, None);
        vk_device.destroy_render_pass(render_pass, None);

        vk_device.destroy_device(None);
        vk_instance.destroy_instance(None);
    }

    log::info!("Clean shutdown complete");
    Ok(())
}
