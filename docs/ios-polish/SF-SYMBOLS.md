# Native symbols in the Rust canvas

On iOS the app registers a `limn_egui::IconRasterizer` before its first paint. UIKit
loads symbols through `UIImage systemImageNamed:withConfiguration:`; limn keeps
layout, hit testing, tint, accessibility labels and its existing texture cache.
The renderer is not registered by desktop builds.

Home/Library and transport use filled symbols; ordinary actions use outline
variants. Search keeps the standard magnifying glass. This follows Apple's
[SF Symbols design guidance](https://developer.apple.com/design/human-interface-guidelines/sf-symbols).
The exhaustive match covers the current 126 semantic variants plus 12 recognized
Lucide SVG assets. GitHub, colored file artwork and unknown custom marks retain
limn's original artwork. The Surco launch/app mark is unchanged.

Smart Shuffle stays distinguishable from ordinary Shuffle (`shuffle.circle`
versus `shuffle`). Connection buttons use a dedicated valid Lucide SVG that maps
to `airplay.audio`; `Icon::Audio` keeps `waveform` for media/level displays. The
portable fallback asset is Lucide's [airplay](https://github.com/lucide-icons/lucide/blob/main/icons/airplay.svg),
covered by the existing ISC notice in `LICENSES/ISC-lucide.txt`.

The public UIKit lookup may return nil when a symbol is unavailable on an older
OS; [the API documents that behavior](https://developer.apple.com/documentation/uikit/uiimage/init(systemname:withconfiguration:)).
Null leaves the SVG renderer available. No Apple symbol artwork is bundled or
exported, and no diagnostic scan runs during app startup.

## Buffer contract and validation

The Objective-C bridge returns an owned square white premultiplied RGBA buffer,
or null. Rust accepts physical sides 1–512, copies using
`ColorImage::from_rgba_premultiplied`, and frees the C allocation through RAII.
Using the unmultiplied constructor would multiply antialiased coverage again;
the regression test observed `[32,64,96,128]` becoming `[16,32,48,128]` before the fix.

The initial mapping/decoder tests ran RED (2 passed, 4 failed), then GREEN. Separate
RED→GREEN checks preserve Smart Shuffle's distinct state and verify that the actual
mobile connection button paints the routing glyph rather than a waveform.
Final local checks: 240 workspace tests passed with the same 3 existing external
service tests ignored, plus workspace/all-targets Clippy with warnings denied and
`--locked`. These checks used base `e975e16b6d19bac63f17a2aca61edaf6bef99daa` and
sibling limn `7907efe7b9d01a3b4293b9967cb6e89c57a523bc` without overrides.

Linux tests verify mapping, glyph selection, bounds and premultiplication. They do
not establish UIKit availability or native appearance. The final iOS integration
must pin the reviewed framework rasterizer from [limn PR #60](https://github.com/Niiihuel/limn/pull/60)
and validate native compilation and simulator screenshots before claiming it works
on iOS. A physical iPhone visual check remains distinct from simulator evidence.
