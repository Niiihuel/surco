# Settings and Bluetooth output reference

The requested reference is the locally installed Spotify desktop client,
`1.2.90.451.gb094aab0`. Surco keeps its own components, palette and wording.
Bluetooth speakers and headphones are the user's selected priority.

## Evidence and scope

REA opened and extracted `Apps/xpui.spa` without executing its JavaScript.

- Archive SHA-256: `1a9a055b98f37dd3c56afd4e86288e35c0697e1db02971cb56a7542811e3c777`.
- Extraction Evidence: `ev_6f486b9006a0065cf1a10f33a0ba749be50d7f4910078b962e02cace9b7570cd`.
- Artifact manifest: `agm_1a8085935367ba426a2a2415b8ee442dd66e0494d89f3f24e42be39e950de517`.
- Focused settings analysis: `ev_a2384b343d826b9538cd8e999fba8242697173e65ca73d2c643ffcc3c1eccad5`.
- Focused connections analysis: `ev_7eae3aa84e76358e5556115f9f62fa41c36c7b2fbb1e91b558a24da7ef4d636f`.

The focused directories contain unchanged files from that extraction. Settings
analysis parsed both selected scripts without parse failures. Connections
analysis parsed three selected scripts, retaining static/module facts but
reporting partial graph coverage after resource limits. These are observations
of shipped resources and static candidates, not a recording of Spotify running.
Native service internals, account-dependent availability and wireless hardware
behavior were not established by this inspection.

## Findings

`xpui-routes-desktop-settings.css` declares a centered settings column capped at
900 px, with 32 px padding and 24 px between blocks. Rows separate explanatory
copy from controls; a stacked variant handles narrower space. The corresponding
script groups playback, quality, output, appearance, library, storage and other
preferences. Surco adopts the hierarchy and roomier spacing using limn.

Spotify exposes two different device paths:

- **Local audio output:** `xpui-routes-desktop-settings.js`, Webpack module
  `21525`, calls `AudioOutputDevicesAPI`, reads `audioDevices()`, selects with
  `setSelectedAudioOutputDevice()`, and subscribes to device-list/selection
  changes. It includes the system default and unavailable-device handling.
- **Remote Connect:** `dwp-panel-section.js`, module `24055`, contains the current
  device, available destinations, unavailable/restricted states, discovery,
  playback transfer and device menus. `dwp-now-playing-bar.js`, module `45273`,
  exposes the panel from the player. Remote controls call `transferPlayback()`;
  discovery calls `startActiveDiscovery()`. Cast and session features belong to
  that service path.

`9738.js`, module `95714`, reads physical-output information, including Bluetooth
transport/accessory metadata. This does not demonstrate Bluetooth pairing inside
the Spotify panel. Copying Connect's presentation would not implement its remote
transport in Surco.

## Surco design and behavior

Settings use a readable column, clear section headings, larger controls and rows
that stack at narrow widths. Connections are near the top. Existing library
sort/view and panel-visibility preferences are exposed as additional working
options. Clearing recently opened items is named to match its actual scope.

Connections use the existing output IDs and audio engine. Users pair speakers
or headphones in their operating system, refresh Surco's list, then select the
output. The system-default route remains available. Refresh has loading and
failure states. Device-switch failures reported by the engine produce a notice;
falling back to the system route is only announced after that operation succeeds.
Some existing fallbacks inside the audio engine remain silent. An engine that
cannot start still requires restarting Surco after fixing the system audio setup.

Linux enumerates PipeWire/PulseAudio sinks with `pactl`; other targets use CPAL's
device enumeration. Bluetooth works when the operating system exposes the
connected device as an audio output. This change does not add a Bluetooth pairing
stack, Spotify Connect, Chromecast, AirPlay or DLNA transport.

## Verification

State tests cover refresh failure/retry, retained device selection and output
fallback notices. Layout tests cover narrow/wide controls, accessible names and
connection states. Runtime captures use isolated temporary configuration/data.
Physical Bluetooth playback still requires a paired speaker or headphone for
end-to-end hardware verification.
