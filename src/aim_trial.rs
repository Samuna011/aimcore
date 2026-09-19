//! M3 STATIC_CLICK: front-cone random spheres, 5 hits, time score.

use bevy::prelude::*;

use sense_types::{AimShotRecord, AimTargetEventRecord, AimTrialRecord};

use crate::{
    camera_ctrl::YawPitch,
    config::{ExperimentSettings, ValidationState},
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

pub const AIM_APP_VERSION: &str = "0.1.0";
pub const AIM_EXPERIMENT_ID: &str = "aim_lab";
pub const AIM_EXPERIMENT_VERSION: &str = "0.7.0";
pub const AIM_TRIAL_TYPE: &str = "STATIC_CLICK";
pub const STATIC_CLICK_TASK_VERSION: &str = "1";

pub fn format_target_id(ordinal: u32) -> String {
    format!("target_{ordinal:03}")
}

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

#[derive(Debug, Clone, PartialEq)]
pub struct AimPersistStatus {
    pub trial_id: Option<String>,
    pub score_secs: f64,
    pub hits: u32,
    pub shots: u32,
    pub accuracy: f64,
    pub saved_ok: bool,
    pub error: Option<String>,
}

/// Map insert Ok/Err into HUD-facing persist status (score always filled from trial).
pub fn aim_persist_status_from_insert(
    trial: &AimTrial,
    result: Result<String, String>,
) -> AimPersistStatus {
    let shots = trial.shot_log.len() as u32;
    let hits = trial.hits;
    let accuracy = if shots == 0 {
        0.0
    } else {
        hits as f64 / shots as f64
    };
    let score_secs = trial.score_secs.unwrap_or(0.0);
    match result {
        Ok(id) => AimPersistStatus {
            trial_id: Some(id),
            score_secs,
            hits,
            shots,
            accuracy,
            saved_ok: true,
            error: None,
        },
        Err(error) => AimPersistStatus {
            trial_id: None,
            score_secs,
            hits,
            shots,
            accuracy,
            saved_ok: false,
            error: Some(error),
        },
    }
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
    pub start_unix_ms: i64,
    pub score_secs: Option<f64>,
    pub shot_log: Vec<AimShotRecord>,
    pub target_events: Vec<AimTargetEventRecord>,
    pub random_seed: u64,
    pub current_target_id: String,
    pub next_target_ordinal: u32,
    pub last_persist: Option<AimPersistStatus>,
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
            start_unix_ms: 0,
            score_secs: None,
            shot_log: Vec::new(),
            target_events: Vec::new(),
            random_seed: 0,
            current_target_id: String::new(),
            next_target_ordinal: 1,
            last_persist: None,
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
    start_unix_ms: i64,
    random_seed: u64,
) -> bool {
    if validation.is_running() || trial.phase == AimPhase::Armed {
        return false;
    }
    pose.yaw_deg = 0.0;
    pose.pitch_deg = 0.0;
    // rng_state = seed (reproducibility handle is random_seed; LCG state is runtime-only).
    trial.random_seed = random_seed;
    trial.rng_state = random_seed;
    trial.phase = AimPhase::Armed;
    trial.hits = 0;
    trial.last_hit = None;
    trial.score_secs = None;
    trial.shot_log.clear();
    trial.target_events.clear();
    trial.next_target_ordinal = 1;
    trial.current_target_id.clear();
    trial.start_timestamp_ns = now_ns;
    trial.start_unix_ms = start_unix_ms;
    spawn_next_target(trial, now_ns);
    true
}

pub fn cancel_aim_trial(trial: &mut AimTrial) {
    if trial.phase == AimPhase::Armed {
        trial.hits = 0;
        trial.shot_log.clear();
        trial.target_events.clear();
        trial.score_secs = None;
        trial.last_hit = None;
        trial.current_target_id.clear();
        trial.next_target_ordinal = 1;
    }
    trial.phase = AimPhase::Idle;
}

pub fn static_click_task_config_json() -> String {
    format!(
        r#"{{"hits_required":{},"target_radius":{},"aim_distance":{},"yaw_half_deg":{},"pitch_up_deg":{},"pitch_down_deg":{},"floor_clearance":{},"rng":"lcg","rng_version":"1"}}"#,
        AIM_HITS_TO_FINISH,
        AIM_TARGET_RADIUS,
        AIM_DISTANCE,
        AIM_YAW_HALF_DEG,
        AIM_PITCH_UP_DEG,
        AIM_PITCH_DOWN_DEG,
        AIM_FLOOR_CLEARANCE,
    )
}

pub fn build_completed_aim_trial_record(
    settings: &ExperimentSettings,
    processor_id: &str,
    processor_version: &str,
    processor_config_json: &str,
    trial: &AimTrial,
    end_unix_ms: i64,
    end_timestamp_ns: u64,
) -> AimTrialRecord {
    let shots = trial.shot_log.len() as u32;
    let hits = trial.hits;
    let misses = shots.saturating_sub(hits);
    let accuracy = if shots == 0 {
        0.0
    } else {
        hits as f64 / shots as f64
    };
    let duration_secs =
        (end_timestamp_ns.saturating_sub(trial.start_timestamp_ns)) as f64 / 1e9;
    let score_secs = trial.score_secs.unwrap_or(duration_secs);

    AimTrialRecord {
        id: String::new(),
        app_version: AIM_APP_VERSION.into(),
        experiment_id: AIM_EXPERIMENT_ID.into(),
        experiment_version: AIM_EXPERIMENT_VERSION.into(),
        trial_type: AIM_TRIAL_TYPE.into(),
        status: "completed".into(),
        processor_id: processor_id.into(),
        processor_version: processor_version.into(),
        processor_config_json: processor_config_json.into(),
        dpi: settings.dpi,
        sensitivity: settings.sensitivity,
        polling_rate_hz: settings.polling_rate_hz as f64,
        fov_degrees_h: settings.fov_degrees_h,
        pitch_model_id: String::new(),
        pitch_model_version: String::new(),
        pitch_config_json: "{}".into(),
        resolution_width: 0,
        resolution_height: 0,
        aspect_ratio: 0.0,
        random_seed: trial.random_seed,
        task_version: STATIC_CLICK_TASK_VERSION.into(),
        hardware_config_json: "{}".into(),
        view_config_json: "{}".into(),
        task_config_json: static_click_task_config_json(),
        metrics_json: "{}".into(),
        start_unix_ms: trial.start_unix_ms,
        end_unix_ms,
        start_timestamp_ns: trial.start_timestamp_ns,
        end_timestamp_ns,
        duration_secs,
        hits,
        shots,
        misses,
        score_secs,
        accuracy,
    }
}

fn push_spawn_event(trial: &mut AimTrial, timestamp_ns: u64, yaw_deg: f64, pitch_deg: f64) {
    let event_index = trial.target_events.len() as u32;
    trial.target_events.push(AimTargetEventRecord {
        target_id: trial.current_target_id.clone(),
        event_index,
        timestamp_ns,
        event_type: "spawn".into(),
        position_x: trial.current_center.x as f64,
        position_y: trial.current_center.y as f64,
        position_z: trial.current_center.z as f64,
        yaw_deg: Some(yaw_deg),
        pitch_deg: Some(pitch_deg),
        velocity_x: 0.0,
        velocity_y: 0.0,
        velocity_z: 0.0,
        event_data_json: format!(r#"{{"radius":{}}}"#, AIM_TARGET_RADIUS),
    });
}

fn push_despawn_event(trial: &mut AimTrial, timestamp_ns: u64) {
    let event_index = trial.target_events.len() as u32;
    trial.target_events.push(AimTargetEventRecord {
        target_id: trial.current_target_id.clone(),
        event_index,
        timestamp_ns,
        event_type: "despawn".into(),
        position_x: trial.current_center.x as f64,
        position_y: trial.current_center.y as f64,
        position_z: trial.current_center.z as f64,
        yaw_deg: None,
        pitch_deg: None,
        velocity_x: 0.0,
        velocity_y: 0.0,
        velocity_z: 0.0,
        event_data_json: "{}".into(),
    });
}

fn spawn_next_target(trial: &mut AimTrial, timestamp_ns: u64) {
    let yaw = next_range(&mut trial.rng_state, -AIM_YAW_HALF_DEG, AIM_YAW_HALF_DEG);
    let pitch = next_range(&mut trial.rng_state, -AIM_PITCH_DOWN_DEG, AIM_PITCH_UP_DEG);
    trial.current_center = front_cone_center(yaw, pitch);
    trial.current_target_id = format_target_id(trial.next_target_ordinal);
    trial.next_target_ordinal = trial.next_target_ordinal.saturating_add(1);
    push_spawn_event(trial, timestamp_ns, yaw, pitch);
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
    trial.shot_log.push(AimShotRecord {
        shot_index: trial.shot_log.len() as u32,
        timestamp_ns,
        hit,
        yaw_deg: pose.yaw_deg,
        pitch_deg: pose.pitch_deg,
        target_x: center[0],
        target_y: center[1],
        target_z: center[2],
        target_radius: AIM_TARGET_RADIUS as f64,
        target_id: trial.current_target_id.clone(),
    });
    trial.last_hit = Some(hit);
    trial.last_yaw_deg = pose.yaw_deg;
    trial.last_pitch_deg = pose.pitch_deg;
    trial.last_timestamp_ns = timestamp_ns;

    if !hit {
        // Miss: keep same target; no miss target-event (shots own outcome).
        return false;
    }

    push_despawn_event(trial, timestamp_ns);
    trial.hits = trial.hits.saturating_add(1);
    if trial.hits >= AIM_HITS_TO_FINISH {
        let elapsed = timestamp_ns.saturating_sub(trial.start_timestamp_ns) as f64 / 1e9;
        trial.score_secs = Some(elapsed);
        trial.phase = AimPhase::Idle;
        return true;
    }

    spawn_next_target(trial, timestamp_ns);
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
        assert!(start_aim_trial(&mut pose, &mut trial, ValidationState::Idle, now, 1_700_000_000_000
        , 42));
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
    fn shots_buffer_on_hit_and_miss() {
        let mut trial = AimTrial::default();
        let mut pose = YawPitch::default();
        let now = 1_000_000_000u64;
        assert!(start_aim_trial(&mut pose, &mut trial, ValidationState::Idle, now, 1_700_000_000_000
        , 42));

        pose.yaw_deg = 90.0;
        assert!(!apply_aim_shot(&mut trial, &pose, now + 1));
        assert_eq!(trial.shot_log.len(), 1);
        assert!(!trial.shot_log[0].hit);
        assert_eq!(trial.hits, 0);

        trial.current_center = front_cone_center(0.0, 0.0);
        pose.yaw_deg = 0.0;
        pose.pitch_deg = 0.0;
        assert!(!apply_aim_shot(&mut trial, &pose, now + 2));
        assert_eq!(trial.shot_log.len(), 2);
        assert!(trial.shot_log[1].hit);
        assert_eq!(trial.hits, 1);
    }

    #[test]
    fn fifth_hit_sets_score_and_shot_count() {
        let mut trial = AimTrial::default();
        let mut pose = YawPitch::default();
        let start = 5_000_000_000u64;
        assert!(start_aim_trial(&mut pose, &mut trial, ValidationState::Idle, start, 1_700_000_000_000
        , 42));
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
                assert_eq!(trial.shot_log.len(), AIM_HITS_TO_FINISH as usize);
                assert_eq!(trial.shot_log.len() as u32, trial.hits);
            }
        }
    }

    #[test]
    fn cancel_armed_clears_run_without_persist_ok() {
        let mut trial = AimTrial::default();
        let mut pose = YawPitch::default();
        let now = 1_000_000_000u64;
        assert!(start_aim_trial(&mut pose, &mut trial, ValidationState::Idle, now, 1_700_000_000_000
        , 42));

        pose.yaw_deg = 90.0;
        assert!(!apply_aim_shot(&mut trial, &pose, now + 1));
        assert_eq!(trial.shot_log.len(), 1);

        cancel_aim_trial(&mut trial);
        assert_eq!(trial.phase, AimPhase::Idle);
        assert!(trial.score_secs.is_none());
        assert!(trial.last_persist.is_none());
        assert!(trial.shot_log.is_empty());
        assert!(trial.target_events.is_empty());
        assert_eq!(trial.hits, 0);
    }

    #[test]
    fn cancel_armed_keeps_prior_last_persist() {
        let prior = AimPersistStatus {
            trial_id: Some("aim_20260918_000001".into()),
            score_secs: 0.42,
            hits: 5,
            shots: 6,
            accuracy: 5.0 / 6.0,
            saved_ok: true,
            error: None,
        };
        let mut trial = AimTrial {
            last_persist: Some(prior.clone()),
            ..Default::default()
        };
        let mut pose = YawPitch::default();
        let now = 1_000_000_000u64;
        assert!(start_aim_trial(&mut pose, &mut trial, ValidationState::Idle, now, 1_700_000_000_000
        , 42));
        pose.yaw_deg = 90.0;
        assert!(!apply_aim_shot(&mut trial, &pose, now + 1));

        cancel_aim_trial(&mut trial);
        assert_eq!(trial.last_persist, Some(prior));
        assert!(trial.score_secs.is_none());
    }

    #[test]
    fn build_record_snapshots_task_and_processor() {
        let mut trial = AimTrial::default();
        let mut pose = YawPitch::default();
        let start_ns = 5_000_000_000u64;
        let end_ns = start_ns + 600_000_000;
        assert!(start_aim_trial(&mut pose, &mut trial, ValidationState::Idle, start_ns, 1_700_000_000_000
        , 42));

        for i in 0..AIM_HITS_TO_FINISH {
            trial.current_center = front_cone_center(0.0, 0.0);
            pose.yaw_deg = 0.0;
            pose.pitch_deg = 0.0;
            apply_aim_shot(&mut trial, &pose, start_ns + (i as u64 + 1) * 100_000_000);
        }

        let settings = ExperimentSettings::default();
        let record = build_completed_aim_trial_record(
            &settings,
            "rawaccel_linear",
            "0.2.0",
            r#"{"acceleration":0.01}"#,
            &trial,
            1_700_000_000_600,
            end_ns,
        );

        assert_eq!(record.trial_type, "STATIC_CLICK");
        assert_eq!(record.experiment_version, "0.7.0");
        assert_eq!(record.experiment_id, "aim_lab");
        assert_eq!(record.status, "completed");
        assert!(record.id.is_empty());
        assert_eq!(record.processor_id, "rawaccel_linear");
        assert_eq!(record.processor_version, "0.2.0");
        assert_eq!(record.processor_config_json, r#"{"acceleration":0.01}"#);
        assert!(record.task_config_json.contains("\"hits_required\":5"));
        assert!(record.task_config_json.contains("\"rng\":\"lcg\""));
        assert!(record.task_config_json.contains("\"rng_version\":\"1\""));
        assert_eq!(record.random_seed, 42);
        assert_eq!(record.task_version, STATIC_CLICK_TASK_VERSION);
        assert_eq!(record.hits, 5);
        assert_eq!(record.shots, 5);
        assert_eq!(record.misses, 0);
        assert!((record.accuracy - 1.0).abs() < 1e-9);
        assert!((record.score_secs - 0.5).abs() < 1e-9);
        assert_eq!(record.start_unix_ms, 1_700_000_000_000);
        assert_eq!(record.end_unix_ms, 1_700_000_000_600);
    }

    #[test]
    fn same_seed_yields_same_spawn_centers() {
        let mut centers_a = Vec::new();
        let mut centers_b = Vec::new();
        for centers in [&mut centers_a, &mut centers_b] {
            let mut trial = AimTrial::default();
            let mut pose = YawPitch::default();
            let now = 1_000_000_000u64;
            assert!(start_aim_trial(
                &mut pose,
                &mut trial,
                ValidationState::Idle,
                now,
                1_700_000_000_000,
                42,
            ));
            assert_eq!(trial.random_seed, 42);
            centers.push(trial.current_center);
            for i in 0..2 {
                trial.current_center = front_cone_center(0.0, 0.0);
                pose.yaw_deg = 0.0;
                pose.pitch_deg = 0.0;
                assert!(!apply_aim_shot(
                    &mut trial,
                    &pose,
                    now + (i as u64 + 1) * 10
                ));
                centers.push(trial.current_center);
            }
        }
        assert_eq!(centers_a.len(), 3);
        assert_eq!(centers_a, centers_b);

        // Different seed must diverge (seed drives LCG, not XOR scramble of opaque state).
        let mut trial_other = AimTrial::default();
        let mut pose = YawPitch::default();
        assert!(start_aim_trial(
            &mut pose,
            &mut trial_other,
            ValidationState::Idle,
            1_000_000_000u64,
            1_700_000_000_000,
            43,
        ));
        assert_eq!(trial_other.random_seed, 43);
        assert_ne!(centers_a[0], trial_other.current_center);
    }

    #[test]
    fn spawn_and_despawn_events_without_miss_lifecycle() {
        let mut trial = AimTrial::default();
        let mut pose = YawPitch::default();
        let now = 1_000_000_000u64;
        assert!(start_aim_trial(
            &mut pose,
            &mut trial,
            ValidationState::Idle,
            now,
            1_700_000_000_000,
            7,
        ));
        assert_eq!(trial.target_events.len(), 1);
        assert_eq!(trial.target_events[0].event_type, "spawn");
        assert_eq!(trial.target_events[0].target_id, "target_001");
        assert_eq!(trial.current_target_id, "target_001");

        pose.yaw_deg = 90.0;
        assert!(!apply_aim_shot(&mut trial, &pose, now + 1));
        assert_eq!(
            trial.target_events.len(),
            1,
            "miss must not emit target lifecycle events"
        );
        assert!(trial
            .target_events
            .iter()
            .all(|e| e.event_type == "spawn" || e.event_type == "despawn"));
        assert!(!trial
            .target_events
            .iter()
            .any(|e| e.event_type == "miss" || e.event_type == "hit"));

        trial.current_center = front_cone_center(0.0, 0.0);
        pose.yaw_deg = 0.0;
        pose.pitch_deg = 0.0;
        assert!(!apply_aim_shot(&mut trial, &pose, now + 2));
        assert_eq!(trial.target_events.len(), 3);
        assert_eq!(trial.target_events[1].event_type, "despawn");
        assert_eq!(trial.target_events[1].target_id, "target_001");
        assert_eq!(trial.target_events[2].event_type, "spawn");
        assert_eq!(trial.target_events[2].target_id, "target_002");
        assert_eq!(trial.current_target_id, "target_002");
    }

    #[test]
    fn shots_carry_target_id() {
        let mut trial = AimTrial::default();
        let mut pose = YawPitch::default();
        let now = 1_000_000_000u64;
        assert!(start_aim_trial(
            &mut pose,
            &mut trial,
            ValidationState::Idle,
            now,
            1_700_000_000_000,
            99,
        ));

        pose.yaw_deg = 90.0;
        assert!(!apply_aim_shot(&mut trial, &pose, now + 1));
        assert_eq!(trial.shot_log[0].target_id, "target_001");

        trial.current_center = front_cone_center(0.0, 0.0);
        pose.yaw_deg = 0.0;
        pose.pitch_deg = 0.0;
        assert!(!apply_aim_shot(&mut trial, &pose, now + 2));
        assert_eq!(trial.shot_log[1].target_id, "target_001");
        assert_eq!(trial.current_target_id, "target_002");

        pose.yaw_deg = 90.0;
        assert!(!apply_aim_shot(&mut trial, &pose, now + 3));
        assert_eq!(trial.shot_log[2].target_id, "target_002");
    }

    #[test]
    fn persist_status_ok_and_err_shapes() {
        let mut trial = AimTrial {
            hits: 5,
            score_secs: Some(0.42),
            ..Default::default()
        };
        trial.shot_log = vec![
            AimShotRecord {
                shot_index: 0,
                timestamp_ns: 1,
                hit: true,
                yaw_deg: 0.0,
                pitch_deg: 0.0,
                target_x: 0.0,
                target_y: 0.0,
                target_z: 0.0,
                target_radius: 0.25,
                target_id: String::new(),
            };
            6
        ];

        let ok = aim_persist_status_from_insert(&trial, Ok("aim_20260918_000001".into()));
        assert!(ok.saved_ok);
        assert_eq!(ok.trial_id.as_deref(), Some("aim_20260918_000001"));
        assert!(ok.error.is_none());
        assert_eq!(ok.hits, 5);
        assert_eq!(ok.shots, 6);
        assert!((ok.accuracy - 5.0 / 6.0).abs() < 1e-9);
        assert!((ok.score_secs - 0.42).abs() < 1e-9);

        let err = aim_persist_status_from_insert(&trial, Err("disk full".into()));
        assert!(!err.saved_ok);
        assert!(err.trial_id.is_none());
        assert_eq!(err.error.as_deref(), Some("disk full"));
        assert_eq!(err.hits, 5);
        assert_eq!(err.shots, 6);
        assert!((err.score_secs - 0.42).abs() < 1e-9);
    }
}
