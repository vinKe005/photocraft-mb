//! Browser layout policy, kept independent of browser APIs for regression tests.

/// Explicit URL modes override detection. Otherwise narrow windows and touch devices
/// use the phone controls, including after rotation and when a keyboard opens.
pub fn enabled(query: &str, width: f64, height: f64, coarse_pointer: bool) -> bool {
    let flag = |name: &str| query.trim_start_matches('?').split('&').any(|part| part == name || part.strip_suffix("=1") == Some(name));
    if flag("desktop") {
        return false;
    }
    if flag("mobile") {
        return true;
    }
    if !width.is_finite() || !height.is_finite() || width <= 0.0 || height <= 0.0 {
        return false;
    }
    width <= 720.0 || (coarse_pointer && width.max(height) <= 1400.0)
}

#[cfg(test)]
mod tests {
    use super::enabled;

    #[test]
    fn phone_rotation_keyboard_tablet_and_desktop() {
        for (width, height) in [(390.0, 844.0), (844.0, 390.0), (390.0, 280.0), (1024.0, 768.0)] {
            assert!(enabled("", width, height, true));
        }
        assert!(enabled("", 600.0, 800.0, false));
        assert!(!enabled("", 1280.0, 800.0, false));
        assert!(!enabled("", 1920.0, 1080.0, true));
    }

    #[test]
    fn explicit_modes_are_exact_and_desktop_wins() {
        assert!(enabled("?webgl&mobile", 1920.0, 1080.0, false));
        assert!(enabled("?mobile=1", 1920.0, 1080.0, false));
        assert!(!enabled("?desktop", 390.0, 844.0, true));
        assert!(!enabled("?mobile&desktop=1", 390.0, 844.0, true));
        assert!(!enabled("?file=mobile&notmobile&mobile=0", 1920.0, 1080.0, false));
        assert!(enabled("?file=desktop&notdesktop", 390.0, 844.0, true));
    }

    #[test]
    fn invalid_geometry_does_not_force_phone_controls() {
        for (width, height) in [(f64::NAN, 800.0), (390.0, f64::INFINITY), (0.0, 800.0), (390.0, -1.0)] {
            assert!(!enabled("", width, height, true));
        }
    }
}
