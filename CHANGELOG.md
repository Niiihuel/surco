# Changelog

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
