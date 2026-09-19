//! M4.a GRIDSHOT: 3×3 exclusive-cell geometry and deterministic cell picks (pure logic).

use bevy::prelude::Vec3;

use crate::aim_trial::{AIM_CAMERA_ORIGIN, AIM_TARGET_RADIUS};

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

/// Occupancy of the 3×3 grid: at most one live target per cell.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GridOccupancy {
    /// Parallel lists of occupied cells and their target ids (same length ≤ 3).
    cells: Vec<(i32, i32)>,
    target_ids: Vec<String>,
}

impl GridOccupancy {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.cells.len()
    }

    pub fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }

    pub fn occupied_cells(&self) -> &[(i32, i32)] {
        &self.cells
    }

    pub fn is_occupied(&self, row: i32, col: i32) -> bool {
        self.cells.iter().any(|&(r, c)| r == row && c == col)
    }

    pub fn target_at(&self, row: i32, col: i32) -> Option<&str> {
        self.cells
            .iter()
            .position(|&(r, c)| r == row && c == col)
            .map(|i| self.target_ids[i].as_str())
    }

    /// Place `target_id` in `(row, col)`. Panics if the cell is already occupied.
    pub fn occupy(&mut self, row: i32, col: i32, target_id: impl Into<String>) {
        assert!(
            !self.is_occupied(row, col),
            "cell ({row},{col}) already occupied"
        );
        self.cells.push((row, col));
        self.target_ids.push(target_id.into());
    }

    pub fn vacate(&mut self, row: i32, col: i32) -> Option<String> {
        let i = self.cells.iter().position(|&(r, c)| r == row && c == col)?;
        self.cells.remove(i);
        Some(self.target_ids.remove(i))
    }
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

/// Uniform pick among currently vacant cells (assumes at least one vacant).
pub fn pick_vacant_cell(rng: &mut u64, occupied: &[(i32, i32)]) -> (i32, i32) {
    let vacant: Vec<(i32, i32)> = GRIDSHOT_CELLS
        .iter()
        .copied()
        .filter(|c| !occupied.contains(c))
        .collect();
    assert!(
        !vacant.is_empty(),
        "pick_vacant_cell requires at least one vacant cell"
    );
    vacant[next_index(rng, vacant.len())]
}

#[cfg(test)]
mod tests {
    use super::*;

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
            let (r, c) = pick_vacant_cell(&mut rng, &occupied);
            assert!(!occupied.contains(&(r, c)), "picked occupied ({r},{c})");
            assert!((-1..=1).contains(&r));
            assert!((-1..=1).contains(&c));
        }
    }

    #[test]
    fn grid_occupancy_exclusive_cells() {
        let mut g = GridOccupancy::new();
        assert!(g.is_empty());
        g.occupy(0, 0, "target_001");
        g.occupy(1, -1, "target_002");
        assert_eq!(g.len(), 2);
        assert_eq!(g.occupied_cells(), &[(0, 0), (1, -1)]);
        assert!(g.is_occupied(0, 0));
        assert_eq!(g.target_at(0, 0), Some("target_001"));
        assert_eq!(g.vacate(0, 0), Some("target_001".into()));
        assert!(!g.is_occupied(0, 0));
        assert_eq!(g.len(), 1);
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
}
