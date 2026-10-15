//! Phone interactions share the desktop commands and collection.

use super::*;

const SEARCH_DELAY: Duration = Duration::from_millis(280);
const SEARCH_HISTORY: usize = 12;

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

#[derive(Default)]
pub(crate) struct MobileUi {
    pub search_editing: bool,
    pub search_filter: SearchFilter,
    pub pending: Option<(String, Instant)>,
    /// The host consumes this once after changing the view.
    pub blur: bool,
    pub context: Option<TrackContext>,
    pub connections: bool,
    pub route: Option<(String, String)>,
    pub native_routes: bool,
    pub context_drag: Option<limn::BottomSheetDrag>,
    pub connections_drag: Option<limn::BottomSheetDrag>,
    pub notice: Option<String>,
}

impl State {
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
        if self.mobile_ui.context_drag.as_mut().is_some_and(|drag| drag.handle_action(action)) {
            self.mobile_ui.context = None;
            self.mobile_ui.context_drag = None;
            return Some(Vec::new());
        }
        if self.mobile_ui.connections_drag.as_mut().is_some_and(|drag| drag.handle_action(action)) {
            self.mobile_ui.connections = false;
            self.mobile_ui.connections_drag = None;
            return Some(Vec::new());
        }
        match action {
            UiAction::InputChanged { id, value } if id == "search" => {
                self.mobile_query(value, Instant::now());
                return Some(Vec::new());
            }
            UiAction::Submitted { id, .. } if id == "search" => return Some(self.mobile_submit()),
            UiAction::Clicked { id } => {
                if let Some(row) = id.strip_prefix("more:") {
                    self.mobile_ui.connections = false;
                    self.mobile_ui.connections_drag = None;
                    self.mobile_ui.context = self.track_at(row).cloned().map(|track| TrackContext {
                        row: row.to_owned(), track,
                        playlist: match self.page { Page::Playlist(id) if row.starts_with("page-track:") => Some(id), _ => None },
                        upcoming: row.starts_with("upcoming:").then(|| self.player.upcoming.clone()),
                    });
                    self.mobile_ui.search_editing = false;
                    self.mobile_ui.blur = true;
                    self.mobile_ui.context_drag = Some(limn::BottomSheetDrag::new("mobile-context-close"));
                    return Some(Vec::new());
                }
                if let Some((action, row)) = id.strip_prefix("do:").and_then(|rest| rest.split_once(':')) {
                    let commands = self.act(action, row);
                    self.mobile_ui.context = None;
                    self.mobile_ui.context_drag = None;
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
                    "mobile-search" => {
                        self.mobile_search = true;
                        self.mobile_ui.search_editing = false;
                        self.mobile_ui.blur = true;
                        self.focus = None;
                        if self.page != Page::Search { self.go(Page::Search); }
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
                    "mobile-notice-close" => self.mobile_ui.notice = None,
                    "mobile-context-close" => { self.mobile_ui.context = None; self.mobile_ui.context_drag = None; }
                    "mobile-connections" | "setting:connections" => {
                        self.mobile_ui.context = None;
                        self.mobile_ui.context_drag = None;
                        self.mobile_ui.connections = true;
                        self.mobile_ui.connections_drag = Some(limn::BottomSheetDrag::new("mobile-connections-close"));
                        self.mobile_ui.blur = true;
                        self.mobile_ui.search_editing = false;
                    }
                    "mobile-connections-close" => { self.mobile_ui.connections = false; self.mobile_ui.connections_drag = None; }
                    // Navigation must cancel delayed requests from the search tab.
                    "home" | "mobile-library" | "mobile-playing" | "settings" | "back" => {
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
}
