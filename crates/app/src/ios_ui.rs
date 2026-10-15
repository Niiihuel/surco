//! Native keyboard geometry and the visible system audio-route control.

use eframe::egui::Rect;

fn logical_rect(rect: Rect, native_scale: f32, pixels_per_point: f32) -> Option<Rect> {
    let scale = native_scale / pixels_per_point;
    (rect.is_finite() && scale.is_finite() && scale > 0.0)
        .then(|| Rect::from_min_max(rect.min * scale, rect.max * scale))
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
            })
        }

        /// Update at the start of each UI frame, before reading keyboard geometry or route.
        ///
        /// # Safety
        /// `view` must point to the live UIKit UIView supplied by raw-window-handle for
        /// this window. The pointer only needs to remain valid during this main-thread call;
        /// the native owner stores a weak UIView reference.
        pub unsafe fn update_view(&mut self, view: NonNull<c_void>) {
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

        /// Amount to remove from the *current* content bottom. Safe area is counted once.
        pub fn keyboard_occlusion(&self, full_window: Rect, content_rect: Rect) -> f32 {
            keyboard_occlusion(full_window, content_rect, self.keyboard_rect(full_window))
        }

        pub fn current_route(&self) -> Option<&CurrentRoute> {
            self.route.as_ref()
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
pub use native::{CurrentRoute, NativeUi};

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
