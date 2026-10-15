//! Phone interactions share the desktop commands and collection.

use super::*;

const SEARCH_DELAY: Duration = Duration::from_millis(280);
const SEARCH_HISTORY: usize = 12;

fn sheet_dismiss_id(handle: &str) -> Option<&str> {
    let id = handle.strip_suffix(":drag")?;
    matches!(id, "mobile-context-close" | "mobile-connections-close" | "playlist-cancel"
        | "import-cancel" | "import-missed-close" | "share-close" | "confirm-cancel" | "equalizer-close" | "mobile-library-close").then_some(id)
}

pub(crate) fn is_sheet_handle(id: &str) -> bool { sheet_dismiss_id(id).is_some() }

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum SearchFilter {
    #[default]
    All,
    Tracks,
    Artists,
    Albums,
}

pub(crate) struct TrackContext {
    pub row: String,
    pub track: Track,
    pub playlist: Option<u64>,
    pub upcoming: Option<Vec<Track>>,
}

pub(crate) struct LibraryContext {
    pub key: String,
    pub title: String,
    pub artwork_url: Option<String>,
}

fn library_target(key: &str) -> Option<Target> {
    key.strip_prefix("album:").map(|id| Target::Album(id.into()))
        .or_else(|| key.strip_prefix("artist:").map(|id| Target::Artist(id.into())))
}

#[derive(Default)]
pub(crate) struct MobileUi {
    pub search_editing: bool,
    pub search_filter: SearchFilter,
    pub pending: Option<(String, Instant)>,
    /// The host consumes this once after changing the view.
    pub blur: bool,
    pub context: Option<TrackContext>,
    pub library: Option<LibraryContext>,
    pub connections: bool,
    pub route: Option<(String, String)>,
    pub native_routes: bool,
    pub keyboard_visible: bool,
    pub sheet_drag: Option<(String, limn::BottomSheetDrag)>,
    pub notice: Option<String>,
}

impl State {
    fn mobile_library_context(&self, key: &str) -> Option<LibraryContext> {
        let (title, artwork_url) = if let Some(id) = key.strip_prefix("playlist:") {
            let playlist = self.collection.playlist(id.parse().ok()?)?;
            (playlist.name.as_str(), playlist.tracks.first().and_then(|track| track.artwork_url.clone()))
        } else if key == "downloads" { ("Descargas", None) }
        else if key == "liked" { ("Tus me gusta", None) }
        else {
            let target = library_target(key)?;
            let entry = self.collection.kept.iter().find(|entry| entry.target() == target)?;
            (entry.title(), entry.artwork().clone())
        };
        Some(LibraryContext { key: key.into(), title: title.into(), artwork_url })
    }

    pub(crate) fn mobile_query(&mut self, value: &str, now: Instant) {
        self.query = value.to_owned();
        self.mobile_ui.search_editing = true;
        self.submitted = None;
        self.searching = false;
        self.search_error = None;
        self.found = Found::default();
        let query = value.trim();
        // Importing or opening a pasted link must be an explicit action.
        let link = query.contains("://") || transfer::source(query).is_some();
        self.mobile_ui.pending = (query.chars().count() >= 2 && !link)
            .then(|| (query.to_owned(), now + SEARCH_DELAY));
    }

    pub(crate) fn mobile_tick(&mut self, now: Instant) -> Vec<Command> {
        if !self.mobile || !self.mobile_ui.pending.as_ref().is_some_and(|(_, due)| *due <= now) {
            return Vec::new();
        }
        let (query, _) = self.mobile_ui.pending.take().unwrap();
        if self.page != Page::Search || !self.mobile_search || self.query.trim() != query {
            return Vec::new();
        }
        self.search()
    }

    fn mobile_submit(&mut self) -> Vec<Command> {
        let query = self.query.trim().to_owned();
        self.mobile_ui.pending = None;
        self.mobile_ui.search_editing = false;
        self.mobile_ui.blur = true;
        if !query.is_empty() && !query.contains("://") && transfer::source(&query).is_none() {
            self.collection.searches.retain(|held| held.to_lowercase() != query.to_lowercase());
            self.collection.searches.insert(0, query);
            self.collection.searches.truncate(SEARCH_HISTORY);
        }
        self.search()
    }

    pub(super) fn mobile_action(&mut self, action: &UiAction) -> Option<Vec<Command>> {
        if let UiAction::DragStarted { id, .. } = action {
            if let Some(dismiss) = sheet_dismiss_id(id) {
                self.mobile_ui.sheet_drag = Some((dismiss.to_owned(), limn::BottomSheetDrag::new(dismiss)));
            }
        }
        if let Some((dismiss, drag)) = &mut self.mobile_ui.sheet_drag {
            if drag.handle_action(action) {
                let id = dismiss.clone();
                self.mobile_ui.sheet_drag = None;
                return Some(self.update(&UiAction::Clicked { id }));
            }
            if matches!(action, UiAction::DragCancelled { .. } | UiAction::Dropped { .. }) {
                self.mobile_ui.sheet_drag = None;
                return Some(Vec::new());
            }
        }
        if matches!(action, UiAction::Clicked { .. }) { self.mobile_ui.sheet_drag = None; }
        match action {
            UiAction::InputChanged { id, value } if id == "search" => {
                self.mobile_query(value, Instant::now());
                return Some(Vec::new());
            }
            UiAction::Submitted { id, .. } if id == "search" => return Some(self.mobile_submit()),
            UiAction::Clicked { id } => {
                if let Some(key) = id.strip_prefix("library-more:") {
                    self.mobile_ui.library = self.mobile_library_context(key);
                    self.mobile_ui.context = None;
                    self.mobile_ui.connections = false;
                    self.mobile_ui.search_editing = false;
                    self.mobile_ui.blur = true;
                    return Some(Vec::new());
                }
                if let Some(action) = id.strip_prefix("library-action:") {
                    let context = self.mobile_ui.library.take()?;
                    let key = context.key;
                    return Some(match action {
                        "pin" => { self.collection.pin(&key); Vec::new() }
                        "remove" => {
                            if let Some(target) = library_target(&key) {
                                self.collection.kept.retain(|entry| entry.target() != target);
                                self.collection.pinned.retain(|held| held != &key);
                            }
                            Vec::new()
                        }
                        "open" => {
                            if let Some(target) = library_target(&key) { self.open(target) }
                            else if let Some(id) = key.strip_prefix("playlist:").and_then(|id| id.parse::<u64>().ok()) {
                                let index = self.collection.playlists.iter().position(|playlist| playlist.id == id);
                                index.map_or_else(Vec::new, |index| self.click(&format!("playlist:{index}")))
                            } else { self.click(&key) }
                        }
                        _ => Vec::new(),
                    });
                }
                if let Some(row) = id.strip_prefix("more:") {
                    self.mobile_ui.library = None;
                    self.mobile_ui.connections = false;
                    self.mobile_ui.context = self.track_at(row).cloned().map(|track| TrackContext {
                        row: row.to_owned(), track,
                        playlist: match self.page { Page::Playlist(id) if row.starts_with("page-track:") => Some(id), _ => None },
                        upcoming: row.starts_with("upcoming:").then(|| self.player.upcoming.clone()),
                    });
                    self.mobile_ui.search_editing = false;
                    self.mobile_ui.blur = true;
                    return Some(Vec::new());
                }
                if let Some((action, row)) = id.strip_prefix("do:").and_then(|rest| rest.split_once(':')) {
                    let commands = self.act(action, row);
                    self.mobile_ui.context = None;
                    self.menu = None;
                    return Some(commands);
                }
                if let Some(index) = id.strip_prefix("recent-search:").and_then(|value| value.parse::<usize>().ok()) {
                    if let Some(query) = self.collection.searches.get(index).cloned() {
                        self.query = query;
                        return Some(self.mobile_submit());
                    }
                    return Some(Vec::new());
                }
                if let Some(query) = id.strip_prefix("browse:") {
                    self.mobile_search = true;
                    self.page = Page::Search;
                    self.history.clear();
                    self.query = query.to_owned();
                    return Some(self.mobile_submit());
                }
                if let Some(filter) = id.strip_prefix("search-filter:") {
                    self.mobile_ui.search_filter = match filter { "tracks" => SearchFilter::Tracks, "artists" => SearchFilter::Artists, "albums" => SearchFilter::Albums, _ => SearchFilter::All };
                    return Some(Vec::new());
                }
                match id.as_str() {
                    "mobile-search" | "mobile-discover" => {
                        self.mobile_search = true;
                        if self.page != Page::Search { self.go(Page::Search); }
                        let editing = id == "mobile-discover";
                        self.mobile_ui.search_editing = editing;
                        self.mobile_ui.blur = !editing;
                        self.focus = editing.then(|| "search".into());
                    }
                    "mobile-search-edit" => {
                        self.mobile_ui.search_editing = true;
                        self.mobile_ui.blur = false;
                        self.focus = Some("search".into());
                    }
                    "mobile-search-cancel" => {
                        self.mobile_ui.search_editing = false;
                        self.mobile_ui.pending = None;
                        self.mobile_ui.blur = true;
                        self.focus = None;
                    }
                    "mobile-search-clear" => {
                        self.mobile_query("", Instant::now());
                        self.focus = Some("search".into());
                        self.mobile_ui.blur = false;
                    }
                    "search-submit" => return Some(self.mobile_submit()),
                    "recent-search-clear" => self.collection.searches.clear(),
                    "mobile-context-close" => self.mobile_ui.context = None,
                    "mobile-library-close" => self.mobile_ui.library = None,
                    "mobile-connections-close" => self.mobile_ui.connections = false,
                    "mobile-notice-close" => self.mobile_ui.notice = None,
                    "mobile-connections" | "setting:connections" => {
                        self.mobile_ui.library = None;
                        self.mobile_ui.context = None;
                        self.mobile_ui.connections = true;
                        self.mobile_ui.blur = true;
                        self.mobile_ui.search_editing = false;
                    }
                    // Navigation must cancel delayed requests from the search tab.
                    "home" | "mobile-library" | "mobile-playing" | "settings" | "back" => {
                        self.mobile_ui.library = None;
                        self.mobile_ui.pending = None;
                        self.mobile_ui.search_editing = false;
                        self.mobile_ui.context = None;
                        self.mobile_ui.blur = true;
                        return None;
                    }
                    _ => return None,
                }
                return Some(Vec::new());
            }
            _ => {}
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::super::*;

    fn phone() -> State { State { mobile: true, ..State::default() } }
    fn click(state: &mut State, id: &str) -> Vec<Command> {
        state.update(&UiAction::Clicked { id: id.into() })
    }

    #[test]
    fn search_debounces_and_drops_stale_answers_without_losing_focus() {
        let mut state = phone();
        click(&mut state, "mobile-search");
        assert!(state.focus.is_none(), "opening the tab does not summon the keyboard");
        click(&mut state, "mobile-search-edit");
        assert_eq!(state.focus.as_deref(), Some("search"));
        let now = Instant::now();
        state.mobile_query("Cerati", now);
        assert!(state.mobile_tick(now + Duration::from_millis(200)).is_empty());
        state.mobile_query("Cerati vivo", now + Duration::from_millis(200));
        assert!(state.mobile_tick(now + Duration::from_millis(300)).is_empty());
        assert_eq!(state.mobile_tick(now + Duration::from_millis(500)), vec![Command::Search("Cerati vivo".into())]);
        assert!(state.mobile_ui.search_editing);
        state.mobile_query("Soda", now + Duration::from_secs(1));
        assert!(state.submitted.is_none(), "old replies are invalidated as soon as typing changes");
        click(&mut state, "mobile-search-cancel");
        assert!(!state.mobile_ui.search_editing);
        assert!(state.mobile_tick(now + Duration::from_secs(5)).is_empty());
    }

    #[test]
    fn discovery_focuses_search_while_the_search_tab_waits_for_a_tap() {
        let mut state = phone();
        state.page = Page::Library;
        click(&mut state, "mobile-discover");
        assert!(state.page == Page::Search && state.mobile_search && state.mobile_ui.search_editing);
        assert_eq!(state.focus.as_deref(), Some("search"));
        assert!(!state.mobile_ui.blur);
        click(&mut state, "mobile-search");
        assert!(!state.mobile_ui.search_editing && state.mobile_ui.blur && state.focus.is_none());
    }

    #[test]
    fn submit_remembers_unique_queries_but_links_wait_for_explicit_submit() {
        let mut state = phone();
        let now = Instant::now();
        state.mobile_query("https://youtu.be/song", now);
        assert!(state.mobile_tick(now + Duration::from_secs(2)).is_empty());
        state.mobile_query("  Cerati  ", now);
        click(&mut state, "search-submit");
        assert_eq!(state.collection.searches, vec!["Cerati"]);
        assert!(state.mobile_ui.blur);
        state.mobile_query("cerati", now);
        click(&mut state, "search-submit");
        assert_eq!(state.collection.searches, vec!["cerati"]);
        click(&mut state, "mobile-search-clear");
        assert!(state.query.is_empty() && state.submitted.is_none());
        assert!(state.mobile_ui.search_editing);
    }

    #[test]
    fn open_actions_hold_the_track_even_when_search_results_change() {
        let mut state = phone();
        let track = Track { id: "first".into(), title: "Primera".into(), artist: "Artista".into(), artist_id: None, album: None, album_id: None, artwork_url: None, duration_ms: 1 };
        state.found.tracks.push(track.clone());
        click(&mut state, "more:track:0");
        state.found.tracks.clear();
        assert_eq!(click(&mut state, "do:queue:track:0"), vec![Command::Enqueue(track)]);
        assert!(state.mobile_ui.context.is_none());
    }

    #[test]
    fn queue_actions_keep_duplicate_occurrences_and_reject_a_changed_queue() {
        let mut state = phone();
        let track = Track { id: "same".into(), title: "Tema".into(), artist: "Artista".into(), artist_id: None, album: None, album_id: None, artwork_url: None, duration_ms: 1 };
        state.player.upcoming = vec![track.clone(), track];
        click(&mut state, "more:upcoming:1");
        let snapshot = state.player.upcoming.clone();
        assert_eq!(click(&mut state, "do:unqueue:upcoming:1"), vec![Command::UnqueueIfUnchanged { index: 1, snapshot }]);
        click(&mut state, "more:upcoming:1");
        state.player.upcoming.remove(0);
        assert!(click(&mut state, "do:unqueue:upcoming:1").is_empty());
        assert!(state.mobile_ui.notice.is_some());
    }

    #[test]
    fn old_collections_load_with_empty_search_history() {
        let old = r#"{"kept":[],"recent":[]}"#;
        let collection: Collection = serde_json::from_str(old).unwrap();
        assert!(collection.searches.is_empty());
    }

    #[test]
    fn swiping_a_form_cancels_instead_of_confirming_it() {
        let mut state = phone();
        state.creating = Some(Draft::default());
        let id = "playlist-cancel:drag".to_owned();
        state.update(&UiAction::DragStarted { id: id.clone(), position: limn::Point::new(40.0, 20.0) });
        state.update(&UiAction::Dropped { id, target: None, position: limn::Point::new(42.0, 100.0) });
        assert!(state.creating.is_none() && state.collection.playlists.is_empty());
        state.confirm = Some(Confirm::ClearDownloads);
        let id = "confirm-cancel:drag".to_owned();
        state.update(&UiAction::DragStarted { id: id.clone(), position: limn::Point::new(40.0, 20.0) });
        state.update(&UiAction::DragCancelled { id });
        assert!(state.confirm.is_some(), "cancelled gestures leave the sheet open");
    }
}
