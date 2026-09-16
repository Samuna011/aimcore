# Task 6 Report: Bevy scene, HFOV camera, egui shell

## Status

Implemented the Bevy 0.19.1 application shell with a fixed eye-height
`Camera3d`, gray floor, unlit reference cube, centered egui crosshair, and
Validation Lab HUD. The HUD displays the required pitch warnings and derives
eDPI and cm/360 from `sense_math` using the 1600 DPI / 0.175 sensitivity
configuration represented as `sense_types::SensitivityConfig`.

No `MouseMotion` reader or camera-yaw system is present. The cursor is locked
and hidden while the primary window is focused, then released when focus is
lost.

## HFOV and TDD

The `fov.rs` unit test was added first. Its initial targeted run failed because
`vertical_fov_radians` did not exist. After implementing
`2 * atan(tan(hfov / 2) / aspect)`, the test passed for both 16:9 and 4:3 and
recovered the same configured horizontal FOV from both vertical projections.

The camera uses Bevy's vertical `PerspectiveProjection::fov`. An update system
recomputes that value from the current primary-window aspect ratio, preserving
the configured 103-degree horizontal FOV through window resizing.

## Dependency and Bevy 0.19 API choices

- Selected `bevy_egui` 0.42.0 because its published dependencies explicitly
  target Bevy 0.19; `bevy_egui` 0.35 is from an older Bevy generation.
- Used `EguiPrimaryContextPass` and the fallible `EguiContexts::ctx_mut()` API
  required by `bevy_egui` 0.42.
- Used Bevy 0.19 `Single` system parameters, `Camera3d`, `Mesh3d`,
  `MeshMaterial3d`, and `CursorOptions`.
- Added a Windows-only exact `windows` 0.62.2 root dependency. Without it,
  `gpu-allocator`'s broad Windows version range was unified with the existing
  `sense-input-win` 0.58 dependency, while `wgpu-hal` 29 compiled against
  Windows 0.62 types. The exact root edge lets both API versions coexist and
  keeps `gpu-allocator` aligned with `wgpu-hal`.

## Verification

`cargo build`, `cargo fmt --all -- --check`, and `cargo test --workspace` all
exited 0. The full workspace suite (including the HFOV unit test) passed and
IDE diagnostics reported no errors. The GUI was not launched as part of
automated verification.
