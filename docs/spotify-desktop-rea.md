# Desktop layout reference

The reference is the locally installed Spotify desktop client, `1.2.90.451.gb094aab0`.
Its `Apps/xpui.spa` was unzipped and its CSS read without running anything.
Class names are minified hashes, so the evidence is the design token or the hash.
Surco keeps its own components, palette and wording. It takes no green and no
branding.

## Measured

- **Shell:** a black window. Panels are `#121212` with an 8 px radius, an 8 px gap
  between them and around them (`--panel-gap`).
- **Top bar:** `48px + 2 × gap`. Home and the avatar are 48 px circles in `#1f1f1f`.
  The search is a pill in `#1f1f1f`: hover adds a 1 px `#ffffff1a` ring, focus a
  2 px white ring.
- **Library filter chips:** fully round, at least 32 px tall, `#ffffff1a`, `#ffffff24`
  under the pointer.
- **Cards:** 16 px padding with an 8 px radius, `#282828` under the pointer (.3 s).
  The play button sits 8 px from the bottom and end. It rises from `translateY(8px)`
  and opacity 0 over .3 s.
- **Sticky page header:** 64 px, fading in once the page scrolls.
- **Now-playing bar:** 72 px, in three zones (30 % / 40 % up to 722 px / 30 %).
- **Progress bar:** 4 px. The 12 px thumb shows only under the pointer or with focus.
- **Type:** titles at 24 / 32 / 48 px and headlines at 64 / 96 px. Text is `#fff`
  or `#b3b3b3`.
- **Motion:** 50–300 ms, eased `cubic-bezier(.3,0,0,1)`.

The panel widths, the track list's columns and the search width are set by script
the bundle does not ship, so they were not measured.

## Taken by Surco (0.8.0)

- Shelf tiles are inset cards whose play button rises under the pointer.
- The seek and volume bars show their thumb only under the pointer.
- A page's play button and name stay in a 64 px bar once its actions scroll away.
  The page's wash fades down through that bar, and the table's heads pin under it.
- In a track table the number turns into play under the pointer.
- Rows are 56 px, page titles 48 px, and the library's filters are pills.
- The middle panel's scrollbar floats over the page, so a page's wash reaches the
  panel's edge.
