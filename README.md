# Surco

A native music player for the desktop, written entirely in Rust. The interface is drawn
with [limn](https://github.com/Niiihuel/limn); there is no web view. *Surco* is the
groove of a record: where the needle travels.

![The home page: mixes and picks made from what is played](docs/screenshots/home.jpg)

It plays without an account. What is liked, kept and listed stays on the machine that
plays it; a playlist is shared as a link that carries it whole.

## What it does

- Search as you type, artist and album pages, and a home page that grows with what is
  played: mixes of the artists played most, a card for the time of day, what is played
  again and again, picks for fans of an artist and news from the ones followed.
- Gapless playback with crossfade, a ten-band equaliser, even loudness and a choice of
  output device and quality.
- Synced lyrics, the song's own video beside it or over the whole window, the queue,
  shuffle with the catalog's picks woven in.
- A library that searches, sorts, pins and lists as lines, rows or tiles; local
  playlists, likes and downloads for listening without a network.
- Import public Spotify, YouTube Music and Apple Music playlists or albums by pasting
  their links in the search bar, or from the library and settings. Export a playlist
  as artist-and-title text to paste into a playlist transfer service. Spotify imports
  read up to 100 tracks; songs that cannot be matched are reported after the import.
- A bell for what the artists followed release, and for later versions of the app.
- A local profile with a picture of its own, and the account moved to a phone or another
  computer on the same network, either way: a camera reads its QR, or its twelve-character
  code is typed.

| | |
|---|---|
| ![What plays, its video tall at the top of the right panel](docs/screenshots/playing.jpg) | ![Synced lyrics with their translation](docs/screenshots/lyrics.jpg) |
| ![A playlist and a song's menu](docs/screenshots/playlist.jpg) | ![Search results as they are typed](docs/screenshots/search.jpg) |
| ![The local profile](docs/screenshots/profile.jpg) | ![Moving the account to another device](docs/screenshots/handoff.jpg) |

## Crates

| Crate | What it is |
|---|---|
| `crates/app` | The window: state, views and the worker that talks to the network and the engine. |
| `crates/core` | `Track`, the play queue, lyrics and the picks of the home page. No I/O. |
| `crates/youtube` | YouTube Music as a catalog and as a source of audio and video streams. |
| `crates/audio` | Decode, two-deck output with gapless and crossfade, equaliser. |
| `crates/video` | Reads a fragmented MP4 and decodes its H.264 pictures. |

## Build

limn is used by path and must sit next to this repository as `../lienzo`.

```sh
cargo run -p surco
cargo test --workspace
```

On NixOS, through the shell that brings the system libraries:

```sh
nix-shell shell.nix --run "cargo run -p surco"
```

On other Linux systems the build needs the development packages of ALSA, and at run time
Wayland or X11 with libxkbcommon. `--search <text>`, `--play <n>` and
`--screenshot <file.png> --after <ms>` drive the window from the command line, for
checking a build without touching it.

## iOS

The iOS target keeps the engine and interface in Rust with limn. `cargo run -p surco -- --mobile`
previews the phone layout on a desktop. The **iOS** GitHub Actions workflow builds an
iPhone simulator app and an unsigned device app; the simulator run saves a screenshot.
Anyone can put it on their own iPhone from a fork, with or without a Mac or a paid Apple
account: see [Build Surco for your iPhone](docs/ios-build-yourself.md). The project's own
TestFlight setup is in [the TestFlight guide](docs/testflight.md), and the remaining
hardware checks in [the iOS validation notes](docs/ios.md).

## Releases

A tag `v1.2.3` that matches the version in `Cargo.toml` builds Linux, Windows and macOS
and publishes them together. A running app finds the release, fetches the program of its
system, checks it against the release's sums and puts it in its own place; the user
restarts when they choose. `minimum` in `Cargo.toml` is the oldest version that may keep
running: a build older than the minimum of the latest release has to take it in before
it goes on.

## License

Surco is free software under the [GNU General Public License](LICENSE), version 3 or
later.

`crates/audio` is adapted from [noFAYZ/zuno](https://github.com/noFAYZ/zuno)
(Apache-2.0, see `LICENSES/Apache-2.0.txt`); each adapted file names its origin and what
changed. The icons under `crates/app/assets/icons` are from Lucide (ISC, see
`LICENSES/ISC-lucide.txt`). The interface is set in
[Figtree](https://github.com/erikdkennedy/figtree) (SIL Open Font License 1.1, see
`LICENSES/OFL-1.1-figtree.txt`), and Replay's headlines in
[Dela Gothic One](https://github.com/syakuzen/DelaGothic) (SIL Open Font License 1.1, its Latin
letters only, see `LICENSES/OFL-1.1-dela-gothic-one.txt`), the fonts under
`crates/app/assets/fonts`.

The import illustration uses service icons from [Simple Icons](https://simpleicons.org)
([CC0](https://github.com/simple-icons/simple-icons/blob/develop/LICENSE.md)). The
Spotify, YouTube Music and Apple Music marks identify supported sources and belong to
their respective owners. Surco is an independent project and is not affiliated with
these services.
