---
version: 1
slug: "crates-app-src-view-mobile-settings-rs"
primary_target: "crates/app/src/view/mobile/settings.rs"
related_targets: ["crates/app/src/view.rs"]
---

# Surface: phone Settings

Scope: the phone's Settings page (`crates/app/src/view/mobile/settings.rs`), the import sheet and the handoff sheet it opens. Mode: Operate. The desktop and TV settings are out of scope.

Audience and job: someone on their phone, often while music plays. They come here to bring a playlist in from another app, to move their music to another device on the same Wi-Fi, or to flip a sound or playback switch.

Constraints: BitChord's dark look and aurora's liquid glass. iOS-grouped inset cards. Touch rows of at least 52 pt. Moving data is local network only.

## Direction contract

THESIS: Settings is one grouped list, as iOS Settings is. You see the switches without opening anything, and "Tu música" (import, move to another device) comes first. It refuses the list of eight parts that each hide a few switches behind a chevron.

OWN-WORLD:
- Black canvas, raised #1c1c1e cards with radius 14, and 0.5 hairlines that start under the words.
- Row glyphs are bare white 22 pt line icons, never on a tile; the profile's face is the only filled mark.
- Small bold uppercase captions in muted white.
- Apple's green switches; white words; destructive red only on rows that erase something.
- Glass only on the header's circles and the bars.

STORY: The visitor sees their profile and the two moves at once. Each move opens a sheet that does it in one step: paste a link and see the list, or show the QR and the code. Then they tune sound and playback inline.

FIRST VIEWPORT:
1. The large title "Configuración".
2. The profile row: face 52, name, and "N playlists · N me gusta".
3. "TU MÚSICA" with "Importar playlist" and "Pasar a otro dispositivo", each with a bare white glyph.
4. "SONIDO" with its switches and the equaliser.

Signature move: the import sheet recognises the link's service as it is pasted, then names the list and its song count before "Importar".

FORM: grouped single list, structure 2 of 6 on the ordered list; seed:28f696bb97b0870c.

FINISH: unreviewed and undocumented is unfinished; this build ends with the finish review, the verdict, DESIGN.md, and every shipping raster carrying its provenance
