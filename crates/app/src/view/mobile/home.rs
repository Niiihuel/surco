use super::*;

/// BitChord's hero card: 70% of the width up to 320, a little taller than wide, its corners 18.
const HERO_SHARE: f32 = 0.7;
const HERO_MOST: f32 = 320.0;
const HERO_RATIO: f32 = 0.92;
pub(in crate::view) const HERO_RADIUS: f32 = 18.0;
/// BitChord's recents: pages of four rows, 88% of the width up to 400, so the next one shows.
const PAGE_ROWS: usize = 4;
const PAGE_SHARE: f32 = 0.88;
const PAGE_MOST: f32 = 400.0;
const ROW_ART: f32 = 48.0;
const ROW_HEIGHT: f32 = 60.0;
/// The darkening a hero card's words stand on, from clear to BitChord's black at 78%, a step
/// darker as limn blends in linear light.
const HERO_SCRIM: [Color; 3] = [Color::rgba(0, 0, 0, 0), Color::rgba(0, 0, 0, 150), Color::rgba(0, 0, 0, 225)];

/// A shelf's heading on a phone, as BitChord's: its name, then what it is, `about`, under it.
pub(in crate::view) fn shelf_heading(title: &str, about: Option<&str>, palette: ComponentPalette) -> Element {
    let title = Text::new(title, palette).font_size(17.0).truncate().render().font_family(FontFamily::SansBold);
    match about {
        Some(about) => Element::column(vec![title, Text::new(about, palette).font_size(14.0).muted().truncate().render()]).gap(1.0),
        None => title,
    }
}

/// A mix as BitChord's hero card, leading a phone's home: the picture of its first song tall
/// and rounded, its name and who it plays over a darkening at its foot.
pub(in crate::view) fn hero(state: &State, index: usize, mix: &Mix, width: f32, palette: ComponentPalette) -> Element {
    let row = format!("made:{index}");
    let side = (width * HERO_SHARE).min(HERO_MOST).round();
    let tall = (side / HERO_RATIO).round();
    let picture = match mix.covers().first().and_then(|url| state.artwork.get(*url)) {
        Some(image) => Element::raster_image(image.clone().cover()),
        None => Element::panel(Element::spacer()).background(palette.selected),
    };
    let label = match &mix.kind {
        MixKind::Moment => t!("desktop-moment"),
        _ => t!("desktop-mix"),
    };
    let words = Element::column(vec![
        Text::new(label.to_uppercase(), palette).font_size(12.0).ink(Color::rgba(255, 255, 255, 190)).truncate().render().font_family(FontFamily::SansBold),
        Text::new(mix_name(mix), palette).font_size(20.0).truncate().render().font_family(FontFamily::SansBold),
        Text::new(mix_detail(mix), palette).font_size(14.0).ink(Color::rgba(255, 255, 255, 200)).truncate().render(),
    ]).gap(2.0);
    let foot = Element::column(vec![Element::spacer().flex(1.0), words]).padding(16.0).vertical_gradient(HERO_SCRIM.to_vec())
        .size(None, Some((tall * 0.5).round()));
    let card = Element::stack(vec![picture.size(Some(side), Some(tall)), Element::column(vec![Element::spacer().flex(1.0), foot])])
        .size(Some(side), Some(tall)).radius(HERO_RADIUS).border(THUMB_RIM, 0.5).clip_children();
    let said = limn::Access::new(limn::Role::Button).name(format!("{} · {}", mix_name(mix), mix_detail(mix)));
    Element::clickable(row, card).accessible(said).radius(HERO_RADIUS)
}

/// BitChord's recents on a phone: what was played as rows of its cover, its name and what it
/// is, four to a page and the pages side by side, the next one showing at the edge.
pub(in crate::view) fn recents(state: &State, width: f32, palette: ComponentPalette) -> Option<Element> {
    let span = (width * PAGE_SHARE).min(PAGE_MOST).round();
    let line = |id: &str, name: &str, about: &str, picture: Element| {
        let copy = Element::column(vec![
            Text::new(name, palette).font_size(16.0).truncate().render().font_family(FontFamily::SansMedium),
            Text::new(about, palette).font_size(14.0).muted().truncate().render(),
        ]).gap(2.0).centered_y().flex(1.0);
        Element::clickable(id, Element::row(vec![picture.centered_y(), copy]).gap(12.0))
            .accessible(limn::Access::new(limn::Role::Button).name(format!("{name} · {about}")))
            .size(Some(span), Some(ROW_HEIGHT))
    };
    let mut rows = Vec::new();
    if !state.saved.is_empty() {
        let picture = library_tile(Icon::Custom(DOWNLOAD), palette).size(Some(ROW_ART), Some(ROW_ART));
        rows.push(line("home-downloads", &t!("common-downloads"), &t!("common-playlist"), picture).dispatch_as("downloads"));
    }
    for (index, entry) in state.collection.recent.iter().enumerate() {
        let id = format!("recent:{index}");
        let (about, radius) = match entry {
            Entry::Album(album) => (album.artist.clone().unwrap_or_else(|| t!("common-album").into_owned()), 7.0),
            Entry::Artist(_) => (t!("common-artist").into_owned(), ROW_ART / 2.0),
        };
        let picture = cover(state, entry.artwork(), ROW_ART, palette).radius(radius).border(THUMB_RIM, 0.5);
        rows.push(mobile_card_context(state, &id, line(&id, entry.title(), &about, picture)));
    }
    rows.truncate(QUICK_MOST);
    if rows.is_empty() { return None; }
    let mut rows = rows.into_iter().peekable();
    let mut pages = Vec::new();
    while rows.peek().is_some() {
        pages.push(Element::column(rows.by_ref().take(PAGE_ROWS).collect()).gap(4.0).size(Some(span), None));
    }
    Some(headed_shelf(state, "home-recents", None, None, &t!("mobile-home-recent"), pages, palette))
}

pub(super) fn content(state: &State, width: f32, palette: ComponentPalette) -> Element {
    let empty = state.collection.recent.is_empty() && state.collection.kept.is_empty()
        && state.collection.heard.is_empty() && state.saved.is_empty();
    if !empty { return super::super::home(state, width, palette); }
    let intro = Element::column(vec![
        Text::new(t!("mobile-home-title"), palette).font_size(36.0).render(),
        Text::new(t!("mobile-home-intro"), palette).font_size(16.0).muted().render(),
        Button::new("mobile-discover", t!("mobile-home-discover"), palette).icon(Icon::Search)
            .variant(ButtonVariant::Default).sizing(limn::ButtonSizing { height: 48.0, ..ControlSize::Spacious.button_sizing() }).round().render(),
    ]).gap(20.0).padding(24.0);
    let panel = Element::panel(intro).background(SCHEME.raised).radius(16.0);
    ScrollArea::new("mobile-home", Element::column(vec![panel,
        heading(&t!("mobile-home-curiosity"), palette), search::browse(4, width - 32.0, 2, 98.0, palette),
        Element::row(vec![Element::icon(Icon::Heart).foreground(palette.muted).centered_y(),
            Text::new(t!("mobile-home-favorites"), palette).muted().render().flex(1.0)]).gap(12.0).padding(8.0),
    ]).gap(24.0).padding(16.0)).render().flex(1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use limn::UiTree;
    use surco_core::AlbumRef;

    #[test]
    fn recents_come_four_to_a_page_with_the_next_page_showing() {
        let mut state = State::default();
        state.mobile = true;
        state.collection.recent = (0..6).map(|index| Entry::Album(AlbumRef {
            id: format!("album-{index}"), title: format!("Álbum {index}"), artist: Some("Una banda".into()), year: String::new(), artwork_url: None,
        })).collect();
        let width = 390.0;
        let mut tree = UiTree::new(recents(&state, width, SCHEME.palette()).unwrap());
        tree.set_reduced_motion(true);
        let frame = tree.layout(Size::new(width, 600.0));
        let row = |index: usize| frame.access.iter().find(|node| node.id == format!("recent:{index}")).unwrap_or_else(|| panic!("recent:{index}")).rect;
        assert!((row(0).height - ROW_HEIGHT).abs() < 0.01, "{:?}", row(0));
        assert!((row(0).width - (width * PAGE_SHARE).round()).abs() < 0.5, "88% of the width: {:?}", row(0));
        for index in 1..4 {
            assert!((row(index).x - row(0).x).abs() < 0.5 && row(index).y > row(index - 1).y, "the first page holds four, one under the other");
        }
        assert!(row(4).x > row(0).x + row(0).width && (row(4).y - row(0).y).abs() < 0.5, "the fifth starts the next page, beside it");
        assert!(row(4).x < width, "and shows at the edge");
    }
}
