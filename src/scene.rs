use bevy::{
    camera::Projection,
    prelude::*,
    window::PrimaryWindow,
};

use crate::{camera_ctrl::YawPitch, config::ExperimentSettings, fov::vertical_fov_radians};

const INITIAL_ASPECT_RATIO: f64 = 16.0 / 9.0;

pub fn setup_scene(
    mut commands: Commands,
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
    // Opaque scene floor removed — aim arena provides a see-through grid floor.
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
