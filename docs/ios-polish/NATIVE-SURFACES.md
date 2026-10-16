# Native iOS surfaces

The player, pages, playback engine and app state remain in Rust/limn. UIKit owns the iOS search editor, context-menu interaction, paste control and modal sheet presentation. The reusable horizontal cancellation and scroll-edge geometry fixes belong to limn.

Search reserves one `ios-search` slot for a stable `UISearchBar`. Native text and
focus events share a monotonic revision inside an opaque session; Rust validates
both that session and the current navigation/editing intent. Results arriving do
not recreate the editor or reset its caret. Marked IME text stays with UIKit;
rotation waits for corrected geometry without interpreting an old rectangle as a
request to resign focus. Cancel retains the query, and Return explicitly submits
text or a pasted link. The native editor supplies Paste and Clear itself.

`SurcoSurfaces` attaches a `UIContextMenuInteraction` to the existing canvas, rather than adding transparent views over cards. It snapshots the selected menu before presenting. Rust captures the corresponding track/collection target and only accepts actions offered for that opening. A native menu ends after its selection has been queued, preserving that target across list updates.

`SurcoSheet` renders controlled text, fields, actions, segments, sliders and the visible system route picker. `UISheetPresentationController` handles medium/large detents, interactive dismissal, transitions and scroll handoff. The header, fields and actions share one scroll viewport above the keyboard layout guide, so landscape editing keeps its full touch target. Native fields keep their first responder and scroll position across model updates; editing, viewport or text-size changes reveal the active field without rebuilding it. The import field occupies the full form width and uses `UITextField`'s standard selection, Paste editing menu and keyboard shortcuts, without a second Paste button. Pasting changes the draft only; importing still requires its explicit action. Confirmation uses `UIAlertController`.

The migration card uses `UIPasteControl` on iOS 16 and later with an explicit `UIPasteConfigurationSupporting` recipient. It accepts the representations declared by `NSURL` and `NSString`, including UTF-16 text copied from native apps. If a promised URL cannot be loaded, one available text representation is tried; a tap delivers at most one result. Availability checks do not read pasteboard text. An asynchronous result is discarded if the control was hidden, retargeted or destroyed. iOS 15 retains an explicit-tap fallback for strings and URLs.

The bridge runs on the main thread and passes copied UTF-8 JSON. Native events carry the menu/sheet identity; Rust rejects stale or unoffered actions. Payloads are cached until content, scale or geometry changes. Destroying or changing the backing canvas detaches native controls and invalidates callbacks.

iOS icons use the device's SF Symbols through `surco_ui_symbol(name, length, pixels)`. The main-thread call returns a `pixels × pixels` white premultiplied RGBA mask, or NULL for an unavailable symbol. The caller releases it with `surco_ui_free_symbol`. limn caches this mask by semantic icon and physical size, then applies its existing color and opacity. Rust maps only interface symbols; Surco artwork keeps its identity. Other platforms retain their existing icon pack. Tab icons use filled variants with text labels, following Apple's [SF Symbols](https://developer.apple.com/design/human-interface-guidelines/sf-symbols) and [tab bar guidance](https://developer.apple.com/design/human-interface-guidelines/tab-bars).

Acceptance requires the Rust controller regressions, normal dependency-graph workspace tests, framework checks, a real Xcode build and native interaction tests. Linux source review and a startup screenshot alone do not establish native gesture, paste or editing behavior.

Primary API references:

- [Context-menu interaction](https://developer.apple.com/documentation/uikit/adding-context-menus-in-your-app)
- [Sheet presentation](https://developer.apple.com/documentation/uikit/uisheetpresentationcontroller)
- [Paste control](https://developer.apple.com/documentation/uikit/uipastecontrol)
- [Paste configuration](https://developer.apple.com/documentation/uikit/uipasteconfiguration)
- [Native editing menus](https://developer.apple.com/documentation/uikit/uieditmenuinteraction)
- [Keyboard layout guide](https://developer.apple.com/documentation/uikit/uiview/keyboardlayoutguide)
- [Scroll-edge conventions](https://developer.apple.com/design/human-interface-guidelines/scroll-views)
