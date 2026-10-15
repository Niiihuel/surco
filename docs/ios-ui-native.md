# Native keyboard and audio-route UI

`crates/app/src/ios_ui.rs` owns the Rust interface; `ios/SurcoView.m` owns UIKit objects. The bridge stays in Surco. Playback and rendering remain in the existing Rust engine and limn UI.

## Host integration

Declare the module with `#[cfg(any(target_os = "ios", test))] mod ios_ui;` so its pure geometry tests also run on desktop. Only construct/use `ios_ui::NativeUi` inside the iOS host, on the main thread:

```rust,ignore
let mut native = ios_ui::NativeUi::new(&creation.egui_ctx)?;
// At the beginning of every UI frame:
if let raw_window_handle::RawWindowHandle::UiKit(handle) = frame.window_handle()?.as_raw() {
    unsafe { native.update_view(handle.ui_view) };
}
```

Import `raw_window_handle::HasWindowHandle` for the public eframe `Frame` implementation. `update_view` accepts `NonNull<c_void>` pointing to the live window's UIView. Rust owns the native handle; Objective-C keeps the UIView weak. Its `Rc<Context>` makes `NativeUi` `!Send`/`!Sync` and supplies a stable shared pointer for repaint callbacks. On drop, native observers are removed and the repaint callback/context are cleared before Rust releases its Rc. Already queued notification blocks only capture a weak native owner.

Read `keyboard_top(full_window)` or `keyboard_occlusion(full_window, content_rect)` after updating the view. Rects use egui screen coordinates. Use the full window rect before keyboard or safe-area reductions; pass the current content rect separately. The occlusion function only removes content below the keyboard top, so an existing safe-area inset or viewport resize is never subtracted twice. Floating or partial-width keyboards do not resize the entire window. UIKit screen coordinates are converted to the attached UIView, then UIKit points become egui points through `contentScaleFactor / ctx.pixels_per_point()` once. The bridge reports geometry and requests repaint; the host controls scrolling and layout.

`current_route()` returns `Option<&CurrentRoute>` with public `name` and `port_type` strings from the current AVAudioSession outputs. The host can copy these into `MobileUi.route`. Route notifications request repaint; JSON is copied and parsed only when the native route revision changes. No network request, microphone permission, Bluetooth scan or invented device is involved.

For the `ios-route-picker` native slot, call `route_picker(rect, visible)` every frame. Its rect must include the limn area's screen origin (`area.min`). The bridge converts to UIView points and places a visible, interactive `AVRoutePickerView` in that slot. Use at least a 44-point hit target. Pass `visible = false` whenever the device sheet closes, another modal covers it, or no slot exists; false immediately hides the view and disables hit testing. Slots outside the window or with invalid geometry are also hidden. Set `set_route_picker_colors(tint, active)` from the app palette if needed. The system owns the picker button, accessibility and destination selection.

AVRoutePickerView presents its system route UI when tapped. There is no private-button invocation, artificial AVPlayer or programmatic attempt to open that UI. Initial Bluetooth pairing belongs to iOS Settings; the route sheet should describe this when a device is not yet paired. Device availability is owned by iOS, rather than a custom list of every nearby/paired speaker.

## Validation

The Rust tests cover docked/floating/hidden keyboard geometry, safe-area accounting, Retina/egui zoom and native-slot visibility. In isolation, the module can be loaded through a temporary test harness without changing the host module declaration. Native UIKit/AVKit compile and linkage still require the macOS CI build. Physical checks need an iPhone/iPad: type in every input, rotate with keyboard visible, dismiss/reopen sheets, cover the route button with another modal, select paired Bluetooth headphones/speaker and AirPlay, then background/foreground the app. A simulator cannot establish physical Bluetooth behavior.

## Primary references

- [Apple keyboard frame coordinate space](https://developer.apple.com/documentation/uikit/uiresponder/keyboardframeenduserinfokey): the notification's end frame uses screen coordinates; iOS 16.1+ identifies the UIScreen in `notification.object`.
- [Apple coordinate-space conversion](https://developer.apple.com/documentation/uikit/uicoordinatespace/convert(_:to:)-3imkt): convert from the notification screen into the attached view instead of assuming window and screen frames match.
- [Apple UIView](https://developer.apple.com/documentation/uikit/uiview): UIKit main-thread ownership, view bounds, subviews and content scale.
- [Apple AVRoutePickerView](https://developer.apple.com/documentation/avkit/avroutepickerview): visible system destination control; `prioritizesVideoDevices = false` for audio. Its `player` property is macOS-only and is not needed here.
- [Apple route-change handling](https://developer.apple.com/documentation/avfaudio/responding-to-audio-route-changes): current route outputs provide real port names and types.
- [raw-window-handle 0.6.2 UIKit handle](https://docs.rs/raw-window-handle/0.6.2/raw_window_handle/struct.UiKitWindowHandle.html): the public main-thread UIView pointer used by the eframe/winit host.
