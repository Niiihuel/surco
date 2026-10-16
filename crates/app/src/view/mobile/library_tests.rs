use super::*;
use crate::backend::Command;
use crate::state::Entry;
use limn::{Point, UiAction, UiEvent, UiTree};
use surco_core::ArtistRef;

fn state() -> State {
    let mut state = State::default();
    state.mobile = true;
    state.page = Page::Library;
    state.settings.import_dismissed = true;
    state.collection.kept.push(Entry::Artist(ArtistRef { id: "held".into(), name: "Un artista de nombre muy largo que debe entrar".into(), subscribers: None, artwork_url: None }));
    state.collection.playlists.push(Playlist { id: 7, name: "Una playlist de nombre largo".into(), collaborative: false, tracks: Vec::new() });
    state
}

fn click(state: &mut State, id: &str) -> Vec<Command> { state.update(&UiAction::Clicked { id: id.into() }) }

#[test]
fn library_menus_keep_the_stable_key_after_the_collection_moves() {
    let mut state = state();
    click(&mut state, "library-more:artist:held");
    let title = state.mobile_ui.library.as_ref().unwrap().title.clone();
    state.collection.kept.insert(0, Entry::Artist(ArtistRef { id: "different".into(), name: "Otro artista".into(), subscribers: None, artwork_url: None }));
    if let Entry::Artist(artist) = &mut state.collection.kept[1] { artist.name = "Nombre actualizado".into(); }
    assert_eq!(state.mobile_ui.library.as_ref().unwrap().title, title);
    click(&mut state, "library-action:pin");
    assert_eq!(state.collection.pinned, vec!["artist:held"]);
    click(&mut state, "library-more:artist:held");
    assert_eq!(click(&mut state, "library-action:open"), vec![Command::Open(Target::Artist("held".into()))]);
    click(&mut state, "library-more:artist:held");
    click(&mut state, "library-action:remove");
    assert!(matches!(state.collection.kept.as_slice(), [Entry::Artist(artist)] if artist.id == "different"));
}

#[test]
fn explicit_library_overflow_and_the_hold_share_a_touch_sized_action() {
    let mut state = state();
    for width in [320.0, 390.0, 430.0] {
        for mode in [LibraryView::List, LibraryView::Compact, LibraryView::Grid] {
            state.settings.library_view = mode;
            let size = Size::new(width, 600.0);
            let mut tree = UiTree::new(library(&state, false, width, SCHEME.palette()));
            tree.set_reduced_motion(true);
            let frame = tree.layout(size);
            for id in ["library-more:downloads", "library-more:playlist:7", "library-more:artist:held", "library-search"] {
                let node = frame.access.iter().find(|node| node.id == id).unwrap_or_else(|| panic!("{id} in {mode:?} at {width}"));
                let rect = node.rect;
                assert!(rect.width >= 43.99 && rect.height >= 43.99 && rect.x >= 0.0 && rect.x + rect.width <= width + 0.01, "{id}: {rect:?}");
            }
            let row = frame.access.iter().find(|node| node.id == "kept:0").unwrap().rect;
            let actions = tree.handle_event(UiEvent::ContextMenu { position: Point::new(row.x + 8.0, row.y + 8.0) });
            assert!(actions.iter().any(|action| matches!(action, UiAction::Clicked { id } if id == "library-more:artist:held")), "hold and overflow open the same sheet: {actions:?}");
        }
    }
    state.mobile = false;
    let frame = UiTree::new(library(&state, false, 390.0, SCHEME.palette())).layout(Size::new(390.0, 600.0));
    assert!(!frame.access.iter().any(|node| node.id.starts_with("library-more:")));
}

#[test]
fn library_sheet_actions_fit_and_the_shared_drag_closes_it() {
    let mut state = state();
    click(&mut state, "library-more:artist:held");
    for width in [320.0, 390.0, 430.0] {
        let size = Size::new(width, 360.0);
        let mut tree = UiTree::new(render(&state, Element::spacer(), SCHEME.palette()));
        tree.set_reduced_motion(true);
        tree.layout(size);
        for id in ["library-action:open", "library-action:pin", "library-action:remove", "mobile-library-close:close"] {
            tree.reveal(id);
            let frame = tree.layout(size);
            let rect = frame.access.iter().find(|node| node.id == id).unwrap().rect;
            assert!(rect.width >= 43.99 && rect.height >= 43.99 && rect.x >= 0.0 && rect.y >= 0.0 && rect.x + rect.width <= width + 0.01 && rect.y + rect.height <= size.height + 0.01, "{id}: {rect:?}");
        }
    }
    let id = "mobile-library-close:drag".to_owned();
    state.update(&UiAction::DragStarted { id: id.clone(), position: Point::new(20.0, 20.0) });
    state.update(&UiAction::Dropped { id, target: None, position: Point::new(22.0, 100.0) });
    assert!(state.mobile_ui.library.is_none());
}
