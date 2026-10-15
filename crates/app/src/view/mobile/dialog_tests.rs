use super::*;
use crate::state::{Confirm, Draft};
use limn::{Frame, UiTree};

fn tree(state: &State, size: Size) -> UiTree {
    let mut tree = UiTree::new(super::view(state, size));
    tree.set_reduced_motion(true);
    tree.layout(size);
    tree
}

fn target(frame: &Frame, id: &str, size: Size) {
    let node = frame
        .access
        .iter()
        .find(|node| node.id == id)
        .unwrap_or_else(|| panic!("{id} is present"));
    let rect = node.rect;
    assert!(
        rect.width >= 43.99 && rect.height >= 43.99,
        "{id} is touch sized: {rect:?}"
    );
    assert!(
        rect.x >= -0.01
            && rect.y >= -0.01
            && rect.x + rect.width <= size.width + 0.01
            && rect.y + rect.height <= size.height + 0.01,
        "{id} fits the keyboard viewport: {rect:?}"
    );
}

fn reveal(tree: &mut UiTree, id: &str, size: Size) {
    tree.reveal(id);
    target(&tree.layout(size), id, size);
}

#[test]
fn playlist_fields_scroll_while_creation_actions_stay_above_the_keyboard() {
    let size = Size::new(320.0, 240.0);
    let mut state = State::default();
    state.mobile = true;
    state.page = Page::Library;
    state.creating = Some(Draft {
        name: "Un nombre largo para nuestra playlist de viaje".into(),
        collaborative: false,
        first: Some(Track { id: "first".into(), title: "Una canción de nombre largo para nuestra playlist de viaje ".repeat(8), artist: "Un artista".into(), artist_id: None, album: None, album_id: None, artwork_url: None, duration_ms: 180_000 }),
    });
    let full = Size::new(390.0, 844.0);
    assert!(!tree(&state, full).layout(full).access.iter().any(|node| node.id.starts_with("playlist-kind:")),
        "phone creation does not offer collaboration before sharing accounts exist");
    let mut tree = tree(&state, size);
    reveal(&mut tree, "playlist-name", size);
    tree.scroll_to_end("modal_body:playlist-cancel");
    let frame = tree.layout(size);
    for id in [
        "playlist-cancel",
        "playlist-create",
        "playlist-cancel:close",
    ] {
        target(&frame, id, size);
    }
    assert!(
        tree.scroll_offset("modal_body:playlist-cancel") > 0.0,
        "the form can move without moving its footer"
    );
}

#[test]
fn importing_has_a_touch_field_and_fixed_actions_in_a_short_viewport() {
    let mut state = State::default();
    state.mobile = true;
    state.importing = Some("https://music.youtube.com/playlist?list=long-link".into());
    for size in [Size::new(320.0, 240.0), Size::new(320.0, 360.0), Size::new(390.0, 360.0), Size::new(430.0, 360.0)] {
        let mut tree = tree(&state, size);
        reveal(&mut tree, "import-link", size);
        assert!(tree.focus("import-link"));
        reveal(&mut tree, "ios-paste:import-link", size);
        let frame = tree.layout(size);
        for id in ["import-cancel", "import-go", "import-cancel:close"] {
            target(&frame, id, size);
        }
        tree.scroll_to_end("modal_body:import-cancel");
        let frame = tree.layout(size);
        if size.height == 240.0 {
            assert!(!crate::mobile_input::can_paste(&tree, &frame, "import-link"), "the focused field scrolled out of view");
        }
        assert!(tree.focus("import-link"));
        tree.reveal("import-link");
        let frame = tree.layout(size);
        assert!(crate::mobile_input::can_paste(&tree, &frame, "import-link"));
    }
}

#[test]
fn sharing_missing_songs_and_confirmation_keep_touch_footers_visible() {
    let size = Size::new(320.0, 360.0);
    let mut state = State::default();
    state.mobile = true;
    state.sharing = Some(Track {
        id: "song".into(),
        title: "Una canción para compartir".into(),
        artist: "Artista".into(),
        artist_id: None,
        album: None,
        album_id: None,
        artwork_url: None,
        duration_ms: 180_000,
    });
    for id in ["share-copy", "share-close:close"] {
        target(&tree(&state, size).layout(size), id, size);
    }
    state.sharing = None;
    state.imported = Some((1, vec!["Una canción que no se encontró".into(); 20]));
    state.missed_open = true;
    for id in [
        "import-missed-copy",
        "import-missed-close",
        "import-missed-close:close",
    ] {
        target(&tree(&state, size).layout(size), id, size);
    }
    state.missed_open = false;
    state.collection.playlists.push(Playlist { id: 1, name: "Una playlist de un nombre muy largo que no debe cubrir los botones al confirmar su eliminación".into(), collaborative: false, tracks: Vec::new() });
    state.confirm = Some(Confirm::DeletePlaylist(1));
    let frame = tree(&state, size).layout(size);
    let dialog = frame.access.iter().find(|node| node.id == "confirm-cancel:dialog").unwrap();
    let spoken = dialog.description.as_deref().unwrap();
    assert!(spoken.contains(&state.collection.playlists[0].name), "the reader identifies the actual playlist");
    assert!(spoken.contains("canciones"), "the reader hears the deletion's consequences");
    for id in ["confirm-yes", "confirm-cancel", "confirm-cancel:close"] {
        target(&frame, id, size);
    }
}

#[test]
fn equalizer_sliders_fit_and_reveal_without_hiding_the_close_control() {
    let size = Size::new(320.0, 360.0);
    let mut state = State::default();
    state.mobile = true;
    state.equalizer_open = true;
    let mut tree = tree(&state, size);
    reveal(&mut tree, "equalizer-reset", size);
    for id in ["equalizer:preamp", "equalizer:0", "equalizer:9"] {
        reveal(&mut tree, id, size);
    }
    target(&tree.layout(size), "equalizer-close:close", size);
}
