use bevy::{
    camera::Projection,
    math::primitives::Plane3d,
    prelude::*,
    window::PrimaryWindow,
};

use crate::{camera_ctrl::YawPitch, config::ExperimentSettings, fov::vertical_fov_radians};

const INITIAL_ASPECT_RATIO: f64 = 16.0 / 9.0;

pub fn setup_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    settings: Res<ExperimentSettings>,
) {
    commands.insert_resource(ClearColor(Color::srgb(0.025, 0.03, 0.04)));

    commands.spawn((
        Camera3d::default(),
        YawPitch::default(),
        Projection::Perspective(PerspectiveProjection {
            fov: vertical_fov_radians(settings.fov_degrees_h, INITIAL_ASPECT_RATIO),
            ..default()
        }),
        Transform::from_xyz(0.0, 1.6, 4.0).looking_at(Vec3::new(0.0, 1.6, -4.0), Vec3::Y),
    ));

    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(30.0, 30.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.18, 0.19, 0.21),
            perceptual_roughness: 0.95,
            ..default()
        })),
    ));
}

pub fn maintain_horizontal_fov(
    window: Single<&Window, With<PrimaryWindow>>,
    settings: Res<ExperimentSettings>,
    mut camera: Single<&mut Projection, With<Camera3d>>,
) {
    let aspect = window.width() as f64 / window.height().max(1.0) as f64;
    if let Projection::Perspective(perspective) = &mut **camera {
        perspective.fov = vertical_fov_radians(settings.fov_degrees_h, aspect);
    }
}
