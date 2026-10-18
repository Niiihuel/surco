# Search page reference

The reference is the locally installed Spotify desktop client, `1.2.90.451`. REA read
`Apps/xpui.spa`'s JavaScript, CSS and strings without running it, logging in or calling
Spotify (`xpui-routes-search.js`, `xpui-snapshot.js`). Some shared modules live in the V8
snapshot instead, so the history's length and the grid's column width are unknown.

## What Spotify does

- **The page before typing** is only Browse all. Recent searches drop down from the top
  bar's field while it is empty and focused.
- **The drop-down:** a raised panel, 8 px radius and padding, at most 398 px tall (590 at
  1200 px windows) and scrolling past that, shadowed `0 16px 24px #0000004d`, fading in
  over .22 s. "Búsquedas recientes" at 16 px bold over a row per entry: a 48 px picture,
  the name over its kind ("Canción • artista", "Artista"), a `#ffffff1a` hover, and a cross
  at the end shown only under the pointer or with focus. "Eliminar búsquedas recientes"
  below, behind a confirmation.
- **What is kept:** results that were opened, not typed queries.
- **Browse all tiles:** 16:9, 8 px radius, clipped. The title top-left, 16 px in, 24 px
  bold (16 px at 520 px or narrower), clear of the art's 30 %. The art is a square 45 % of
  the tile wide in the bottom-right corner, tilted 25°, 4 px radius, shadowed. Colours come
  from the server.
- **Loading:** a 48 px square and two bars with a shimmer in the drop-down, a skeleton
  shelf on the home page, and nothing on Browse all.

## What Surco does

The drop-down is the same, from the desktop's field. Surco keeps typed queries, which are
what it has to remember, each a row with the magnifier for picture; the cross clears one
and the button below all of them, at once. A click away closes it with the keyboard. limn
draws no rotation, so a genre's art is a record out of its sleeve, set square inside the
tile, whose clip is square. Every cover on its way pulses where it will be, in step, and the
home page's picks stand as skeleton shelves until they come.
