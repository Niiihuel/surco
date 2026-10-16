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

## System clipboard and external links

`NativeUi::copy_text` writes to the iPhone's general pasteboard after an explicit
copy action. Song/playlist sharing and limn's Copy/Cut actions use the same path.
The synchronous FFI passes UTF-8 bytes with their length; NSString copies them,
so Unicode and embedded NULs do not truncate the payload.

Search uses one native `UISearchBar` attached to the canvas's `ios-search` slot.
UIKit owns its editing menu, clear and Cancel controls, selection, caret and
keyboard shortcuts. Rust owns query debounce, recent searches, results and
navigation. Session and revision checks discard stale text/focus callbacks;
ordinary results and reflow retain the live editor, including marked IME text.
Return submits and dismisses the keyboard; Cancel keeps the query and results.

The playlist migration card uses a native `UIPasteControl` on iOS 16 and later.
Its recipient accepts the representations supported by
`NSString` and `NSURL`, including UTF-16 text. A URL representation that cannot be
loaded falls back to one available text representation. Results are bound to the
original target and discarded if the control is hidden, retargeted or destroyed.
Pasting opens an editable draft; it never starts importing automatically.
iOS 15 uses an explicit-tap fallback for the card.

The import drawer contains a native `UITextField` with a `UIPasteControl` beside
it. UIKit owns selection, the **Pegar** editing menu and physical-keyboard
shortcuts; the visible control exists because iOS may offer AutoFill instead of
the edit menu on an empty URL field. Pasting replaces the whole draft, including
while the field is editing, and never prompts for pasteboard access. Typing
updates Rust's draft without replacing the live field or caret. Return dismisses
the keyboard; only the explicit Import action fetches a playlist. iOS 15 uses an
explicit-tap fallback.

## Header and tab bar

Outside the player and lyrics, UIKit draws the phone's header and tab bar over
the `ios-header` and `ios-tabs` slots that the limn layout reserves. Rust sends
the title, the header's buttons, the tabs and their selection; UIKit returns only
the id of a button from the last payload, which Rust validates before acting.
Rows scroll beneath both bars. The header follows iOS 26's scroll-edge
convention: a blur and the page's tint fade out below it, and its buttons are
Liquid Glass circles on iOS 26 (a translucent circle before). The tab bar uses
the Spotify iPhone kit: SF Symbols with labels, white when selected, over a
canvas scrim that is clear above the mini-player and solid by the tab labels.
Both fade instead of popping while the player slides over them.

`NativeUi::paste_text` remains the explicit paste bridge for canvas-editor
fallbacks. The Host binds a request to its original field and checks that it is
still visible, focused, enabled and outside an open context menu before reading.
`PasteTo` delivers the result to that field. Empty/unavailable text leaves it
unchanged. Normal search text follows typing's debounce; links still wait for
explicit submission.

Clipboard contents are never polled, logged or read to decide whether a button is
enabled. iOS may ask for paste permission; the bridge respects its result. The
native editors provide their own editing menu and keyboard shortcuts. Any
explicit egui paste event for a canvas fallback is redirected to the system
clipboard rather than its app-local fallback.

Surco enables eframe's `links` feature so its existing external-link actions use
webbrowser's public UIKit implementation. No separate URL-opening bridge is needed.

## Validation

Rust tests cover docked/floating/hidden keyboard geometry, safe-area accounting, Retina/egui zoom, native-slot visibility, focus intent and gesture cancellation. Layout/state tests cover 320/390/430-point widths, short form viewports, overflow/hold equivalence and captured library/track identities. Native UIKit/AVKit compilation and linking, including the final Host/UI closure changes, passed in [iOS run 38711825772](https://github.com/Niiihuel/surco/actions/runs/38711825772) for code head `16bd78e21c67a2563ff6999eb7030ef9303ff502` (simulator and unsigned device). [Desktop CI](https://github.com/Niiihuel/surco/actions/runs/38711825798) also passed, and [UI PR #3](https://github.com/Niiihuel/surco/pull/3) is merged.

The later [clipboard/link PR #4](https://github.com/Niiihuel/surco/pull/4), code head
`787764e5d4aed7b899fdadf8d4629e30d57a7ab7`, passed
[iOS simulator/device CI](https://github.com/Niiihuel/surco/actions/runs/38717476331)
and [Linux/macOS/Windows CI](https://github.com/Niiihuel/surco/actions/runs/38717476437).
PR #4 merged into `main` as `e5c0e8df3a2a2ee356f104d261b4f97aca77c54b`.
Local validation passed 189 workspace tests (3 existing network tests ignored)
and workspace/all-target Clippy with warnings denied. Paste layout is tested at
320/390/430 points, including revealing a focused import field that was scrolled
offscreen at 320×240. Rendered search and import controls were inspected separately.

Physical checks remain for the user: type in every input, rotate with keyboard visible, dismiss/reopen and drag sheets, cover the route button with another modal, copy to another app and paste back into search/import, select paired Bluetooth headphones/speakers and AirPlay, then background/foreground and lock the iPhone. Calls, headset controls and Dynamic Island behavior also require a supported physical device. Simulator startup does not establish those results. Signed build details belong in [the TestFlight delivery guide](testflight.md).

## Primary references

- [Apple keyboard frame coordinate space](https://developer.apple.com/documentation/uikit/uiresponder/keyboardframeenduserinfokey): the notification's end frame uses screen coordinates; iOS 16.1+ identifies the UIScreen in `notification.object`.
- [Apple coordinate-space conversion](https://developer.apple.com/documentation/uikit/uicoordinatespace/convert(_:to:)-3imkt): convert from the notification screen into the attached view instead of assuming window and screen frames match.
- [Apple UIView](https://developer.apple.com/documentation/uikit/uiview): UIKit main-thread ownership, view bounds, subviews and content scale.
- [Apple native editing menus](https://developer.apple.com/documentation/uikit/uieditmenuinteraction): standard `UITextField` editors are preconfigured for selection and editing actions.
- [Apple paste configuration](https://developer.apple.com/documentation/uikit/uipasteconfiguration): derive accepted representations from the `NSString` and `NSURL` classes the recipient loads.
- [Apple AVRoutePickerView](https://developer.apple.com/documentation/avkit/avroutepickerview): visible system destination control; `prioritizesVideoDevices = false` for audio. Its `player` property is macOS-only and is not needed here.
- [Apple route-change handling](https://developer.apple.com/documentation/avfaudio/responding-to-audio-route-changes): current route outputs provide real port names and types.
- [raw-window-handle 0.6.2 UIKit handle](https://docs.rs/raw-window-handle/0.6.2/raw_window_handle/struct.UiKitWindowHandle.html): the public main-thread UIView pointer used by the eframe/winit host.
