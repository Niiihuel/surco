# Surco

The native desktop player of [dnmusic](https://github.com/Niiihuel/dnmusic), written
entirely in Rust. The interface is drawn with [limn](https://github.com/Niiihuel/limn);
there is no web view. *Surco* is the groove of a record: where the needle travels.

It plays without an account. What is liked, kept and listed stays on the machine that
plays it; a playlist is shared as a link that carries it whole.

## What it does

- Search, artist and album pages, and a home page of picks made from what is played.
- Gapless playback with crossfade, a ten-band equaliser, even loudness and a choice of
  output device and quality.
- Synced lyrics, the song's own video beside it or over the whole window, the queue,
  shuffle with the catalog's picks woven in.
- A library that searches, sorts, pins and lists as lines, rows or tiles; local
  playlists, likes and downloads for listening without a network.
- A bell for what the artists followed release, and for later versions of the app.

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
cargo run -p dnmusic
cargo test --workspace
```

On NixOS, through the shell that brings the system libraries:

```sh
nix-shell shell.nix --run "cargo run -p dnmusic"
```

On other Linux systems the build needs the development packages of ALSA, and at run time
Wayland or X11 with libxkbcommon. `--search <text>`, `--play <n>` and
`--screenshot <file.png> --after <ms>` drive the window from the command line, for
checking a build without touching it.

## License

Surco is free software under the [GNU General Public License](LICENSE), version 3 or
later.

`crates/audio` is adapted from [noFAYZ/zuno](https://github.com/noFAYZ/zuno)
(Apache-2.0, see `LICENSES/Apache-2.0.txt`); each adapted file names its origin and what
changed. The icons under `crates/app/assets/icons` are from Lucide (ISC, see
`LICENSES/ISC-lucide.txt`).
