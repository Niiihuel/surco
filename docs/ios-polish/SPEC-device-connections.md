# device-connections

Objective: a device sheet that reflects real iOS output and lets the user select Bluetooth speakers/headphones or AirPlay. Shared commands, style, structure and boundaries: [contract](README.md).

Acceptance:
- Show the current route's real name/type and accessible entry points from player/mini-player/settings.
- Host a visible native AVRoutePickerView through public UIView/raw-window-handle APIs; keep its frame/visibility aligned with the limn layout. No private-button tapping or fake AVPlayer.
- Keep route changes synchronized with playback recovery and paused state; pairing instructions direct unpaired Bluetooth devices to the system.
- Native bridge remains in Surco. Any generic native-view layout primitive required by the framework is a separate reviewed limn change.
- Test route-state rendering and lifetime/visibility decisions; validate native compile/link. Physical Bluetooth/AirPlay selection still requires actual devices.
