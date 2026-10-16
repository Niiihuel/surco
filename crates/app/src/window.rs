//! The window's own frame where the system's would take a row of its own: its buttons
//! sit at the end of the app's top bar and the bar's empty stretches move the window.
//! Native drag and resize commands leave snapping to the system.

use eframe::egui::{self, pos2, vec2, Color32, CursorIcon, Rect, Sense, Stroke, ViewportCommand};
use egui::viewport::ResizeDirection;

const CONTROL_HEIGHT: f32 = 32.0;
const CONTROL_WIDTH: f32 = 46.0;
const EDGE: f32 = 5.0;
/// Where macOS's three lights end, from the window's left.
const LIGHTS: f32 = 72.0;

/// Who draws the window's frame.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Chrome {
    /// The system, in a frame of its own around the app.
    System,
    /// The app: its buttons at the top bar's right end, as Windows places them.
    Drawn,
    /// The system's buttons over the app's top bar, at its left end, as macOS places them.
    Overlaid,
}

impl Chrome {
    pub fn current(mobile: bool) -> Self {
        if mobile { Self::System }
        else if cfg!(target_os = "windows") { Self::Drawn }
        else if cfg!(target_os = "macos") { Self::Overlaid }
        // A Wayland compositor decorates the window as it likes; X11 gets the Windows frame.
        else if cfg!(target_os = "linux") && std::env::var_os("WAYLAND_DISPLAY").is_none() { Self::Drawn }
        else { Self::System }
    }

    /// The room the window's buttons take at the top bar's left and right ends.
    pub fn inset(self) -> (f32, f32) {
        match self {
            Self::System => (0.0, 0.0),
            Self::Drawn => (0.0, 3.0 * CONTROL_WIDTH),
            Self::Overlaid => (LIGHTS, 0.0),
        }
    }
}

/// Draws the frame and returns where the app goes. `bar` is the top bar's height, and
/// `on_app` tells whether a point, from the app's corner, presses one of its controls.
pub fn show(ui: &mut egui::Ui, cinema: bool, chrome: Chrome, bar: f32, on_app: impl Fn(egui::Vec2) -> bool) -> Rect {
    let bounds = ui.max_rect();
    let (fullscreen, maximized) = ui.input(|input| (input.viewport().fullscreen.unwrap_or(false), input.viewport().maximized.unwrap_or(false)));
    if chrome == Chrome::System || cinema || fullscreen {
        return bounds;
    }
    let drawn = chrome == Chrome::Drawn;
    let inner = if drawn && !maximized {
        ui.painter().rect_filled(bounds, 0.0, Color32::BLACK);
        bounds.shrink(EDGE)
    } else {
        bounds
    };
    let (left, right) = chrome.inset();
    let caption = Rect::from_min_max(pos2(inner.left() + left, inner.top()), pos2(inner.right() - right, inner.top() + bar));
    let response = ui.interact(caption, ui.id().with("window-drag"), Sense::CLICK | Sense::DRAG);
    // A press on one of the bar's own controls is the app's.
    // A double click is told on its release, when the press has no origin left.
    let free = ui.input(|input| input.pointer.press_origin()).or(response.interact_pointer_pos()).is_some_and(|origin| !on_app(origin - inner.min));
    if free && response.double_clicked() {
        ui.ctx().send_viewport_cmd(ViewportCommand::Maximized(!maximized));
    } else if free && response.drag_started_by(egui::PointerButton::Primary) {
        ui.ctx().send_viewport_cmd(ViewportCommand::StartDrag);
    }
    if !drawn {
        return inner;
    }
    // Over the app's top bar, which is drawn after this.
    let painter = ui.ctx().layer_painter(egui::LayerId::new(egui::Order::Foreground, ui.id().with("window-controls")));
    for (index, label, command) in [
        (0, "Minimizar", ViewportCommand::Minimized(true)),
        (1, if maximized { "Restaurar" } else { "Maximizar" }, ViewportCommand::Maximized(!maximized)),
        (2, "Cerrar", ViewportCommand::Close),
    ] {
        let rect = Rect::from_min_size(pos2(caption.right() + index as f32 * CONTROL_WIDTH, inner.top()), vec2(CONTROL_WIDTH, CONTROL_HEIGHT));
        // Like the native caption, leave Tab to the app and keyboard window commands to Alt+Space.
        let button = ui.interact(rect, ui.id().with(("window-control", index)), Sense::CLICK);
        button.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label));
        if button.hovered() || button.has_focus() || button.is_pointer_button_down_on() {
            let fill = if index == 2 { Color32::from_rgb(196, 43, 28) } else { Color32::from_gray(34) };
            painter.rect_filled(rect, 0.0, fill);
        }
        let mark = Rect::from_center_size(rect.center(), vec2(10.0, 10.0));
        let stroke = Stroke::new(1.0, Color32::WHITE);
        match index {
            0 => { painter.line_segment([mark.left_center(), mark.right_center()], stroke); }
            1 => {
                let square = if maximized { mark.translate(vec2(2.0, -2.0)) } else { mark };
                painter.rect_stroke(square, 0.0, stroke, egui::StrokeKind::Inside);
                if maximized {
                    let front = mark.translate(vec2(-2.0, 2.0));
                    painter.rect_filled(front, 0.0, Color32::BLACK);
                    painter.rect_stroke(front, 0.0, stroke, egui::StrokeKind::Inside);
                }
            }
            _ => {
                painter.line_segment([mark.left_top(), mark.right_bottom()], stroke);
                painter.line_segment([mark.right_top(), mark.left_bottom()], stroke);
            }
        }
        if button.clicked() {
            ui.ctx().send_viewport_cmd(command);
        }
        button.on_hover_text(label);
    }
    // An undecorated window has no native border hit regions.
    if !maximized {
        if let Some(direction) = ui.input(|input| input.pointer.hover_pos().and_then(|point| resize_direction(bounds, point))) {
            let cursor = match direction {
                ResizeDirection::North | ResizeDirection::South => CursorIcon::ResizeVertical,
                ResizeDirection::East | ResizeDirection::West => CursorIcon::ResizeHorizontal,
                ResizeDirection::NorthEast | ResizeDirection::SouthWest => CursorIcon::ResizeNeSw,
                ResizeDirection::NorthWest | ResizeDirection::SouthEast => CursorIcon::ResizeNwSe,
            };
            ui.ctx().set_cursor_icon(cursor);
            if ui.input(|input| input.pointer.primary_pressed()) {
                ui.ctx().send_viewport_cmd(ViewportCommand::BeginResize(direction));
            }
        }
    }
    inner
}

fn resize_direction(bounds: Rect, point: egui::Pos2) -> Option<ResizeDirection> {
    if !bounds.contains(point) {
        return None;
    }
    let left = point.x < bounds.left() + EDGE;
    let right = point.x > bounds.right() - EDGE;
    let top = point.y < bounds.top() + EDGE;
    let bottom = point.y > bounds.bottom() - EDGE;
    match (left, right, top, bottom) {
        (true, _, true, _) => Some(ResizeDirection::NorthWest),
        (_, true, true, _) => Some(ResizeDirection::NorthEast),
        (true, _, _, true) => Some(ResizeDirection::SouthWest),
        (_, true, _, true) => Some(ResizeDirection::SouthEast),
        (true, _, _, _) => Some(ResizeDirection::West),
        (_, true, _, _) => Some(ResizeDirection::East),
        (_, _, true, _) => Some(ResizeDirection::North),
        (_, _, _, true) => Some(ResizeDirection::South),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const WIDTH: f32 = 800.0;
    const HEIGHT: f32 = 600.0;
    const BAR: f32 = 56.0;

    fn frame(ctx: &egui::Context, events: Vec<egui::Event>, cinema: bool, maximized: bool, fullscreen: bool) -> (Rect, egui::FullOutput) {
        let mut input = egui::RawInput { screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, vec2(WIDTH, HEIGHT))), events, ..Default::default() };
        let viewport = input.viewports.get_mut(&egui::ViewportId::ROOT).unwrap();
        viewport.maximized = Some(maximized);
        viewport.fullscreen = Some(fullscreen);
        let mut content = Rect::NOTHING;
        // A control of the app's sits at the bar's start, where a back button would be.
        let mut output = ctx.run_ui(input, |ui| { content = show(ui, cinema, Chrome::Drawn, BAR, |point| point.x < 60.0); });
        // There is no renderer in these interaction tests to upload the font atlas.
        output.textures_delta.clear();
        output.viewport_output.get_mut(&egui::ViewportId::ROOT).unwrap().commands.retain(|command| !matches!(command, ViewportCommand::SetTheme(_)));
        (content, output)
    }

    fn button(point: egui::Pos2, pressed: bool) -> egui::Event {
        egui::Event::PointerButton { pos: point, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE }
    }

    fn commands(output: &egui::FullOutput) -> &[ViewportCommand] {
        &output.viewport_output[&egui::ViewportId::ROOT].commands
    }

    #[test]
    fn content_shares_its_top_row_with_the_caption_and_keeps_room_for_resize_edges() {
        let ctx = egui::Context::default();
        let (content, _) = frame(&ctx, vec![], false, false, false);
        assert_eq!(content, Rect::from_min_max(pos2(EDGE, EDGE), pos2(WIDTH - EDGE, HEIGHT - EDGE)));
        let (content, _) = frame(&ctx, vec![], false, true, false);
        assert_eq!(content, Rect::from_min_max(pos2(0.0, 0.0), pos2(WIDTH, HEIGHT)));
        for (cinema, fullscreen) in [(true, false), (false, true)] {
            let (content, _) = frame(&ctx, vec![], cinema, false, fullscreen);
            assert_eq!(content, Rect::from_min_size(egui::Pos2::ZERO, vec2(WIDTH, HEIGHT)));
        }
    }

    #[test]
    fn caption_buttons_minimize_maximize_restore_and_close_without_dragging() {
        for (index, maximized, expected) in [
            (0, false, ViewportCommand::Minimized(true)),
            (1, false, ViewportCommand::Maximized(true)),
            (1, true, ViewportCommand::Maximized(false)),
            (2, false, ViewportCommand::Close),
        ] {
            let ctx = egui::Context::default();
            let edge = if maximized { 0.0 } else { EDGE };
            let point = pos2(WIDTH - edge - (2.5 - index as f32) * CONTROL_WIDTH, edge + CONTROL_HEIGHT / 2.0);
            frame(&ctx, vec![], false, maximized, false);
            let (_, pressed) = frame(&ctx, vec![egui::Event::PointerMoved(point), button(point, true)], false, maximized, false);
            assert!(commands(&pressed).is_empty());
            let (_, released) = frame(&ctx, vec![button(point, false)], false, maximized, false);
            assert_eq!(commands(&released), &[expected]);
        }
    }

    #[test]
    fn dragging_the_caption_asks_the_system_to_move_the_window() {
        let ctx = egui::Context::default();
        let point = pos2(120.0, EDGE + BAR / 2.0);
        frame(&ctx, vec![], false, false, false);
        frame(&ctx, vec![egui::Event::PointerMoved(point), button(point, true)], false, false, false);
        let (_, moved) = frame(&ctx, vec![egui::Event::PointerMoved(point + vec2(20.0, 0.0))], false, false, false);
        assert_eq!(commands(&moved), &[ViewportCommand::StartDrag]);
        frame(&ctx, vec![button(point + vec2(20.0, 0.0), false)], false, false, false);
        let control = pos2(30.0, EDGE + BAR / 2.0);
        frame(&ctx, vec![egui::Event::PointerMoved(control), button(control, true)], false, false, false);
        let (_, moved) = frame(&ctx, vec![egui::Event::PointerMoved(control + vec2(20.0, 0.0))], false, false, false);
        assert!(commands(&moved).is_empty(), "a press on the app's own control does not move the window");
    }

    #[test]
    fn the_window_buttons_take_the_side_each_system_puts_them_on() {
        assert_eq!(Chrome::Drawn.inset(), (0.0, 3.0 * CONTROL_WIDTH));
        assert_eq!(Chrome::Overlaid.inset(), (LIGHTS, 0.0));
        assert_eq!(Chrome::current(true), Chrome::System, "a phone has no window frame");
    }

    #[test]
    fn double_clicking_the_caption_maximizes_and_restores_without_dragging() {
        for maximized in [false, true] {
            let ctx = egui::Context::default();
            let point = pos2(120.0, EDGE + BAR / 2.0);
            frame(&ctx, vec![], false, maximized, false);
            frame(&ctx, vec![egui::Event::PointerMoved(point), button(point, true)], false, maximized, false);
            frame(&ctx, vec![button(point, false)], false, maximized, false);
            frame(&ctx, vec![button(point, true)], false, maximized, false);
            let (_, released) = frame(&ctx, vec![button(point, false)], false, maximized, false);
            assert_eq!(commands(&released), &[ViewportCommand::Maximized(!maximized)]);
        }
    }

    #[test]
    fn the_caption_leaves_the_apps_tab_order_and_keyboard_activation_alone() {
        let ctx = egui::Context::default();
        frame(&ctx, vec![], false, false, false);
        let key = |key| egui::Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::NONE };
        for event in [key(egui::Key::Tab), key(egui::Key::Space), key(egui::Key::Enter)] {
            let (_, output) = frame(&ctx, vec![event], false, false, false);
            assert!(commands(&output).is_empty());
            assert_eq!(ctx.memory(|memory| memory.focused()), None);
        }
    }

    #[test]
    fn all_edges_and_corners_resize_but_fullscreen_and_maximized_windows_do_not() {
        for (point, direction) in [
            (pos2(1.0, 1.0), ResizeDirection::NorthWest),
            (pos2(WIDTH / 2.0, 1.0), ResizeDirection::North),
            (pos2(WIDTH - 1.0, 1.0), ResizeDirection::NorthEast),
            (pos2(1.0, HEIGHT / 2.0), ResizeDirection::West),
            (pos2(WIDTH - 1.0, HEIGHT / 2.0), ResizeDirection::East),
            (pos2(1.0, HEIGHT - 1.0), ResizeDirection::SouthWest),
            (pos2(WIDTH / 2.0, HEIGHT - 1.0), ResizeDirection::South),
            (pos2(WIDTH - 1.0, HEIGHT - 1.0), ResizeDirection::SouthEast),
        ] {
            let ctx = egui::Context::default();
            let (_, output) = frame(&ctx, vec![egui::Event::PointerMoved(point), button(point, true)], false, false, false);
            assert_eq!(commands(&output), &[ViewportCommand::BeginResize(direction)]);
        }
        let bounds = Rect::from_min_size(egui::Pos2::ZERO, vec2(WIDTH, HEIGHT));
        assert_eq!(resize_direction(bounds, bounds.center()), None);
        assert_eq!(resize_direction(bounds, pos2(-1.0, HEIGHT / 2.0)), None);
        for (cinema, maximized, fullscreen) in [(true, false, false), (false, true, false), (false, false, true)] {
            let ctx = egui::Context::default();
            let point = pos2(WIDTH - 1.0, HEIGHT - 1.0);
            let (_, output) = frame(&ctx, vec![egui::Event::PointerMoved(point), button(point, true)], cinema, maximized, fullscreen);
            assert!(commands(&output).is_empty());
        }
    }
}
