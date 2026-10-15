# Native keyboard and audio-route UI

`crates/app/src/ios_ui.rs` owns the Rust interface; `ios/SurcoView.m` owns UIKit objects. The bridge stays in Surco. Playback and rendering remain in the existing Rust engine and limn UI.

## Host integration

The Host declares `#[cfg(any(target_os = "ios", test))] mod ios_ui;`, so the pure geometry tests also run on desktop. It constructs `ios_ui::NativeUi` on the iOS main thread and updates the attached UIView before reading keyboard/route state. The integration pattern is:

```rust,ignore
let mut native = ios_ui::NativeUi::new(&creation.egui_ctx)?;
// At the beginning of every UI frame:
if let raw_window_handle::RawWindowHandle::UiKit(handle) = frame.window_handle()?.as_raw() {
    unsafe { native.update_view(handle.ui_view) };
}
```

Import `raw_window_handle::HasWindowHandle` for the public eframe `Frame` implementation. `update_view` accepts `NonNull<c_void>` pointing to the live window's UIView. Rust owns the native handle; Objective-C keeps the UIView weak. Its `Rc<Context>` makes `NativeUi` `!Send`/`!Sync` and supplies a stable shared pointer for repaint callbacks. On drop, native observers are removed and the repaint callback/context are cleared before Rust releases its Rc. Already queued notification blocks only capture a weak native owner.

The Host reads `keyboard_top(full_window)` and `keyboard_occlusion(full_window, content_rect)` after updating the view. Rects use egui screen coordinates. The full window rect precedes keyboard or safe-area reductions; the current content rect is passed separately. The occlusion function only removes content below the keyboard top, so an existing safe-area inset or viewport resize is never subtracted twice. Floating or partial-width keyboards do not resize the entire window. UIKit screen coordinates are converted to the attached UIView, then UIKit points become egui points through `contentScaleFactor / ctx.pixels_per_point()` once. The bridge reports geometry and requests repaint; the Host animates its content inset, honors reduced motion and keeps a minimum usable content height.

`current_route()` returns `Option<&CurrentRoute>` with public `name` and `port_type` strings from the current AVAudioSession outputs. The Host copies these into `MobileUi.route`. Route notifications request repaint; JSON is copied and parsed only when the native route revision changes. No network request, microphone permission, Bluetooth scan or invented device is involved.

For the `ios-route-picker` native slot, the Host calls `route_picker(rect, visible)` after each layout. Its rect includes the limn area's screen origin (`area.min`). The bridge converts to UIView points and places a visible, interactive `AVRoutePickerView` in that slot. The layout reserves a 44-point hit target. The Host passes false whenever the device sheet closes, another modal covers it, or no slot exists; false immediately hides the view and disables hit testing. Slots outside the window or with invalid geometry are also hidden. `set_route_picker_colors(tint, active)` uses the app palette. The system owns the picker button, accessibility and destination selection.

AVRoutePickerView presents its system route UI when tapped. There is no private-button invocation, artificial AVPlayer or programmatic attempt to open that UI. Initial Bluetooth pairing belongs to iOS Settings; the route sheet should describe this when a device is not yet paired. Device availability is owned by iOS, rather than a custom list of every nearby/paired speaker.

## Focus and touch boundary

limn's merged [stable-focus #46](https://github.com/Niiihuel/limn/pull/46), [bottom-sheet #47](https://github.com/Niiihuel/limn/pull/47) and [touch-hold/focus-dismissal #48](https://github.com/Niiihuel/limn/pull/48) supply reusable UI behavior. Surco chooses the commands and dismissal intent: Search-tab navigation waits for a tap, Home discovery focuses the field, and cancel/navigation/sheet opening clears focus deliberately. The Host consumes `MobileUi.blur` and forwards the final focused field to IME once per frame.

Touch holds and explicit overflow use the same action sheet; movement/cancel prevents activation on release. The library sheet captures the canonical item key, title and artwork instead of an index, while track actions capture the selected track. Form, confirmation, library and device sheets use the shared controlled drag interpreter; dragging a confirmation cancels it. Their scrolling bodies leave close controls and action footers visible in a short keyboard viewport.

## Validation

Rust tests cover docked/floating/hidden keyboard geometry, safe-area accounting, Retina/egui zoom, native-slot visibility, focus intent and gesture cancellation. Layout/state tests cover 320/390/430-point widths, short form viewports, overflow/hold equivalence and captured library/track identities. Native UIKit/AVKit compilation and linking, including the Host integration, passed in the intermediate [iOS run 38709439039](https://github.com/Niiihuel/surco/actions/runs/38709439039) at `dd9ef703d76ddde1eee29dc3737e89d3d7db9490` (simulator and unsigned device). Later UI closure changes still require final-head CI; [UI PR #3](https://github.com/Niiihuel/surco/pull/3) is pending merge.

Physical checks remain for the user: type in every input, rotate with keyboard visible, dismiss/reopen and drag sheets, cover the route button with another modal, select paired Bluetooth headphones/speakers and AirPlay, then background/foreground and lock the iPhone. Calls, headset controls and Dynamic Island behavior also require a supported physical device. Simulator startup does not establish those results; this document does not record a newly signed beta or final release validation.

## Primary references

- [Apple keyboard frame coordinate space](https://developer.apple.com/documentation/uikit/uiresponder/keyboardframeenduserinfokey): the notification's end frame uses screen coordinates; iOS 16.1+ identifies the UIScreen in `notification.object`.
- [Apple coordinate-space conversion](https://developer.apple.com/documentation/uikit/uicoordinatespace/convert(_:to:)-3imkt): convert from the notification screen into the attached view instead of assuming window and screen frames match.
- [Apple UIView](https://developer.apple.com/documentation/uikit/uiview): UIKit main-thread ownership, view bounds, subviews and content scale.
- [Apple AVRoutePickerView](https://developer.apple.com/documentation/avkit/avroutepickerview): visible system destination control; `prioritizesVideoDevices = false` for audio. Its `player` property is macOS-only and is not needed here.
- [Apple route-change handling](https://developer.apple.com/documentation/avfaudio/responding-to-audio-route-changes): current route outputs provide real port names and types.
- [raw-window-handle 0.6.2 UIKit handle](https://docs.rs/raw-window-handle/0.6.2/raw_window_handle/struct.UiKitWindowHandle.html): the public main-thread UIView pointer used by the eframe/winit host.
