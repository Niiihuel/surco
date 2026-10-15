//! Controlled UIKit surfaces. Native callbacks carry opaque snapshot IDs, never live row indices.

use std::collections::{HashMap, HashSet};
use eframe::egui::Pos2;
use limn::{Frame, Point, UiAction};
use serde::{Deserialize, Serialize};
use crate::{backend::Command, state::{self, mobile::{CardContext, LibraryContext}, Confirm, State}};

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub(crate) struct Payload {
    pub menus: Vec<Menu>,
    pub sheet: Option<Sheet>,
    pub paste: Option<PasteSlot>,
    pub search: Option<SearchSlot>,
    pub reduce_motion: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub(crate) struct Menu { pub id: String, pub title: String, pub rect: [f32; 4], pub items: Vec<Action> }

#[derive(Clone, Debug, PartialEq, Serialize)]
pub(crate) struct Action {
    pub id: String, pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subtitle: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symbol: Option<String>,
    pub destructive: bool, pub selected: bool,
}

impl Action {
    fn new(id: impl Into<String>, title: impl Into<String>, symbol: &str) -> Self {
        Self { id: id.into(), title: title.into(), subtitle: None, symbol: Some(symbol.into()), destructive: false, selected: false }
    }
    fn destructive(mut self) -> Self { self.destructive = true; self }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub(crate) struct Sheet {
    pub id: String, pub title: String, pub description: String, pub dismiss: String,
    pub rows: Vec<Row>, pub actions: Vec<Action>, pub large: bool, pub confirmation: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum Row {
    Text { title: String, secondary: bool },
    Action(Action),
    Field { id: String, value: String, placeholder: String, keyboard: String, paste: bool },
    Slider { id: String, title: String, value: f32, min: f32, max: f32 },
    Segments { id: String, items: Vec<Action>, selected: usize },
    Route { title: String, subtitle: String },
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub(crate) struct PasteSlot { pub target: String, pub rect: [f32; 4] }

#[derive(Clone, Debug, PartialEq, Serialize)]
pub(crate) struct SearchSlot {
    pub session: String, pub rect: [f32; 4], pub text: String,
    pub editing: bool, pub compact: bool, pub revision: u64,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum Event {
    MenuOpen { id: String },
    MenuAction { id: String, action: String },
    MenuClose { id: String },
    SheetAction { sheet: String, action: String },
    SheetInput { sheet: String, id: String, text: String },
    SheetSlider { sheet: String, id: String, value: f32 },
    SheetClose { sheet: String },
    Paste { target: String, text: String },
    SearchEdit { session: String, revision: u64, text: String },
    SearchFocus { session: String, revision: u64, editing: bool },
    SearchSubmit { session: String },
    SearchCancel { session: String },
}

#[derive(Clone)]
struct MenuBinding { target: CardContext, actions: HashSet<String> }

struct SheetBinding { key: String, sheet: Sheet }

struct SearchBinding { session: String, revision: u64, intent: u64 }

#[derive(Default)]
pub(crate) struct Controller {
    menus: HashMap<String, MenuBinding>,
    active_menu: Option<(String, MenuBinding)>,
    sheet: Option<SheetBinding>,
    paste: Option<String>,
    next_sheet: u64,
    search: Option<SearchBinding>,
    next_search: u64,
}

impl Controller {
    pub fn payload(&mut self, state: &State, frame: &Frame, origin: Pos2) -> Payload {
        let mut payload = Payload { reduce_motion: state.settings.reduce_motion || !state.motion, ..Payload::default() };
        self.menus.clear();
        self.paste = None;
        if !state.mobile { self.sheet = None; self.search = None; self.active_menu = None; return payload; }
        if let Some((key, mut sheet)) = sheet_spec(state) {
            self.search = None;
            sheet.id = match self.sheet.as_ref().filter(|held| held.key == key) {
                Some(held) => held.sheet.id.clone(),
                None => { self.next_sheet += 1; format!("surco-sheet:{}", self.next_sheet) }
            };
            self.sheet = Some(SheetBinding { key, sheet: sheet.clone() });
            self.active_menu = None;
            payload.sheet = Some(sheet);
            return payload;
        }
        self.sheet = None;
        let search_rect = search_visible(state).then(|| frame.access.iter().find(|node|
            node.id == "ios-search" && !node.disabled)).flatten()
            .and_then(|node| visible_rect(frame, 0, node.rect, true))
            .and_then(|rect| screen_rect(rect, origin))
            .filter(|rect| rect[2] >= 44.0 && rect[3] >= 44.0);
        if let Some(rect) = search_rect {
            if self.search.as_ref().is_none_or(|binding| binding.intent != state.mobile_ui.search_intent) {
                self.next_search += 1;
                self.search = Some(SearchBinding { session: format!("surco-search:{}", self.next_search), revision: 0,
                    intent: state.mobile_ui.search_intent });
            }
            let binding = self.search.as_ref().unwrap();
            payload.search = Some(SearchSlot { session: binding.session.clone(), rect,
                text: state.query.clone(), editing: state.mobile_ui.search_editing,
                compact: state.mobile_search_compact(), revision: binding.revision });
        } else { self.search = None; }
        for (index, region) in frame.regions.iter().enumerate() {
            if region.kind != limn::HitKind::Button { continue; }
            let Some(visible) = visible_rect(frame, index + 1, region.rect, false) else { continue };
            let center = Point::new(visible.x + visible.width / 2.0, visible.y + visible.height / 2.0);
            if frame.context_menu_at(center).is_none() { continue; }
            let Some(target) = state.card_context(&region.id) else { continue };
            let Some(rect) = screen_rect(visible, origin) else { continue };
            let (identity, title) = context_identity(&target);
            let id = format!("native-menu:{}:{}:{identity}", region.id.len(), region.id);
            if self.menus.contains_key(&id) { continue; }
            let items = context_actions(state, &target);
            payload.menus.push(Menu { id: id.clone(), title: title.into(), rect, items: items.clone() });
            self.menus.insert(id, MenuBinding { target, actions: items.into_iter().map(|item| item.id).collect() });
        }
        for target in ["import-card"] {
            if !paste_valid(state, target) { continue; }
            let id = format!("ios-paste:{target}");
            let Some(node) = frame.access.iter().find(|node| node.id == id && !node.disabled) else { continue };
            let Some(visible) = visible_rect(frame, 0, node.rect, false) else { continue };
            let center = Point::new(visible.x + visible.width / 2.0, visible.y + visible.height / 2.0);
            if frame.press_region(center).is_some_and(|region| match region.kind {
                limn::HitKind::Surface => true,
                limn::HitKind::Button => region.id != id,
                _ => false,
            }) { continue; }
            if let Some(rect) = screen_rect(visible, origin).filter(|rect| rect[2] >= 44.0 && rect[3] >= 44.0) {
                self.paste = Some(target.into());
                payload.paste = Some(PasteSlot { target: target.into(), rect });
                break;
            }
        }
        payload
    }

    pub fn apply(&mut self, state: &mut State, event: Event) -> Vec<Command> {
        if !state.mobile { return Vec::new(); }
        match event {
            Event::SearchEdit { session, revision, text } if self.current_search(state, &session) => {
                let binding = self.search.as_mut().unwrap();
                if revision > binding.revision {
                    binding.revision = revision;
                    return state.update(&UiAction::InputChanged { id: "search".into(), value: text });
                }
            }
            Event::SearchFocus { session, revision, editing } if self.current_search(state, &session) => {
                let binding = self.search.as_mut().unwrap();
                if revision > binding.revision {
                    binding.revision = revision;
                    state.mobile_ui.search_editing = editing;
                    state.mobile_ui.blur = !editing;
                    state.focus = None;
                }
            }
            Event::SearchSubmit { session } if self.current_search(state, &session) => {
                return state.update(&UiAction::Submitted { id: "search".into(), value: state.query.clone() });
            }
            Event::SearchCancel { session } if self.current_search(state, &session) => {
                return state.update(&UiAction::Clicked { id: "mobile-search-cancel".into() });
            }
            Event::MenuOpen { id } if sheet_spec(state).is_none() => {
                self.active_menu = self.menus.get(&id).cloned().map(|menu| (id, menu));
            }
            Event::MenuAction { id, action } if sheet_spec(state).is_none() => {
                if let Some((_, menu)) = self.active_menu.as_ref().filter(|(held, menu)| *held == id && menu.actions.contains(&action)) {
                    let target = menu.target.clone();
                    self.active_menu = None;
                    return state.apply_card_context(target, &action);
                }
            }
            Event::MenuClose { id } if self.active_menu.as_ref().is_some_and(|(held, _)| *held == id) => self.active_menu = None,
            Event::SheetAction { sheet, action } => {
                if let Some(spec) = self.current_sheet(state, &sheet).filter(|spec| allowed_actions(spec).contains(action.as_str())) {
                    let closing = action == spec.dismiss;
                    if closing { self.sheet = None; }
                    return state.update(&UiAction::Clicked { id: action });
                }
            }
            Event::SheetInput { sheet, id, text } if self.current_sheet(state, &sheet).is_some_and(|spec|
                spec.rows.iter().any(|row| matches!(row, Row::Field { id: field, .. } if *field == id))) =>
                return state.update(&UiAction::InputChanged { id, value: text }),
            Event::SheetSlider { sheet, id, value } if value.is_finite() => {
                if let Some((min, max)) = self.current_sheet(state, &sheet).and_then(|spec| spec.rows.iter().find_map(|row| match row {
                    Row::Slider { id: slider, min, max, .. } if *slider == id && max > min => Some((*min, *max)), _ => None,
                })) {
                    return state.update(&UiAction::SliderChanged { id, fraction: ((value - min) / (max - min)).clamp(0.0, 1.0), committed: true });
                }
            }
            Event::SheetClose { sheet } => {
                if let Some(spec) = self.current_sheet(state, &sheet) {
                    let id = spec.dismiss;
                    self.sheet = None;
                    return state.update(&UiAction::Clicked { id });
                }
            }
            Event::Paste { target, text } if !text.is_empty() && paste_valid(state, &target) => {
                let valid = if target == "import-link" {
                    self.sheet.as_ref().is_some_and(|held| self.current_sheet(state, &held.sheet.id).is_some_and(|spec|
                        spec.rows.iter().any(|row| matches!(row, Row::Field { id, paste: true, .. } if *id == target))))
                } else { self.paste.as_deref() == Some(target.as_str()) };
                if valid {
                    if target == "import-card" {
                        state.importing = Some(text);
                        state.import_unknown = false;
                        state.mobile_ui.blur = true;
                    } else {
                        return state.update(&UiAction::InputChanged { id: target, value: text });
                    }
                }
            }
            _ => {}
        }
        Vec::new()
    }

    fn current_search(&self, state: &State, session: &str) -> bool {
        search_visible(state) && self.search.as_ref().is_some_and(|binding|
            binding.session == session && binding.intent == state.mobile_ui.search_intent)
    }

    fn current_sheet(&self, state: &State, id: &str) -> Option<Sheet> {
        let held = self.sheet.as_ref().filter(|held| held.sheet.id == id)?;
        sheet_spec(state).filter(|(key, _)| *key == held.key).map(|(_, sheet)| sheet)
    }
}

pub(crate) fn has_sheet(state: &State) -> bool {
    state.equalizer_open || state.confirm.is_some() || (state.missed_open && state.imported.is_some())
        || state.importing.is_some() || state.creating.is_some() || state.sharing.is_some()
        || state.mobile_ui.connections || state.mobile_ui.library.is_some() || state.mobile_ui.context.is_some()
}

fn visible_rect(frame: &Frame, floor: usize, rect: limn::Rect, in_header: bool) -> Option<limn::Rect> {
    let mut top = rect.y;
    let mut bottom = rect.y + rect.height;
    for overlay in frame.regions.iter().skip(floor).filter(|region| region.kind == limn::HitKind::Surface) {
        if overlay.rect.intersection(rect).is_none() { continue; }
        match overlay.id.as_str() {
            "mobile-header-bar" if in_header => {}
            "mobile-header-bar" => top = top.max(overlay.rect.y + overlay.rect.height),
            "mobile-mini-bar" | "mobile-nav-bar" => bottom = bottom.min(overlay.rect.y),
            // A floating limn surface owns this card until it is dismissed.
            _ => return None,
        }
    }
    (bottom > top).then(|| limn::Rect::new(rect.x, top, rect.width, bottom - top))
}

fn screen_rect(rect: limn::Rect, origin: Pos2) -> Option<[f32; 4]> {
    let rect = [rect.x + origin.x, rect.y + origin.y, rect.width, rect.height];
    (rect.iter().all(|value| value.is_finite()) && rect[2] > 0.0 && rect[3] > 0.0).then_some(rect)
}

fn context_identity(context: &CardContext) -> (&str, &str) {
    match context {
        CardContext::Track(context) => (&context.track.id, &context.track.title),
        CardContext::Library(context) => (&context.key, &context.title),
    }
}

fn context_actions(state: &State, context: &CardContext) -> Vec<Action> {
    let CardContext::Track(context) = context else {
        let CardContext::Library(context) = context else { unreachable!() };
        return library_actions(state, context);
    };
    let mut items: Vec<_> = crate::view::track_actions(state, &context.row, &context.track).into_iter().flatten().map(|action| {
        let name = action.id.strip_prefix("do:").and_then(|id| id.split_once(':')).map_or("", |(name, _)| name);
        let symbol = match name {
            "queue" | "add-new" => "text.badge.plus", "next" => "text.line.first.and.arrowtriangle.forward",
            "share" => "square.and.arrow.up", "like" => if state.is_liked(&context.track.id) { "heart.fill" } else { "heart" },
            "album" => "opticaldisc", "artist" => "person", "save" => "arrow.down.circle",
            "unsave" => "trash", "remove" => "minus.circle", _ => "music.note.list",
        };
        Action { destructive: matches!(name, "unsave" | "remove"), selected: name == "like" && state.is_liked(&context.track.id),
            subtitle: action.detail, ..Action::new(action.id, action.label, symbol) }
    }).collect();
    if context.upcoming.is_some() {
        items.insert(0, Action::new(format!("do:unqueue:{}", context.row), "Quitar de la cola", "minus.circle").destructive());
    }
    // Playlist membership belongs to the captured row, even if its page has changed.
    let remove = format!("do:remove:{}", context.row);
    if context.playlist.is_some() && !items.iter().any(|item| item.id == remove) {
        items.push(Action::new(remove, "Quitar de esta playlist", "minus.circle").destructive());
    }
    items
}

fn library_actions(state: &State, context: &LibraryContext) -> Vec<Action> {
    let mut items = vec![Action::new("library-action:open", "Abrir", "chevron.right")];
    let kept = context.entry.as_ref().is_none_or(|entry| state.card_entry_kept(entry));
    if kept {
        let pinned = state.collection.pinned.contains(&context.key);
        items.push(Action { selected: pinned, ..Action::new("library-action:pin", if pinned { "Desfijar" } else { "Fijar arriba" }, if pinned { "pin.slash" } else { "pin" }) });
    }
    if context.entry.is_some() {
        let artist = context.key.starts_with("artist:");
        if kept {
            items.push(Action::new("library-action:remove", if artist { "Dejar de seguir" } else { "Quitar de tu biblioteca" }, "minus.circle").destructive());
        } else {
            items.push(Action::new("library-action:keep", if artist { "Seguir artista" } else { "Guardar en tu biblioteca" }, if artist { "person.badge.plus" } else { "plus.circle" }));
        }
    }
    items
}

fn text(title: impl Into<String>, secondary: bool) -> Row { Row::Text { title: title.into(), secondary } }

fn sheet_spec(state: &State) -> Option<(String, Sheet)> {
    let shell = |title: &str, description: &str, dismiss: &str| Sheet { id: String::new(), title: title.into(), description: description.into(),
        dismiss: dismiss.into(), rows: Vec::new(), actions: Vec::new(), large: false, confirmation: false };
    // Match limn's overlay order. UIKit receives precisely the top surface.
    if state.equalizer_open {
        let mut sheet = shell("Ecualizador", "Ajustá el sonido de este equipo.", "equalizer-close");
        sheet.large = true;
        sheet.rows.push(text(if state.equalizer == surco_audio::equalizer::EqualizerValues::FLAT { "Plano" } else { "Personalizado" }, true));
        sheet.rows.push(Row::Slider { id: "equalizer:preamp".into(), title: "Preamplificador".into(), value: state.equalizer.preamp_db, min: -state::EQUALIZER_DB, max: state::EQUALIZER_DB });
        sheet.rows.extend(surco_audio::equalizer::BAND_HZ.iter().zip(state.equalizer.bands_db).enumerate().map(|(i, (hz, gain))| Row::Slider {
            id: format!("equalizer:{i}"), title: if *hz >= 1000.0 { format!("{:.0} kHz", hz / 1000.0) } else { format!("{hz:.0} Hz") },
            value: gain, min: -state::EQUALIZER_DB, max: state::EQUALIZER_DB,
        }));
        sheet.actions = vec![Action::new("equalizer-reset", "Restablecer", "arrow.counterclockwise"), Action::new("equalizer-close", "Listo", "checkmark")];
        return Some(("equalizer".into(), sheet));
    }
    if let Some(confirm) = &state.confirm {
        let (title, description, yes) = match confirm {
            Confirm::DeletePlaylist(id) => (format!("¿Eliminar «{}»?", state.collection.playlist(*id).map_or("esta playlist", |list| list.name.as_str())), "La playlist se borra de tu biblioteca. Las canciones no se tocan.", "Eliminar"),
            Confirm::ClearDownloads => ("¿Eliminar las descargas?".into(), "Ya no vas a tener acceso a lo que descargaste en este equipo.", "Eliminar"),
            Confirm::ClearQueue => ("¿Borrar la cola?".into(), "Se quitan las canciones que agregaste. Lo que venía sonando sigue.", "Borrar"),
            Confirm::ClearRecent => ("¿Borrar el historial?".into(), "Los accesos rápidos de Inicio quedan vacíos.", "Borrar"),
        };
        let mut sheet = shell(&title, description, "confirm-cancel");
        sheet.confirmation = true;
        sheet.actions = vec![Action::new("confirm-cancel", "Cancelar", "xmark"), Action::new("confirm-yes", yes, "trash").destructive()];
        return Some((format!("confirm:{confirm:?}"), sheet));
    }
    if let Some((id, missed)) = state.imported.as_ref().filter(|_| state.missed_open) {
        let mut sheet = shell("No se encontraron", "No están en el catálogo con ese nombre. Podés buscarlas a mano y agregarlas.", "import-missed-close");
        sheet.large = true;
        sheet.rows = missed.iter().map(|song| text(song, false)).collect();
        sheet.actions = vec![Action::new("import-missed-copy", "Copiar la lista", "doc.on.doc"), Action::new("import-missed-close", "Cerrar", "xmark")];
        return Some((format!("missed:{id}"), sheet));
    }
    if let Some(link) = &state.importing {
        let mut sheet = shell("Importar de otras apps", "Queda como una playlist tuya, con las canciones que el catálogo tenga.", "import-cancel");
        sheet.rows = vec![text("Enlace de la lista", false), Row::Field { id: "import-link".into(), value: link.clone(),
            placeholder: "https://open.spotify.com/playlist/…".into(), keyboard: "url".into(), paste: true },
            text(if state.import_unknown { "Ese enlace no es de una playlist de Spotify, Apple Music ni YouTube Music." } else { "De Spotify, Apple Music o YouTube Music: una playlist o un álbum, que sean públicos." }, true),
            text("Para llevar una playlist tuya a otra app, abrila y elegí «Copiar canciones».", true)];
        sheet.actions = vec![Action::new("import-cancel", "Cancelar", "xmark"), Action::new("import-go", "Importar", "square.and.arrow.down")];
        return Some(("import".into(), sheet));
    }
    if let Some(draft) = &state.creating {
        let mut sheet = shell("Nueva playlist", "Juntá canciones bajo un nombre.", "playlist-cancel");
        sheet.rows = vec![text("Título", false), Row::Field { id: "playlist-name".into(), value: draft.name.clone(), placeholder: state.collection.default_name(), keyboard: "default".into(), paste: false },
            Row::Segments { id: "playlist-kind".into(), items: vec![Action::new("playlist-kind:normal", "Normal", "music.note.list"), Action::new("playlist-kind:collaborative", "Colaborativa", "person.2")], selected: usize::from(draft.collaborative) }];
        if let Some(track) = &draft.first { sheet.rows.push(text(format!("Empieza con «{}».", track.title), true)); }
        sheet.actions = vec![Action::new("playlist-cancel", "Cancelar", "xmark"), Action::new("playlist-create", "Crear", "plus")];
        return Some((format!("playlist:{}", draft.first.as_ref().map_or("", |track| track.id.as_str())), sheet));
    }
    if let Some(track) = &state.sharing {
        let mut sheet = shell("Compartir canción", "El enlace la abre en YouTube Music; pegado en el buscador de Surco, se abre acá.", "share-close");
        sheet.rows = vec![text(&track.title, false), text(&track.artist, true), text(state::share_link(track), true), Row::Action(Action::new("share-card", "Reproducir o pausar", "playpause"))];
        sheet.actions = vec![Action::new("share-copy", if state.copied { "Enlace copiado" } else { "Copiar enlace" }, "doc.on.doc"), Action::new("share-close", "Listo", "checkmark")];
        return Some((format!("share:{}", track.id), sheet));
    }
    if state.mobile_ui.connections {
        let mut sheet = shell("Escuchar en", "Elegí dónde sigue la música.", "mobile-connections-close");
        sheet.rows = vec![Row::Route { title: "AirPlay o Bluetooth".into(), subtitle: state.mobile_ui.route.as_ref().map_or("Salida del sistema", |(name, _)| name.as_str()).into() },
            text("¿Un dispositivo nuevo?", false), text("Vinculá tus auriculares o parlantes en Ajustes > Bluetooth del iPhone. Después elegilos desde el selector de audio.", true),
            Row::Slider { id: "volume".into(), title: "Volumen de Surco".into(), value: state.volume, min: 0.0, max: 1.0 }];
        sheet.actions.push(Action::new("mobile-connections-close", "Listo", "checkmark"));
        return Some(("connections".into(), sheet));
    }
    if let Some(context) = &state.mobile_ui.library {
        let mut sheet = shell(&context.title, "Opciones de biblioteca", "mobile-library-close");
        sheet.rows = library_actions(state, context).into_iter().map(Row::Action).collect();
        return Some((format!("library:{}", context.key), sheet));
    }
    if let Some(context) = &state.mobile_ui.context {
        let mut sheet = shell(&context.track.title, &context.track.artist, "mobile-context-close");
        sheet.large = true;
        sheet.rows = context_actions(state, &CardContext::Track(context.clone())).into_iter().map(Row::Action).collect();
        return Some((format!("track:{}:{}:{:?}", context.row, context.track.id, context.playlist), sheet));
    }
    None
}

fn allowed_actions(sheet: &Sheet) -> HashSet<&str> {
    let mut ids: HashSet<_> = sheet.actions.iter().map(|action| action.id.as_str()).collect();
    ids.insert(&sheet.dismiss);
    for row in &sheet.rows {
        match row {
            Row::Action(action) => { ids.insert(&action.id); }
            Row::Segments { items, .. } => ids.extend(items.iter().map(|action| action.id.as_str())),
            _ => {}
        }
    }
    ids
}

fn paste_valid(state: &State, target: &str) -> bool {
    match target {
        "import-link" => state.importing.is_some() && sheet_spec(state).is_some_and(|(key, _)| key == "import"),
        "import-card" => state.page == state::Page::Library && !state.settings.import_dismissed && !has_sheet(state),
        _ => false,
    }
}

fn search_visible(state: &State) -> bool {
    state.mobile && state.mobile_ui.native_surfaces && state.page == state::Page::Search
        && state.mobile_search && !has_sheet(state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use limn::{Component, ContextMenu, ContextMenuEntry, ContextMenuItem, Element, Size, UiTree};
    use surco_core::Track;

    fn track(id: &str) -> Track {
        Track { id: id.into(), title: format!("Track {id}"), artist: "Artist".into(), artist_id: None,
            album: None, album_id: None, artwork_url: None, duration_ms: 180_000 }
    }

    fn frame() -> Frame {
        UiTree::new(ContextMenu::new("row-context", Element::button("track:0", "A track").size(Some(200.0), Some(80.0)),
            vec![ContextMenuEntry::Item(ContextMenuItem::new("more:track:0", "Opciones"))], limn::ComponentPalette::neutral_dark())
            .open_action("more:track:0").render()).layout(Size::new(240.0, 160.0))
    }

    fn phone() -> State { let mut state = State::default(); state.mobile = true; state }

    #[test]
    fn opened_menu_keeps_its_track_when_the_visible_row_is_replaced() {
        let mut state = phone();
        state.found.tracks.push(track("first"));
        let mut controller = Controller::default();
        let first = controller.payload(&state, &frame(), Pos2::new(12.0, 59.0));
        assert_eq!(first.menus.len(), 1);
        assert_eq!(first.menus[0].rect, [12.0, 59.0, 240.0, 160.0]);
        let id = first.menus[0].id.clone();
        assert!(controller.apply(&mut state, Event::MenuOpen { id: id.clone() }).is_empty());
        state.found.tracks[0] = track("second");
        let second = controller.payload(&state, &frame(), Pos2::ZERO);
        assert_ne!(second.menus[0].id, id);
        assert!(controller.apply(&mut state, Event::MenuAction { id: id.clone(), action: "downloads-clear".into() }).is_empty());
        assert_eq!(controller.apply(&mut state, Event::MenuAction { id: id.clone(), action: "do:queue:track:0".into() }),
            vec![Command::Enqueue(track("first"))]);
        assert!(controller.apply(&mut state, Event::MenuAction { id, action: "do:queue:track:0".into() }).is_empty(), "callbacks are consumed once");
    }

    #[test]
    fn sheet_identity_survives_editing_but_rejects_callbacks_after_retargeting() {
        let mut state = phone();
        state.importing = Some(String::new());
        let mut controller = Controller::default();
        let id = controller.payload(&state, &Frame::default(), Pos2::ZERO).sheet.unwrap().id;
        controller.apply(&mut state, Event::SheetInput { sheet: id.clone(), id: "import-link".into(), text: "https://open.spotify.com/playlist/example".into() });
        assert_eq!(controller.payload(&state, &Frame::default(), Pos2::ZERO).sheet.unwrap().id, id);
        assert!(state.importing.as_ref().unwrap().contains("example"));
        assert!(state.import_progress.is_none());
        state.importing = None;
        state.creating = Some(state::Draft::default());
        let replacement = controller.payload(&state, &Frame::default(), Pos2::ZERO).sheet.unwrap();
        assert_ne!(replacement.id, id);
        assert!(controller.apply(&mut state, Event::SheetInput { sheet: id.clone(), id: "import-link".into(), text: "stale".into() }).is_empty());
        controller.apply(&mut state, Event::SheetClose { sheet: id });
        assert!(state.importing.is_none() && state.creating.is_some());
    }

    #[test]
    fn covered_cards_cannot_publish_native_menus_or_reopen_after_dismissal() {
        let mut state = phone();
        state.found.tracks.push(track("first"));
        let mut controller = Controller::default();
        let id = controller.payload(&state, &frame(), Pos2::ZERO).menus[0].id.clone();
        controller.apply(&mut state, Event::MenuOpen { id: id.clone() });
        controller.apply(&mut state, Event::MenuClose { id: id.clone() });
        assert!(controller.apply(&mut state, Event::MenuAction { id, action: "do:queue:track:0".into() }).is_empty());
        state.confirm = Some(Confirm::ClearQueue);
        assert!(controller.payload(&state, &frame(), Pos2::ZERO).menus.is_empty());
    }

    #[test]
    fn menu_rectangles_exclude_header_and_tab_bar_even_when_the_card_center_is_visible() {
        let mut state = phone();
        state.found.tracks.push(track("first"));
        let card = ContextMenu::new("card-context", Element::button("track:0", "Song"), Vec::new(), limn::ComponentPalette::neutral_dark())
            .open_action("more:track:0").render();
        let bars = Element::column(vec![Element::spacer().size(None, Some(76.0)).block_pointer("mobile-header-bar"),
            Element::spacer().flex(1.0), Element::spacer().size(None, Some(64.0)).block_pointer("mobile-nav-bar")]);
        let frame = UiTree::new(Element::stack(vec![card, bars])).layout(Size::new(240.0, 300.0));
        let payload = Controller::default().payload(&state, &frame, Pos2::new(0.0, 59.0));
        assert_eq!(payload.menus.len(), 1);
        assert_eq!(payload.menus[0].rect, [0.0, 135.0, 240.0, 160.0]);
    }

    #[test]
    fn artist_card_actions_capture_identity_and_do_not_toggle_a_later_row() {
        let first = surco_core::ArtistRef { id: "first".into(), name: "First artist".into(), subscribers: None, artwork_url: None };
        let mut state = phone();
        state.collection.recent.push(state::Entry::Artist(first.clone()));
        let frame = UiTree::new(ContextMenu::new("recent-context", Element::button("recent:0", "Artist").size(Some(200.0), Some(80.0)),
            vec![ContextMenuEntry::Item(ContextMenuItem::new("entry-more:recent:0", "Opciones"))], limn::ComponentPalette::neutral_dark())
            .open_action("entry-more:recent:0").render()).layout(Size::new(240.0, 160.0));
        assert_eq!(state.card_context_action("recent:0").as_deref(), Some("entry-more:recent:0"));
        let mut controller = Controller::default();
        let id = controller.payload(&state, &frame, Pos2::ZERO).menus[0].id.clone();
        controller.apply(&mut state, Event::MenuOpen { id: id.clone() });
        state.collection.recent[0] = state::Entry::Artist(surco_core::ArtistRef { id: "second".into(), name: "Second artist".into(), ..first.clone() });
        controller.payload(&state, &frame, Pos2::ZERO);
        controller.apply(&mut state, Event::MenuAction { id, action: "library-action:keep".into() });
        assert_eq!(state.collection.kept, vec![state::Entry::Artist(first)]);
    }

    #[test]
    fn playlist_menu_opens_the_captured_id_after_library_reordering() {
        let mut state = phone();
        state.collection.playlists = vec![state::Playlist { id: 7, name: "First".into(), collaborative: false, tracks: Vec::new() },
            state::Playlist { id: 9, name: "Second".into(), collaborative: false, tracks: Vec::new() }];
        let frame = UiTree::new(ContextMenu::new("playlist-context", Element::button("playlist:0", "First"), Vec::new(), limn::ComponentPalette::neutral_dark())
            .open_action("entry-more:playlist:0").render()).layout(Size::new(240.0, 160.0));
        let mut controller = Controller::default();
        let id = controller.payload(&state, &frame, Pos2::ZERO).menus[0].id.clone();
        controller.apply(&mut state, Event::MenuOpen { id: id.clone() });
        state.collection.playlists.reverse();
        controller.payload(&state, &frame, Pos2::ZERO);
        controller.apply(&mut state, Event::MenuAction { id, action: "library-action:open".into() });
        assert_eq!(state.page, state::Page::Playlist(7));
    }

    #[test]
    fn only_the_current_top_sheet_accepts_its_declared_controls() {
        let mut state = phone();
        state.equalizer_open = true;
        state.confirm = Some(Confirm::ClearQueue);
        let mut controller = Controller::default();
        let sheet = controller.payload(&state, &Frame::default(), Pos2::ZERO).sheet.unwrap();
        assert_eq!(sheet.title, "Ecualizador");
        let id = sheet.id;
        assert!(controller.apply(&mut state, Event::SheetAction { sheet: id.clone(), action: "confirm-yes".into() }).is_empty());
        assert!(state.confirm.is_some());
        assert!(controller.apply(&mut state, Event::SheetSlider { sheet: id.clone(), id: "volume".into(), value: 0.2 }).is_empty());
        assert!(controller.apply(&mut state, Event::SheetSlider { sheet: id.clone(), id: "equalizer:preamp".into(), value: f32::NAN }).is_empty());
        let commands = controller.apply(&mut state, Event::SheetSlider { sheet: id.clone(), id: "equalizer:preamp".into(), value: 6.0 });
        assert_eq!(state.equalizer.preamp_db, 6.0);
        assert_eq!(commands, vec![Command::Equalizer(state.equalizer)]);
        controller.apply(&mut state, Event::SheetClose { sheet: id.clone() });
        assert!(!state.equalizer_open && state.confirm.is_some());
        let confirm = controller.payload(&state, &Frame::default(), Pos2::ZERO).sheet.unwrap();
        assert_ne!(confirm.id, id);
        assert!(confirm.confirmation);
    }

    #[test]
    fn share_sheet_retargeting_rejects_a_copy_for_the_previous_track() {
        let mut state = phone();
        state.sharing = Some(track("first"));
        let mut controller = Controller::default();
        let first = controller.payload(&state, &Frame::default(), Pos2::ZERO).sheet.unwrap().id;
        state.sharing = Some(track("second"));
        assert!(controller.apply(&mut state, Event::SheetAction { sheet: first.clone(), action: "share-copy".into() }).is_empty());
        assert!(state.copy.is_none());
        let second = controller.payload(&state, &Frame::default(), Pos2::ZERO).sheet.unwrap().id;
        assert_ne!(first, second);
        controller.apply(&mut state, Event::SheetAction { sheet: second, action: "share-copy".into() });
        assert_eq!(state.copy.as_deref(), Some(state::share_link(&track("second")).as_str()));
    }

    #[test]
    fn payload_matches_the_native_json_contract() {
        let mut state = phone();
        state.importing = Some("https://example.com/ñ".into());
        let payload = Controller::default().payload(&state, &Frame::default(), Pos2::ZERO);
        let json = serde_json::to_value(payload).unwrap();
        assert_eq!(json["sheet"]["rows"][1]["kind"], "field");
        assert_eq!(json["sheet"]["rows"][1]["paste"], true);
        assert!(json["menus"].as_array().unwrap().is_empty());
        let action = serde_json::to_value(Row::Action(Action::new("test", "Title", "heart"))).unwrap();
        assert_eq!(action["kind"], "action");
        assert_eq!(action["id"], "test");
        assert_eq!(serde_json::from_str::<Event>(r#"{"kind":"sheet_input","sheet":"1","id":"import-link","text":"ñ"}"#).unwrap(),
            Event::SheetInput { sheet: "1".into(), id: "import-link".into(), text: "ñ".into() });
    }

    #[test]
    fn native_payload_combines_app_and_system_reduced_motion_preferences() {
        let mut state = phone();
        state.motion = true;
        let mut controller = Controller::default();
        assert_eq!(serde_json::to_value(controller.payload(&state, &Frame::default(), Pos2::ZERO)).unwrap()["reduce_motion"], false);
        state.settings.reduce_motion = true;
        assert_eq!(serde_json::to_value(controller.payload(&state, &Frame::default(), Pos2::ZERO)).unwrap()["reduce_motion"], true);
        state.settings.reduce_motion = false;
        state.motion = false;
        assert_eq!(serde_json::to_value(controller.payload(&state, &Frame::default(), Pos2::ZERO)).unwrap()["reduce_motion"], true);
    }

    #[test]
    fn native_search_owns_one_editor_and_never_a_second_paste_button() {
        let mut state = phone();
        state.mobile_ui.native_surfaces = true;
        state.mobile_search = true;
        let size = Size::new(390.0, 844.0);
        let frame = UiTree::new(crate::view::view(&state, size)).layout(size);
        let slot = frame.access.iter().find(|node| node.id == "ios-search").unwrap();
        assert_eq!(slot.role, limn::Role::Group, "UIKit supplies the real search editor");
        assert!(!frame.access.iter().any(|node| node.id == "ios-paste:search"));
        let mut controller = Controller::default();
        let payload = controller.payload(&state, &frame, Pos2::new(0.0, 59.0));
        assert!(payload.paste.is_none());
        let search = payload.search.unwrap();
        assert_eq!(search.rect[1], slot.rect.y + 59.0);
        assert!(!search.compact && !search.editing, "empty Search starts with its idle heading and no keyboard intention");
        controller.apply(&mut state, Event::SearchFocus { session: search.session.clone(), revision: 1, editing: true });
        assert!(controller.apply(&mut state, Event::SearchEdit { session: search.session.clone(), revision: 2,
            text: "https://youtu.be/a-song".into() }).is_empty());
        assert_eq!(state.query, "https://youtu.be/a-song");
        assert!(state.mobile_ui.search_editing && state.mobile_ui.pending.is_none());
        assert!(state.focus.is_none(), "the limn editor must not compete for the native keyboard");
        controller.apply(&mut state, Event::SearchEdit { session: search.session.clone(), revision: 1, text: "old".into() });
        assert_eq!(state.query, "https://youtu.be/a-song", "older native input cannot overwrite the editor");
        let refreshed = controller.payload(&state, &frame, Pos2::ZERO).search.unwrap();
        assert_eq!(refreshed.session, search.session);
        assert_eq!(refreshed.revision, 2);
        assert!(refreshed.compact && refreshed.editing);
        controller.apply(&mut state, Event::SearchFocus { session: search.session.clone(), revision: 3, editing: false });
        controller.apply(&mut state, Event::SearchFocus { session: search.session.clone(), revision: 1, editing: true });
        assert!(!state.mobile_ui.search_editing, "an old focus callback must not reopen the keyboard");
        let unfocused = controller.payload(&state, &frame, Pos2::ZERO).search.unwrap();
        assert!(unfocused.compact && !unfocused.editing, "a query keeps the compact visual state after keyboard dismissal");
        controller.apply(&mut state, Event::SearchSubmit { session: search.session });
        assert!(matches!(&state.page, state::Page::Loading(crate::backend::Target::Song(id)) if id == "a-song"),
            "the explicit Return gesture opens the pasted link");
    }

    #[test]
    fn search_callbacks_do_not_cross_navigation_or_an_explicit_recent_selection() {
        let mut state = phone();
        state.mobile_ui.native_surfaces = true;
        state.mobile_search = true;
        let frame = UiTree::new(crate::view::view(&state, Size::new(390.0, 844.0))).layout(Size::new(390.0, 844.0));
        let mut controller = Controller::default();
        let first = controller.payload(&state, &frame, Pos2::ZERO).search.unwrap().session;
        controller.apply(&mut state, Event::SearchEdit { session: first.clone(), revision: 1, text: "niñez".into() });
        controller.apply(&mut state, Event::SearchCancel { session: first.clone() });
        assert_eq!(state.query, "niñez");
        assert!(!state.mobile_ui.search_editing && state.mobile_ui.pending.is_none());
        state.page = state::Page::Library;
        controller.payload(&state, &frame, Pos2::ZERO);
        controller.apply(&mut state, Event::SearchEdit { session: first.clone(), revision: 3, text: "late".into() });
        assert_eq!(state.query, "niñez");
        state.page = state::Page::Search;
        let second = controller.payload(&state, &frame, Pos2::ZERO).search.unwrap().session;
        assert_ne!(first, second);
        state.collection.searches = vec!["Elegida explícitamente".into()];
        state.update(&UiAction::Clicked { id: "recent-search:0".into() });
        controller.apply(&mut state, Event::SearchEdit { session: second, revision: 9, text: "old composition".into() });
        assert_eq!(state.query, "Elegida explícitamente", "explicit selection invalidates callbacks before the next paint");
    }

    #[test]
    fn native_equalizer_keeps_all_shared_controls_and_reports_its_current_state() {
        let mut state = phone();
        state.equalizer_open = true;
        let mut controller = Controller::default();
        let sheet = controller.payload(&state, &Frame::default(), Pos2::ZERO).sheet.unwrap();
        assert_eq!(sheet.rows.iter().filter(|row| matches!(row, Row::Slider { .. })).count(), surco_audio::equalizer::BAND_HZ.len() + 1);
        assert!(sheet.actions.iter().any(|action| action.id == "equalizer-reset"));
        assert!(sheet.rows.iter().any(|row| matches!(row, Row::Text { title, .. } if title == "Plano")));
        state.equalizer.preamp_db = 3.0;
        let sheet = controller.payload(&state, &Frame::default(), Pos2::ZERO).sheet.unwrap();
        assert!(sheet.rows.iter().any(|row| matches!(row, Row::Text { title, .. } if title == "Personalizado")));
    }

    #[test]
    fn import_card_paste_opens_a_draft_and_never_submits_it() {
        let mut state = phone();
        state.page = state::Page::Library;
        let frame = UiTree::new(Element::panel(Element::spacer()).size(Some(112.0), Some(44.0))
            .accessible(limn::Access::new(limn::Role::Group).id("ios-paste:import-card")))
            .layout(Size::new(240.0, 160.0));
        let mut controller = Controller::default();
        assert_eq!(controller.payload(&state, &frame, Pos2::ZERO).paste.unwrap().target, "import-card");
        assert!(controller.apply(&mut state, Event::Paste { target: "import-card".into(), text: "https://open.spotify.com/playlist/example".into() }).is_empty());
        assert_eq!(state.importing.as_deref(), Some("https://open.spotify.com/playlist/example"));
        assert!(state.import_progress.is_none() && !state.settings.import_dismissed);
        controller.apply(&mut state, Event::SheetClose { sheet: "old".into() });
        assert!(controller.apply(&mut state, Event::Paste { target: "search".into(), text: "wrong field".into() }).is_empty());
        assert!(state.query.is_empty());
    }
}
