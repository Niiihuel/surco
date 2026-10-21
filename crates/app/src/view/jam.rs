//! Listening together, as Spotify's Jam lays it out: out of a room, the way to start one or
//! join one by its code; in one, its people as faces, what comes next with who added it, and
//! what is said and felt in it as it plays. The room also shows in the right panel and as a
//! pill in the player's bar, so it is in sight wherever the app is.

use super::*;
use crate::room::{Line, Phase, RoomView, Said};
use crate::together::REACTIONS;

/// How far each face of a group reaches over the one before it.
const OVERLAP: f32 = 0.32;
/// The widest the room's two columns stand together.
const WIDEST: f32 = 1100.0;
/// The width from which what comes and what is said stand side by side.
const SIDE_BY_SIDE: f32 = 760.0;

/// A name's initial on its colour, round, `side` across.
pub(super) fn face(name: &str, side: f32) -> Element {
    let initial: String = name.chars().next().map(|first| first.to_uppercase().collect()).unwrap_or_default();
    Element::panel(Element::text(initial).font_size(side * 0.44).font_family(FontFamily::SansBold).foreground(CANVAS).centered())
        .background(name_colour(name)).radius(side / 2.0).size(Some(side), Some(side))
}

/// Faces one over the next, as a group is shown: each ringed in `ground` so they part, the
/// first in front, and past `most` a count of the rest.
fn faces(names: &[&str], side: f32, most: usize, ground: Color, palette: ComponentPalette) -> Element {
    let ring = (side * 0.06).max(2.0);
    let step = side * (1.0 - OVERLAP);
    let mut shown: Vec<Element> = names.iter().take(most).map(|name| {
        Element::panel(face(name, side).centered()).background(ground).radius(side / 2.0 + ring).size(Some(side + 2.0 * ring), Some(side + 2.0 * ring))
    }).collect();
    let rest = names.len().saturating_sub(most);
    if rest > 0 {
        let count = Text::new(format!("+{rest}"), palette).font_size(side * 0.36).render().font_family(FontFamily::SansBold).centered();
        shown.push(Element::panel(count).background(palette.selected).radius(side / 2.0 + ring).size(Some(side + 2.0 * ring), Some(side + 2.0 * ring)));
    }
    let count = shown.len();
    // Painted from the last, so the first stands in front.
    let layers = shown.into_iter().enumerate().rev().map(|(index, face)| Element::row(vec![Element::spacer().size(Some(index as f32 * step), None), face])).collect();
    Element::stack(layers).size(Some(side + 2.0 * ring + count.saturating_sub(1) as f32 * step), Some(side + 2.0 * ring))
}

fn names(room: &RoomView) -> Vec<&str> {
    room.members.iter().map(|member| member.name.as_str()).collect()
}

/// The room's name: its host's.
fn room_name(room: &RoomView) -> String {
    let host = room.members.iter().find(|member| member.host).map_or("", |member| member.name.as_str());
    t!("common-together-room-of", name = host).into_owned()
}

/// How the room stands, said under its name.
fn phase(room: &RoomView) -> String {
    let standing = match room.phase {
        Phase::Connecting => t!("common-together-connecting"),
        Phase::Waiting => t!("common-together-waiting"),
        Phase::Live if room.hosting => t!("common-together-hosting", relays = room.relays),
        Phase::Live => t!("common-together-live", relays = room.relays),
        Phase::LostHost => t!("common-together-lost"),
        Phase::Ended => t!("common-together-ended"),
    };
    format!("{} · {standing}", t!("common-together-listening", count = room.members.len().max(1)))
}

/// Who added a song still to come, as the room keeps it.
fn adder<'a>(room: &'a RoomView, id: &str) -> Option<&'a str> {
    room.added.iter().rev().find(|(added, _)| added == id).map(|(_, name)| name.as_str())
}

/// How the way to listening together reads where it is offered, beside the devices and over
/// the queue: out of a room, what it is; in one, the room.
pub(crate) fn entry(state: &State) -> (String, String) {
    match &state.room {
        Some(room) => (room_name(room), t!("common-together-listening", count = room.members.len().max(1)).into_owned()),
        None => (t!("common-together").into_owned(), t!("common-together-entry").into_owned()),
    }
}

/// The way to the room at the head of the queue, as Spotify's Jam offers it there.
pub(super) fn queue_entry(state: &State, palette: ComponentPalette) -> Element {
    let (title, detail) = entry(state);
    let leading = match &state.room {
        Some(room) => faces(&names(room), 28.0, 3, CANVAS, palette),
        None => Element::panel(Element::icon(Icon::Custom(glyph::TOGETHER)).foreground(palette.text).centered()).background(palette.selected).radius(20.0).size(Some(40.0), Some(40.0)),
    };
    let row = Element::row(vec![
        leading.centered_y(),
        Element::column(vec![
            Text::new(title, palette).font_size(15.0).truncate().render().font_family(FontFamily::SansBold),
            Text::new(detail, palette).role(TextRole::Caption).muted().truncate().render(),
        ]).gap(2.0).flex(1.0).centered_y(),
        Element::icon(Icon::Custom(glyph::CHEVRON_RIGHT)).foreground(palette.muted).centered_y(),
    ]).gap(12.0);
    Element::clickable("together", Element::panel(row).padding(12.0).radius(RadiusRole::Card.px()).background(Color { a: 14, ..palette.text }).hover_background(Color { a: 26, ..palette.text }))
        .accessible(limn::Access::new(limn::Role::Button).name(t!("common-together")))
}

/// The page: out of a room, the two ways in; in one, the room.
pub(super) fn page(state: &State, width: f32, palette: ComponentPalette) -> Element {
    match &state.room {
        Some(room) => in_room(state, room, width, palette),
        None => ways_in(state, width, palette),
    }
}

/// Starting a room, or joining one by its code: two cards side by side where there is room.
fn ways_in(state: &State, width: f32, palette: ComponentPalette) -> Element {
    let raised = ComponentPalette { surface: CHOSEN, ..palette };
    let inner = (width - 24.0).clamp(240.0, WIDEST);
    let mut sections = Vec::new();
    if !state.mobile {
        sections.push(Element::column(vec![
            Text::new(t!("common-together"), palette).font_size(HERO_TITLE).render().font_family(FontFamily::SansBold),
            Text::new(t!("common-together-detail"), palette).muted().render(),
        ]).gap(8.0));
    }
    let card = |parts: Vec<Element>| Element::panel(Element::column(parts).gap(12.0)).padding(24.0).background(CHOSEN).radius(RadiusRole::Card.px());
    let title = |words: std::borrow::Cow<'static, str>| Text::new(words, raised).font_size(22.0).render().font_family(FontFamily::SansBold);
    // A few faces where the room will gather.
    let gathered = faces(&["Ana", "Luz", "Teo"], 44.0, 3, CHOSEN, raised);
    let host = card(vec![
        gathered,
        Element::spacer().size(None, Some(4.0)),
        title(t!("common-together-host")),
        Text::new(t!("common-together-host-detail"), raised).muted().render(),
        Element::spacer().size(None, Some(4.0)),
        Element::row(vec![card_button("room-host", t!("common-together-host-go"), true).icon(Icon::Custom(glyph::TOGETHER)).render()]),
    ]);
    let mut join = vec![
        Element::panel(Element::icon(Icon::Custom(glyph::LISTEN)).large_icon(24.0).foreground(palette.text).centered()).background(palette.selected).radius(22.0).size(Some(44.0), Some(44.0)),
        Element::spacer().size(None, Some(4.0)),
        title(t!("common-together-join")),
        Text::new(t!("common-together-join-detail"), raised).muted().render(),
        Element::spacer().size(None, Some(4.0)),
        Element::row(vec![
            TextField::new("room-code", &state.room_code, "XXXX-XXXX-XXXX", raised).label(t!("common-code")).render().flex(1.0),
            card_button("room-join", t!("common-together-join-go"), true).enabled(!state.room_code.trim().is_empty()).render().centered_y(),
        ]).gap(8.0),
    ];
    if let Some(why) = &state.room_error {
        join.push(Alert::new(t!("common-together-join"), why, palette).id("room-error").compact(true).render());
    }
    let join = card(join);
    sections.push(if !state.mobile && inner >= SIDE_BY_SIDE {
        Element::row(vec![host.flex(1.0), join.flex(1.0)]).gap(16.0)
    } else {
        Element::column(vec![host, join]).gap(16.0)
    });
    page_scroll("together", Element::column(vec![Element::column(sections).gap(24.0).size(Some(inner), None).centered_x()]).padding(12.0))
}

/// In a room: its people and the way to invite more, what comes next and who added it, and
/// what is said and felt in it.
fn in_room(state: &State, room: &RoomView, width: f32, palette: ComponentPalette) -> Element {
    let raised = ComponentPalette { surface: CHOSEN, ..palette };
    let inner = (width - 24.0).clamp(240.0, WIDEST);
    let wide = !state.mobile && inner >= SIDE_BY_SIDE;
    // The room's head, washed in its host's colour: the faces, its name, how it stands, and
    // what can be done in it.
    let host = room.members.iter().find(|member| member.host).map_or("", |member| member.name.as_str());
    let mut actions = vec![card_button("room-invite", t!("common-together-invite"), true).icon(Icon::Custom(glyph::TOGETHER)).render()];
    actions.push(card_button("room-copy", t!("common-together-copy"), false).icon(Icon::Copy).render());
    actions.push(Element::spacer().flex(1.0));
    actions.push(card_button("room-leave", if room.hosting { t!("common-together-close") } else { t!("common-together-leave") }, false).render());
    let mut head = vec![
        faces(&names(room), if wide { 56.0 } else { 44.0 }, 6, shade(name_colour(host), 0.55), raised),
        Element::column(vec![
            Text::new(room_name(room), palette).font_size(if wide { 40.0 } else { 28.0 }).truncate().render().font_family(FontFamily::SansBold),
            Text::new(phase(room), palette).font_size(14.0).render().foreground(SUBDUED),
        ]).gap(4.0),
        Element::row(actions).gap(8.0).wrap().line_gap(8.0),
    ];
    if room.hosting {
        head.push(mobile::settings::switch(&t!("common-together-open"), Some(&t!("common-together-open-detail")), "room-open", room.open));
    } else {
        head.push(Text::new(if room.open { t!("common-together-guest-open") } else { t!("common-together-guest-suggest") }, palette).role(TextRole::Caption).render().foreground(SUBDUED));
    }
    let wash = name_colour(host);
    let head = Element::panel(Element::column(head).gap(16.0)).padding(24.0).radius(RadiusRole::Card.px())
        .vertical_gradient(vec![shade(wash, 0.45), shade(wash, 0.75)]);
    let queue = queue_card(state, room, raised);
    let activity = activity_card(state, room, raised);
    let body = if wide {
        Element::row(vec![queue.flex(1.0), activity.flex(1.0)]).gap(16.0)
    } else {
        Element::column(vec![activity, queue]).gap(16.0)
    };
    page_scroll("together", Element::column(vec![Element::column(vec![head, body]).gap(16.0).size(Some(inner), None).centered_x()]).padding(12.0))
}

/// What comes next in the room, each with the face of who added it.
fn queue_card(state: &State, room: &RoomView, raised: ComponentPalette) -> Element {
    // The host's own queue is the room's; a guest hears it from the host.
    let next: &[Track] = if room.hosting { &state.player.upcoming } else { &room.next };
    let mut parts = vec![Text::new(t!("common-together-queue"), raised).font_size(18.0).render().font_family(FontFamily::SansBold)];
    if next.is_empty() {
        parts.push(Text::new(t!("common-together-queue-empty"), raised).muted().render());
    }
    parts.extend(next.iter().take(10).enumerate().map(|(index, track)| {
        let mut row = ListRow::new(format!("room-next:{index}"), &track.title, raised)
            .sizing(TRACK_ROW)
            .leading(middle(cover(state, &track.artwork_url, COVER, raised)))
            .subtitle(&track.artist)
            .title_fade(true)
            .radius(RadiusRole::Card);
        if let Some(name) = adder(room, &track.id) {
            row = row.trailing(limn::Tooltip::new(face(name, 24.0), format!("room-added:{index}"), t!("common-together-added-by", name = name), raised).render().centered_y());
        }
        row.render()
    }));
    Element::panel(Element::column(parts).gap(8.0)).padding(20.0).background(CHOSEN).radius(RadiusRole::Card.px())
}

/// What is said and felt in the room, the latest last: messages beside their face, who came
/// in and what they added said quietly, reactions large; under it the quick reactions and
/// the field to say something.
fn activity_card(state: &State, room: &RoomView, raised: ComponentPalette) -> Element {
    let mut parts = vec![Text::new(t!("common-together-activity"), raised).font_size(18.0).render().font_family(FontFamily::SansBold)];
    if room.chat.is_empty() {
        parts.push(Text::new(t!("common-together-chat-empty"), raised).muted().render());
    }
    parts.extend(room.chat.iter().rev().take(40).rev().map(|line| said(line, raised)));
    let reactions = REACTIONS.iter().map(|emoji| {
        Button::new(format!("room-react:{emoji}"), *emoji, raised).variant(ButtonVariant::Secondary).size(ControlSize::Regular).round().render()
    }).collect();
    parts.push(Element::spacer().size(None, Some(4.0)));
    parts.push(Element::row(reactions).gap(6.0).wrap().line_gap(6.0).accessible(limn::Access::new(limn::Role::Group).name(t!("common-together-react"))));
    parts.push(Element::row(vec![
        TextField::new("room-chat", &state.room_chat, "", raised).label(t!("common-together-chat-placeholder")).render().flex(1.0),
        card_button("room-send", t!("common-together-send"), true).enabled(!state.room_chat.trim().is_empty()).render().centered_y(),
    ]).gap(8.0));
    Element::panel(Element::column(parts).gap(10.0)).padding(20.0).background(CHOSEN).radius(RadiusRole::Card.px())
}

/// A line of the room's activity.
fn said(line: &Line, raised: ComponentPalette) -> Element {
    let who = if line.mine { t!("common-together-you").into_owned() } else { line.name.clone() };
    match line.said {
        Said::Chat => Element::row(vec![
            face(&line.name, 28.0),
            Element::column(vec![
                Text::new(who, raised).font_size(13.0).render().font_family(FontFamily::SansBold),
                Text::new(&line.text, raised).render(),
            ]).gap(2.0).flex(1.0),
        ]).gap(10.0),
        Said::Reacted => Element::row(vec![
            face(&line.name, 28.0).centered_y(),
            Text::new(who, raised).font_size(13.0).render().font_family(FontFamily::SansBold).centered_y(),
            Element::text(line.text.clone()).font_size(24.0).foreground(raised.text).centered_y(),
        ]).gap(10.0),
        Said::Joined => Text::new(t!("common-together-joined", name = who.as_str()), raised).role(TextRole::Caption).muted().render().center_content(),
        Said::Suggested => Element::row(vec![
            face(&line.name, 20.0).centered_y(),
            Text::new(t!("common-together-suggested", name = who.as_str(), title = line.text.as_str()), raised).role(TextRole::Caption).muted().render().flex(1.0).centered_y(),
        ]).gap(8.0),
    }
}

/// The room in the panel of what plays: its faces and name, the last of what was said, and
/// the way to the room.
pub(super) fn room_panel(state: &State, width: f32, palette: ComponentPalette) -> Option<Element> {
    let room = state.room.as_ref()?;
    let raised = ComponentPalette { surface: CHOSEN, ..palette };
    let open = Link::new("together", t!("common-together-open-room"), raised).font_size(12.0).render();
    let head = Element::row(vec![
        faces(&names(room), 28.0, 4, CHOSEN, raised).centered_y(),
        Element::column(vec![
            Text::new(room_name(room), raised).font_size(15.0).truncate().render().font_family(FontFamily::SansBold),
            Text::new(t!("common-together-listening", count = room.members.len().max(1)), raised).role(TextRole::Caption).muted().render(),
        ]).gap(2.0).flex(1.0).centered_y(),
    ]).gap(12.0);
    let mut parts = vec![head];
    parts.extend(room.chat.iter().rev().take(2).rev().map(|line| said(line, raised)));
    parts.push(Element::row(vec![open, Element::spacer().flex(1.0)]));
    Some(super::panel_card(parts, width))
}

/// The room as a pill in the player's bar: its faces and how many are in it; it opens the room.
pub(super) fn bar_pill(state: &State, palette: ComponentPalette) -> Option<Element> {
    let room = state.room.as_ref()?;
    let count = room.members.len().max(1);
    let pill = Element::panel(Element::row(vec![
        faces(&names(room), 20.0, 3, CANVAS, palette).centered_y(),
        Text::new(count.to_string(), palette).font_size(13.0).render().font_family(FontFamily::SansBold).centered_y(),
    ]).gap(6.0)).padding(4.0).background(Color { a: 26, ..palette.text }).hover_background(Color { a: 46, ..palette.text }).round();
    let said = t!("common-together-in-room", count = count);
    Some(limn::Tooltip::new(Element::clickable("together", pill).accessible(limn::Access::new(limn::Role::Button).name(said.clone())), "room-pill", said, palette).render().centered_y())
}

/// The card that invites others: the room's code, large, and the way to copy it.
pub(super) fn invite(state: &State, window_tree: Element, width: f32) -> Element {
    let Some(room) = state.room.as_ref().filter(|_| state.room_invite) else { return window_tree };
    let palette = Card::palette(GREY_CARD);
    let code = Element::panel(Element::text(&room.code).font_family(FontFamily::SansBold).font_size(34.0).foreground(palette.text).centered())
        .background(palette.selected).radius(RadiusRole::Card.px()).padding(16.0);
    let copy = card_button("room-copy", t!("common-together-copy"), false).icon(Icon::Copy).render().centered_x();
    let body = Element::column(vec![faces(&names(room), 40.0, 6, GREY_CARD.surface, palette).centered_x(), code, copy]).gap(14.0);
    Card { body: Some(body), widest: 440.0, returns: Some("room-invite"),
        ..Card::new("room-invite", t!("common-together-invite-title"), t!("common-together-invite-detail"), ("room-invite-close", t!("common-close"))) }
        .render(window_tree, width)
}
