/// Converts a horizontal field of view to Bevy's vertical perspective FOV.
pub fn vertical_fov_radians(horizontal_fov_deg: f64, aspect: f64) -> f32 {
    (2.0 * ((horizontal_fov_deg.to_radians() / 2.0).tan() / aspect).atan()) as f32
}

#[cfg(test)]
mod tests {
    use super::vertical_fov_radians;

    #[test]
    fn vertical_fov_changes_with_aspect_while_horizontal_fov_stays_constant() {
        let horizontal_fov_deg = 103.0;
        let vfov_16_9 = vertical_fov_radians(horizontal_fov_deg, 16.0 / 9.0);
        let vfov_4_3 = vertical_fov_radians(horizontal_fov_deg, 4.0 / 3.0);

        assert!(vfov_16_9 < vfov_4_3);

        let recovered_hfov_16_9 = 2.0 * ((vfov_16_9 as f64 / 2.0).tan() * (16.0 / 9.0)).atan();
        let recovered_hfov_4_3 = 2.0 * ((vfov_4_3 as f64 / 2.0).tan() * (4.0 / 3.0)).atan();
        let expected_hfov = horizontal_fov_deg.to_radians();

        assert!((recovered_hfov_16_9 - expected_hfov).abs() < 1.0e-6);
        assert!((recovered_hfov_4_3 - expected_hfov).abs() < 1.0e-6);
    }
}
