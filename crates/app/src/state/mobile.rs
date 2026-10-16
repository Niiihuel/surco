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

#[derive(Clone)]
pub(crate) struct TrackContext {
    pub row: String,
    pub track: Track,
    pub playlist: Option<u64>,
    pub upcoming: Option<Vec<Track>>,
}

#[derive(Clone)]
pub(crate) struct LibraryContext {
    pub key: String,
    pub title: String,
    pub artwork_url: Option<String>,
    pub entry: Option<Entry>,
}

#[derive(Clone)]
pub(crate) enum CardContext {
    Track(TrackContext),
    Library(LibraryContext),
}

fn library_target(key: &str) -> Option<Target> {
    key.strip_prefix("album:").map(|id| Target::Album(id.into()))
        .or_else(|| key.strip_prefix("artist:").map(|id| Target::Artist(id.into())))
}

/// Protected edges inside the full phone canvas, in logical points.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct ContentInsets {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
}

#[derive(Default)]
pub(crate) struct MobileUi {
    /// Host-owned safe edges; bottom includes docked keyboard occlusion once.
    /// Backgrounds still cover the full window, while controls use these insets.
    pub content_insets: ContentInsets,
    /// Host-owned translation of the full player surface in logical points.
    pub player_offset: f32,
    /// An explicit dismissal requests animation before the Host navigates back.
    pub player_close: bool,
    /// Animated Search header height; None outside the Search tab.
    pub search_header: Option<f32>,
    /// The shared chrome condenses only after the current title scrolls above it.
    pub compact_header: bool,
    pub search_editing: bool,
    /// Explicit recent/browse/clear actions supersede any live native composition.
    pub search_intent: u64,
    pub search_filter: SearchFilter,
    pub pending: Option<(String, Instant)>,
    /// The host consumes this once after changing the view.
    pub blur: bool,
    pub context: Option<TrackContext>,
    pub library: Option<LibraryContext>,
    pub connections: bool,
    pub route: Option<(String, String)>,
    pub native_routes: bool,
    /// UIKit context menus, sheets and paste controls are attached to the live canvas.
    pub native_surfaces: bool,
    pub keyboard_visible: bool,
    pub sheet_drag: Option<(String, limn::BottomSheetDrag)>,
    pub notice: Option<String>,
    /// The route uncovered by closing the player. It is already on screen
    /// beneath the player, so it must not fade in again.
    pub revealed: Option<String>,
}

impl State {
    /// Results keep the compact search chrome after Return/Cancel; this is
    /// presentation only and must never request keyboard focus.
    pub(crate) fn mobile_search_compact(&self) -> bool {
        self.mobile_ui.search_editing || !self.query.trim().is_empty()
    }

    pub(crate) fn mobile_library_context(&self, key: &str) -> Option<LibraryContext> {
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
        let entry = library_target(key).and_then(|target| self.collection.kept.iter().find(|entry| entry.target() == target)).cloned();
        Some(LibraryContext { key: key.into(), title: title.into(), artwork_url, entry })
    }

    pub(crate) fn card_entry_kept(&self, entry: &Entry) -> bool {
        self.collection.kept.iter().any(|held| held.is(entry))
    }

    /// Capture an identity while its row is visible, before asynchronous catalog updates.
    pub(crate) fn card_context(&self, row: &str) -> Option<CardContext> {
        if let Some(track) = self.track_at(row) {
            return Some(CardContext::Track(TrackContext { row: row.into(), track: track.clone(),
                playlist: match self.page { Page::Playlist(id) if row.starts_with("page-track:") => Some(id), _ => None },
                upcoming: row.starts_with("upcoming:").then(|| self.player.upcoming.clone()),
            }));
        }
        let row = row.strip_prefix("home-").unwrap_or(row);
        if row == "downloads" || row == "liked" {
            return self.mobile_library_context(row).map(CardContext::Library);
        }
        let (kind, index) = row.split_once(':')?;
        let index = index.parse::<usize>().ok()?;
        if kind == "playlist" {
            return self.mobile_library_context(&format!("playlist:{}", self.collection.playlists.get(index)?.id)).map(CardContext::Library);
        }
        let entry = match kind {
            "recent" => self.collection.recent.get(index).cloned(),
            "kept" => self.collection.kept.get(index).cloned(),
            "pick-album" => self.picked_albums.get(index).cloned().map(Entry::Album),
            "pick-artist" => self.picked_artists.get(index).cloned().map(Entry::Artist),
            "album" => self.found.albums.get(index).cloned().map(Entry::Album),
            "artist" => self.found.artists.get(index).cloned().map(Entry::Artist),
            "page-album" => match &self.page { Page::Artist(artist) => artist.albums.get(index).cloned().map(Entry::Album), _ => None },
            "page-single" => match &self.page { Page::Artist(artist) => artist.singles.get(index).cloned().map(Entry::Album), _ => None },
            _ => None,
        }?;
        let key = match &entry { Entry::Album(album) => format!("album:{}", album.id), Entry::Artist(artist) => format!("artist:{}", artist.id) };
        Some(CardContext::Library(LibraryContext { key, title: entry.title().into(), artwork_url: entry.artwork().clone(), entry: Some(entry) }))
    }

    pub(crate) fn card_context_action(&self, row: &str) -> Option<String> {
        self.card_context(row).map(|context| match context {
            CardContext::Track(_) => format!("more:{row}"),
            CardContext::Library(_) => format!("entry-more:{row}"),
        })
    }

    #[cfg(any(target_os = "ios", test))]
    pub(crate) fn apply_card_context(&mut self, context: CardContext, action: &str) -> Vec<Command> {
        match context {
            CardContext::Track(context) => { self.mobile_ui.context = Some(context); }
            CardContext::Library(context) => { self.mobile_ui.library = Some(context); }
        }
        self.update(&UiAction::Clicked { id: action.into() })
    }

    pub(crate) fn mobile_query(&mut self, value: &str, now: Instant) {
        self.query = value.to_owned();
        self.mobile_ui.search_editing = true;
        self.submitted = None;
        self.searching = false;
        self.search_error = None;
        let query = value.trim();
        if query.is_empty() { self.found = Found::default(); }
        // Importing or opening a pasted link must be an explicit action.
        let link = query.contains("://") || transfer::source(query).is_some();
        self.mobile_ui.pending = (query.chars().count() >= 2 && !link)
            .then(|| (query.to_owned(), now + SEARCH_DELAY));
    }

    pub(crate) fn mobile_tick(&mut self, now: Instant) -> Vec<Command> {
        if !self.mobile || self.mobile_ui.pending.as_ref().is_none_or(|(_, due)| *due > now) {
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
                if let Some(row) = id.strip_prefix("entry-more:") {
                    self.mobile_ui.library = self.card_context(row).and_then(|context| match context { CardContext::Library(context) => Some(context), _ => None });
                    self.mobile_ui.context = None;
                    self.mobile_ui.connections = false;
                    self.mobile_ui.search_editing = false;
                    self.mobile_ui.blur = true;
                    return Some(Vec::new());
                }
                if let Some(target) = id.strip_prefix("ios-paste:") {
                    let valid = match target {
                        "search" if self.page == Page::Search => {
                            self.mobile_search = true;
                            self.mobile_ui.search_editing = true;
                            true
                        }
                        "import-link" => self.importing.is_some(),
                        _ => false,
                    };
                    if valid {
                        self.focus = Some(target.into());
                        self.paste = Some(target.into());
                        self.mobile_ui.blur = false;
                    }
                    return Some(Vec::new());
                }
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
                        "keep" => {
                            if let Some(entry) = context.entry.filter(|entry| !self.card_entry_kept(entry)) { self.collection.toggle(entry); }
                            Vec::new()
                        }
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
                    self.mobile_ui.context = self.card_context(row).and_then(|context| match context { CardContext::Track(context) => Some(context), _ => None });
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
                if let Some(index) = id.strip_prefix("recent-search-remove:").and_then(|value| value.parse::<usize>().ok()) {
                    if index < self.collection.searches.len() { self.collection.searches.remove(index); }
                    return Some(Vec::new());
                }
                if let Some(index) = id.strip_prefix("recent-search:").and_then(|value| value.parse::<usize>().ok()) {
                    if let Some(query) = self.collection.searches.get(index).cloned() {
                        self.mobile_ui.search_intent = self.mobile_ui.search_intent.wrapping_add(1);
                        self.query = query;
                        return Some(self.mobile_submit());
                    }
                    return Some(Vec::new());
                }
                if let Some(query) = id.strip_prefix("browse:") {
                    self.mobile_ui.search_intent = self.mobile_ui.search_intent.wrapping_add(1);
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
                    "mobile-player-dismiss" => {
                        self.mobile_ui.player_close = self.page == Page::Playing && !self.queue_open;
                    }
                    "mobile-search" | "mobile-discover" => {
                        self.mobile_search = true;
                        if self.page != Page::Search { self.go(Page::Search); }
                        let editing = id == "mobile-discover";
                        self.mobile_ui.search_editing = editing;
                        self.mobile_ui.blur = !editing;
                        self.focus = (editing && !self.mobile_ui.native_surfaces).then(|| "search".into());
                    }
                    "mobile-search-edit" => {
                        self.mobile_ui.search_editing = true;
                        self.mobile_ui.blur = false;
                        self.focus = (!self.mobile_ui.native_surfaces).then(|| "search".into());
                    }
                    "mobile-search-cancel" => {
                        self.mobile_ui.search_editing = false;
                        self.mobile_ui.pending = None;
                        self.mobile_ui.blur = true;
                        self.focus = None;
                    }
                    "mobile-search-clear" => {
                        self.mobile_ui.search_intent = self.mobile_ui.search_intent.wrapping_add(1);
                        self.mobile_query("", Instant::now());
                        self.focus = (!self.mobile_ui.native_surfaces).then(|| "search".into());
                        self.mobile_ui.blur = false;
                    }
                    "search-submit" => return Some(self.mobile_submit()),
                    "recent-search-clear" => self.collection.searches.clear(),
                    "mobile-context-close" => self.mobile_ui.context = None,
                    "mobile-library-close" => self.mobile_ui.library = None,
                    "mobile-connections-close" => self.mobile_ui.connections = false,
                    "mobile-notice-close" => {
                        self.mobile_ui.notice = None;
                        if self.page == Page::Lyrics { self.lyric_shown = None; }
                    }
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
    fn submitted_and_canceled_queries_keep_compact_chrome_without_keyboard_intent() {
        let mut state = phone();
        state.mobile_ui.native_surfaces = true;
        state.mobile_search = true;
        state.mobile_query("Consulta", Instant::now());
        click(&mut state, "search-submit");
        assert!(state.mobile_search_compact());
        assert!(!state.mobile_ui.search_editing && state.mobile_ui.blur && state.focus.is_none());
        click(&mut state, "mobile-search-cancel");
        assert_eq!(state.query, "Consulta");
        assert!(state.mobile_search_compact());
        assert!(!state.mobile_ui.search_editing && state.focus.is_none());
        state.mobile_query("  ", Instant::now());
        assert!(state.mobile_search_compact(), "the active editor stays compact even when cleared");
        click(&mut state, "mobile-search-cancel");
        assert!(!state.mobile_search_compact(), "an empty unfocused search returns to its idle heading");
    }

    #[test]
    fn editing_a_new_query_retains_selectable_results_until_the_new_answer_arrives() {
        let mut state = phone();
        state.mobile_search = true;
        let track = Track { id: "held".into(), title: "Resultado actual".into(), artist: "Artista".into(),
            artist_id: None, album: None, album_id: None, artwork_url: None, duration_ms: 1 };
        state.found.tracks = vec![track.clone()];
        state.submitted = Some("anterior".into());
        let now = Instant::now();
        state.mobile_query("nueva", now);
        assert_eq!(state.found.tracks, vec![track.clone()]);
        assert!(state.submitted.is_none(), "a reply for the former query is already stale during debounce");
        state.apply(Event::SearchResults { query: "anterior".into(), found: Found::default() });
        assert_eq!(state.found.tracks, vec![track.clone()]);
        assert_eq!(state.mobile_tick(now + Duration::from_millis(300)), vec![Command::Search("nueva".into())]);
        assert_eq!(state.found.tracks, vec![track.clone()]);
        state.apply(Event::SearchFailed { query: "nueva".into(), message: "Sin conexión".into() });
        assert_eq!(state.found.tracks, vec![track]);
        assert!(state.search_error.is_some());
        state.mobile_query("", now);
        assert!(state.found.tracks.is_empty() && state.mobile_ui.pending.is_none());
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
        state.collection.searches = vec!["Spinetta".into(), "cerati".into()];
        click(&mut state, "recent-search-remove:0");
        assert_eq!(state.collection.searches, vec!["cerati"], "the x forgets only its own search");
        assert!(state.query.is_empty(), "forgetting a search does not run it");
        click(&mut state, "recent-search-remove:5");
        assert_eq!(state.collection.searches, vec!["cerati"]);
    }

    #[test]
    fn clipboard_actions_keep_the_payload_and_explicit_paste_target() {
        let mut state = phone();
        let text = "Surco · ñ 東京\0después";
        assert!(state.update(&UiAction::CopyRequested { text: text.into() }).is_empty());
        assert_eq!(state.copy.as_deref(), Some(text));
        assert!(click(&mut state, "ios-paste:search").is_empty());
        assert_eq!(state.paste.as_deref(), Some("search"));
        assert!(state.mobile_search && state.mobile_ui.search_editing);
        assert_eq!(state.focus.as_deref(), Some("search"));
        state.paste = None;
        assert!(click(&mut state, "ios-paste:import-link").is_empty());
        assert!(state.paste.is_none(), "a closed form cannot request clipboard access");
        state.importing = Some(String::new());
        click(&mut state, "ios-paste:import-link");
        assert_eq!(state.paste.as_deref(), Some("import-link"));
        assert!(state.update(&UiAction::PasteRequested { id: "creating-name".into() }).is_empty());
        assert_eq!(state.paste.as_deref(), Some("creating-name"));
        assert!(state.importing.as_ref().is_some_and(String::is_empty));
    }

    #[test]
    fn clipboard_paste_uses_query_debounce_but_links_and_imports_wait_for_submit() {
        let mut state = phone();
        click(&mut state, "ios-paste:search");
        assert!(state.update(&UiAction::InputChanged { id: "search".into(), value: "Cerati".into() }).is_empty());
        assert_eq!(state.mobile_tick(Instant::now() + Duration::from_secs(2)), vec![Command::Search("Cerati".into())]);
        assert!(state.update(&UiAction::InputChanged { id: "search".into(), value: "https://youtu.be/song".into() }).is_empty());
        assert!(state.mobile_ui.pending.is_none());
        assert!(state.mobile_tick(Instant::now() + Duration::from_secs(2)).is_empty());
        assert!(state.open_url.is_none());
        state.importing = Some(String::new());
        let link = "https://open.spotify.com/playlist/example";
        assert!(state.update(&UiAction::InputChanged { id: "import-link".into(), value: link.into() }).is_empty());
        assert_eq!(state.importing.as_deref(), Some(link));
        assert!(state.import_progress.is_none());
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
