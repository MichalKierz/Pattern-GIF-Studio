#[test]
fn dynamic_egui_sections_use_stable_salted_ids() {
    let ui_sources = [
        include_str!("../src/ui/create_tab.rs"),
        include_str!("../src/ui/parameter_panel.rs"),
        include_str!("../src/ui/patterns_panel.rs"),
        include_str!("../src/ui/effects_panel.rs"),
        include_str!("../src/ui/formula_source_panel.rs"),
        include_str!("../src/ui/colors_panel.rs"),
        include_str!("../src/ui/shape_panel.rs"),
    ];
    let all_ui_sources = ui_sources.join("\n");

    assert!(!all_ui_sources.contains("from_id_source("));
    assert!(!all_ui_sources.contains(".id_source("));

    for required_salt in [
        "pattern_morph_camera_{index}",
        "effect_blend_mode_{index}",
        "effect_morph_camera_{index}",
        "{id_prefix}_formula_layer_blend_{index}",
        "{id_prefix}_domain_pipeline_{index}",
        "create_parameter_scroll",
    ] {
        assert!(
            all_ui_sources.contains(required_salt),
            "missing stable egui id salt: {required_salt}"
        );
    }
}
