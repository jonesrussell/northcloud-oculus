//! Render target creation and UI camera spawning for WorldPanel

use bevy::{
    camera::RenderTarget,
    prelude::*,
    render::render_resource::TextureFormat,
};
use bevy_egui::EguiContext;

use super::{WorldPanel, WorldPanelCamera, WorldPanelDefaults};

/// Creates a render target texture for a WorldPanel
pub fn create_render_target(
    images: &mut Assets<Image>,
    resolution: UVec2,
) -> Handle<Image> {
    let image = Image::new_target_texture(
        resolution.x,
        resolution.y,
        TextureFormat::Rgba8Unorm,
        Some(TextureFormat::Rgba8UnormSrgb),
    );

    images.add(image)
}

/// Spawns the UI camera that renders to a WorldPanel's texture
pub fn spawn_ui_camera(
    commands: &mut Commands,
    panel_entity: Entity,
    texture: Handle<Image>,
    clear_color: Color,
    order: isize,
) -> Entity {
    commands
        .spawn((
            Camera2d,
            Camera {
                order,
                clear_color: ClearColorConfig::Custom(clear_color),
                ..default()
            },
            RenderTarget::Image(texture.into()),
            WorldPanelCamera {
                panel: panel_entity,
            },
            EguiContext::default(),
        ))
        .id()
}

/// Parameters for spawning a WorldPanel
pub struct WorldPanelParams {
    /// Panel size in meters
    pub size: Vec2,
    /// Optional explicit resolution (if None, calculated from defaults)
    pub resolution: Option<UVec2>,
    /// World transform for the panel
    pub transform: Transform,
    /// Optional clear color override
    pub clear_color: Option<Color>,
    /// Camera render order (lower renders first)
    pub camera_order: isize,
}

impl Default for WorldPanelParams {
    fn default() -> Self {
        Self {
            size: Vec2::new(1.0, 0.75),
            resolution: None,
            transform: Transform::from_xyz(0.0, 1.5, -2.0),
            clear_color: None,
            camera_order: -1,
        }
    }
}

/// Spawns a complete WorldPanel with render target, UI camera, and display quad
///
/// Returns the panel entity. The UI camera entity is stored in the WorldPanel component.
pub fn spawn_world_panel(
    commands: &mut Commands,
    images: &mut Assets<Image>,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    defaults: &WorldPanelDefaults,
    params: WorldPanelParams,
) -> Entity {
    let clear_color = params.clear_color.unwrap_or(defaults.clear_color);

    let resolution = params.resolution.unwrap_or_else(|| {
        UVec2::new(
            (params.size.x * defaults.pixels_per_meter as f32) as u32,
            (params.size.y * defaults.pixels_per_meter as f32) as u32,
        )
    });

    let texture = create_render_target(images, resolution);

    let panel_material = materials.add(StandardMaterial {
        base_color_texture: Some(texture.clone()),
        unlit: true,
        alpha_mode: AlphaMode::Blend,
        double_sided: true,
        cull_mode: None,
        ..default()
    });

    let panel_mesh = meshes.add(Rectangle::new(params.size.x, params.size.y));

    let panel_entity = commands
        .spawn((
            Mesh3d(panel_mesh),
            MeshMaterial3d(panel_material),
            params.transform,
        ))
        .id();

    let ui_camera = spawn_ui_camera(
        commands,
        panel_entity,
        texture.clone(),
        clear_color,
        params.camera_order,
    );

    commands.entity(panel_entity).insert(WorldPanel {
        size: params.size,
        resolution,
        ui_camera,
        texture,
    });

    panel_entity
}

/// Helper to spawn a WorldPanel facing a target position (e.g., the user)
#[allow(clippy::too_many_arguments)]
pub fn spawn_world_panel_facing(
    commands: &mut Commands,
    images: &mut Assets<Image>,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    defaults: &WorldPanelDefaults,
    size: Vec2,
    position: Vec3,
    look_at: Vec3,
) -> Entity {
    let transform = Transform::from_translation(position).looking_at(look_at, Vec3::Y);

    spawn_world_panel(
        commands,
        images,
        meshes,
        materials,
        defaults,
        WorldPanelParams {
            size,
            transform,
            ..default()
        },
    )
}
