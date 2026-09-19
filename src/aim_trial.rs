//! M3 STATIC_CLICK: front-cone random spheres, 5 hits, time score.

use bevy::prelude::*;

use sense_types::{
    AimCameraSampleRecord, AimInputSampleRecord, AimShotRecord, AimTargetEventRecord,
    AimTrialRecord,
};

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
pub const AIM_EXPERIMENT_VERSION: &str = "0.9.0";
pub const AIM_TRIAL_TYPE: &str = "STATIC_CLICK";
pub const STATIC_CLICK_TASK_VERSION: &str = "1";
pub const PITCH_MODEL_ID: &str = "unverified_0.1";
pub const PITCH_MODEL_VERSION: &str = "1";

pub fn pitch_config_json() -> String {
    format!(
        r#"{{"yaw_deg_per_count_at_sens_1":{},"pitch_deg_per_count_at_sens_1":{},"pitch_sign":"+dy_look_down","pitch_clamp_deg":{},"certainty":"UNCERTAIN"}}"#,
        sense_math::VALORANT_YAW_DEG_PER_COUNT_AT_SENS_1,
        sense_math::VALORANT_YAW_DEG_PER_COUNT_AT_SENS_1,
        sense_math::PITCH_LIMIT_DEG,
    )
}

pub fn hardware_config_json() -> String {
    r#"{"mouse_model":"","mouse_connection":"","firmware":"","display_refresh_hz":null}"#.into()
}

pub fn view_config_json(horizontal_fov_deg: f64) -> String {
    format!(
        r#"{{"projection":"perspective","horizontal_fov_deg":{horizontal_fov_deg},"vertical_fov_deg":null,"camera_mode":"yaw_pitch","presentation_mode":"AutoNoVsync"}}"#
    )
}

pub fn format_target_id(ordinal: u32) -> String {
    format!("target_{ordinal:03}")
}

#[derive(Component)]
pub struct AimTarget;

/// Index into the AimTarget render pool (0..GRIDSHOT_CONCURRENT).
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct AimTargetSlot(pub usize);

#[derive(Component)]
pub struct AimArena;

#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AimPhase {
    #[default]
    Idle,
    Armed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AimTaskKind {
    #[default]
    StaticClick,
    Gridshot,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LiveAimTarget {
    pub target_id: String,
    pub row: i32,
    pub col: i32,
    pub center: Vec3,
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

/// Immutable experiment/run config captured at Start Aim; persist must use only this.
#[derive(Debug, Clone, PartialEq)]
pub struct AimRunConfigSnapshot {
    pub processor_id: String,
    pub processor_version: String,
    pub processor_config_json: String,
    pub dpi: f64,
    pub sensitivity: f64,
    pub polling_rate_hz: f64,
    pub fov_degrees_h: f64,
    pub pitch_model_id: String,
    pub pitch_model_version: String,
    pub pitch_config_json: String,
    pub resolution_width: u32,
    pub resolution_height: u32,
    pub aspect_ratio: f64,
    pub hardware_config_json: String,
    pub view_config_json: String,
}

impl AimRunConfigSnapshot {
    pub fn from_live(
        settings: &ExperimentSettings,
        processor_id: &str,
        processor_version: &str,
        processor_config_json: &str,
        resolution_width: u32,
        resolution_height: u32,
    ) -> Self {
        let aspect_ratio = if resolution_height == 0 {
            0.0
        } else {
            resolution_width as f64 / resolution_height as f64
        };
        Self {
            processor_id: processor_id.into(),
            processor_version: processor_version.into(),
            processor_config_json: processor_config_json.into(),
            dpi: settings.dpi,
            sensitivity: settings.sensitivity,
            polling_rate_hz: settings.polling_rate_hz as f64,
            fov_degrees_h: settings.fov_degrees_h,
            pitch_model_id: PITCH_MODEL_ID.into(),
            pitch_model_version: PITCH_MODEL_VERSION.into(),
            pitch_config_json: pitch_config_json(),
            resolution_width,
            resolution_height,
            aspect_ratio,
            hardware_config_json: hardware_config_json(),
            view_config_json: view_config_json(settings.fov_degrees_h),
        }
    }
}

#[derive(Resource, Debug, Clone)]
pub struct AimTrial {
    pub phase: AimPhase,
    pub task_kind: AimTaskKind,
    pub live_targets: Vec<LiveAimTarget>,
    pub hits: u32,
    pub last_hit: Option<bool>,
    pub last_yaw_deg: f64,
    pub last_pitch_deg: f64,
    pub last_timestamp_ns: u64,
    pub current_center: Vec3,
    pub start_timestamp_ns: u64,
    pub accumulated_pause_ns: u64,
    pub paused_at_qpc: Option<u64>,
    pub start_unix_ms: i64,
    pub score_secs: Option<f64>,
    pub shot_log: Vec<AimShotRecord>,
    pub target_events: Vec<AimTargetEventRecord>,
    pub input_log: Vec<AimInputSampleRecord>,
    pub camera_log: Vec<AimCameraSampleRecord>,
    pub random_seed: u64,
    pub current_target_id: String,
    pub next_target_ordinal: u32,
    pub last_persist: Option<AimPersistStatus>,
    /// Captured at Start; required for completed persist. Cleared on cancel.
    pub config_snapshot: Option<AimRunConfigSnapshot>,
    input_seq: u64,
    last_raw_timestamp_ns: Option<u64>,
    last_yaw_for_cam: f64,
    last_pitch_for_cam: f64,
    pub(crate) rng_state: u64,
}

impl Default for AimTrial {
    fn default() -> Self {
        Self {
            phase: AimPhase::Idle,
            task_kind: AimTaskKind::StaticClick,
            live_targets: Vec::new(),
            hits: 0,
            last_hit: None,
            last_yaw_deg: 0.0,
            last_pitch_deg: 0.0,
            last_timestamp_ns: 0,
            current_center: front_cone_center(0.0, 0.0),
            start_timestamp_ns: 0,
            accumulated_pause_ns: 0,
            paused_at_qpc: None,
            start_unix_ms: 0,
            score_secs: None,
            shot_log: Vec::new(),
            target_events: Vec::new(),
            input_log: Vec::new(),
            camera_log: Vec::new(),
            random_seed: 0,
            current_target_id: String::new(),
            next_target_ordinal: 1,
            last_persist: None,
            config_snapshot: None,
            input_seq: 0,
            last_raw_timestamp_ns: None,
            last_yaw_for_cam: 0.0,
            last_pitch_for_cam: 0.0,
            rng_state: 0xC0FFEE,
        }
    }
}

impl AimTrial {
    /// Raw QPC interval that the next `push_aim_input_sample` would record.
    pub fn next_input_dt_ns(&self, timestamp_ns: u64) -> u64 {
        match self.last_raw_timestamp_ns {
            Some(prev) => timestamp_ns.saturating_sub(prev),
            None => 0,
        }
    }

    /// Buffer one raw+processed input sample. `dt_ns` is the raw QPC interval (0 first).
    pub fn push_aim_input_sample(
        &mut self,
        timestamp_ns: u64,
        raw_dx: i32,
        raw_dy: i32,
        processed_dx: f64,
        processed_dy: f64,
        dt_used_ns: u64,
        input_speed: Option<f64>,
        acceleration_scale: Option<f64>,
    ) {
        let dt_ns = self.next_input_dt_ns(timestamp_ns);
        let sequence_number = self.input_seq;
        self.input_seq = self.input_seq.saturating_add(1);
        self.last_raw_timestamp_ns = Some(timestamp_ns);
        self.input_log.push(AimInputSampleRecord {
            timestamp_ns,
            sequence_number,
            raw_dx,
            raw_dy,
            processed_dx,
            processed_dy,
            dt_ns,
            dt_used_ns,
            input_speed,
            acceleration_scale,
        });
    }

    /// Buffer camera pose after apply; deltas are vs previous sample (0,0 first).
    pub fn push_aim_camera_sample(&mut self, timestamp_ns: u64, yaw_deg: f64, pitch_deg: f64) {
        let (yaw_delta_deg, pitch_delta_deg) = if self.camera_log.is_empty() {
            (0.0, 0.0)
        } else {
            (
                yaw_deg - self.last_yaw_for_cam,
                pitch_deg - self.last_pitch_for_cam,
            )
        };
        self.last_yaw_for_cam = yaw_deg;
        self.last_pitch_for_cam = pitch_deg;
        self.camera_log.push(AimCameraSampleRecord {
            timestamp_ns,
            yaw_deg,
            pitch_deg,
            yaw_delta_deg,
            pitch_delta_deg,
        });
    }

    pub(crate) fn clear_sample_logs(&mut self) {
        self.input_log.clear();
        self.camera_log.clear();
        self.input_seq = 0;
        self.last_raw_timestamp_ns = None;
        self.last_yaw_for_cam = 0.0;
        self.last_pitch_for_cam = 0.0;
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

/// Draw a random front-cone pose: `(center, yaw_off_deg, pitch_off_deg)`.
pub fn random_front_cone_pose(rng: &mut u64) -> (Vec3, f64, f64) {
    let yaw = next_range(rng, -AIM_YAW_HALF_DEG, AIM_YAW_HALF_DEG);
    // Asymmetric pitch: less downward so targets stay visible above the floor.
    let pitch = next_range(rng, -AIM_PITCH_DOWN_DEG, AIM_PITCH_UP_DEG);
    (front_cone_center(yaw, pitch), yaw, pitch)
}

/// Wall span minus completed pauses; if currently paused, freezes at paused_at.
pub fn active_elapsed_ns(trial: &AimTrial, now_ns: u64) -> u64 {
    let end = trial.paused_at_qpc.unwrap_or(now_ns);
    end.saturating_sub(trial.start_timestamp_ns)
        .saturating_sub(trial.accumulated_pause_ns)
}

pub fn begin_aim_pause(trial: &mut AimTrial, now_ns: u64) {
    if trial.phase != AimPhase::Armed || trial.paused_at_qpc.is_some() {
        return;
    }
    trial.paused_at_qpc = Some(now_ns);
}

pub fn end_aim_pause(trial: &mut AimTrial, now_ns: u64) {
    if let Some(at) = trial.paused_at_qpc.take() {
        trial.accumulated_pause_ns =
            trial.accumulated_pause_ns.saturating_add(now_ns.saturating_sub(at));
    }
}

pub fn start_aim_trial(
    pose: &mut YawPitch,
    trial: &mut AimTrial,
    validation: ValidationState,
    now_ns: u64,
    start_unix_ms: i64,
    random_seed: u64,
    config: AimRunConfigSnapshot,
) -> bool {
    if validation.is_running() || trial.phase == AimPhase::Armed {
        return false;
    }
    pose.yaw_deg = 0.0;
    pose.pitch_deg = 0.0;
    // rng_state = seed (reproducibility handle is random_seed; LCG state is runtime-only).
    trial.random_seed = random_seed;
    trial.rng_state = random_seed;
    trial.task_kind = AimTaskKind::StaticClick;
    trial.live_targets.clear();
    trial.phase = AimPhase::Armed;
    trial.hits = 0;
    trial.last_hit = None;
    trial.score_secs = None;
    trial.shot_log.clear();
    trial.target_events.clear();
    trial.clear_sample_logs();
    trial.next_target_ordinal = 1;
    trial.current_target_id.clear();
    trial.start_timestamp_ns = now_ns;
    trial.accumulated_pause_ns = 0;
    trial.paused_at_qpc = None;
    trial.start_unix_ms = start_unix_ms;
    trial.config_snapshot = Some(config);
    spawn_next_target(trial, now_ns);
    true
}

pub fn cancel_aim_trial(trial: &mut AimTrial) {
    if trial.phase == AimPhase::Armed {
        trial.hits = 0;
        trial.shot_log.clear();
        trial.target_events.clear();
        trial.clear_sample_logs();
        trial.score_secs = None;
        trial.last_hit = None;
        trial.current_target_id.clear();
        trial.next_target_ordinal = 1;
        trial.live_targets.clear();
        trial.config_snapshot = None;
        trial.accumulated_pause_ns = 0;
        trial.paused_at_qpc = None;
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
    trial: &AimTrial,
    end_unix_ms: i64,
    end_timestamp_ns: u64,
) -> AimTrialRecord {
    let snap = trial
        .config_snapshot
        .as_ref()
        .expect("completed aim trial must have Start config snapshot");
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

    let (trial_type, task_version, task_config_json) = match trial.task_kind {
        AimTaskKind::StaticClick => (
            AIM_TRIAL_TYPE.to_string(),
            STATIC_CLICK_TASK_VERSION.to_string(),
            static_click_task_config_json(),
        ),
        AimTaskKind::Gridshot => (
            crate::aim_gridshot::GRIDSHOT_TRIAL_TYPE.to_string(),
            crate::aim_gridshot::GRIDSHOT_TASK_VERSION.to_string(),
            crate::aim_gridshot::gridshot_task_config_json(),
        ),
    };

    AimTrialRecord {
        id: String::new(),
        app_version: AIM_APP_VERSION.into(),
        experiment_id: AIM_EXPERIMENT_ID.into(),
        experiment_version: AIM_EXPERIMENT_VERSION.into(),
        trial_type,
        status: "completed".into(),
        processor_id: snap.processor_id.clone(),
        processor_version: snap.processor_version.clone(),
        processor_config_json: snap.processor_config_json.clone(),
        dpi: snap.dpi,
        sensitivity: snap.sensitivity,
        polling_rate_hz: snap.polling_rate_hz,
        fov_degrees_h: snap.fov_degrees_h,
        pitch_model_id: snap.pitch_model_id.clone(),
        pitch_model_version: snap.pitch_model_version.clone(),
        pitch_config_json: snap.pitch_config_json.clone(),
        resolution_width: snap.resolution_width,
        resolution_height: snap.resolution_height,
        aspect_ratio: snap.aspect_ratio,
        random_seed: trial.random_seed,
        task_version,
        hardware_config_json: snap.hardware_config_json.clone(),
        view_config_json: snap.view_config_json.clone(),
        task_config_json,
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
    let (center, yaw, pitch) = random_front_cone_pose(&mut trial.rng_state);
    trial.current_center = center;
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
        let elapsed = active_elapsed_ns(trial, timestamp_ns) as f64 / 1e9;
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
    let mesh = meshes.add(Sphere::new(AIM_TARGET_RADIUS));
    let material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.15, 0.85, 0.35),
        unlit: true,
        ..default()
    });
    for slot in 0..crate::aim_gridshot::GRIDSHOT_CONCURRENT {
        commands.spawn((
            AimTarget,
            AimTargetSlot(slot),
            Mesh3d(mesh.clone()),
            MeshMaterial3d(material.clone()),
            Transform::from_translation(AIM_CAMERA_ORIGIN + Vec3::NEG_Z * AIM_DISTANCE),
            Visibility::Hidden,
        ));
    }
}

/// See-through floor with gray grid lines (no wall box).
pub fn spawn_aim_arena(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let floor_extent = 24.0_f32;
    let half = floor_extent * 0.5;
    let floor_z = -1.0_f32;
    let y = 0.02_f32;
    let line_t = 0.025_f32;
    let step = 1.0_f32;

    commands.spawn((
        AimArena,
        Mesh3d(meshes.add(Plane3d::default().mesh().size(floor_extent, floor_extent))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgba(0.35, 0.38, 0.42, 0.08),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            cull_mode: None,
            ..default()
        })),
        Transform::from_xyz(0.0, 0.0, floor_z),
    ));

    let line_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.45, 0.48, 0.52),
        unlit: true,
        ..default()
    });

    let mut z = -half;
    while z <= half + 1e-4 {
        commands.spawn((
            AimArena,
            Mesh3d(meshes.add(Cuboid::new(floor_extent, line_t, line_t))),
            MeshMaterial3d(line_mat.clone()),
            Transform::from_xyz(0.0, y, floor_z + z),
        ));
        z += step;
    }
    let mut x = -half;
    while x <= half + 1e-4 {
        commands.spawn((
            AimArena,
            Mesh3d(meshes.add(Cuboid::new(line_t, line_t, floor_extent))),
            MeshMaterial3d(line_mat.clone()),
            Transform::from_xyz(x, y, floor_z),
        ));
        x += step;
    }
}

pub fn sync_aim_target(
    trial: Res<AimTrial>,
    mut targets: Query<(&AimTargetSlot, &mut Visibility, &mut Transform), With<AimTarget>>,
) {
    let armed = trial.phase == AimPhase::Armed;
    for (slot, mut visibility, mut transform) in &mut targets {
        if !armed {
            *visibility = Visibility::Hidden;
            continue;
        }
        match trial.task_kind {
            AimTaskKind::StaticClick => {
                if slot.0 == 0 {
                    *visibility = Visibility::Visible;
                    transform.translation = trial.current_center;
                } else {
                    *visibility = Visibility::Hidden;
                }
            }
            AimTaskKind::Gridshot => {
                if let Some(live) = trial.live_targets.get(slot.0) {
                    *visibility = Visibility::Visible;
                    transform.translation = live.center;
                } else {
                    *visibility = Visibility::Hidden;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> AimRunConfigSnapshot {
        AimRunConfigSnapshot::from_live(
            &ExperimentSettings::default(),
            "none",
            "0.1.0",
            "{}",
            1920,
            1080,
        )
    }

    #[test]
    fn left_button_flag() {
        assert!(left_button_down(RI_MOUSE_LEFT_BUTTON_DOWN));
        assert!(!left_button_down(0));
    }

    #[test]
    fn front_cone_centers_stay_forward_and_above_floor() {
        let mut rng = 42u64;
        for _ in 0..80 {
            let (c, _, _) = random_front_cone_pose(&mut rng);
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
        , 42, test_config()));
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
        , 42, test_config()));

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
        , 42, test_config()));
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
        , 42, test_config()));

        pose.yaw_deg = 90.0;
        assert!(!apply_aim_shot(&mut trial, &pose, now + 1));
        assert_eq!(trial.shot_log.len(), 1);

        cancel_aim_trial(&mut trial);
        assert_eq!(trial.phase, AimPhase::Idle);
        assert!(trial.score_secs.is_none());
        assert!(trial.last_persist.is_none());
        assert!(trial.shot_log.is_empty());
        assert!(trial.target_events.is_empty());
        assert!(trial.input_log.is_empty());
        assert!(trial.camera_log.is_empty());
        assert!(trial.config_snapshot.is_none());
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
        , 42, test_config()));
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
        let settings = ExperimentSettings::default();
        let config = AimRunConfigSnapshot::from_live(
            &settings,
            "rawaccel_linear",
            "0.2.0",
            r#"{"acceleration":0.01}"#,
            1920,
            1080,
        );
        assert!(start_aim_trial(
            &mut pose,
            &mut trial,
            ValidationState::Idle,
            start_ns,
            1_700_000_000_000,
            42,
            config,
        ));

        for i in 0..AIM_HITS_TO_FINISH {
            trial.current_center = front_cone_center(0.0, 0.0);
            pose.yaw_deg = 0.0;
            pose.pitch_deg = 0.0;
            apply_aim_shot(&mut trial, &pose, start_ns + (i as u64 + 1) * 100_000_000);
        }

        let record = build_completed_aim_trial_record(&trial, 1_700_000_000_600, end_ns);

        assert_eq!(record.trial_type, "STATIC_CLICK");
        assert_eq!(record.experiment_version, "0.9.0");
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
        assert_eq!(record.pitch_model_id, "unverified_0.1");
        assert_eq!(record.pitch_model_version, "1");
        assert!(record
            .pitch_config_json
            .contains("\"yaw_deg_per_count_at_sens_1\":0.07"));
        assert!(record
            .pitch_config_json
            .contains("\"pitch_sign\":\"+dy_look_down\""));
        assert!(record.pitch_config_json.contains("\"certainty\":\"UNCERTAIN\""));
        assert_eq!(record.resolution_width, 1920);
        assert_eq!(record.resolution_height, 1080);
        assert!((record.aspect_ratio - 1920.0 / 1080.0).abs() < 1e-9);
        assert!(record.hardware_config_json.contains("\"mouse_model\":\"\""));
        assert!(record
            .hardware_config_json
            .contains("\"display_refresh_hz\":null"));
        assert!(record
            .view_config_json
            .contains("\"projection\":\"perspective\""));
        assert!(record.view_config_json.contains(&format!(
            "\"horizontal_fov_deg\":{}",
            settings.fov_degrees_h
        )));
        assert!(record
            .view_config_json
            .contains("\"presentation_mode\":\"AutoNoVsync\""));
        assert_eq!(record.hits, 5);
        assert_eq!(record.shots, 5);
        assert_eq!(record.misses, 0);
        assert!((record.accuracy - 1.0).abs() < 1e-9);
        assert!((record.score_secs - 0.5).abs() < 1e-9);
        assert_eq!(record.start_unix_ms, 1_700_000_000_000);
        assert_eq!(record.end_unix_ms, 1_700_000_000_600);
    }

    #[test]
    fn start_captures_config_snapshot_fields() {
        let mut trial = AimTrial::default();
        let mut pose = YawPitch::default();
        let mut settings = ExperimentSettings::default();
        settings.dpi = 1600.0;
        settings.sensitivity = 0.2;
        settings.polling_rate_hz = 500;
        settings.fov_degrees_h = 90.0;
        let config = AimRunConfigSnapshot::from_live(
            &settings,
            "rawaccel_linear",
            "0.2.0",
            r#"{"acceleration":0.01}"#,
            1280,
            720,
        );
        assert!(start_aim_trial(
            &mut pose,
            &mut trial,
            ValidationState::Idle,
            1_000_000_000,
            1_700_000_000_000,
            7,
            config.clone(),
        ));
        let snap = trial.config_snapshot.as_ref().expect("snapshot");
        assert_eq!(snap.processor_id, "rawaccel_linear");
        assert_eq!(snap.processor_version, "0.2.0");
        assert_eq!(snap.processor_config_json, r#"{"acceleration":0.01}"#);
        assert_eq!(snap.dpi, 1600.0);
        assert_eq!(snap.sensitivity, 0.2);
        assert_eq!(snap.polling_rate_hz, 500.0);
        assert_eq!(snap.fov_degrees_h, 90.0);
        assert_eq!(snap.resolution_width, 1280);
        assert_eq!(snap.resolution_height, 720);
        assert!((snap.aspect_ratio - 1280.0 / 720.0).abs() < 1e-9);
        assert_eq!(snap.pitch_model_id, PITCH_MODEL_ID);
        assert_eq!(snap.pitch_model_version, PITCH_MODEL_VERSION);
        assert!(!snap.pitch_config_json.is_empty());
        assert!(!snap.hardware_config_json.is_empty());
        assert!(snap.view_config_json.contains("\"horizontal_fov_deg\":90"));
        assert_eq!(*snap, config);
    }

    #[test]
    fn settings_mutation_after_start_does_not_change_built_record() {
        let mut trial = AimTrial::default();
        let mut pose = YawPitch::default();
        let mut settings = ExperimentSettings::default();
        settings.dpi = 3200.0;
        settings.sensitivity = 0.09;
        settings.fov_degrees_h = 103.0;
        let config = AimRunConfigSnapshot::from_live(
            &settings,
            "none",
            "0.1.0",
            "{}",
            1920,
            1080,
        );
        let start_ns = 5_000_000_000u64;
        assert!(start_aim_trial(
            &mut pose,
            &mut trial,
            ValidationState::Idle,
            start_ns,
            1_700_000_000_000,
            42,
            config,
        ));

        // Mutate live settings after Start — must not affect persist snapshot.
        settings.dpi = 800.0;
        settings.sensitivity = 9.99;
        settings.fov_degrees_h = 50.0;
        settings.polling_rate_hz = 125;
        settings.processor_id = "rawaccel_linear".into();

        for i in 0..AIM_HITS_TO_FINISH {
            trial.current_center = front_cone_center(0.0, 0.0);
            pose.yaw_deg = 0.0;
            pose.pitch_deg = 0.0;
            apply_aim_shot(&mut trial, &pose, start_ns + (i as u64 + 1) * 100_000_000);
        }

        let record = build_completed_aim_trial_record(&trial, 1_700_000_000_600, start_ns + 600_000_000);
        assert_eq!(record.dpi, 3200.0);
        assert_eq!(record.sensitivity, 0.09);
        assert_eq!(record.fov_degrees_h, 103.0);
        assert_eq!(record.polling_rate_hz, ExperimentSettings::default().polling_rate_hz as f64);
        assert_eq!(record.processor_id, "none");
        assert_eq!(record.resolution_width, 1920);
        assert_eq!(record.resolution_height, 1080);
        assert!(record.view_config_json.contains("\"horizontal_fov_deg\":103"));
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
                test_config(),
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
            test_config(),
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
            test_config(),
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
            test_config(),
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

    #[test]
    fn push_input_sample_raw_dt_and_seq() {
        let mut trial = AimTrial::default();
        // First sample: dt_ns = 0; none-processor path uses dt_used_ns = dt_ns, speed/scale None.
        trial.push_aim_input_sample(
            1_000_000_000,
            10,
            -4,
            10.0,
            -4.0,
            0, // dt_used_ns matches first-sample dt_ns
            None,
            None,
        );
        assert_eq!(trial.input_log.len(), 1);
        let s0 = &trial.input_log[0];
        assert_eq!(s0.timestamp_ns, 1_000_000_000);
        assert_eq!(s0.sequence_number, 0);
        assert_eq!(s0.raw_dx, 10);
        assert_eq!(s0.raw_dy, -4);
        assert_eq!(s0.processed_dx, 10.0);
        assert_eq!(s0.processed_dy, -4.0);
        assert_eq!(s0.dt_ns, 0);
        assert_eq!(s0.dt_used_ns, 0);
        assert!(s0.input_speed.is_none());
        assert!(s0.acceleration_scale.is_none());

        // Second sample: raw QPC interval; dt_used from Linear clamp (e.g. 1 ms while raw was 0.1 ms).
        trial.push_aim_input_sample(
            1_000_100_000,
            3,
            4,
            4.5,
            6.0,
            1_000_000, // dt_used_ns from eval.dt_ms * 1e6
            Some(50.0),
            Some(1.5),
        );
        assert_eq!(trial.input_log.len(), 2);
        let s1 = &trial.input_log[1];
        assert_eq!(s1.sequence_number, 1);
        assert_eq!(s1.dt_ns, 100_000); // raw 0.1 ms
        assert_eq!(s1.dt_used_ns, 1_000_000);
        assert_eq!(s1.input_speed, Some(50.0));
        assert_eq!(s1.acceleration_scale, Some(1.5));
    }

    #[test]
    fn push_camera_sample_deltas_from_previous() {
        let mut trial = AimTrial::default();
        trial.push_aim_camera_sample(1_000_000_000, 1.5, -0.25);
        assert_eq!(trial.camera_log.len(), 1);
        let c0 = &trial.camera_log[0];
        assert_eq!(c0.timestamp_ns, 1_000_000_000);
        assert_eq!(c0.yaw_deg, 1.5);
        assert_eq!(c0.pitch_deg, -0.25);
        assert_eq!(c0.yaw_delta_deg, 0.0);
        assert_eq!(c0.pitch_delta_deg, 0.0);

        trial.push_aim_camera_sample(1_000_001_000, 2.0, -0.5);
        let c1 = &trial.camera_log[1];
        assert!((c1.yaw_delta_deg - 0.5).abs() < 1e-12);
        assert!((c1.pitch_delta_deg - (-0.25)).abs() < 1e-12);
    }

    #[test]
    fn active_elapsed_excludes_pause_interval() {
        let mut trial = AimTrial::default();
        trial.phase = AimPhase::Armed;
        trial.start_timestamp_ns = 1_000;
        // play 10s
        assert_eq!(active_elapsed_ns(&trial, 1_000 + 10_000_000_000), 10_000_000_000);
        begin_aim_pause(&mut trial, 1_000 + 10_000_000_000);
        // wall +30s while paused → still 10s active
        assert_eq!(active_elapsed_ns(&trial, 1_000 + 40_000_000_000), 10_000_000_000);
        end_aim_pause(&mut trial, 1_000 + 40_000_000_000);
        // +5s more play → 15s active
        assert_eq!(active_elapsed_ns(&trial, 1_000 + 45_000_000_000), 15_000_000_000);
    }

    #[test]
    fn start_and_cancel_clear_input_and_camera_logs() {
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
            test_config(),
        ));

        trial.push_aim_input_sample(now + 1, 1, 0, 1.0, 0.0, 0, None, None);
        trial.push_aim_camera_sample(now + 1, 0.1, 0.0);
        assert!(!trial.input_log.is_empty());
        assert!(!trial.camera_log.is_empty());

        cancel_aim_trial(&mut trial);
        assert!(trial.input_log.is_empty());
        assert!(trial.camera_log.is_empty());

        // Restart must also clear any leftover (and reset seq/timing).
        trial.input_log.push(AimInputSampleRecord {
            timestamp_ns: 99,
            sequence_number: 99,
            raw_dx: 0,
            raw_dy: 0,
            processed_dx: 0.0,
            processed_dy: 0.0,
            dt_ns: 0,
            dt_used_ns: 0,
            input_speed: None,
            acceleration_scale: None,
        });
        trial.camera_log.push(AimCameraSampleRecord {
            timestamp_ns: 99,
            yaw_deg: 9.0,
            pitch_deg: 9.0,
            yaw_delta_deg: 0.0,
            pitch_delta_deg: 0.0,
        });
        assert!(start_aim_trial(
            &mut pose,
            &mut trial,
            ValidationState::Idle,
            now + 10,
            1_700_000_000_010,
            7,
            test_config(),
        ));
        assert!(trial.input_log.is_empty());
        assert!(trial.camera_log.is_empty());
        trial.push_aim_input_sample(now + 11, 2, 0, 2.0, 0.0, 0, None, None);
        assert_eq!(trial.input_log[0].sequence_number, 0);
        assert_eq!(trial.input_log[0].dt_ns, 0);
    }
}
