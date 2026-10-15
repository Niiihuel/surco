# Native iOS surfaces

The player, pages, playback engine and app state remain in Rust/limn. UIKit owns the iOS context-menu interaction, paste control and modal sheet presentation. The reusable horizontal cancellation and scroll-edge geometry fixes belong to limn.

`SurcoSurfaces` attaches a `UIContextMenuInteraction` to the existing canvas, rather than adding transparent views over cards. It snapshots the selected menu before presenting. Rust captures the corresponding track/collection target and only accepts actions offered for that opening. A native menu ends after its selection has been queued, preserving that target across list updates.

`SurcoSheet` renders controlled text, fields, actions, segments, sliders and the visible system route picker. `UISheetPresentationController` handles medium/large detents, interactive dismissal, transitions and scroll handoff. Native fields keep their first responder and scroll position across model updates; the keyboard layout guide positions the footer. Confirmation uses `UIAlertController`.

The migration card uses `UIPasteControl` on iOS 16 and later with an explicit `UIPasteConfigurationSupporting` recipient. Availability checks do not read pasteboard text. An asynchronous result is discarded if the control was hidden, retargeted or destroyed. iOS 15 retains an explicit-tap fallback.

The bridge runs on the main thread and passes copied UTF-8 JSON. Native events carry the menu/sheet identity; Rust rejects stale or unoffered actions. Payloads are cached until content, scale or geometry changes. Destroying or changing the backing canvas detaches native controls and invalidates callbacks.

Acceptance requires the Rust controller regressions, normal dependency-graph workspace tests, framework checks, a real Xcode build and native interaction tests. Linux source review and a startup screenshot alone do not establish native gesture, paste or editing behavior.

Primary API references:

- [Context-menu interaction](https://developer.apple.com/documentation/uikit/adding-context-menus-in-your-app)
- [Sheet presentation](https://developer.apple.com/documentation/uikit/uisheetpresentationcontroller)
- [Paste control](https://developer.apple.com/documentation/uikit/uipastecontrol)
- [Keyboard layout guide](https://developer.apple.com/documentation/uikit/uiview/keyboardlayoutguide)
- [Scroll-edge conventions](https://developer.apple.com/design/human-interface-guidelines/scroll-views)
