//! Touch actions with independently sized glyphs and primary playback circles.

use super::*;

pub(in crate::view) fn action(id: &str, glyph: Icon, label: &str, glyph_size: f32, palette: ComponentPalette) -> Element {
    // IconButton keeps its canonical 16-point glyph. A standalone icon lets
    // phone actions grow while retaining one semantic button and touch target.
    Element::clickable(id, Element::icon(glyph).large_icon(glyph_size).foreground(palette.text).centered())
        .accessible(limn::Access::new(limn::Role::Button).name(label))
        .background(Color::TRANSPARENT).hover_background(palette.hover).pressed_background(palette.selected)
        .transition_ms(limn::tokens().motion.hover_ms).radius(RadiusRole::Control.px())
        .size(Some(TAP), Some(TAP))
}

pub(in crate::view) fn playback(id: &str, playing: bool, diameter: f32, glyph_size: f32, palette: ComponentPalette) -> Element {
    let glyph = if playing { Icon::PauseFill } else { Icon::PlayFill };
    let circle = Element::panel(Element::icon(glyph).large_icon(glyph_size).foreground(SCHEME.canvas).centered())
        .background(palette.text).hover_background(ROUND_HOVER).pressed_background(ROUND_HELD)
        .transition_ms(limn::tokens().motion.hover_ms)
        .hover_surface(format!("{id}:circle"), [id])
        .radius(diameter / 2.0).size(Some(diameter), Some(diameter));
    let hit = diameter.max(TAP);
    Element::clickable(id, circle.centered())
        .accessible(limn::Access::new(limn::Role::Button).name(if playing { "Pausar" } else { "Reproducir" }))
        .radius(hit / 2.0).size(Some(hit), Some(hit))
}

#[cfg(test)]
mod tests {
    use super::*;
    use limn::{DrawCommand, Key, Point, UiAction, UiEvent, UiTree};

    #[test]
    fn phone_action_glyphs_grow_without_changing_their_touch_target_or_dispatch() {
        for glyph_size in [22.0, 24.0, 28.0] {
            let mut tree = UiTree::new(action("visible-action", Icon::SkipForward, "Siguiente canción", glyph_size, SCHEME.palette()).dispatch_as("next"));
            tree.set_reduced_motion(true);
            let frame = tree.layout(Size::new(TAP, TAP));
            let target = frame.access.iter().find(|node| node.id == "visible-action").unwrap();
            assert_eq!((target.rect.width, target.rect.height), (44.0, 44.0));
            assert_eq!(target.name, "Siguiente canción");
            assert!(frame.commands.iter().any(|command| matches!(command,
                DrawCommand::Icon { icon: Icon::SkipForward, rect, .. }
                if rect.width == glyph_size && rect.height == glyph_size)));
            let point = Point::new(1.0, 1.0);
            tree.handle_event(UiEvent::PointerDown(point));
            assert_eq!(tree.handle_event(UiEvent::PointerUp(point)), vec![UiAction::Clicked { id: "next".into() }]);
            tree.handle_event(UiEvent::Key { key: Key::Tab, shift: false, ctrl: false });
            assert_eq!(tree.handle_event(UiEvent::Key { key: Key::Enter, shift: false, ctrl: false }),
                vec![UiAction::Clicked { id: "next".into() }]);
        }
    }

    #[test]
    fn playback_has_one_white_circle_and_dark_live_glyph_inside_a_full_touch_target() {
        for (diameter, glyph_size) in [(32.0_f32, 18.0), (44.0, 22.0), (56.0, 28.0), (72.0, 32.0)] {
            for playing in [false, true] {
                let hit = diameter.max(TAP);
                let mut tree = UiTree::new(playback("primary", playing, diameter, glyph_size, SCHEME.palette()).dispatch_as("play-pause"));
                tree.set_reduced_motion(true);
                let frame = tree.layout(Size::new(hit, hit));
                let targets: Vec<_> = frame.access.iter().filter(|node| node.id == "primary").collect();
                assert_eq!(targets.len(), 1);
                assert_eq!(targets[0].name, if playing { "Pausar" } else { "Reproducir" });
                assert_eq!((targets[0].rect.width, targets[0].rect.height), (hit, hit));
                assert_eq!(frame.regions.iter().filter(|region| region.id == "primary").count(), 1);
                let circles: Vec<_> = frame.commands.iter().filter_map(|command| match command {
                    DrawCommand::FillRect { rect, color, radius } if *color == Color::WHITE => Some((*rect, *radius)),
                    _ => None,
                }).collect();
                assert_eq!(circles.len(), 1, "one visible primary circle");
                assert_eq!((circles[0].0.width, circles[0].0.height, circles[0].1), (diameter, diameter, diameter / 2.0));
                assert_eq!(circles[0].0.x, (hit - diameter) / 2.0);
                assert_eq!(circles[0].0.y, (hit - diameter) / 2.0);
                let glyph = if playing { Icon::PauseFill } else { Icon::PlayFill };
                assert!(frame.commands.iter().any(|command| matches!(command,
                    DrawCommand::Icon { rect, icon, tint } if *icon == glyph && *tint == SCHEME.canvas
                    && rect.width == glyph_size && rect.height == glyph_size)));
                // The mini player's 32-point circle still responds throughout
                // its 44-point target, including outside the visible disc.
                let point = Point::new(1.0, 1.0);
                tree.handle_event(UiEvent::PointerDown(point));
                assert_eq!(tree.handle_event(UiEvent::PointerUp(point)), vec![UiAction::Clicked { id: "play-pause".into() }]);
            }
        }
    }
}
