//! Native keyboard geometry and the visible system audio-route control.

use eframe::egui::Rect;

fn logical_rect(rect: Rect, native_scale: f32, pixels_per_point: f32) -> Option<Rect> {
    let scale = native_scale / pixels_per_point;
    (rect.is_finite() && scale.is_finite() && scale > 0.0)
        .then(|| Rect::from_min_max(rect.min * scale, rect.max * scale))
}

fn logical_safe_rect(full: Rect, insets: [f32; 4], native_scale: f32, pixels_per_point: f32) -> Option<Rect> {
    let scale = native_scale / pixels_per_point;
    let [top, right, bottom, left] = insets.map(|value| value * scale);
    if !full.is_finite() || !full.is_positive() || !scale.is_finite() || scale <= 0.0
        || [top, right, bottom, left].iter().any(|value| !value.is_finite() || *value < 0.0) {
        return None;
    }
    let safe = Rect::from_min_max(
        full.min + eframe::egui::vec2(left, top),
        full.max - eframe::egui::vec2(right, bottom),
    );
    safe.is_positive().then_some(safe)
}

fn docked_keyboard_top(full: Rect, keyboard: Option<Rect>) -> Option<f32> {
    let keyboard = keyboard?;
    if !full.is_finite()
        || !keyboard.is_finite()
        || !full.is_positive()
        || !keyboard.is_positive()
        || keyboard.min.x > full.min.x + 1.0
        || keyboard.max.x < full.max.x - 1.0
        || keyboard.max.y < full.max.y - 1.0
        || keyboard.min.y >= full.max.y
    {
        return None;
    }
    Some(keyboard.min.y.max(full.min.y))
}

fn keyboard_occlusion(full: Rect, content: Rect, keyboard: Option<Rect>) -> f32 {
    if !content.is_finite() || !content.is_positive() {
        return 0.0;
    }
    docked_keyboard_top(full, keyboard).map_or(0.0, |top| {
        (content.max.y - top).clamp(0.0, content.height())
    })
}

fn picker_visible(slot: Rect, visible: bool, full: Rect) -> bool {
    visible
        && slot.is_finite()
        && slot.is_positive()
        && full.is_finite()
        && full.is_positive()
        && full.contains_rect(slot)
}

#[cfg(target_os = "ios")]
mod native {
    use super::*;
    use eframe::egui::{Color32, Context, pos2, vec2};
    use serde::Deserialize;
    use std::ffi::{CStr, c_char, c_void};
    use std::ptr::NonNull;
    use std::rc::Rc;

    #[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
    pub struct CurrentRoute {
        pub name: String,
        pub port_type: String,
    }

    // UIKit points relative to UIView.bounds.origin, not pixels or safe-area coordinates.
    #[repr(C)]
    #[derive(Default)]
    struct Geometry {
        width: f64,
        height: f64,
        scale: f64,
        safe_top: f64,
        safe_right: f64,
        safe_bottom: f64,
        safe_left: f64,
        keyboard_x: f64,
        keyboard_y: f64,
        keyboard_width: f64,
        keyboard_height: f64,
        route_revision: u64,
    }

    unsafe extern "C" {
        fn surco_ui_create(wake: extern "C" fn(*mut c_void), context: *mut c_void) -> *mut c_void;
        fn surco_ui_destroy(handle: *mut c_void);
        fn surco_ui_update(handle: *mut c_void, view: *mut c_void, geometry: *mut Geometry);
        fn surco_ui_copy_route(handle: *mut c_void) -> *mut c_char;
        fn surco_ui_free_string(value: *mut c_char);
        fn surco_ui_picker(handle: *mut c_void, x: f64, y: f64, w: f64, h: f64, visible: i32);
        fn surco_ui_picker_colors(handle: *mut c_void, tint: u32, active: u32);
        fn surco_ui_copy_text(bytes: *const u8, length: usize) -> i32;
        fn surco_ui_paste_text(length: *mut usize) -> *mut u8;
        fn surco_reduce_motion_enabled() -> i32;
        fn surco_liquid_glass() -> i32;
        fn surco_ui_on_screen() -> i32;
        fn surco_ui_take_memory_warning() -> i32;
        fn surco_ui_surfaces(handle: *mut c_void, bytes: *const u8, length: usize, scale: f64) -> i32;
        fn surco_ui_take_surface_events(handle: *mut c_void) -> *mut c_char;
        fn surco_take_opened_link() -> *mut c_char;
        fn surco_scan_code(hint: *const c_char, close: *const c_char, denied_title: *const c_char, denied_body: *const c_char, settings: *const c_char, cancel: *const c_char);
        fn surco_pick_photo();
        fn surco_take_picked_photo(bytes: *mut *mut u8, length: *mut usize) -> i32;
    }

    /// Opens the camera, inside Surco, on the code of an account on another device; what it
    /// reads comes back as a link Surco was opened with. Safe from any thread.
    pub fn scan_code(hint: &str, close: &str, denied_title: &str, denied_body: &str, settings: &str, cancel: &str) {
        let text = |value: &str| std::ffi::CString::new(value).unwrap_or_default();
        let (hint, close, denied_title, denied_body, settings, cancel) = (text(hint), text(close), text(denied_title), text(denied_body), text(settings), text(cancel));
        unsafe { surco_scan_code(hint.as_ptr(), close.as_ptr(), denied_title.as_ptr(), denied_body.as_ptr(), settings.as_ptr(), cancel.as_ptr()) };
    }

    /// Where the system's photo picker is.
    pub enum Picked {
        Open,
        Photo(Vec<u8>),
        Left,
    }

    /// Opens the system's photo picker. Safe from any thread.
    pub fn pick_photo() {
        unsafe { surco_pick_photo() };
    }

    /// The picture picked, once, or whether the picker is still open.
    pub fn take_picked_photo() -> Picked {
        let (mut bytes, mut length) = (std::ptr::null_mut(), 0usize);
        match unsafe { surco_take_picked_photo(&mut bytes, &mut length) } {
            0 => Picked::Open,
            1 if !bytes.is_null() => {
                let photo = unsafe { std::slice::from_raw_parts(bytes, length) }.to_vec();
                unsafe { surco_ui_free_string(bytes.cast()) };
                Picked::Photo(photo)
            }
            _ => Picked::Left,
        }
    }

    /// The link Surco was last opened with, once: the code of an account another device
    /// serves, as the camera hands it over.
    pub fn take_opened_link() -> Option<String> {
        let link = unsafe { surco_take_opened_link() };
        if link.is_null() { return None; }
        let text = unsafe { CStr::from_ptr(link) }.to_string_lossy().into_owned();
        unsafe { surco_ui_free_string(link) };
        Some(text)
    }

    /// Read the system preference on UIKit's main thread; the host combines it with its setting.
    /// https://developer.apple.com/documentation/uikit/uiaccessibility/isreducemotionenabled
    pub fn reduce_motion_enabled() -> bool {
        unsafe { surco_reduce_motion_enabled() != 0 }
    }

    /// iOS 26 or later, whose bars and sheets are Liquid Glass.
    pub fn liquid_glass() -> bool {
        unsafe { surco_liquid_glass() != 0 }
    }

    /// Whether the app is on screen rather than in the background or behind the lock
    /// screen, where audio plays on and nothing needs drawing. Safe from any thread.
    pub fn on_screen() -> bool {
        unsafe { surco_ui_on_screen() != 0 }
    }

    /// Whether iOS warned of low memory since the last time this was asked.
    pub fn take_memory_warning() -> bool {
        unsafe { surco_ui_take_memory_warning() != 0 }
    }

    extern "C" fn wake(context: *mut c_void) {
        // The native owner clears this callback synchronously before `context` is dropped.
        unsafe { &*context.cast::<Context>() }.request_repaint();
    }

    /// Main-thread owner of native controls attached to this app's winit UIView.
    /// Its Rc context prevents moving or sharing UIKit state across threads.
    pub struct NativeUi {
        handle: NonNull<c_void>,
        ctx: Rc<Context>,
        geometry: Geometry,
        route: Option<CurrentRoute>,
        route_revision: u64,
        surface_payload: Option<(String, f32)>,
        view: Option<NonNull<c_void>>,
    }

    impl NativeUi {
        pub fn new(ctx: &Context) -> Result<Self, String> {
            let ctx = Rc::new(ctx.clone());
            let handle =
                NonNull::new(unsafe { surco_ui_create(wake, Rc::as_ptr(&ctx).cast_mut().cast()) })
                    .ok_or_else(|| "iOS native UI must be created on the main thread".to_owned())?;
            Ok(Self {
                handle,
                ctx,
                geometry: Geometry::default(),
                route: None,
                route_revision: 0,
                surface_payload: None,
                view: None,
            })
        }

        /// Update at the start of each UI frame, before reading keyboard geometry or route.
        ///
        /// # Safety
        /// `view` must point to the live UIKit UIView supplied by raw-window-handle for
        /// this window. The pointer only needs to remain valid during this main-thread call;
        /// the native owner stores a weak UIView reference.
        pub unsafe fn update_view(&mut self, view: NonNull<c_void>) {
            if self.view != Some(view) {
                self.surface_payload = None;
                self.view = Some(view);
            }
            unsafe { surco_ui_update(self.handle.as_ptr(), view.as_ptr(), &mut self.geometry) };
            if self.geometry.route_revision != self.route_revision {
                self.route_revision = self.geometry.route_revision;
                let json = unsafe { surco_ui_copy_route(self.handle.as_ptr()) };
                self.route = if json.is_null() {
                    None
                } else {
                    let route = serde_json::from_slice(unsafe { CStr::from_ptr(json) }.to_bytes());
                    unsafe { surco_ui_free_string(json) };
                    match route {
                        Ok(route) => Some(route),
                        Err(error) => {
                            eprintln!("Surco could not read the iOS audio route: {error}");
                            None
                        }
                    }
                };
            }
        }

        fn keyboard_rect(&self, full: Rect) -> Option<Rect> {
            let rect = Rect::from_min_size(
                pos2(
                    self.geometry.keyboard_x as f32,
                    self.geometry.keyboard_y as f32,
                ),
                vec2(
                    self.geometry.keyboard_width as f32,
                    self.geometry.keyboard_height as f32,
                ),
            );
            logical_rect(
                rect,
                self.geometry.scale as f32,
                self.ctx.pixels_per_point(),
            )
            .filter(Rect::is_positive)
            .map(|rect| rect.translate(full.min.to_vec2()))
        }

        /// Docked keyboard top in egui screen coordinates. Floating keyboards return None.
        pub fn keyboard_top(&self, full_window: Rect) -> Option<f32> {
            docked_keyboard_top(full_window, self.keyboard_rect(full_window))
        }

        /// Safe area of this live UIView, including apps without a UIWindowScene.
        pub fn safe_content_rect(&self, full_window: Rect) -> Option<Rect> {
            if !self.geometry.width.is_finite() || !self.geometry.height.is_finite()
                || self.geometry.width <= 0.0 || self.geometry.height <= 0.0 { return None; }
            logical_safe_rect(full_window, [
                self.geometry.safe_top as f32, self.geometry.safe_right as f32,
                self.geometry.safe_bottom as f32, self.geometry.safe_left as f32,
            ], self.geometry.scale as f32, self.ctx.pixels_per_point())
        }

        /// Amount to remove from the *current* content bottom. Safe area is counted once.
        pub fn keyboard_occlusion(&self, full_window: Rect, content_rect: Rect) -> f32 {
            keyboard_occlusion(full_window, content_rect, self.keyboard_rect(full_window))
        }

        pub fn current_route(&self) -> Option<&CurrentRoute> {
            self.route.as_ref()
        }

        /// Publish controlled UIKit surfaces only when their content/geometry changes.
        pub fn update_surfaces(&mut self, payload: &impl serde::Serialize) -> Result<(), String> {
            let json = serde_json::to_string(payload).map_err(|error| error.to_string())?;
            let scale = self.ctx.pixels_per_point() / self.geometry.scale as f32;
            if !scale.is_finite() || scale <= 0.0 { return Err("Invalid native surface scale".into()); }
            if self.surface_payload.as_ref().is_some_and(|(old, old_scale)| old == &json && *old_scale == scale) {
                return Ok(());
            }
            if unsafe { surco_ui_surfaces(self.handle.as_ptr(), json.as_ptr(), json.len(), f64::from(scale)) } == 0 {
                return Err("iOS could not update its controls".into());
            }
            self.surface_payload = Some((json, scale));
            Ok(())
        }

        pub fn take_surface_events<T: serde::de::DeserializeOwned>(&mut self) -> Result<Vec<T>, String> {
            let json = unsafe { surco_ui_take_surface_events(self.handle.as_ptr()) };
            if json.is_null() { return Err("iOS could not read its control events".into()); }
            let events = serde_json::from_slice(unsafe { CStr::from_ptr(json) }.to_bytes());
            unsafe { surco_ui_free_string(json) };
            events.map_err(|error| format!("Invalid iOS control event: {error}"))
        }

        /// Write only in response to an explicit Copy/Cut action. NSString copies the bytes.
        pub fn copy_text(&mut self, text: &str) -> Result<(), String> {
            if unsafe { surco_ui_copy_text(text.as_ptr(), text.len()) } != 0 {
                Ok(())
            } else {
                Err("iOS could not write text to the system clipboard".into())
            }
        }

        /// Read once after a user paste gesture. iOS may request paste permission.
        /// Never call this from update_view, a timer or an availability check.
        pub fn paste_text(&mut self) -> Result<Option<String>, String> {
            let mut length = 0;
            let bytes = unsafe { surco_ui_paste_text(&mut length) };
            if bytes.is_null() { return Ok(None); }
            let text = String::from_utf8(unsafe { std::slice::from_raw_parts(bytes, length) }.to_vec());
            unsafe { surco_ui_free_string(bytes.cast()) };
            text.map(Some).map_err(|_| "iOS returned invalid clipboard text".into())
        }

        /// Place the real system button in an egui screen-coordinate slot (including area.min).
        /// The host must pass false when another modal covers the slot or the sheet is closed.
        pub fn route_picker(&mut self, rect: Rect, visible: bool) {
            let scale = self.geometry.scale as f32 / self.ctx.pixels_per_point();
            let full = logical_rect(
                Rect::from_min_size(
                    pos2(0.0, 0.0),
                    vec2(self.geometry.width as f32, self.geometry.height as f32),
                ),
                self.geometry.scale as f32,
                self.ctx.pixels_per_point(),
            );
            let visible = full.is_some_and(|full| picker_visible(rect, visible, full));
            let rect = if visible {
                Rect::from_min_max(rect.min / scale, rect.max / scale)
            } else {
                Rect::ZERO
            };
            unsafe {
                surco_ui_picker(
                    self.handle.as_ptr(),
                    rect.min.x as f64,
                    rect.min.y as f64,
                    rect.width() as f64,
                    rect.height() as f64,
                    i32::from(visible),
                );
            }
        }

        pub fn set_route_picker_colors(&mut self, tint: Color32, active: Color32) {
            let rgba = |color: Color32| u32::from_be_bytes(color.to_srgba_unmultiplied());
            unsafe { surco_ui_picker_colors(self.handle.as_ptr(), rgba(tint), rgba(active)) };
        }
    }

    impl Drop for NativeUi {
        fn drop(&mut self) {
            // Removes observers and clears callback/context before Rust releases its Rc.
            unsafe { surco_ui_destroy(self.handle.as_ptr()) };
        }
    }
}

#[cfg(target_os = "ios")]
pub use native::{CurrentRoute, NativeUi, Picked, on_screen, pick_photo, reduce_motion_enabled, scan_code, take_memory_warning, take_opened_link, take_picked_photo};

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::{pos2, vec2};

    #[test]
    fn docked_keyboard_reduces_content_without_double_counting_safe_area() {
        let full = Rect::from_min_size(pos2(0.0, 0.0), vec2(390.0, 844.0));
        let content = Rect::from_min_max(pos2(0.0, 59.0), pos2(390.0, 810.0));
        let keyboard = Rect::from_min_max(pos2(0.0, 500.0), pos2(390.0, 844.0));
        assert_eq!(docked_keyboard_top(full, Some(keyboard)), Some(500.0));
        assert_eq!(keyboard_occlusion(full, content, Some(keyboard)), 310.0);
        let already_resized = Rect::from_min_max(content.min, pos2(390.0, 500.0));
        assert_eq!(
            keyboard_occlusion(full, already_resized, Some(keyboard)),
            0.0
        );
        assert_eq!(keyboard_occlusion(full, content, None), 0.0);
    }

    #[test]
    fn floating_and_offscreen_keyboards_do_not_resize_the_whole_window() {
        let full = Rect::from_min_size(pos2(0.0, 0.0), vec2(1024.0, 1366.0));
        let floating = Rect::from_min_size(pos2(400.0, 900.0), vec2(320.0, 250.0));
        let small_bottom = Rect::from_min_size(pos2(400.0, 1116.0), vec2(320.0, 250.0));
        let hidden = Rect::from_min_size(pos2(0.0, 1366.0), vec2(1024.0, 300.0));
        for keyboard in [floating, small_bottom, hidden] {
            assert_eq!(docked_keyboard_top(full, Some(keyboard)), None);
        }
    }

    #[test]
    fn retina_and_egui_zoom_are_converted_once() {
        let native = Rect::from_min_size(pos2(0.0, 500.0), vec2(390.0, 344.0));
        assert_eq!(logical_rect(native, 3.0, 3.0), Some(native));
        let zoomed = logical_rect(native, 3.0, 6.0).unwrap();
        assert_eq!(zoomed.min.y, 250.0);
        assert_eq!(zoomed.width(), 195.0);
        assert_eq!(logical_rect(native, 3.0, 0.0), None);
    }

    #[test]
    fn live_view_safe_edges_convert_once_and_follow_full_canvas_origin() {
        let full = Rect::from_min_size(pos2(10.0, 20.0), vec2(402.0, 874.0));
        assert_eq!(logical_safe_rect(full, [62.0, 0.0, 34.0, 0.0], 3.0, 3.0),
            Some(Rect::from_min_max(pos2(10.0, 82.0), pos2(412.0, 860.0))));
        let zoomed = Rect::from_min_size(full.min, vec2(201.0, 437.0));
        assert_eq!(logical_safe_rect(zoomed, [62.0, 0.0, 34.0, 0.0], 3.0, 6.0),
            Some(Rect::from_min_max(pos2(10.0, 51.0), pos2(211.0, 440.0))));
        assert_eq!(logical_safe_rect(full, [f32::NAN, 0.0, 34.0, 0.0], 3.0, 3.0), None);
        assert_eq!(logical_safe_rect(full, [-1.0, 0.0, 34.0, 0.0], 3.0, 3.0), None);
        assert_eq!(logical_safe_rect(full, [800.0, 0.0, 100.0, 0.0], 3.0, 3.0), None);
        assert_eq!(logical_safe_rect(full, [62.0, 0.0, 34.0, 0.0], 3.0, 0.0), None);
    }

    #[test]
    fn a_native_control_never_remains_visible_outside_its_slot_or_window() {
        let full = Rect::from_min_size(pos2(0.0, 0.0), vec2(390.0, 844.0));
        let slot = Rect::from_min_size(pos2(20.0, 600.0), vec2(44.0, 44.0));
        assert!(picker_visible(slot, true, full));
        assert!(!picker_visible(slot, false, full));
        assert!(!picker_visible(
            slot.translate(vec2(-50.0, 0.0)),
            true,
            full
        ));
        assert!(!picker_visible(Rect::NOTHING, true, full));
    }
}
