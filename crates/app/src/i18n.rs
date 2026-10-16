//! The words of the interface in the user's language. Each language is a Fluent catalog
//! of several files under `locales/`, one per part of the app, read into the binary.
//! A text is asked for by its id with `t!`, and its variables by name:
//! `t!("common-songs", count = 3)`.

use std::borrow::Cow;
use std::sync::LazyLock;

use fluent_bundle::{FluentArgs, FluentResource};
use serde::{Deserialize, Serialize};

type Bundle = fluent_bundle::concurrent::FluentBundle<FluentResource>;

/// The parts of the app with a catalog file of their own, the same in every language.
macro_rules! catalog {
    ($language:literal: $($part:literal),+) => {
        [$(($part, include_str!(concat!("../locales/", $language, "/", $part, ".ftl")))),+]
    };
}
const SPANISH: [(&str, &str); 6] = catalog!("es": "common", "desktop", "mobile", "state", "backend", "native");
const ENGLISH: [(&str, &str); 6] = catalog!("en": "common", "desktop", "mobile", "state", "backend", "native");

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Language {
    #[default]
    Spanish,
    English,
}

impl Language {
    pub const ALL: [Language; 2] = [Language::Spanish, Language::English];

    /// The one the system speaks: Spanish for any Spanish, English for anything else.
    pub fn system() -> Self {
        match sys_locale::get_locale() {
            Some(locale) if locale.to_ascii_lowercase().starts_with("es") => Language::Spanish,
            _ => Language::English,
        }
    }

    /// Its BCP 47 tag, which also names its catalog's folder.
    pub fn tag(self) -> &'static str {
        match self {
            Language::Spanish => "es",
            Language::English => "en",
        }
    }

    /// Its own name, as it is offered in the other language too.
    pub fn name(self) -> &'static str {
        match self {
            Language::Spanish => "Español",
            Language::English => "English",
        }
    }

    pub fn from_tag(tag: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|language| language.tag() == tag)
    }

    fn sources(self) -> &'static [(&'static str, &'static str)] {
        match self {
            Language::Spanish => &SPANISH,
            Language::English => &ENGLISH,
        }
    }
}

static BUNDLES: LazyLock<[Bundle; 2]> = LazyLock::new(|| Language::ALL.map(bundle));

fn bundle(language: Language) -> Bundle {
    let mut bundle = Bundle::new_concurrent(vec![language.tag().parse().expect("a valid language tag")]);
    // Fluent wraps each variable in invisible direction marks, which limn would draw.
    bundle.set_use_isolating(false);
    for (part, source) in language.sources() {
        let resource = FluentResource::try_new((*source).to_owned())
            .unwrap_or_else(|(_, errors)| panic!("locales/{}/{part}.ftl does not parse: {errors:?}", language.tag()));
        bundle.add_resource(resource)
            .unwrap_or_else(|errors| panic!("locales/{}/{part}.ftl repeats an id: {errors:?}", language.tag()));
    }
    bundle
}

// The app's threads share one language; each test keeps its own, so one that switches it
// does not change what the others read.
#[cfg(not(test))]
static CURRENT: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);
#[cfg(test)]
thread_local! { static CURRENT: std::cell::Cell<u8> = const { std::cell::Cell::new(0) }; }

pub fn set(language: Language) {
    #[cfg(not(test))]
    CURRENT.store(language as u8, std::sync::atomic::Ordering::Relaxed);
    #[cfg(test)]
    CURRENT.with(|current| current.set(language as u8));
}

pub fn current() -> Language {
    #[cfg(not(test))]
    let index = CURRENT.load(std::sync::atomic::Ordering::Relaxed);
    #[cfg(test)]
    let index = CURRENT.with(std::cell::Cell::get);
    Language::ALL[index as usize % Language::ALL.len()]
}

/// The text `id` names in `language`. A plain one is borrowed from its catalog as it
/// is; one with variables is written out. A missing id shows as itself, which the tests
/// below keep from reaching anyone.
pub fn text(language: Language, id: &str, args: Option<&FluentArgs>) -> Cow<'static, str> {
    let bundle = &BUNDLES[language as usize];
    let Some(pattern) = bundle.get_message(id).and_then(|message| message.value()) else {
        return Cow::Owned(id.to_owned());
    };
    let mut errors = Vec::new();
    bundle.format_pattern(pattern, args, &mut errors)
}

/// A text as the `&'static str` some tables keep. A plain one is the catalog's own; one
/// written out is kept once, so asking for it again does not grow the program.
pub fn fixed(text: Cow<'static, str>) -> &'static str {
    match text {
        Cow::Borrowed(text) => text,
        Cow::Owned(text) => {
            static KEPT: std::sync::Mutex<std::collections::BTreeSet<&'static str>> = std::sync::Mutex::new(std::collections::BTreeSet::new());
            let mut kept = KEPT.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            match kept.get(text.as_str()) {
                Some(kept) => kept,
                None => {
                    let text = text.leak();
                    kept.insert(text);
                    text
                }
            }
        }
    }
}

/// The text an id names in the current language, with its variables by name.
macro_rules! t {
    ($id:literal) => {
        $crate::i18n::text($crate::i18n::current(), $id, None)
    };
    ($id:literal, $($name:ident = $value:expr),+ $(,)?) => {{
        let mut args = fluent_bundle::FluentArgs::new();
        $(args.set(stringify!($name), $value);)+
        $crate::i18n::text($crate::i18n::current(), $id, Some(&args))
    }};
}
pub(crate) use t;

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    fn ids(language: Language) -> BTreeSet<String> {
        // A message starts its line with its id; comments, terms and continuations do not.
        language.sources().iter().flat_map(|(_, source)| source.lines().filter_map(|line| {
            let (id, _) = line.split_once('=')?;
            let id = id.trim_end();
            id.chars().next().is_some_and(|first| first.is_ascii_alphabetic()).then(|| id.to_owned())
        })).collect()
    }

    #[test]
    fn every_language_has_every_text_and_parses() {
        let spanish = ids(Language::Spanish);
        for language in Language::ALL {
            let _ = &BUNDLES[language as usize];
            let found = ids(language);
            assert_eq!(found.difference(&spanish).collect::<Vec<_>>(), Vec::<&String>::new(), "{} has ids Spanish lacks", language.tag());
            assert_eq!(spanish.difference(&found).collect::<Vec<_>>(), Vec::<&String>::new(), "{} lacks these ids", language.tag());
        }
    }

    #[test]
    fn every_id_the_code_asks_for_is_in_the_catalog() {
        let known = ids(Language::Spanish);
        let mut missing = Vec::new();
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut folders = vec![root];
        while let Some(folder) = folders.pop() {
            for entry in std::fs::read_dir(folder).unwrap().flatten() {
                let path = entry.path();
                if path.is_dir() { folders.push(path); continue; }
                if path.extension().is_none_or(|extension| extension != "rs") { continue; }
                let source = std::fs::read_to_string(&path).unwrap();
                for (index, _) in source.match_indices("t!(\"") {
                    // `format!(` ends in the same letters.
                    if source[..index].chars().next_back().is_some_and(|before| before.is_alphanumeric() || before == '_') { continue; }
                    let id: String = source[index + 4..].chars().take_while(|character| *character != '"').collect();
                    if !known.contains(&id) { missing.push(format!("{}: {id}", path.display())); }
                }
            }
        }
        assert!(missing.is_empty(), "ids missing from the catalog: {missing:#?}");
    }

    #[test]
    fn plain_texts_are_borrowed_and_variables_are_filled_without_marks() {
        assert!(matches!(text(Language::Spanish, "common-cancel", None), Cow::Borrowed("Cancelar")));
        assert_eq!(text(Language::English, "common-cancel", None), "Cancel");
        let count = |language, count: usize| {
            let mut args = FluentArgs::new();
            args.set("count", count);
            text(language, "common-songs", Some(&args)).into_owned()
        };
        assert_eq!((count(Language::Spanish, 1), count(Language::Spanish, 3)), ("1 canción".into(), "3 canciones".into()));
        assert_eq!((count(Language::English, 1), count(Language::English, 3)), ("1 song".into(), "3 songs".into()));
        assert_eq!(text(Language::English, "no-such-text", None), "no-such-text");
        let written = || fixed(Cow::Owned("1 song".into()));
        assert!(std::ptr::eq(written(), written()), "a written-out text is kept once");
    }

    #[test]
    fn the_language_is_chosen_per_thread_in_tests_and_read_back() {
        assert_eq!(current(), Language::Spanish, "Spanish until something else is chosen");
        set(Language::English);
        assert_eq!(t!("common-cancel"), "Cancel");
        set(Language::Spanish);
        assert_eq!(Language::from_tag("en"), Some(Language::English));
    }
}
