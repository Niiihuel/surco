# Changelog

## 0.1.3

### Added

- Spotify playlists longer than 100 tracks import whole. When Spotify's web player
  service cannot be reached, the first 100 still import.
- An import shows how many of its songs have been looked up, out of how many.
- After an import, the playlist lists the songs that were not found, ready to copy.
- The bell shows how many things it has not shown yet.
- The notice of a later version shows that release's notes, with their links and
  pictures. A required update shows a progress bar.

### Changed

- An imported song is searched for in up to three ways before it is given up, which
  finds recent releases the catalog did not return at first.

### Fixed

- The library's search field fills its row.
- The search field's cross button clears what was typed, and a pasted link no longer
  stays in the field after its list comes in.

## 0.1.2

### Added

- Import public playlists and albums from Spotify, YouTube Music and Apple Music
  through the library, settings or search bar. Spotify imports read up to 100 tracks.
- Copy a playlist as `Artist - Title` text for use in playlist transfer services.
- Clear the search field with its new cross button.
- Windows window controls in Surco's own title bar: drag, minimize, maximize,
  restore and close.

### Changed

- When playback is the only animation, reduce window drawing to about 30 frames
  per second. User interaction keeps the display's normal refresh rate.

### Fixed

- Public YouTube Music links import correctly, including album playlists whose
  response has no playlist header.
