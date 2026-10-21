//! The first run, in steps as Spotify's sign-up: bring the account from another device, or
//! start fresh with a name, the genres and the artists the home grows from. The artists
//! picked are followed, which is what the home recommends from.

use super::*;

/// How many artists the picker asks for before it is done.
pub const PICKS_FEWEST: usize = 3;
/// How many artists come in after one picked, as Spotify's "more like".
const MORE_LIKE: usize = 4;
/// Spotify's pause before what is typed is searched for.
const ARTIST_SEARCH_DELAY: Duration = Duration::from_millis(250);
/// What the picker offers when no genre was chosen: genres of the home's browse page.
const DEFAULT_TASTE: [&str; 6] = ["Rock argentino", "Pop latino", "Trap argentino", "Indie latino", "Cumbia", "Reggaetón"];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Step {
    #[default]
    Welcome,
    /// The account comes from another device.
    Bring,
    Name,
    Genres,
    Artists,
    Done,
}

/// Where the first run is, and what has been chosen in it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Onboarding {
    pub step: Step,
    /// Opened again from Settings: only the genres and the artists.
    pub taste_only: bool,
    pub name: String,
    /// What each genre chosen searches for.
    pub genres: Vec<String>,
    /// The artists offered, and those picked, in the order picked.
    pub pool: Vec<ArtistRef>,
    pub picked: Vec<ArtistRef>,
    pub query: String,
    /// What the query found, in the pool's place while there is one.
    pub found: Vec<ArtistRef>,
    pub loading: bool,
    search_due: Option<(String, Instant)>,
}

impl Onboarding {
    /// The artists the picker shows: what was searched for, or the pool.
    pub fn shown(&self) -> &[ArtistRef] {
        if self.query.trim().is_empty() { &self.pool } else { &self.found }
    }

    pub fn picks(&self, artist: &ArtistRef) -> bool {
        self.picked.iter().any(|picked| picked.id == artist.id)
    }
}

impl State {
    /// A first run: no name and the steps never gone through. What was heard or liked in a
    /// version from before the steps does not skip them.
    pub fn first_run(&self) -> bool {
        !self.settings.onboarded && self.profile.name.trim().is_empty()
    }

    /// Opens the steps: all of them, or only the genres and the artists.
    pub fn begin_onboarding(&mut self, taste_only: bool) -> Vec<Command> {
        let step = if taste_only { Step::Genres } else { Step::Welcome };
        self.onboarding = Some(Onboarding { step, taste_only, name: self.profile.name.clone(), ..Onboarding::default() });
        Vec::new()
    }

    /// The step's clicks, while the steps are open.
    pub(super) fn onboarding_click(&mut self, id: &str) -> Option<Vec<Command>> {
        let onboarding = self.onboarding.as_mut()?;
        let commands = match id {
            "onboarding-fresh" => {
                onboarding.step = Step::Name;
                self.focus = Some("onboarding-name".to_owned());
                Vec::new()
            }
            // The account comes from the other device, as the dialog that moves it reads it,
            // here a step of its own; what comes takes the place of what is here, as bringing
            // one's data is meant to.
            "onboarding-bring" => {
                onboarding.step = Step::Bring;
                self.handoff = Some(Handoff { receiving: true, ..Handoff::default() });
                self.focus = Some("handoff-address".to_owned());
                Vec::new()
            }
            "onboarding-back" => {
                if onboarding.step == Step::Bring { self.handoff = None; }
                onboarding.step = match (onboarding.step, onboarding.taste_only) {
                    (Step::Bring | Step::Name, _) => Step::Welcome,
                    (Step::Genres, false) => Step::Name,
                    (Step::Artists, _) => Step::Genres,
                    (step, _) => step,
                };
                Vec::new()
            }
            "onboarding-next" | "onboarding-skip" => match onboarding.step {
                Step::Welcome | Step::Bring => Vec::new(),
                Step::Name if onboarding.name.trim().is_empty() => Vec::new(),
                Step::Name => {
                    onboarding.step = Step::Genres;
                    Vec::new()
                }
                Step::Genres => {
                    onboarding.step = Step::Artists;
                    if id == "onboarding-skip" { onboarding.genres.clear(); }
                    let queries = if onboarding.genres.is_empty() { DEFAULT_TASTE.map(str::to_owned).to_vec() } else { onboarding.genres.clone() };
                    onboarding.pool.clear();
                    onboarding.loading = true;
                    vec![Command::FindArtists { key: "pool".into(), queries }]
                }
                Step::Artists if id == "onboarding-next" && onboarding.picked.len() < PICKS_FEWEST => Vec::new(),
                Step::Artists if onboarding.taste_only => return Some(self.finish_onboarding()),
                Step::Artists => {
                    onboarding.step = Step::Done;
                    Vec::new()
                }
                Step::Done => return Some(self.finish_onboarding()),
            },
            "onboarding-done" => return Some(self.finish_onboarding()),
            _ if id.starts_with("onboarding-genre:") => {
                let genre = id["onboarding-genre:".len()..].to_owned();
                match onboarding.genres.iter().position(|chosen| *chosen == genre) {
                    Some(at) => drop(onboarding.genres.remove(at)),
                    None => onboarding.genres.push(genre),
                }
                Vec::new()
            }
            _ if id.starts_with("onboarding-artist:") => {
                let artist_id = &id["onboarding-artist:".len()..];
                if let Some(at) = onboarding.picked.iter().position(|picked| picked.id == artist_id) {
                    onboarding.picked.remove(at);
                    return Some(Vec::new());
                }
                let Some(artist) = onboarding.shown().iter().find(|shown| shown.id == artist_id).cloned() else { return Some(Vec::new()) };
                // One picked from a search joins the pool, where what is like it comes in after it.
                if !onboarding.pool.iter().any(|offered| offered.id == artist.id) { onboarding.pool.insert(0, artist.clone()); }
                onboarding.picked.push(artist.clone());
                vec![Command::FindArtists { key: format!("like:{}", artist.id), queries: vec![artist.name] }]
            }
            _ => return None,
        };
        Some(commands)
    }

    /// What is typed in the picker's search: searched for once typing pauses.
    pub(super) fn onboarding_type(&mut self, value: &str, now: Instant) {
        let Some(onboarding) = self.onboarding.as_mut() else { return };
        onboarding.query = value.to_owned();
        let query = value.trim();
        if query.is_empty() { onboarding.found.clear(); }
        onboarding.search_due = (!query.is_empty()).then(|| (query.to_owned(), now + ARTIST_SEARCH_DELAY));
    }

    /// When the picker's search is due, if it is.
    pub fn onboarding_search_due(&self) -> Option<Instant> {
        self.onboarding.as_ref()?.search_due.as_ref().map(|(_, due)| *due)
    }

    /// Searches the picker's query once its pause is over.
    pub fn onboarding_tick(&mut self, now: Instant) -> Vec<Command> {
        let Some(onboarding) = self.onboarding.as_mut() else { return Vec::new() };
        if onboarding.search_due.as_ref().is_none_or(|(_, due)| *due > now) { return Vec::new(); }
        let (query, _) = onboarding.search_due.take().unwrap();
        onboarding.loading = true;
        vec![Command::FindArtists { key: format!("search:{query}"), queries: vec![query] }]
    }

    /// Artists the catalog found for the picker: its pool, what was searched for, or what is
    /// like one picked, which comes in after it.
    pub(super) fn onboarding_found(&mut self, key: &str, artists: Vec<ArtistRef>) -> Vec<Command> {
        let Some(onboarding) = self.onboarding.as_mut() else { return Vec::new() };
        let mut covers: Vec<Option<String>> = Vec::new();
        if key == "pool" {
            onboarding.loading = false;
            covers = artists.iter().map(|artist| artist.artwork_url.clone()).collect();
            onboarding.pool = artists;
        } else if let Some(query) = key.strip_prefix("search:") {
            if onboarding.query.trim() == query {
                onboarding.loading = false;
                covers = artists.iter().map(|artist| artist.artwork_url.clone()).collect();
                onboarding.found = artists;
            }
        } else if let Some(liked) = key.strip_prefix("like:") {
            let Some(at) = onboarding.pool.iter().position(|offered| offered.id == liked) else { return Vec::new() };
            let new: Vec<ArtistRef> = artists.into_iter()
                .filter(|artist| artist.id != liked && !onboarding.pool.iter().any(|offered| offered.id == artist.id))
                .take(MORE_LIKE)
                .collect();
            covers = new.iter().map(|artist| artist.artwork_url.clone()).collect();
            onboarding.pool.splice(at + 1..at + 1, new);
        }
        cover_requests(&mut self.covers, &covers)
    }

    /// Ends the steps: the name is the account's, the artists picked are followed, and the
    /// home asks for what goes with them.
    fn finish_onboarding(&mut self) -> Vec<Command> {
        let Some(onboarding) = self.onboarding.take() else { return Vec::new() };
        if !onboarding.taste_only {
            let name: String = onboarding.name.trim().chars().take(PROFILE_NAME_MOST).collect();
            if !name.is_empty() { self.profile.name = name; }
        }
        // Followed newest first, the first picked leads.
        for artist in onboarding.picked.into_iter().rev() {
            let entry = Entry::Artist(artist);
            if !self.collection.keeps(&entry) { self.collection.toggle(entry); }
        }
        let fresh = !onboarding.taste_only;
        self.end_first_run();
        // A fresh start is offered the user's music from their other apps, once.
        self.welcome = fresh;
        let mut commands = self.collection_covers();
        commands.extend(self.ask_picks());
        commands
    }

    /// The first run is over, by either way: neither it nor the old greeting comes again.
    pub(super) fn end_first_run(&mut self) {
        self.onboarding = None;
        self.settings.onboarded = true;
        self.settings.welcomed = true;
        self.welcome = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn artist(id: &str) -> ArtistRef {
        ArtistRef { id: id.into(), name: id.to_uppercase(), subscribers: None, artwork_url: None }
    }

    fn click(state: &mut State, id: &str) -> Vec<Command> {
        state.update(&UiAction::Clicked { id: id.into() })
    }

    #[test]
    fn a_fresh_start_names_the_account_and_follows_the_artists_picked() {
        let mut state = State::default();
        assert!(state.first_run());
        state.begin_onboarding(false);
        click(&mut state, "onboarding-fresh");
        assert!(click(&mut state, "onboarding-next").is_empty() && state.onboarding.as_ref().unwrap().step == Step::Name, "a name first");
        state.update(&UiAction::InputChanged { id: "onboarding-name".into(), value: "  Nihuel ".into() });
        click(&mut state, "onboarding-next");
        click(&mut state, "onboarding-genre:Cumbia");
        assert_eq!(click(&mut state, "onboarding-next"), vec![Command::FindArtists { key: "pool".into(), queries: vec!["Cumbia".into()] }]);
        state.apply(Event::ArtistsFound { key: "pool".into(), artists: vec![artist("a"), artist("b"), artist("c"), artist("d")] });
        assert_eq!(click(&mut state, "onboarding-artist:b"), vec![Command::FindArtists { key: "like:b".into(), queries: vec!["B".into()] }]);
        state.apply(Event::ArtistsFound { key: "like:b".into(), artists: vec![artist("b"), artist("x"), artist("a")] });
        let pool: Vec<&str> = state.onboarding.as_ref().unwrap().pool.iter().map(|artist| artist.id.as_str()).collect();
        assert_eq!(pool, ["a", "b", "x", "c", "d"], "what is like it comes in after it, once");
        click(&mut state, "onboarding-artist:a");
        assert!(click(&mut state, "onboarding-next").is_empty() && state.onboarding.as_ref().unwrap().step == Step::Artists, "three before it is done");
        click(&mut state, "onboarding-artist:x");
        click(&mut state, "onboarding-next");
        assert_eq!(state.onboarding.as_ref().unwrap().step, Step::Done);
        click(&mut state, "onboarding-done");
        assert!(state.onboarding.is_none() && state.settings.onboarded && !state.first_run());
        assert_eq!(state.profile.name, "Nihuel");
        let followed: Vec<&str> = state.collection.artists().map(|(_, artist)| artist.id.as_str()).collect();
        assert_eq!(followed, ["b", "a", "x"], "followed in the order picked");
    }

    #[test]
    fn a_fresh_start_is_offered_its_music_from_other_apps_once_and_the_link_says_which() {
        let mut state = State::default();
        state.begin_onboarding(true);
        click(&mut state, "onboarding-skip");
        click(&mut state, "onboarding-skip");
        assert!(!state.welcome, "choosing artists again offers nothing");
        state.begin_onboarding(false);
        state.onboarding.as_mut().unwrap().step = Step::Done;
        click(&mut state, "onboarding-done");
        assert!(state.welcome, "a fresh start ends on the offer to bring music in");
        click(&mut state, "welcome-import");
        assert!(state.importing.is_some() && state.import_source == ImportSource::Spotify);
        click(&mut state, "import-source:youtube");
        assert_eq!(state.import_source, ImportSource::YouTube);
        state.update(&UiAction::InputChanged { id: "import-link".into(), value: "https://music.apple.com/ar/playlist/viaje/pl.u-abc".into() });
        assert_eq!(state.import_source, ImportSource::Apple, "a link pasted says which app it is of");
    }

    #[test]
    fn music_from_a_version_before_the_steps_does_not_skip_them() {
        let mut state = State::default();
        state.collection.heard = vec![Track { id: "a".into(), title: "A".into(), artist: "Ana".into(), artist_id: None, album: None, album_id: None, artwork_url: None, duration_ms: 1 }];
        assert!(state.first_run(), "heard before, but never named nor through the steps");
        state.begin_onboarding(false);
        state.profile.name = "Nihuel".into();
        assert!(!state.first_run());
    }

    #[test]
    fn an_account_brought_in_is_the_end_of_the_first_run() {
        let mut state = State::default();
        state.begin_onboarding(false);
        click(&mut state, "onboarding-bring");
        assert_eq!(state.onboarding.as_ref().unwrap().step, Step::Bring);
        assert!(state.handoff.as_ref().is_some_and(|handoff| handoff.receiving));
        assert_eq!(click(&mut state, "handoff-scan"), vec![Command::ScanCode], "a phone reads the code with its camera");
        click(&mut state, "onboarding-back");
        assert!(state.handoff.is_none() && state.onboarding.as_ref().unwrap().step == Step::Welcome, "back where it started");

        click(&mut state, "onboarding-bring");
        state.update(&UiAction::InputChanged { id: "handoff-address".into(), value: "10.0.0.2:4000/abc1".into() });
        click(&mut state, "handoff-fetch");
        let profile = Profile { name: "Nihuel".into(), ..Profile::default() };
        state.apply(Event::Fetched(Ok(crate::handoff::Bundle { version: 1, profile: Some(profile), liked: vec![Track { id: "a".into(), title: "A".into(), artist: "Ana".into(), artist_id: None, album: None, album_id: None, artwork_url: None, duration_ms: 1 }], ..Default::default() })));
        click(&mut state, "handoff-take");
        assert!(state.onboarding.is_none() && state.settings.onboarded, "no steps after it");
        assert_eq!(state.profile.name, "Nihuel");
        assert_eq!(state.collection.liked.len(), 1);
    }

    #[test]
    fn skipping_the_genres_offers_the_usual_ones_and_skipping_the_artists_ends() {
        let mut state = State::default();
        state.begin_onboarding(true);
        let asked = click(&mut state, "onboarding-skip");
        assert!(matches!(asked.as_slice(), [Command::FindArtists { queries, .. }] if queries.len() == DEFAULT_TASTE.len()));
        click(&mut state, "onboarding-skip");
        assert!(state.onboarding.is_none() && state.settings.onboarded, "opened again from Settings, it ends there");
    }

    #[test]
    fn the_pickers_search_waits_for_typing_to_pause_and_keeps_what_was_picked() {
        let mut state = State::default();
        state.begin_onboarding(true);
        click(&mut state, "onboarding-next");
        let now = Instant::now();
        state.onboarding_type("cera", now);
        assert!(state.onboarding_tick(now).is_empty());
        assert_eq!(state.onboarding_tick(now + ARTIST_SEARCH_DELAY), vec![Command::FindArtists { key: "search:cera".into(), queries: vec!["cera".into()] }]);
        state.apply(Event::ArtistsFound { key: "search:cera".into(), artists: vec![artist("cerati")] });
        click(&mut state, "onboarding-artist:cerati");
        state.onboarding_type("", now);
        let onboarding = state.onboarding.as_ref().unwrap();
        assert!(onboarding.picks(&artist("cerati")) && onboarding.shown()[0].id == "cerati", "a pick from a search joins the pool");
    }
}
