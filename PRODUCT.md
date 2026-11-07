# Product

<!-- impeccable:product-schema 1 -->

## Platform

adaptive

One app on the desktop (Linux, Windows, macOS), Android phones, iPhone and Android TV. The phone keeps one look on both systems, after BitChord in a dark theme with liquid glass ported from the aurora shell; on iOS 26 UIKit draws the bars and some surfaces natively. Android TV has its own focus-driven layout.

## Users

People who want to listen to music without signing up for anything. Typical jobs: search and play songs, keep playlists and likes, and listen offline from downloads. Some bring their lists from Spotify, Apple Music or YouTube Music, and some move their library between their own phone, computer and TV.

## Product Purpose

A native music player written in Rust, drawn with limn, with no web view and no account. It plays from YouTube Music's catalog, with gapless playback, crossfade, a ten-band equaliser, synced lyrics and the song's video. Success means music plays at once, and what the person keeps stays theirs.

## Positioning

No account and no server of its own. What is liked, kept and listed lives only on the device that plays it. It moves between devices directly over the local network, read from a QR code or a twelve-character code. A playlist is shared as a link that carries the whole list.

## Operating Context

- **Phone:** used one-handed, often while music already plays. The mini player and the floating tab bar stay on screen.
- **Desktop:** Spotify-like panels.
- **TV:** worked with a remote.
- **Importing a list:** it starts in another app (Spotify, Apple Music, YouTube Music), where the person copies the list's link.
- **Moving the account:** both devices are in the same place, on the same Wi-Fi.

## Capabilities and Constraints

- Imports public Spotify, Apple Music and YouTube Music playlists and albums from their links. Songs the catalog lacks are reported after the import.
- Exports a playlist as artist-and-title text.
- Hands off the account over the local network. Parts can be left out: profile, playlists, liked songs, library, settings, equaliser and history.
- Handoff works in both directions. A camera reads the QR code, or the code is typed.
- Moving data happens over the local network only. There is no file export and no cloud (confirmed 2026-11-06).
- Scrobbles to Last.fm and ListenBrainz.
- SponsorBlock categories, output device choice, and streaming quality of up to 140 kbps.
- UI strings in Spanish (es, the default) and English, through Fluent.

## Brand Commitments

- Name: Surco, "the groove of a record".
- The phone's look follows BitChord's video faithfully, dark-themed: measures are in `docs/bitchord-look.md`.
- Motion and glass follow the user's aurora QML shell: liquid glass, spring motion and morphing popovers.

## Evidence on Hand

- Screenshots in `docs/screenshots/`.
- The BitChord reference measures in `docs/bitchord-look.md`.
- No testimonials, user counts or press exist. Do not invent them.

## Product Principles

1. Nothing between the person and the music: no sign-up, no wall, no nag.
2. What is kept belongs to the person and the device. Moving it is explicit, local and reversible in what it carries.
3. The fewest steps for the common case. Detail stays available but folded.
4. Native feel on each device: touch targets, safe areas, back gestures and reduced motion are honoured.

## Accessibility & Inclusion

- Touch targets of at least 44 pt.
- Reduce Motion is honoured: springs become crossfades.
- Controls expose roles and names to VoiceOver and TalkBack through limn's access tree.
