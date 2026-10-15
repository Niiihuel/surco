//! Touch panning and the platform keyboard for the limn host.

use std::collections::VecDeque;

use eframe::egui::{self, Pos2};
use limn::{Frame, HitKind, Point, Role, UiEvent, UiTree};

const PAN_THRESHOLD: f32 = 8.0;

#[derive(Default)]
pub struct TouchInput {
    primary: Option<Touch>,
}

struct Touch {
    device: egui::TouchDeviceId,
    id: egui::TouchId,
    start: Pos2,
    last: Pos2,
    position: Pos2,
    can_pan: bool,
    axis: Option<Axis>,
}

#[derive(Clone, Copy)]
enum Axis {
    Horizontal,
    Vertical,
}

impl Touch {
    fn pan(&mut self, position: Pos2, origin: Pos2) -> Option<Vec<UiEvent>> {
        if !self.can_pan || !position.x.is_finite() || !position.y.is_finite() {
            return None;
        }
        let travel = position - self.start;
        let mut events = Vec::new();
        let delta = if self.axis.is_none() {
            if travel.length() < PAN_THRESHOLD {
                return None;
            }
            self.axis = Some(if travel.x.abs() > travel.y.abs() {
                Axis::Horizontal
            } else {
                Axis::Vertical
            });
            // End the tree's original press before scrolling beneath that row.
            events.push(UiEvent::PointerLeave);
            travel
        } else {
            position - self.last
        };
        self.last = position;
        let (delta_x, delta_y) = match self.axis {
            Some(Axis::Horizontal) => (delta.x, 0.0),
            _ => (0.0, delta.y),
        };
        if delta_x != 0.0 || delta_y != 0.0 {
            events.push(UiEvent::Scroll2D {
                // Keep the gesture attached to the viewport where it began.
                position: relative(self.start, origin),
                delta_x,
                delta_y,
                direct: true,
                shift: false,
            });
        }
        Some(events)
    }
}

impl TouchInput {
    /// Adapt the events returned by `limn_egui::events` using the same frame's
    /// raw egui events. egui-winit already synthesizes a pointer for its first
    /// touch: keep taps and control drags on that path, without sending them twice.
    ///
    /// `is_drag_target` identifies application drag sources. limn's public Frame
    /// exposes slider roles, but does not expose its retained drag-source set.
    pub fn translate(
        &mut self,
        raw: &[egui::Event],
        translated: Vec<UiEvent>,
        origin: Pos2,
        frame: &Frame,
        is_drag_target: impl Fn(&str) -> bool,
    ) -> Vec<UiEvent> {
        if self.primary.is_none()
            && !raw
                .iter()
                .any(|event| matches!(event, egui::Event::Touch { .. }))
        {
            return translated;
        }
        // Each native pointer event has one corresponding translated pointer
        // event. Plans preserve its place among keyboard, clipboard and IME input.
        let mut plans = VecDeque::new();
        for event in raw {
            match event {
                egui::Event::Touch {
                    device_id,
                    id,
                    phase,
                    pos,
                    ..
                } => {
                    if *phase == egui::TouchPhase::Start && self.primary.is_none() {
                        self.primary = Some(Touch {
                            device: *device_id,
                            id: *id,
                            start: *pos,
                            last: *pos,
                            position: *pos,
                            can_pan: can_pan(frame, relative(*pos, origin), &is_drag_target),
                            axis: None,
                        });
                    } else if let Some(touch) = self
                        .primary
                        .as_mut()
                        .filter(|touch| touch.device == *device_id && touch.id == *id)
                    {
                        touch.position = *pos;
                    }
                }
                egui::Event::PointerMoved(position) => {
                    plans.push_back(
                        self.primary
                            .as_mut()
                            .and_then(|touch| touch.pan(*position, origin)),
                    );
                }
                egui::Event::PointerButton {
                    button: egui::PointerButton::Primary,
                    pressed,
                    ..
                } => {
                    let plan = if !pressed {
                        self.primary
                            .as_mut()
                            .and_then(|touch| touch.pan(touch.position, origin))
                    } else {
                        None
                    };
                    plans.push_back(plan);
                }
                egui::Event::PointerGone => {
                    self.primary = None;
                    plans.push_back(None);
                }
                egui::Event::WindowFocused(false) => self.primary = None,
                _ => {}
            }
        }
        let mut events = Vec::with_capacity(translated.len());
        for event in translated {
            if matches!(
                event,
                UiEvent::PointerMove(_)
                    | UiEvent::PointerLeave
                    | UiEvent::PointerDown(_)
                    | UiEvent::PointerDownModified { .. }
                    | UiEvent::PointerDownEditor { .. }
                    | UiEvent::PointerUp(_)
            ) {
                if let Some(Some(replacement)) = plans.pop_front() {
                    events.extend(replacement);
                    continue;
                }
            }
            events.push(event);
        }
        events
    }
}

/// Gate system clipboard reads on the field that requested them, before touching UIKit.
#[cfg(any(target_os = "ios", test))]
pub fn can_paste(tree: &UiTree, frame: &Frame, target: &str) -> bool {
    !tree.context_menu_open()
        && tree.focused_id() == Some(target)
        && frame.regions.iter().any(|region| region.id == target
            && matches!(region.kind, HitKind::TextInput | HitKind::TextArea | HitKind::CodeEditor))
        && !frame.access.iter().any(|node| node.id == target && node.disabled)
}

fn relative(position: Pos2, origin: Pos2) -> Point {
    Point::new(position.x - origin.x, position.y - origin.y)
}

fn can_pan(frame: &Frame, position: Point, is_drag_target: &impl Fn(&str) -> bool) -> bool {
    if let Some(hit) = frame.press_region(position) {
        if matches!(
            hit.kind,
            HitKind::TextInput
                | HitKind::TextArea
                | HitKind::CodeEditor
                | HitKind::CodeDiff
                | HitKind::Scrollbar
        ) || is_drag_target(&hit.id)
            || frame
                .access
                .iter()
                .any(|node| node.id == hit.id && node.role == Role::Slider)
        {
            return false;
        }
    }
    for region in frame
        .regions
        .iter()
        .rev()
        .filter(|region| region.rect.contains(position))
    {
        match region.kind {
            HitKind::Scroll => return true,
            // A floating surface must not scroll the page behind it.
            HitKind::Surface => return false,
            _ => {}
        }
    }
    false
}

/// Call after the current layout. Native integrations use IMEOutput to open
/// the software keyboard; painted text fields alone do not request it.
pub fn update_ime(ctx: &egui::Context, tree: &UiTree, frame: &Frame, origin: Pos2) {
    let ime = if !ctx.input(|input| input.focused) {
        None
    } else if let Some((rect, caret)) = tree.editor_ime_rects() {
        Some((
            screen_rect(rect, origin),
            screen_rect(caret, origin),
            egui::IMEPurpose::Normal,
        ))
    } else {
        tree.focused_id().and_then(|id| {
            let region = frame.regions.iter().rev().find(|region| {
                region.id == id && matches!(region.kind, HitKind::TextInput | HitKind::TextArea)
            })?;
            let node = frame.access.iter().find(|node| node.id == id);
            if node.is_some_and(|node| node.disabled) {
                return None;
            }
            let purpose = if node.is_some_and(|node| node.role == Role::PasswordInput) {
                egui::IMEPurpose::Password
            } else {
                egui::IMEPurpose::Normal
            };
            let rect = screen_rect(region.rect, origin);
            // shortcut: limn exposes simple-field bounds, not their caret geometry;
            // anchor candidate UI inside the field until it publishes that geometry.
            let caret = egui::Rect::from_min_size(
                rect.left_bottom() - egui::vec2(0.0, 1.0),
                egui::vec2(1.0, 1.0),
            );
            Some((rect, caret, purpose))
        })
    };
    ctx.output_mut(|output| {
        output.mutable_text_under_cursor = ime.is_some();
        output.ime = ime.map(|(rect, cursor_rect, purpose)| egui::output::IMEOutput {
            purpose,
            rect,
            cursor_rect,
            should_interrupt_composition: false,
        });
    });
}

fn screen_rect(rect: limn::Rect, origin: Pos2) -> egui::Rect {
    egui::Rect::from_min_size(
        origin + egui::vec2(rect.x, rect.y),
        egui::vec2(rect.width, rect.height),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use limn::{Component, Element, Size, UiAction};

    #[test]
    fn clipboard_reads_require_the_original_focused_visible_field() {
        let size = Size::new(320.0, 200.0);
        let mut tree = UiTree::new(Element::column(vec![
            Element::text_input("original", "", "Search").size(None, Some(44.0)),
            Element::text_input("other", "", "Other").size(None, Some(44.0)),
        ]));
        let frame = tree.layout(size);
        assert!(!can_paste(&tree, &frame, "original"));
        tree.focus("original");
        let frame = tree.layout(size);
        assert!(can_paste(&tree, &frame, "original"));
        assert_eq!(tree.handle_event(UiEvent::PasteTo { id: "original".into(), text: "ñ 東京".into() }),
            vec![UiAction::InputChanged { id: "original".into(), value: "ñ 東京".into() }]);
        assert_eq!(tree.input_value("original"), Some("ñ 東京"));
        tree.handle_event(UiEvent::ContextMenuKeyboard);
        assert!(tree.context_menu_open());
        let frame = tree.layout(size);
        assert!(!can_paste(&tree, &frame, "original"));
        tree.dismiss_context_menu();
        tree.focus("other");
        let frame = tree.layout(size);
        assert!(!can_paste(&tree, &frame, "original"));
        assert!(tree.handle_event(UiEvent::PasteTo { id: "original".into(), text: "ñ 東京".into() }).is_empty());
        assert_eq!(tree.input_value("other"), Some(""));
        tree.set_root(Element::spacer());
        let frame = tree.layout(size);
        assert!(!can_paste(&tree, &frame, "other"));
    }

    // The order egui-winit emits, including its emulated primary pointer.
    fn touch(phase: egui::TouchPhase, position: Pos2) -> Vec<egui::Event> {
        let mut events = vec![egui::Event::Touch {
            device_id: egui::TouchDeviceId(1),
            id: egui::TouchId(1),
            phase,
            pos: position,
            force: None,
        }];
        match phase {
            egui::TouchPhase::Start => {
                events.push(egui::Event::PointerMoved(position));
                events.push(egui::Event::PointerButton {
                    pos: position,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: Default::default(),
                });
            }
            egui::TouchPhase::Move => events.push(egui::Event::PointerMoved(position)),
            egui::TouchPhase::End => {
                events.push(egui::Event::PointerButton {
                    pos: position,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: Default::default(),
                });
                events.push(egui::Event::PointerGone);
            }
            egui::TouchPhase::Cancel => events.push(egui::Event::PointerGone),
        }
        events
    }

    fn translated(raw: &[egui::Event]) -> Vec<UiEvent> {
        let ctx = egui::Context::default();
        let mut events = Vec::new();
        let mut output = ctx.run_ui(
            egui::RawInput {
                events: raw.to_vec(),
                ..Default::default()
            },
            |ui| {
                events = limn_egui::events(ui.ctx(), Pos2::ZERO);
            },
        );
        output.textures_delta.clear();
        events
    }

    fn send(input: &mut TouchInput, tree: &mut UiTree, raw: &[egui::Event]) -> Vec<UiAction> {
        let frame = tree.layout(Size::new(240.0, 160.0));
        input
            .translate(raw, translated(raw), Pos2::ZERO, &frame, |id| id == "drag")
            .into_iter()
            .flat_map(|event| tree.handle_event(event))
            .collect()
    }

    #[test]
    fn a_context_hold_and_touch_pan_share_the_host_without_playing_on_release() {
        let root = || {
            let row = limn::ContextMenu::new("actions-row", Element::button("play-row", "A song").size(Some(200.0), Some(80.0)),
                vec![limn::ContextMenuEntry::Item(limn::ContextMenuItem::new("more-row", "Opciones"))], limn::ComponentPalette::neutral_dark())
                .open_action("more-row").render();
            Element::scroll("songs", Element::column(vec![row, Element::spacer().size(None, Some(500.0))]))
                .size(Some(240.0), Some(160.0))
        };
        let run = |ctx: &egui::Context, tree: &mut UiTree, input: &mut TouchInput, raw: Vec<egui::Event>, time: f64| {
            let frame = tree.layout(Size::new(240.0, 160.0));
            let mut actions = Vec::new();
            let mut output = ctx.run_ui(egui::RawInput { events: raw.clone(), time: Some(time), ..Default::default() }, |ui| {
                let events = limn_egui::events_with_touch(ui.ctx(), Pos2::ZERO, &frame);
                for event in input.translate(&raw, events, Pos2::ZERO, &frame, |_| false) {
                    actions.extend(tree.handle_event(event));
                }
            });
            output.textures_delta.clear();
            actions
        };
        let down = Pos2::new(30.0, 40.0);
        let ctx = egui::Context::default();
        let mut tree = UiTree::new(root());
        let mut input = TouchInput::default();
        assert!(run(&ctx, &mut tree, &mut input, touch(egui::TouchPhase::Start, down), 0.0).is_empty());
        assert_eq!(run(&ctx, &mut tree, &mut input, Vec::new(), 0.5), vec![UiAction::Clicked { id: "more-row".into() }]);
        assert!(run(&ctx, &mut tree, &mut input, touch(egui::TouchPhase::End, down), 0.6).is_empty());

        let ctx = egui::Context::default();
        let mut tree = UiTree::new(root());
        let mut input = TouchInput::default();
        run(&ctx, &mut tree, &mut input, touch(egui::TouchPhase::Start, down), 0.0);
        let moved = down + egui::vec2(0.0, -12.0);
        assert!(run(&ctx, &mut tree, &mut input, touch(egui::TouchPhase::Move, moved), 0.1).is_empty());
        assert!(run(&ctx, &mut tree, &mut input, Vec::new(), 0.6).is_empty());
        assert!(run(&ctx, &mut tree, &mut input, touch(egui::TouchPhase::End, moved), 0.7).is_empty());
    }

    #[test]
    fn a_tap_clicks_once_but_panning_cancels_the_pressed_row() {
        let root = Element::scroll(
            "list",
            Element::column(vec![
                Element::button("row", "A row").size(Some(200.0), Some(100.0)),
                Element::spacer().size(None, Some(500.0)),
            ]),
        )
        .size(Some(240.0), Some(160.0));
        let mut tree = UiTree::new(root);
        let mut input = TouchInput::default();
        let down = Pos2::new(40.0, 60.0);
        send(&mut input, &mut tree, &touch(egui::TouchPhase::Start, down));
        let actions = send(&mut input, &mut tree, &touch(egui::TouchPhase::End, down));
        assert_eq!(
            actions
                .iter()
                .filter(|action| matches!(action, UiAction::Clicked { id } if id == "row"))
                .count(),
            1
        );

        send(&mut input, &mut tree, &touch(egui::TouchPhase::Start, down));
        let mut actions = send(
            &mut input,
            &mut tree,
            &touch(egui::TouchPhase::Move, down - egui::vec2(0.0, 30.0)),
        );
        actions.extend(send(
            &mut input,
            &mut tree,
            &touch(egui::TouchPhase::End, down - egui::vec2(0.0, 30.0)),
        ));
        assert!(
            !actions
                .iter()
                .any(|action| matches!(action, UiAction::Clicked { .. }))
        );
        assert!(tree.scroll_offset("list") > 0.0);
    }

    #[test]
    fn slider_drag_source_and_text_selection_keep_the_pointer_events() {
        for control in [
            Element::clickable("slider", Element::spacer()).slider(0.5, 0, 0.0),
            Element::button("drag", "Reorder").draggable(),
            Element::text_input("query", "some text", "Search"),
        ] {
            let root = Element::scroll(
                "list",
                Element::column(vec![
                    control.size(Some(200.0), Some(60.0)),
                    Element::spacer().size(None, Some(500.0)),
                ]),
            )
            .size(Some(240.0), Some(160.0));
            let mut tree = UiTree::new(root);
            let frame = tree.layout(Size::new(240.0, 160.0));
            let mut input = TouchInput::default();
            for (phase, position) in [
                (egui::TouchPhase::Start, Pos2::new(30.0, 30.0)),
                (egui::TouchPhase::Move, Pos2::new(150.0, 35.0)),
                (egui::TouchPhase::End, Pos2::new(150.0, 35.0)),
            ] {
                let raw = touch(phase, position);
                let expected = translated(&raw);
                assert_eq!(
                    input.translate(&raw, expected.clone(), Pos2::ZERO, &frame, |id| id
                        == "drag"),
                    expected
                );
            }
        }
    }

    #[test]
    fn passive_header_and_scrim_surfaces_do_not_pan_the_underlying_list() {
        let mut frame = Frame::default();
        frame.regions.push(limn::HitRegion { id: "list".into(), rect: limn::Rect::new(0.0, 0.0, 240.0, 500.0), kind: HitKind::Scroll });
        frame.regions.push(limn::HitRegion { id: "header".into(), rect: limn::Rect::new(0.0, 0.0, 240.0, 76.0), kind: HitKind::Surface });
        assert!(!can_pan(&frame, Point::new(20.0, 20.0), &|_| false));
        assert!(can_pan(&frame, Point::new(20.0, 100.0), &|_| false));
        frame.regions.push(limn::HitRegion { id: "scrim".into(), rect: limn::Rect::new(0.0, 0.0, 240.0, 500.0), kind: HitKind::Surface });
        assert!(!can_pan(&frame, Point::new(20.0, 100.0), &|_| false));
    }

    #[test]
    fn the_focused_field_requests_the_keyboard_in_screen_coordinates() {
        let mut tree = UiTree::new(Element::column(vec![
            Element::text_input("query", "", "Search").size(Some(200.0), Some(44.0)),
        ]));
        tree.layout(Size::new(240.0, 160.0));
        assert!(tree.focus("query"));
        let frame = tree.layout(Size::new(240.0, 160.0));
        let ctx = egui::Context::default();
        let origin = Pos2::new(12.0, 28.0);
        let mut output = ctx.run_ui(Default::default(), |ui| {
            update_ime(ui.ctx(), &tree, &frame, origin)
        });
        output.textures_delta.clear();
        let ime = output.platform_output.ime.expect("native keyboard request");
        assert_eq!(ime.purpose, egui::IMEPurpose::Normal);
        assert_eq!(ime.rect.min, origin);
        assert_eq!(ime.rect.width(), 200.0);
        let mut output = ctx.run_ui(
            egui::RawInput {
                focused: false,
                ..Default::default()
            },
            |ui| update_ime(ui.ctx(), &tree, &frame, origin),
        );
        output.textures_delta.clear();
        assert!(output.platform_output.ime.is_none());
    }
}
