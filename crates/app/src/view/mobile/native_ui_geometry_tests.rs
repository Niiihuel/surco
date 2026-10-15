//! Verify test-only XCUI entry coordinates against the real retained UI frame.

use super::*;
use limn::UiTree;
use serde_json::{Value, json};

#[test]
fn native_ui_entry_geometry_matches_the_real_phone_frames() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../../../ios/SurcoNativeUITests/EntryGeometry.json"
    ))
    .unwrap();
    let storage: std::collections::HashMap<String, String> = serde_json::from_str(include_str!(
        "../../../../../ios/SurcoNativeUITests/SyntheticCollection.ron"
    ))
    .unwrap();
    let mut state = State::default();
    state.mobile = true;
    state.mobile_ui.native_surfaces = true;
    state.collection = serde_json::from_str(&storage["collection"]).unwrap();
    state.settings = serde_json::from_str(&storage["settings"]).unwrap();
    let size = Size::new(
        fixture["viewport"][0].as_f64().unwrap() as f32,
        fixture["viewport"][1].as_f64().unwrap() as f32,
    );
    let context = eframe::egui::Context::default();
    limn_egui::install_fonts(&context);
    context.set_pixels_per_point(fixture["pixels_per_point"].as_f64().unwrap() as f32);
    context.begin_pass(eframe::egui::RawInput::default());
    let mut targets = serde_json::Map::new();
    for (page, key, ids) in [
        (
            Page::Search,
            "home",
            vec!["mobile-library", "settings", "heard:0"],
        ),
        (
            Page::Library,
            "library",
            vec!["playlist-new", "import-open", "ios-paste:import-card"],
        ),
        (Page::Settings, "settings", vec!["setting:connections"]),
    ] {
        state.page = page;
        let mut tree = UiTree::new(super::view(&state, size));
        limn_egui::attach_fonts(&mut tree, &context);
        tree.set_reduced_motion(true);
        let frame = tree.layout(size);
        let mut entries = serde_json::Map::new();
        for id in ids {
            let node = frame
                .access
                .iter()
                .find(|node| node.id == id)
                .unwrap_or_else(|| panic!("{key}:{id} is visible"));
            let rect = node.rect;
            assert!(
                rect.x >= 0.0
                    && rect.y >= 0.0
                    && rect.x + rect.width <= size.width + 0.01
                    && rect.y + rect.height <= size.height + 0.01,
                "entry target {key}:{id} fits the actual viewport: {rect:?}"
            );
            entries.insert(id.into(), json!([rect.x, rect.y, rect.width, rect.height]));
        }
        targets.insert(key.into(), Value::Object(entries));
    }
    let actual = Value::Object(targets);
    let mut output = context.end_pass();
    // This layout-only fixture does not upload the font atlas to a renderer.
    output.textures_delta.clear();
    eprintln!(
        "SURCO_NATIVE_ENTRY_GEOMETRY={}",
        serde_json::to_string(&actual).unwrap()
    );
    assert_eq!(
        fixture["targets"], actual,
        "update the XCUI coordinate fixture from the real Frame when layout changes"
    );
}
