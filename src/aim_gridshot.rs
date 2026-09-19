//! M4.a GRIDSHOT: 3×3 exclusive-cell geometry and deterministic cell picks (pure logic).

use bevy::prelude::Vec3;

use crate::aim_trial::{
    active_elapsed_ns, AimPhase, AimRunConfigSnapshot, AimTaskKind, AimTrial, LiveAimTarget,
    AIM_CAMERA_ORIGIN, AIM_TARGET_RADIUS,
};
use crate::camera_ctrl::YawPitch;
use crate::config::ValidationState;

pub const GRIDSHOT_DURATION_SECS: f64 = 60.0;
pub const GRIDSHOT_CONCURRENT: usize = 3;
pub const GRIDSHOT_SPACING: f32 = 1.0;
pub const GRIDSHOT_DEPTH_Z: f32 = -6.0;
pub const GRIDSHOT_TASK_VERSION: &str = "1";
pub const GRIDSHOT_GRID_ROWS: i32 = 3;
pub const GRIDSHOT_GRID_COLS: i32 = 3;
pub const GRIDSHOT_TRIAL_TYPE: &str = "GRIDSHOT";

/// All valid cell indices: `row ∈ {-1,0,1}`, `col ∈ {-1,0,1}`.
pub const GRIDSHOT_CELLS: [(i32, i32); 9] = [
    (-1, -1),
    (-1, 0),
    (-1, 1),
    (0, -1),
    (0, 0),
    (0, 1),
    (1, -1),
    (1, 0),
    (1, 1),
];

/// Same LCG as STATIC_CLICK (`aim_trial`): Mulberry-style advance, unit in [0,1).
fn next_unit(rng: &mut u64) -> f64 {
    *rng = rng
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1);
    ((*rng >> 33) as f64) / (u32::MAX as f64 + 1.0)
}

fn next_index(rng: &mut u64, n: usize) -> usize {
    debug_assert!(n > 0);
    (next_unit(rng) * n as f64) as usize % n
}

/// World-space center of a grid cell. `row`/`col` in `{-1,0,1}`.
pub fn gridshot_cell_center(row: i32, col: i32) -> Vec3 {
    Vec3::new(
        col as f32 * GRIDSHOT_SPACING,
        AIM_CAMERA_ORIGIN.y + row as f32 * GRIDSHOT_SPACING,
        GRIDSHOT_DEPTH_Z,
    )
}

pub fn gridshot_task_config_json() -> String {
    format!(
        r#"{{"grid_rows":{},"grid_cols":{},"concurrent_targets":{},"duration_secs":{},"spacing":{},"depth":{},"radius":{},"rng":"lcg","rng_version":"1"}}"#,
        GRIDSHOT_GRID_ROWS,
        GRIDSHOT_GRID_COLS,
        GRIDSHOT_CONCURRENT,
        GRIDSHOT_DURATION_SECS,
        GRIDSHOT_SPACING,
        GRIDSHOT_DEPTH_Z,
        AIM_TARGET_RADIUS,
    )
}

/// Deterministic: draw 3 distinct `(row, col)` from the 9-cell grid via LCG.
pub fn pick_initial_cells(rng: &mut u64) -> [(i32, i32); 3] {
    let mut pool: Vec<(i32, i32)> = GRIDSHOT_CELLS.to_vec();
    let mut out = [(0, 0); 3];
    for slot in &mut out {
        let i = next_index(rng, pool.len());
        *slot = pool.swap_remove(i);
    }
    out
}

/// Uniform pick among vacant cells. `exclude` is never chosen (e.g. the cell just
/// destroyed) so a replacement cannot visually “stick” on the same spot.
pub fn pick_vacant_cell(
    rng: &mut u64,
    occupied: &[(i32, i32)],
    exclude: Option<(i32, i32)>,
) -> (i32, i32) {
    let vacant: Vec<(i32, i32)> = GRIDSHOT_CELLS
        .iter()
        .copied()
        .filter(|c| !occupied.contains(c))
        .filter(|c| exclude.map_or(true, |ex| *c != ex))
        .collect();
    assert!(
        !vacant.is_empty(),
        "pick_vacant_cell requires at least one eligible vacant cell"
    );
    vacant[next_index(rng, vacant.len())]
}

/// Outcome of one Gridshot click while Armed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GridshotShotResult {
    /// Trial not Armed / not Gridshot.
    Ignored,
    Miss,
    Hit,
}

pub fn gridshot_should_end(trial: &AimTrial, now_ns: u64) -> bool {
    active_elapsed_ns(trial, now_ns) as f64 / 1e9 >= GRIDSHOT_DURATION_SECS
}

fn push_gridshot_spawn(trial: &mut AimTrial, live: &LiveAimTarget, timestamp_ns: u64) {
    let event_index = trial.target_events.len() as u32;
    trial.target_events.push(sense_types::AimTargetEventRecord {
        target_id: live.target_id.clone(),
        event_index,
        timestamp_ns,
        event_type: "spawn".into(),
        position_x: live.center.x as f64,
        position_y: live.center.y as f64,
        position_z: live.center.z as f64,
        yaw_deg: None,
        pitch_deg: None,
        velocity_x: 0.0,
        velocity_y: 0.0,
        velocity_z: 0.0,
        event_data_json: format!(
            r#"{{"cell_row":{},"cell_col":{},"radius":{}}}"#,
            live.row, live.col, AIM_TARGET_RADIUS
        ),
    });
}

fn push_gridshot_despawn(trial: &mut AimTrial, live: &LiveAimTarget, timestamp_ns: u64) {
    let event_index = trial.target_events.len() as u32;
    trial.target_events.push(sense_types::AimTargetEventRecord {
        target_id: live.target_id.clone(),
        event_index,
        timestamp_ns,
        event_type: "despawn".into(),
        position_x: live.center.x as f64,
        position_y: live.center.y as f64,
        position_z: live.center.z as f64,
        yaw_deg: None,
        pitch_deg: None,
        velocity_x: 0.0,
        velocity_y: 0.0,
        velocity_z: 0.0,
        event_data_json: format!(
            r#"{{"cell_row":{},"cell_col":{}}}"#,
            live.row, live.col
        ),
    });
}

fn alloc_live_target(trial: &mut AimTrial, row: i32, col: i32) -> LiveAimTarget {
    let target_id = crate::aim_trial::format_target_id(trial.next_target_ordinal);
    trial.next_target_ordinal = trial.next_target_ordinal.saturating_add(1);
    LiveAimTarget {
        target_id,
        row,
        col,
        center: gridshot_cell_center(row, col),
    }
}

/// Expanded radius for miss association (beyond true hit radius) so near-misses
/// still pick a live sphere by closest positive ray approach.
const MISS_ASSOCIATION_RADIUS: f64 = 2.0;

/// Nearest live target for a miss shot: smallest positive `ray_sphere_hit_t` against
/// [`MISS_ASSOCIATION_RADIUS`], else smallest angular error to center among live.
pub fn nearest_live_target_for_miss<'a>(
    live_targets: &'a [LiveAimTarget],
    origin: [f64; 3],
    dir: [f64; 3],
) -> Option<&'a LiveAimTarget> {
    if live_targets.is_empty() {
        return None;
    }

    let mut best_t: Option<(usize, f64)> = None;
    for (i, live) in live_targets.iter().enumerate() {
        let center = [
            live.center.x as f64,
            live.center.y as f64,
            live.center.z as f64,
        ];
        if let Some(t) =
            sense_math::ray_sphere_hit_t(origin, dir, center, MISS_ASSOCIATION_RADIUS)
        {
            if best_t.map_or(true, |(_, bt)| t < bt) {
                best_t = Some((i, t));
            }
        }
    }
    if let Some((i, _)) = best_t {
        return Some(&live_targets[i]);
    }

    let dlen = (dir[0] * dir[0] + dir[1] * dir[1] + dir[2] * dir[2]).sqrt();
    if dlen <= f64::EPSILON {
        return live_targets.first();
    }
    let dx = dir[0] / dlen;
    let dy = dir[1] / dlen;
    let dz = dir[2] / dlen;

    let mut best_cos: Option<(usize, f64)> = None;
    for (i, live) in live_targets.iter().enumerate() {
        let vx = live.center.x as f64 - origin[0];
        let vy = live.center.y as f64 - origin[1];
        let vz = live.center.z as f64 - origin[2];
        let vlen = (vx * vx + vy * vy + vz * vz).sqrt();
        if vlen <= f64::EPSILON {
            continue;
        }
        let cos = (vx * dx + vy * dy + vz * dz) / vlen;
        if best_cos.map_or(true, |(_, bc)| cos > bc) {
            best_cos = Some((i, cos));
        }
    }
    best_cos
        .map(|(i, _)| &live_targets[i])
        .or_else(|| live_targets.first())
}

pub fn start_gridshot_trial(
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
    trial.random_seed = random_seed;
    trial.rng_state = random_seed;
    trial.task_kind = AimTaskKind::Gridshot;
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

    let cells = pick_initial_cells(&mut trial.rng_state);
    for &(row, col) in &cells {
        let live = alloc_live_target(trial, row, col);
        push_gridshot_spawn(trial, &live, now_ns);
        trial.live_targets.push(live);
    }
    if let Some(first) = trial.live_targets.first() {
        trial.current_center = first.center;
        trial.current_target_id = first.target_id.clone();
    }
    true
}

pub fn apply_gridshot_shot(
    trial: &mut AimTrial,
    pose: &YawPitch,
    timestamp_ns: u64,
) -> GridshotShotResult {
    if trial.phase != AimPhase::Armed || trial.task_kind != AimTaskKind::Gridshot {
        return GridshotShotResult::Ignored;
    }

    let origin = [
        AIM_CAMERA_ORIGIN.x as f64,
        AIM_CAMERA_ORIGIN.y as f64,
        AIM_CAMERA_ORIGIN.z as f64,
    ];
    let dir = crate::aim_trial::look_direction_neg_z(pose.yaw_deg, pose.pitch_deg);

    let mut best: Option<(usize, f64)> = None;
    for (i, live) in trial.live_targets.iter().enumerate() {
        let center = [
            live.center.x as f64,
            live.center.y as f64,
            live.center.z as f64,
        ];
        if let Some(t) =
            sense_math::ray_sphere_hit_t(origin, dir, center, AIM_TARGET_RADIUS as f64)
        {
            if best.map_or(true, |(_, bt)| t < bt) {
                best = Some((i, t));
            }
        }
    }

    trial.last_yaw_deg = pose.yaw_deg;
    trial.last_pitch_deg = pose.pitch_deg;
    trial.last_timestamp_ns = timestamp_ns;

    let Some((hit_idx, _)) = best else {
        let nearest = nearest_live_target_for_miss(&trial.live_targets, origin, dir);
        let (target_id, tx, ty, tz) = match nearest {
            Some(live) => (
                live.target_id.clone(),
                live.center.x as f64,
                live.center.y as f64,
                live.center.z as f64,
            ),
            None => (String::new(), 0.0, 0.0, 0.0),
        };
        trial.shot_log.push(sense_types::AimShotRecord {
            shot_index: trial.shot_log.len() as u32,
            timestamp_ns,
            hit: false,
            yaw_deg: pose.yaw_deg,
            pitch_deg: pose.pitch_deg,
            target_x: tx,
            target_y: ty,
            target_z: tz,
            target_radius: AIM_TARGET_RADIUS as f64,
            target_id,
        });
        trial.last_hit = Some(false);
        return GridshotShotResult::Miss;
    };

    let victim = trial.live_targets[hit_idx].clone();
    trial.shot_log.push(sense_types::AimShotRecord {
        shot_index: trial.shot_log.len() as u32,
        timestamp_ns,
        hit: true,
        yaw_deg: pose.yaw_deg,
        pitch_deg: pose.pitch_deg,
        target_x: victim.center.x as f64,
        target_y: victim.center.y as f64,
        target_z: victim.center.z as f64,
        target_radius: AIM_TARGET_RADIUS as f64,
        target_id: victim.target_id.clone(),
    });
    trial.last_hit = Some(true);
    trial.hits = trial.hits.saturating_add(1);

    // shot → despawn → vacant spawn (same logical timestamp)
    push_gridshot_despawn(trial, &victim, timestamp_ns);
    trial.live_targets.remove(hit_idx);

    let occupied: Vec<(i32, i32)> = trial
        .live_targets
        .iter()
        .map(|t| (t.row, t.col))
        .collect();
    let (row, col) = pick_vacant_cell(
        &mut trial.rng_state,
        &occupied,
        Some((victim.row, victim.col)),
    );
    let replacement = alloc_live_target(trial, row, col);
    push_gridshot_spawn(trial, &replacement, timestamp_ns);
    trial.current_center = replacement.center;
    trial.current_target_id = replacement.target_id.clone();
    trial.live_targets.push(replacement);

    GridshotShotResult::Hit
}

/// End Gridshot at `end_ns`: set score, Idle. Returns true if a completed run should persist.
pub fn finish_gridshot_trial(trial: &mut AimTrial, end_ns: u64) -> bool {
    if trial.phase != AimPhase::Armed || trial.task_kind != AimTaskKind::Gridshot {
        return false;
    }
    let elapsed = active_elapsed_ns(trial, end_ns) as f64 / 1e9;
    trial.score_secs = Some(elapsed);
    trial.phase = AimPhase::Idle;
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::aim_trial::{
        build_completed_aim_trial_record, front_cone_center, start_aim_trial, AimPhase,
        AimRunConfigSnapshot, AimTaskKind, AimTrial, AIM_HITS_TO_FINISH, AIM_CAMERA_ORIGIN,
    };
    use crate::camera_ctrl::YawPitch;
    use crate::config::{ExperimentSettings, ValidationState};

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

    /// Invert `look_direction_neg_z` for test aiming.
    fn yaw_pitch_looking_at(target: Vec3) -> (f64, f64) {
        let d = target - AIM_CAMERA_ORIGIN;
        let len = d.length() as f64;
        assert!(len > 1e-6);
        let dx = d.x as f64 / len;
        let dy = d.y as f64 / len;
        let dz = d.z as f64 / len;
        let pitch_deg = dy.asin().to_degrees();
        let yaw_deg = dx.atan2(-dz).to_degrees();
        (yaw_deg, pitch_deg)
    }

    fn assert_exclusive_live(trial: &AimTrial) {
        assert_eq!(trial.live_targets.len(), GRIDSHOT_CONCURRENT);
        let mut cells = Vec::new();
        for t in &trial.live_targets {
            assert!(!cells.contains(&(t.row, t.col)), "duplicate cell {:?}", (t.row, t.col));
            cells.push((t.row, t.col));
            assert_eq!(t.center, gridshot_cell_center(t.row, t.col));
        }
    }

    #[test]
    fn cell_center_matches_spec_geometry() {
        let c = gridshot_cell_center(0, 0);
        assert_eq!(c, Vec3::new(0.0, AIM_CAMERA_ORIGIN.y, GRIDSHOT_DEPTH_Z));
        let ur = gridshot_cell_center(1, 1);
        assert_eq!(
            ur,
            Vec3::new(
                GRIDSHOT_SPACING,
                AIM_CAMERA_ORIGIN.y + GRIDSHOT_SPACING,
                GRIDSHOT_DEPTH_Z
            )
        );
        let ll = gridshot_cell_center(-1, -1);
        assert_eq!(
            ll,
            Vec3::new(
                -GRIDSHOT_SPACING,
                AIM_CAMERA_ORIGIN.y - GRIDSHOT_SPACING,
                GRIDSHOT_DEPTH_Z
            )
        );
    }

    #[test]
    fn pick_initial_cells_are_distinct() {
        let mut rng = 42u64;
        let cells = pick_initial_cells(&mut rng);
        assert_eq!(cells.len(), 3);
        assert_ne!(cells[0], cells[1]);
        assert_ne!(cells[0], cells[2]);
        assert_ne!(cells[1], cells[2]);
        for &(r, c) in &cells {
            assert!((-1..=1).contains(&r));
            assert!((-1..=1).contains(&c));
        }
    }

    #[test]
    fn same_seed_same_initial_cells() {
        let a = pick_initial_cells(&mut 99u64);
        let b = pick_initial_cells(&mut 99u64);
        assert_eq!(a, b);
        let c = pick_initial_cells(&mut 100u64);
        assert_ne!(a, c);
    }

    #[test]
    fn pick_vacant_never_occupied() {
        let occupied = [(-1, 0), (0, 0), (1, 1)];
        let mut rng = 7u64;
        for _ in 0..40 {
            let (r, c) = pick_vacant_cell(&mut rng, &occupied, None);
            assert!(!occupied.contains(&(r, c)), "picked occupied ({r},{c})");
            assert!((-1..=1).contains(&r));
            assert!((-1..=1).contains(&c));
        }
    }

    #[test]
    fn pick_vacant_excludes_destroyed_cell() {
        let occupied = [(-1, 0), (1, 1)]; // after remove; destroyed was (0,0)
        let destroyed = (0, 0);
        let mut rng = 11u64;
        for _ in 0..60 {
            let (r, c) = pick_vacant_cell(&mut rng, &occupied, Some(destroyed));
            assert_ne!((r, c), destroyed);
            assert!(!occupied.contains(&(r, c)));
        }
    }

    #[test]
    fn task_config_json_has_required_knobs() {
        assert_eq!(GRIDSHOT_TASK_VERSION, "1");
        assert_eq!(GRIDSHOT_TRIAL_TYPE, "GRIDSHOT");
        let j = gridshot_task_config_json();
        assert!(j.contains("\"grid_rows\":3"));
        assert!(j.contains("\"grid_cols\":3"));
        assert!(j.contains("\"concurrent_targets\":3"));
        assert!(j.contains("\"duration_secs\":60"));
        assert!(j.contains("\"spacing\":1"));
        assert!(j.contains("\"depth\":-6") || j.contains("\"depth\":-6.0"));
        assert!(j.contains(&format!("\"radius\":{}", AIM_TARGET_RADIUS)));
        assert!(j.contains("\"rng\":\"lcg\""));
        assert!(j.contains("\"rng_version\":\"1\""));
    }

    #[test]
    fn start_spawns_three_exclusive_cells_same_seed() {
        let mut trial_a = AimTrial::default();
        let mut trial_b = AimTrial::default();
        let mut pose = YawPitch::default();
        let now = 1_000_000_000u64;
        let seed = 99u64;
        assert!(start_gridshot_trial(
            &mut pose,
            &mut trial_a,
            ValidationState::Idle,
            now,
            1_700_000_000_000,
            seed,
            test_config(),
        ));
        assert!(start_gridshot_trial(
            &mut pose,
            &mut trial_b,
            ValidationState::Idle,
            now,
            1_700_000_000_000,
            seed,
            test_config(),
        ));
        assert_eq!(trial_a.task_kind, AimTaskKind::Gridshot);
        assert_eq!(trial_a.phase, AimPhase::Armed);
        assert_exclusive_live(&trial_a);
        let cells_a: Vec<_> = trial_a
            .live_targets
            .iter()
            .map(|t| (t.row, t.col))
            .collect();
        let cells_b: Vec<_> = trial_b
            .live_targets
            .iter()
            .map(|t| (t.row, t.col))
            .collect();
        assert_eq!(cells_a, cells_b);
        assert_eq!(
            cells_a,
            pick_initial_cells(&mut seed.clone())
                .into_iter()
                .collect::<Vec<_>>()
        );
        assert_eq!(trial_a.target_events.len(), 3);
        assert!(trial_a.target_events.iter().all(|e| e.event_type == "spawn"));
        assert_eq!(trial_a.live_targets[0].target_id, "target_001");
        assert_eq!(trial_a.live_targets[1].target_id, "target_002");
        assert_eq!(trial_a.live_targets[2].target_id, "target_003");
    }

    #[test]
    fn hit_respawns_vacant_never_reuses_ids() {
        let mut trial = AimTrial::default();
        let mut pose = YawPitch::default();
        let now = 2_000_000_000u64;
        assert!(start_gridshot_trial(
            &mut pose,
            &mut trial,
            ValidationState::Idle,
            now,
            1_700_000_000_000,
            42,
            test_config(),
        ));
        let before_ids: Vec<_> = trial
            .live_targets
            .iter()
            .map(|t| t.target_id.clone())
            .collect();
        let victim = trial.live_targets[0].clone();
        let (yaw, pitch) = yaw_pitch_looking_at(victim.center);
        pose.yaw_deg = yaw;
        pose.pitch_deg = pitch;
        assert_eq!(
            apply_gridshot_shot(&mut trial, &pose, now + 1),
            GridshotShotResult::Hit
        );
        assert_eq!(trial.hits, 1);
        assert_exclusive_live(&trial);
        assert!(!trial
            .live_targets
            .iter()
            .any(|t| t.target_id == victim.target_id));
        assert!(
            !trial
                .live_targets
                .iter()
                .any(|t| t.row == victim.row && t.col == victim.col),
            "replacement must not reuse destroyed cell ({},{})",
            victim.row,
            victim.col
        );
        let after_ids: Vec<_> = trial
            .live_targets
            .iter()
            .map(|t| t.target_id.clone())
            .collect();
        for id in &after_ids {
            if before_ids.contains(id) {
                assert_ne!(id, &victim.target_id);
            } else {
                assert_eq!(id, "target_004");
            }
        }
        // All ids ever issued remain unique in events.
        let mut seen = std::collections::HashSet::new();
        for e in &trial.target_events {
            if e.event_type == "spawn" {
                assert!(seen.insert(e.target_id.clone()), "reused id {}", e.target_id);
            }
        }
        assert_eq!(seen.len(), 4);
        // shot → despawn → spawn order at same timestamp
        let shot = trial.shot_log.last().unwrap();
        assert!(shot.hit);
        assert_eq!(shot.target_id, victim.target_id);
        let events_after_start = &trial.target_events[3..];
        assert_eq!(events_after_start[0].event_type, "despawn");
        assert_eq!(events_after_start[0].target_id, victim.target_id);
        assert_eq!(events_after_start[1].event_type, "spawn");
        assert_eq!(events_after_start[1].target_id, "target_004");
        assert_eq!(events_after_start[0].timestamp_ns, now + 1);
        assert_eq!(events_after_start[1].timestamp_ns, now + 1);
    }

    #[test]
    fn miss_keeps_occupancy_stable() {
        let mut trial = AimTrial::default();
        let mut pose = YawPitch::default();
        let now = 3_000_000_000u64;
        assert!(start_gridshot_trial(
            &mut pose,
            &mut trial,
            ValidationState::Idle,
            now,
            1_700_000_000_000,
            7,
            test_config(),
        ));
        let before = trial.live_targets.clone();
        let events_before = trial.target_events.len();
        pose.yaw_deg = 90.0;
        pose.pitch_deg = 0.0;
        assert_eq!(
            apply_gridshot_shot(&mut trial, &pose, now + 1),
            GridshotShotResult::Miss
        );
        assert_eq!(trial.hits, 0);
        assert_eq!(trial.live_targets, before);
        assert_eq!(trial.target_events.len(), events_before);
        assert_eq!(trial.shot_log.len(), 1);
        assert!(!trial.shot_log[0].hit);
        assert!(!trial.shot_log[0].target_id.is_empty());
        assert!(before.iter().any(|t| t.target_id == trial.shot_log[0].target_id));
    }

    #[test]
    fn miss_stamps_nearest_live_target_id_and_center() {
        let mut trial = AimTrial::default();
        let mut pose = YawPitch::default();
        let now = 3_100_000_000u64;
        assert!(start_gridshot_trial(
            &mut pose,
            &mut trial,
            ValidationState::Idle,
            now,
            1_700_000_000_000,
            7,
            test_config(),
        ));
        let intended = trial.live_targets[0].clone();
        let (yaw, pitch) = yaw_pitch_looking_at(intended.center);
        // ~3° off: misses R=0.25 but still hits expanded association radius.
        pose.yaw_deg = yaw + 3.0;
        pose.pitch_deg = pitch;
        let origin = [
            AIM_CAMERA_ORIGIN.x as f64,
            AIM_CAMERA_ORIGIN.y as f64,
            AIM_CAMERA_ORIGIN.z as f64,
        ];
        let dir = crate::aim_trial::look_direction_neg_z(pose.yaw_deg, pose.pitch_deg);
        let expected = nearest_live_target_for_miss(&trial.live_targets, origin, dir)
            .expect("live targets")
            .clone();
        assert_eq!(
            apply_gridshot_shot(&mut trial, &pose, now + 1),
            GridshotShotResult::Miss
        );
        let shot = &trial.shot_log[0];
        assert!(!shot.hit);
        assert_eq!(shot.target_id, expected.target_id);
        assert!((shot.target_x - expected.center.x as f64).abs() < 1e-9);
        assert!((shot.target_y - expected.center.y as f64).abs() < 1e-9);
        assert!((shot.target_z - expected.center.z as f64).abs() < 1e-9);
        assert!(!shot.target_id.is_empty());
    }

    #[test]
    fn timer_ends_at_exactly_60e9_ns() {
        let mut trial = AimTrial::default();
        trial.phase = AimPhase::Armed;
        trial.start_timestamp_ns = 0;
        assert!(!gridshot_should_end(&trial, 59_999_999_999));
        assert!(gridshot_should_end(&trial, 60_000_000_000));
        trial.start_timestamp_ns = 1_000;
        assert!(gridshot_should_end(&trial, 1_000 + 60_000_000_000));
        assert!(!gridshot_should_end(&trial, 1_000 + 59_999_999_999));
    }

    #[test]
    fn gridshot_end_ignores_paused_wall_time() {
        use crate::aim_trial::{begin_aim_pause, end_aim_pause};

        let mut trial = AimTrial::default();
        trial.phase = AimPhase::Armed;
        trial.task_kind = AimTaskKind::Gridshot;
        trial.start_timestamp_ns = 0;
        begin_aim_pause(&mut trial, 50_000_000_000); // 50s play then pause
        assert!(!gridshot_should_end(&trial, 200_000_000_000)); // huge wall, still paused at 50s
        end_aim_pause(&mut trial, 200_000_000_000);
        assert!(!gridshot_should_end(&trial, 200_000_000_000 + 9_000_000_000)); // 59s active
        assert!(gridshot_should_end(&trial, 200_000_000_000 + 10_000_000_000)); // 60s active
    }

    #[test]
    fn finish_sets_score_and_idle_for_persist() {
        let mut trial = AimTrial::default();
        let mut pose = YawPitch::default();
        let start = 10_000_000_000u64;
        assert!(start_gridshot_trial(
            &mut pose,
            &mut trial,
            ValidationState::Idle,
            start,
            1_700_000_000_000,
            1,
            test_config(),
        ));
        let end = start + 60_000_000_000;
        assert!(finish_gridshot_trial(&mut trial, end));
        assert_eq!(trial.phase, AimPhase::Idle);
        assert!((trial.score_secs.unwrap() - 60.0).abs() < 1e-9);
        let record = build_completed_aim_trial_record(&trial, 1_700_000_000_060, end);
        assert_eq!(record.trial_type, "GRIDSHOT");
        assert_eq!(record.task_version, GRIDSHOT_TASK_VERSION);
        assert!(record.task_config_json.contains("\"concurrent_targets\":3"));
        assert!(record.task_config_json.contains("\"duration_secs\":60"));
    }

    #[test]
    fn finish_crossing_sample_skips_post_end_shot() {
        let mut trial = AimTrial::default();
        let mut pose = YawPitch::default();
        let start = 5_000_000_000u64;
        assert!(start_gridshot_trial(
            &mut pose,
            &mut trial,
            ValidationState::Idle,
            start,
            1_700_000_000_000,
            3,
            test_config(),
        ));
        let end = start + 60_000_000_000;
        assert!(gridshot_should_end(&trial, end));
        // Spec order: end first — do not apply shot on crossing sample.
        assert!(finish_gridshot_trial(&mut trial, end));
        let victim = trial.live_targets.first().cloned();
        if let Some(v) = victim {
            let (yaw, pitch) = yaw_pitch_looking_at(v.center);
            pose.yaw_deg = yaw;
            pose.pitch_deg = pitch;
        }
        assert_eq!(
            apply_gridshot_shot(&mut trial, &pose, end),
            GridshotShotResult::Ignored
        );
        assert!(trial.shot_log.is_empty());
        assert_eq!(trial.hits, 0);
    }

    #[test]
    fn static_click_still_starts_and_finishes() {
        let mut trial = AimTrial::default();
        let mut pose = YawPitch::default();
        let start = 5_000_000_000u64;
        assert!(start_aim_trial(
            &mut pose,
            &mut trial,
            ValidationState::Idle,
            start,
            1_700_000_000_000,
            42,
            test_config(),
        ));
        assert_eq!(trial.task_kind, AimTaskKind::StaticClick);
        assert!(trial.live_targets.is_empty());
        for i in 0..AIM_HITS_TO_FINISH {
            trial.current_center = front_cone_center(0.0, 0.0);
            pose.yaw_deg = 0.0;
            pose.pitch_deg = 0.0;
            crate::aim_trial::apply_aim_shot(
                &mut trial,
                &pose,
                start + (i as u64 + 1) * 100_000_000,
            );
        }
        assert_eq!(trial.phase, AimPhase::Idle);
        assert_eq!(trial.hits, 5);
        let record = build_completed_aim_trial_record(
            &trial,
            1_700_000_000_600,
            start + 500_000_000,
        );
        assert_eq!(record.trial_type, "STATIC_CLICK");
    }
}
