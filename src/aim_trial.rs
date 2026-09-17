//! M3 STATIC_CLICK: front-cone random spheres, 5 hits, time score.

use bevy::prelude::*;

use crate::{
    camera_ctrl::YawPitch,
    config::ValidationState,
};

/// Windows raw input: left button down bit in `RAWINPUT` mouse `ulButtons`.
pub const RI_MOUSE_LEFT_BUTTON_DOWN: u32 = 0x0001;

pub const AIM_TARGET_RADIUS: f32 = 0.25;
pub const AIM_CAMERA_ORIGIN: Vec3 = Vec3::new(0.0, 1.6, 4.0);
pub const AIM_DISTANCE: f32 = 10.0;
pub const AIM_YAW_HALF_DEG: f64 = 25.0;
/// Look-up allowance (positive pitch = look up).
pub const AIM_PITCH_UP_DEG: f64 = 12.0;
/// Look-down allowance — kept small so spheres stay above the floor at D=10.
pub const AIM_PITCH_DOWN_DEG: f64 = 5.0;
pub const AIM_FLOOR_CLEARANCE: f32 = 0.35;
pub const AIM_HITS_TO_FINISH: u32 = 5;

#[derive(Component)]
pub struct AimTarget;

#[derive(Component)]
pub struct AimArena;

#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AimPhase {
    #[default]
    Idle,
    Armed,
}

#[derive(Resource, Debug, Clone)]
pub struct AimTrial {
    pub phase: AimPhase,
    pub hits: u32,
    pub last_hit: Option<bool>,
    pub last_yaw_deg: f64,
    pub last_pitch_deg: f64,
    pub last_timestamp_ns: u64,
    pub current_center: Vec3,
    pub start_timestamp_ns: u64,
    pub score_secs: Option<f64>,
    rng_state: u64,
}

impl Default for AimTrial {
    fn default() -> Self {
        Self {
            phase: AimPhase::Idle,
            hits: 0,
            last_hit: None,
            last_yaw_deg: 0.0,
            last_pitch_deg: 0.0,
            last_timestamp_ns: 0,
            current_center: front_cone_center(0.0, 0.0),
            start_timestamp_ns: 0,
            score_secs: None,
            rng_state: 0xC0FFEE,
        }
    }
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

pub fn front_cone_center(yaw_off_deg: f64, pitch_off_deg: f64) -> Vec3 {
    let d = look_direction_neg_z(yaw_off_deg, pitch_off_deg);
    let mut center = AIM_CAMERA_ORIGIN
        + Vec3::new(d[0] as f32, d[1] as f32, d[2] as f32) * AIM_DISTANCE;
    center.y = center.y.max(AIM_FLOOR_CLEARANCE);
    center
}

fn next_unit(rng: &mut u64) -> f64 {
    *rng = rng
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1);
    ((*rng >> 33) as f64) / (u32::MAX as f64 + 1.0)
}

fn next_range(rng: &mut u64, lo: f64, hi: f64) -> f64 {
    lo + next_unit(rng) * (hi - lo)
}

pub fn random_front_cone_center(rng: &mut u64) -> Vec3 {
    let yaw = next_range(rng, -AIM_YAW_HALF_DEG, AIM_YAW_HALF_DEG);
    // Asymmetric pitch: less downward so targets stay visible above the floor.
    let pitch = next_range(rng, -AIM_PITCH_DOWN_DEG, AIM_PITCH_UP_DEG);
    front_cone_center(yaw, pitch)
}

pub fn start_aim_trial(
    pose: &mut YawPitch,
    trial: &mut AimTrial,
    validation: ValidationState,
    now_ns: u64,
) -> bool {
    if validation.is_running() || trial.phase == AimPhase::Armed {
        return false;
    }
    pose.yaw_deg = 0.0;
    pose.pitch_deg = 0.0;
    trial.rng_state ^= now_ns | 1;
    trial.phase = AimPhase::Armed;
    trial.hits = 0;
    trial.last_hit = None;
    trial.score_secs = None;
    trial.start_timestamp_ns = now_ns;
    trial.current_center = random_front_cone_center(&mut trial.rng_state);
    true
}

pub fn cancel_aim_trial(trial: &mut AimTrial) {
    trial.phase = AimPhase::Idle;
}

/// Returns true if this click ended the whole run (5th hit).
pub fn apply_aim_shot(
    trial: &mut AimTrial,
    pose: &YawPitch,
    timestamp_ns: u64,
) -> bool {
    if trial.phase != AimPhase::Armed {
        return false;
    }
    let origin = [
        AIM_CAMERA_ORIGIN.x as f64,
        AIM_CAMERA_ORIGIN.y as f64,
        AIM_CAMERA_ORIGIN.z as f64,
    ];
    let dir = look_direction_neg_z(pose.yaw_deg, pose.pitch_deg);
    let center = [
        trial.current_center.x as f64,
        trial.current_center.y as f64,
        trial.current_center.z as f64,
    ];
    let hit = sense_math::ray_sphere_hit(origin, dir, center, AIM_TARGET_RADIUS as f64);
    trial.last_hit = Some(hit);
    trial.last_yaw_deg = pose.yaw_deg;
    trial.last_pitch_deg = pose.pitch_deg;
    trial.last_timestamp_ns = timestamp_ns;

    if !hit {
        // Miss: keep same target.
        return false;
    }

    trial.hits = trial.hits.saturating_add(1);
    if trial.hits >= AIM_HITS_TO_FINISH {
        let elapsed = timestamp_ns.saturating_sub(trial.start_timestamp_ns) as f64 / 1e9;
        trial.score_secs = Some(elapsed);
        trial.phase = AimPhase::Idle;
        return true;
    }

    trial.current_center = random_front_cone_center(&mut trial.rng_state);
    false
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
        Transform::from_translation(AIM_CAMERA_ORIGIN + Vec3::NEG_Z * AIM_DISTANCE),
        Visibility::Hidden,
    ));
}

/// Translucent room + edge beams so depth/distance in the front cone are readable.
pub fn spawn_aim_arena(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Camera at z=4 looking −Z; room encloses the spawn cone ahead.
    let room_size = Vec3::new(11.0, 3.4, 12.0);
    let room_center = Vec3::new(0.0, room_size.y * 0.5, -1.0);

    let glass = materials.add(StandardMaterial {
        base_color: Color::srgba(0.35, 0.55, 0.75, 0.12),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        cull_mode: None,
        ..default()
    });
    commands.spawn((
        AimArena,
        Mesh3d(meshes.add(Cuboid::from_size(room_size))),
        MeshMaterial3d(glass),
        Transform::from_translation(room_center),
    ));

    let edge = materials.add(StandardMaterial {
        base_color: Color::srgb(0.55, 0.7, 0.85),
        unlit: true,
        ..default()
    });
    let t = 0.04_f32;
    let hx = room_size.x * 0.5;
    let hy = room_size.y * 0.5;
    let hz = room_size.z * 0.5;
    let corners = [
        Vec3::new(-hx, -hy, -hz),
        Vec3::new(hx, -hy, -hz),
        Vec3::new(-hx, hy, -hz),
        Vec3::new(hx, hy, -hz),
        Vec3::new(-hx, -hy, hz),
        Vec3::new(hx, -hy, hz),
        Vec3::new(-hx, hy, hz),
        Vec3::new(hx, hy, hz),
    ];
    let edges = [
        (0, 1),
        (2, 3),
        (4, 5),
        (6, 7),
        (0, 2),
        (1, 3),
        (4, 6),
        (5, 7),
        (0, 4),
        (1, 5),
        (2, 6),
        (3, 7),
    ];
    for (a, b) in edges {
        let pa = room_center + corners[a];
        let pb = room_center + corners[b];
        let mid = (pa + pb) * 0.5;
        let dir = pb - pa;
        let len = dir.length().max(0.01);
        let rot = Quat::from_rotation_arc(Vec3::Z, dir.normalize());
        let transform = Transform::from_translation(mid)
            .with_rotation(rot)
            .with_scale(Vec3::new(t, t, len));
        commands.spawn((
            AimArena,
            Mesh3d(meshes.add(Cuboid::new(1.0, 1.0, 1.0))),
            MeshMaterial3d(edge.clone()),
            transform,
        ));
    }

    // Floor plate inside the room for stronger ground reference.
    commands.spawn((
        AimArena,
        Mesh3d(meshes.add(Cuboid::new(room_size.x - 0.2, 0.02, room_size.z - 0.2))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.22, 0.24, 0.28),
            unlit: true,
            ..default()
        })),
        Transform::from_translation(Vec3::new(0.0, 0.01, room_center.z)),
    ));
}

pub fn sync_aim_target(
    trial: Res<AimTrial>,
    mut targets: Query<(&mut Visibility, &mut Transform), With<AimTarget>>,
) {
    let vis = if trial.phase == AimPhase::Armed {
        Visibility::Visible
    } else {
        Visibility::Hidden
    };
    for (mut visibility, mut transform) in &mut targets {
        *visibility = vis;
        if trial.phase == AimPhase::Armed {
            transform.translation = trial.current_center;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn left_button_flag() {
        assert!(left_button_down(RI_MOUSE_LEFT_BUTTON_DOWN));
        assert!(!left_button_down(0));
    }

    #[test]
    fn front_cone_centers_stay_forward_and_above_floor() {
        let mut rng = 42u64;
        for _ in 0..80 {
            let c = random_front_cone_center(&mut rng);
            assert!(c.z < AIM_CAMERA_ORIGIN.z - 1.0);
            assert!(
                c.y >= AIM_FLOOR_CLEARANCE - 1e-4,
                "target under floor: y={}",
                c.y
            );
            let to = c - AIM_CAMERA_ORIGIN;
            // After floor clamp, distance may be slightly off the pure cone ray.
            assert!(to.length() > AIM_DISTANCE * 0.85);
            assert!(to.length() < AIM_DISTANCE * 1.15);
        }
    }

    #[test]
    fn miss_keeps_center_hit_advances() {
        let mut trial = AimTrial::default();
        let mut pose = YawPitch::default();
        let now = 1_000_000_000u64;
        assert!(start_aim_trial(&mut pose, &mut trial, ValidationState::Idle, now));
        let first = trial.current_center;

        // Aim away: miss
        pose.yaw_deg = 90.0;
        assert!(!apply_aim_shot(&mut trial, &pose, now + 1));
        assert_eq!(trial.hits, 0);
        assert_eq!(trial.current_center, first);
        assert_eq!(trial.phase, AimPhase::Armed);

        // Aim at target: use look that points at center
        // Approximate: identity hits only dead-ahead; force hit by placing center on -Z
        trial.current_center = front_cone_center(0.0, 0.0);
        pose.yaw_deg = 0.0;
        pose.pitch_deg = 0.0;
        assert!(!apply_aim_shot(&mut trial, &pose, now + 2));
        assert_eq!(trial.hits, 1);
        assert_eq!(trial.phase, AimPhase::Armed);
        assert_ne!(trial.current_center, front_cone_center(0.0, 0.0));
    }

    #[test]
    fn fifth_hit_scores_and_idles() {
        let mut trial = AimTrial::default();
        let mut pose = YawPitch::default();
        let start = 5_000_000_000u64;
        assert!(start_aim_trial(&mut pose, &mut trial, ValidationState::Idle, start));
        for i in 0..AIM_HITS_TO_FINISH {
            trial.current_center = front_cone_center(0.0, 0.0);
            pose.yaw_deg = 0.0;
            pose.pitch_deg = 0.0;
            let done = apply_aim_shot(&mut trial, &pose, start + (i as u64 + 1) * 100_000_000);
            if i + 1 < AIM_HITS_TO_FINISH {
                assert!(!done);
                assert_eq!(trial.phase, AimPhase::Armed);
            } else {
                assert!(done);
                assert_eq!(trial.phase, AimPhase::Idle);
                assert_eq!(trial.hits, 5);
                let score = trial.score_secs.expect("score");
                assert!((score - 0.5).abs() < 1e-9);
            }
        }
    }
}
