use photocraft_ui_egui::{UiState, panels::mobile::layer_sheet_height};

#[test]
fn layer_sheet_preserves_a_canvas_in_portrait_landscape_and_keyboard_sizes() {
    for height in [0.0, 120.0, 360.0, 800.0, 1400.0] {
        let sheet = layer_sheet_height(height);
        assert!((0.0..=360.0).contains(&sheet));
        assert!(sheet <= height * 0.5);
    }
    assert_eq!(layer_sheet_height(f32::NAN), 0.0);
    assert_eq!(layer_sheet_height(f32::INFINITY), 0.0);
}

#[test]
fn mobile_state_roundtrips_without_changing_desktop_panels() {
    let mut state = UiState::default();
    let desktop = state.panels.clone();
    let legacy = serde_json::to_value(&state).unwrap();
    assert!(legacy.get("mobile").is_none(), "desktop snapshots keep their existing default shape");
    let restored_legacy: UiState = serde_json::from_value(legacy).unwrap();
    assert!(restored_legacy.mobile.is_default());
    assert_eq!(restored_legacy.panels, desktop);
    state.mobile.layers_open = true;
    state.mobile.options_open = true;
    let serialized = serde_json::to_value(&state).unwrap();
    let restored: UiState = serde_json::from_value(serialized).unwrap();
    assert_eq!(restored.mobile, state.mobile);
    assert_eq!(restored.panels, desktop);
}
