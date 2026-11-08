//! A phone's queue, as BitChord's, open in the player under the song: what plays, then what
//! follows under the name of where it comes from, each row with a grip to move it by and a
//! cross to take it out.

use super::*;

/// A row's cover, its grip, and the room the row takes, as BitChord's.
const COVER: f32 = 44.0;
const ROW: f32 = 58.0;
const GRIP: &str = include_str!("../../../assets/icons/grip-lines.svg");
/// The names over the rows and the glyphs beside them, white at 55%.
const SOFT: Color = Color::rgba(255, 255, 255, 140);

/// The queue under its large name: what plays, what was queued by hand, and what comes after it.
pub(super) fn content(state: &State, palette: ComponentPalette) -> Element {
    let mut rows = vec![
        Element::panel(Text::new(t!("mobile-queue"), palette).font_size(22.0).render().font_family(FontFamily::SansBold)).padding(PANEL_PAD / 2.0),
    ];
    if let Some(track) = state.current() {
        rows.push(heading(&t!("mobile-now-playing"), None, palette));
        rows.push(now(state, track, palette));
    }
    let upcoming = &state.player.upcoming;
    let queued = state.player.queued.min(upcoming.len());
    if queued > 0 {
        let clear = Button::new("queue-clear", t!("common-clear"), palette).variant(ButtonVariant::Ghost).size(ControlSize::Compact).render();
        rows.push(heading(&t!("desktop-in-queue"), Some(clear), palette));
    }
    for (index, track) in upcoming.iter().enumerate() {
        if index == queued {
            // What the catalog picks plays on by itself, as BitChord's AutoPlay; the rest is the list's.
            rows.push(if state.player.picks.contains(&track.id) { autoplay(palette) } else { heading(&t!("desktop-next-up"), None, palette) });
        }
        // Named by its track, so the row that starts playing closes and the rest glide up.
        rows.push(Element::column(vec![row(state, index, track, palette)]).animate_height(format!("upcoming-row:{}", track.id), false));
    }
    // Listening with others, after the songs.
    rows.push(Element::panel(super::super::jam::queue_entry(state, palette)).padding(PANEL_PAD / 2.0));
    if upcoming.is_empty() {
        rows.push(centered(blank(Icon::Playlist, t!("desktop-queue-empty"), t!("desktop-queue-empty-detail"), None, palette)));
    }
    let list = Element::column(rows).keep_leaving("upcoming-rows");
    panel_scroll("queue-list", Element::column(vec![list]).padding(PANEL_PAD / 2.0)).render().flex(1.0)
}

/// A section's name in the secondary ink, with what acts on it at its end.
fn heading(title: &str, action: Option<Element>, palette: ComponentPalette) -> Element {
    let mut row = vec![Text::new(title, palette).font_size(17.0).ink(SOFT).truncate().render().font_family(FontFamily::SansMedium).centered_y().flex(1.0)];
    row.extend(action.map(Element::centered_y));
    Element::row(row).padding(PANEL_PAD / 2.0).size(None, Some(TAP))
}

/// BitChord's head over what autoplay chose: its glyph, its name and what it is.
fn autoplay(palette: ComponentPalette) -> Element {
    let glyph = Element::icon(Icon::Infinity).large_icon(20.0).foreground(SOFT).size(Some(24.0), None).centered_y();
    let copy = Element::column(vec![
        Text::new(t!("mobile-autoplay"), palette).font_size(17.0).truncate().render().font_family(FontFamily::SansBold),
        Text::new(t!("mobile-autoplay-detail"), palette).font_size(15.0).ink(SOFT).truncate().render(),
    ]).gap(1.0).centered_y().flex(1.0);
    Element::row(vec![glyph, copy]).gap(10.0).padding(PANEL_PAD / 2.0).size(None, Some(ROW))
}

/// A song's cover, name and artist, as the queue's rows draw them.
fn song(state: &State, track: &Track, palette: ComponentPalette) -> Element {
    let copy = Element::column(vec![
        Text::new(&track.title, palette).font_size(17.0).truncate().render().font_family(FontFamily::SansBold),
        Text::new(&track.artist, palette).font_size(15.0).ink(SOFT).truncate().render(),
    ]).gap(1.0).centered_y().flex(1.0);
    Element::row(vec![cover(state, &track.artwork_url, COVER, palette).radius(6.0).border(THUMB_RIM, 0.5).centered_y(), copy]).gap(12.0)
}

/// A cross that takes something out of the queue.
fn cross(id: &str, label: &str) -> Element {
    Element::clickable(id, Element::icon(Icon::Close).large_icon(18.0).foreground(SOFT).centered())
        .accessible(limn::Access::new(limn::Role::Button).name(label)).radius(TAP / 2.0).size(Some(TAP), Some(TAP)).centered_y()
}

/// What plays, its sound beside it; its cross skips it.
fn now(state: &State, track: &Track, palette: ComponentPalette) -> Element {
    let sound = super::super::wave(state, "wave:queue-now".into(), palette).centered_y();
    Element::row(vec![song(state, track, palette).flex(1.0), sound, cross("queue-skip", &t!("common-next-song")).dispatch_as("next")])
        .gap(8.0).padding(PANEL_PAD / 2.0).size(None, Some(ROW))
}

/// A song to come: held by its grip it moves to where it is let go; pressed it plays, and its
/// cross takes it out.
fn row(state: &State, index: usize, track: &Track, palette: ComponentPalette) -> Element {
    let id = format!("upcoming:{index}");
    let grip = Element::clickable(format!("{}{index}", crate::state::mobile::QUEUE_GRIP), Element::icon(Icon::Custom(GRIP)).large_icon(20.0).foreground(SOFT).centered())
        .draggable().accessible(limn::Access::new(limn::Role::Button).name(t!("mobile-move-in-queue", title = track.title.as_str())))
        .size(Some(28.0), Some(TAP)).centered_y();
    let main = Element::clickable(&id, song(state, track, palette)).drop_target()
        .accessible(limn::Access::new(limn::Role::Button).name(format!("{} · {}", track.title, track.artist))).flex(1.0);
    Element::row(vec![grip, main, cross(&format!("do:unqueue:{id}"), &t!("desktop-remove-from-queue"))])
        .gap(6.0).padding(PANEL_PAD / 2.0).size(None, Some(ROW))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{backend::Command, player::PlayerView};
    use limn::{UiAction, UiTree};

    #[test]
    fn a_queue_row_is_moved_by_its_grip_and_taken_out_by_its_cross() {
        let track = |id: &str| Track { id: id.into(), title: id.into(), artist: "Una artista".into(), artist_id: None, album: None, album_id: None, artwork_url: None, duration_ms: 180_000 };
        let mut state = State::default();
        (state.mobile, state.page, state.queue_open) = (true, Page::Playing, true);
        state.show(PlayerView { current: Some(track("now")), upcoming: vec![track("a"), track("b"), track("c")], ..PlayerView::default() });
        let mut tree = UiTree::new(content(&state, SCHEME.palette()));
        tree.set_reduced_motion(true);
        let frame = tree.layout(Size::new(390.0, 800.0));
        for id in ["queue-skip", "upcoming-grip:2", "upcoming:0", "do:unqueue:upcoming:1"] {
            assert_eq!(frame.access.iter().filter(|node| node.id == id).count(), 1, "{id}");
        }
        let dropped = UiAction::Dropped { id: "upcoming-grip:2".into(), position: limn::Point::new(0.0, 0.0), target: Some("upcoming:0".into()) };
        assert_eq!(state.update(&dropped), vec![Command::MoveQueue(2, 0)], "held by its grip, the third goes first");
        assert_eq!(state.update(&UiAction::Clicked { id: "do:unqueue:upcoming:1".into() }), vec![Command::Unqueue(1)]);
    }
}
