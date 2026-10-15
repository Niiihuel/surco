//! Scroll content beneath the phone's floating header and transport bars.

use super::{Color, Element, SCROLL_INDICATOR};
#[cfg(test)]
use super::SCHEME;
use limn::ElementKind;

const TOP_FADE: f32 = 20.0;
const BOTTOM_FADE: f32 = 24.0;

/// Keep scroll viewports beneath the bars, while their first/last rows clear them.
/// A Column containing only Scroll children keeps its layout and paint properties.
/// A Column's fixed controls stay in flow; its direct Scroll gets only the bottom inset.
/// Programmatic reveals must account for the overlaid bars: plain `reveal` reserves
/// only 12px. Use `reveal_at_start` with `top` (a direct Scroll), or TOP_FADE below
/// fixed controls; `scroll_to_end` already includes the bottom content spacer.
pub(super) fn under_bars(mut content: Element, top: f32, bottom: f32, header_edge: Color, footer_edge: Color) -> Element {
    let top = top.max(0.0);
    let bottom = bottom.max(0.0);
    match &mut content.kind {
        ElementKind::Scroll { .. } => {
            scroll_insets(&mut content, top, bottom, header_edge, footer_edge);
            content
        }
        ElementKind::Column(children)
            if !children.is_empty()
                && children
                    .iter()
                    .all(|child| matches!(child.kind, ElementKind::Scroll { .. })) =>
        {
            for child in children {
                scroll_insets(child, top, bottom, header_edge, footer_edge);
            }
            content
        }
        ElementKind::Column(children)
            if children
                .iter()
                .any(|child| matches!(child.kind, ElementKind::Scroll { .. })) =>
        {
            for child in children
                .iter_mut()
                .filter(|child| matches!(child.kind, ElementKind::Scroll { .. }))
            {
                scroll_insets(child, 0.0, bottom, header_edge, footer_edge);
            }
            // Keep the original gap between controls, without adding it after the top inset.
            let body = Element::column(std::mem::take(children))
                .gap(content.style.gap)
                .flex(1.0);
            *children = vec![space(top), body];
            content.style.gap = 0.0;
            content
        }
        _ => Element::column(vec![space(top), content.flex(1.0), space(bottom)]).flex(1.0),
    }
}

fn space(height: f32) -> Element {
    Element::spacer().size(None, Some(height))
}

fn scroll_insets(scroll: &mut Element, top: f32, bottom: f32, header_edge: Color, footer_edge: Color) {
    let ElementKind::Scroll { child, .. } = &mut scroll.kind else {
        return;
    };
    let body = std::mem::replace(child.as_mut(), Element::spacer());
    **child = Element::column(vec![space(top), body, space(bottom)]);
    // The whole transition must sit beside the opaque chrome. A gradient
    // spanning the bar's height spends most of its opacity behind the bar,
    // leaving only a faint sliver visible at the content edge.
    scroll.style.top_fade = Some((header_edge, TOP_FADE));
    scroll.style.bottom_fade = (bottom > 0.0).then_some((footer_edge, BOTTOM_FADE));
    let element = std::mem::replace(scroll, Element::spacer());
    *scroll = element.fade_insets(top, bottom).scrollbar_visibility(SCROLL_INDICATOR);
    if let ElementKind::Scroll { appearance, .. } = &mut scroll.kind {
        *appearance = limn::ScrollbarAppearance::overlay().buttons(false).rail_width(4.0).thumb_width(2.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use limn::{DrawCommand, Frame, HitKind, Point, Rect, Size, UiAction, UiEvent, UiTree};

    const TOP: f32 = 72.0;
    const BOTTOM: f32 = 80.0;

    fn list() -> Element {
        Element::scroll(
            "body",
            Element::column(
                (0..20)
                    .map(|index| {
                        Element::button(format!("row:{index}"), format!("Song {index}"))
                            .size(None, Some(44.0))
                    })
                    .collect(),
            )
            .padding(8.0),
        )
        .flex(1.0)
    }

    fn tree(fixed_controls: bool) -> UiTree {
        let body = if fixed_controls {
            Element::column(vec![
                Element::button("filters", "Filters").size(None, Some(44.0)),
                list(),
            ])
            .gap(6.0)
            .padding(8.0)
        } else {
            list()
        };
        let bars = Element::column(vec![
            Element::button("header", "Header").size(None, Some(TOP)),
            Element::spacer().flex(1.0),
            Element::button("footer", "Transport").size(None, Some(BOTTOM)),
        ]);
        let mut tree = UiTree::new(Element::stack(vec![under_bars(body, TOP, BOTTOM, SCHEME.canvas, SCHEME.canvas), bars]));
        tree.set_reduced_motion(true);
        tree
    }

    fn rect(frame: &Frame, id: &str) -> Rect {
        frame
            .regions
            .iter()
            .find(|region| region.id == id)
            .unwrap_or_else(|| panic!("{id} is visible"))
            .rect
    }

    fn clear_of_bars(rect: Rect, size: Size) {
        assert!(
            rect.y >= TOP - 0.01 && rect.y + rect.height <= size.height - BOTTOM + 0.01,
            "row stays outside floating bars: {rect:?} in {size:?}"
        );
        assert!(rect.height >= 43.99);
    }

    #[test]
    fn full_scroll_keeps_its_viewport_and_reaches_both_ends_above_the_bars() {
        for width in [320.0, 390.0, 430.0] {
            let size = Size::new(width, 360.0);
            let mut tree = tree(false);
            let first = tree.layout(size);
            assert_eq!(
                rect(&first, "body"),
                Rect::new(0.0, 0.0, width, size.height)
            );
            assert_eq!(rect(&first, "row:0").y, TOP + 8.0);
            clear_of_bars(rect(&first, "row:0"), size);
            tree.scroll_to_end("body");
            let end = tree.layout(size);
            clear_of_bars(rect(&end, "row:19"), size);
            assert_eq!(rect(&end, "row:19").y + 44.0, size.height - BOTTOM - 8.0);
            tree.reveal_at_start("row:0", TOP);
            tree.layout(size);
            clear_of_bars(rect(&tree.layout(size), "row:0"), size);
            tree.reveal_at_start("row:19", TOP);
            tree.layout(size);
            clear_of_bars(rect(&tree.layout(size), "row:19"), size);
        }
    }

    #[test]
    fn a_lyrics_column_with_only_a_scroll_keeps_its_full_viewport_and_wash() {
        for width in [320.0, 390.0, 430.0] {
            let size = Size::new(width, 360.0);
            let wash = limn::Color::rgb(26, 35, 48);
            let content = Element::column(vec![list()])
                .bottom_background_fade(wash, 120.0)
                .flex(1.0);
            let content = under_bars(content, TOP, BOTTOM, SCHEME.canvas, SCHEME.canvas);
            assert_eq!(content.style.bottom_background_fade, Some((wash, 120.0)));
            let mut tree = UiTree::new(content);
            tree.set_reduced_motion(true);
            let first = tree.layout(size);
            assert_eq!(
                rect(&first, "body"),
                Rect::new(0.0, 0.0, width, size.height)
            );
            assert_eq!(rect(&first, "row:0").y, TOP + 8.0);
            clear_of_bars(rect(&first, "row:0"), size);
            tree.scroll_to_end("body");
            let end = tree.layout(size);
            clear_of_bars(rect(&end, "row:19"), size);
            assert_eq!(rect(&end, "row:19").y + 44.0, size.height - BOTTOM - 8.0);
        }
    }

    #[test]
    fn fixed_controls_stay_below_the_header_without_double_insets_or_a_second_scroll() {
        for width in [320.0, 390.0, 430.0] {
            let size = Size::new(width, 360.0);
            let mut tree = tree(true);
            let frame = tree.layout(size);
            assert_eq!(rect(&frame, "filters").y, TOP + 8.0);
            assert_eq!(rect(&frame, "row:0").y, TOP + 8.0 + 44.0 + 6.0 + 8.0);
            assert_eq!(
                frame
                    .regions
                    .iter()
                    .filter(|region| region.kind == HitKind::Scroll)
                    .count(),
                1
            );
            assert_eq!(
                rect(&frame, "body").y + rect(&frame, "body").height,
                size.height - 8.0
            );
            clear_of_bars(rect(&frame, "row:0"), size);
            tree.scroll_to_end("body");
            let end = tree.layout(size);
            clear_of_bars(rect(&end, "row:19"), size);
            tree.reveal_at_start("row:0", 24.0);
            tree.layout(size);
            clear_of_bars(rect(&tree.layout(size), "row:0"), size);
            tree.reveal_at_start("row:19", 24.0);
            tree.layout(size);
            clear_of_bars(rect(&tree.layout(size), "row:19"), size);
        }
    }

    #[test]
    fn edge_fades_leave_body_taps_to_the_real_rows_and_bars_to_their_buttons() {
        let size = Size::new(390.0, 360.0);
        let mut tree = tree(false);
        tree.layout(size);
        tree.handle_event(UiEvent::ScrollDirect {
            position: Point::new(24.0, 160.0),
            delta_y: -100.0,
        });
        let frame = tree.layout(size);
        assert!(frame.commands.iter().any(|command| matches!(command,
            DrawCommand::FillVerticalGradient { rect, top, bottom }
                if rect.y == TOP && rect.height == TOP_FADE && *top == SCHEME.canvas && bottom.a == 0)),
            "the full top fade is visible below the header");
        assert!(frame.commands.iter().any(|command| matches!(command,
            DrawCommand::FillVerticalGradient { rect, top, bottom }
                if rect.y + rect.height == size.height - BOTTOM && rect.height == BOTTOM_FADE && top.a == 0 && *bottom == SCHEME.canvas)),
            "the full bottom fade is visible above transport");
        assert_eq!(frame.hit_test(Point::new(24.0, TOP / 2.0)), Some("header"));
        assert_eq!(
            frame.hit_test(Point::new(24.0, size.height - BOTTOM / 2.0)),
            Some("footer")
        );
        for point in [
            Point::new(24.0, TOP + 12.0),
            Point::new(24.0, size.height - BOTTOM - 12.0),
        ] {
            let id = frame.hit_test(point).unwrap().to_owned();
            assert!(
                id.starts_with("row:"),
                "the scrim has no pointer target: {id}"
            );
            tree.handle_event(UiEvent::PointerDown(point));
            assert_eq!(
                tree.handle_event(UiEvent::PointerUp(point)),
                vec![UiAction::Clicked { id }]
            );
        }
    }

    #[test]
    fn static_panes_keep_their_action_between_bars_without_a_new_scroll_area() {
        for width in [320.0, 390.0, 430.0] {
            let size = Size::new(width, 360.0);
            let content = Element::column(vec![
                Element::spacer().flex(1.0),
                Element::button("retry", "Volver a intentar").size(None, Some(44.0)),
                Element::spacer().flex(1.0),
            ])
            .flex(1.0);
            let frame = UiTree::new(under_bars(content, TOP, BOTTOM, SCHEME.canvas, SCHEME.canvas)).layout(size);
            assert!(
                !frame
                    .regions
                    .iter()
                    .any(|region| region.kind == HitKind::Scroll)
            );
            clear_of_bars(rect(&frame, "retry"), size);
        }
    }
}
