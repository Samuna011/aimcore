//! M3 STATIC_CLICK: fixed sphere target, raw LMB-down, ray–sphere hit.

use bevy::prelude::*;
use sense_types::MouseSample;

use crate::{
    camera_ctrl::YawPitch,
    config::{LookCapture, ValidationState},
};

/// Windows raw input: left button down bit in `RAWINPUT` mouse `ulButtons`.
pub const RI_MOUSE_LEFT_BUTTON_DOWN: u32 = 0x0001;

pub const AIM_TARGET_CENTER: Vec3 = Vec3::new(0.0, 1.6, -6.0);
pub const AIM_TARGET_RADIUS: f32 = 0.25;
pub const AIM_CAMERA_ORIGIN: Vec3 = Vec3::new(0.0, 1.6, 4.0);

#[derive(Component)]
pub struct AimTarget;

#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AimPhase {
    #[default]
    Idle,
    Armed,
}

#[derive(Resource, Debug, Clone, Default)]
pub struct AimTrial {
    pub phase: AimPhase,
    pub last_hit: Option<bool>,
    pub last_yaw_deg: f64,
    pub last_pitch_deg: f64,
    pub last_timestamp_ns: u64,
}

pub fn left_button_down(buttons: u32) -> bool {
    buttons & RI_MOUSE_LEFT_BUTTON_DOWN != 0
}

/// Match `apply_yaw_transform`: yaw * pitch, Bevy forward = local −Z.
pub fn look_direction_neg_z(yaw_deg: f64, pitch_deg: f64) -> [f64; 3] {
    let yaw = Quat::from_rotation_y(-(yaw_deg.to_radians() as f32));
    let pitch = Quat::from_rotation_x(pitch_deg.to_radians() as f32);
    let forward = (yaw * pitch) * Vec3::NEG_Z;
    [forward.x as f64, forward.y as f64, forward.z as f64]
}

pub fn start_aim_trial(pose: &mut YawPitch, trial: &mut AimTrial, validation: ValidationState) -> bool {
    if validation.is_running() || trial.phase == AimPhase::Armed {
        return false;
    }
    pose.yaw_deg = 0.0;
    pose.pitch_deg = 0.0;
    trial.phase = AimPhase::Armed;
    trial.last_hit = None;
    true
}

pub fn cancel_aim_trial(trial: &mut AimTrial) {
    trial.phase = AimPhase::Idle;
}

pub fn resolve_aim_click_if_any(
    samples: &[MouseSample],
    pose: &YawPitch,
    look: &LookCapture,
    trial: &mut AimTrial,
) {
    if trial.phase != AimPhase::Armed || !look.enabled {
        return;
    }
    for sample in samples {
        if !left_button_down(sample.buttons) {
            continue;
        }
        let origin = [
            AIM_CAMERA_ORIGIN.x as f64,
            AIM_CAMERA_ORIGIN.y as f64,
            AIM_CAMERA_ORIGIN.z as f64,
        ];
        let dir = look_direction_neg_z(pose.yaw_deg, pose.pitch_deg);
        let center = [
            AIM_TARGET_CENTER.x as f64,
            AIM_TARGET_CENTER.y as f64,
            AIM_TARGET_CENTER.z as f64,
        ];
        let hit = sense_math::ray_sphere_hit(origin, dir, center, AIM_TARGET_RADIUS as f64);
        trial.last_hit = Some(hit);
        trial.last_yaw_deg = pose.yaw_deg;
        trial.last_pitch_deg = pose.pitch_deg;
        trial.last_timestamp_ns = sample.timestamp_ns;
        trial.phase = AimPhase::Idle;
        break;
    }
}

pub fn spawn_aim_target(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.spawn((
        AimTarget,
        Mesh3d(meshes.add(Sphere::new(AIM_TARGET_RADIUS))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.15, 0.85, 0.35),
            unlit: true,
            ..default()
        })),
        Transform::from_translation(AIM_TARGET_CENTER),
        Visibility::Hidden,
    ));
}

pub fn sync_aim_target_visibility(
    trial: Res<AimTrial>,
    mut targets: Query<&mut Visibility, With<AimTarget>>,
) {
    let vis = if trial.phase == AimPhase::Armed {
        Visibility::Visible
    } else {
        Visibility::Hidden
    };
    for mut visibility in &mut targets {
        *visibility = vis;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn left_button_flag() {
        assert!(left_button_down(RI_MOUSE_LEFT_BUTTON_DOWN));
        assert!(!left_button_down(0));
        assert!(left_button_down(RI_MOUSE_LEFT_BUTTON_DOWN | 0x10));
    }

    #[test]
    fn identity_look_points_toward_target() {
        let dir = look_direction_neg_z(0.0, 0.0);
        assert!(dir[2] < -0.9);
        let origin = [
            AIM_CAMERA_ORIGIN.x as f64,
            AIM_CAMERA_ORIGIN.y as f64,
            AIM_CAMERA_ORIGIN.z as f64,
        ];
        let center = [
            AIM_TARGET_CENTER.x as f64,
            AIM_TARGET_CENTER.y as f64,
            AIM_TARGET_CENTER.z as f64,
        ];
        assert!(sense_math::ray_sphere_hit(
            origin,
            dir,
            center,
            AIM_TARGET_RADIUS as f64
        ));
    }
}
