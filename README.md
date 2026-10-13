# dnmusic native

A desktop music player written entirely in Rust. The interface is drawn with
[limn](../lienzo); there is no web view.

## Crates

| Crate | What it is |
|---|---|
| `crates/app` | The window: state, views and the worker that talks to the network and the engine. |
| `crates/core` | `Track` and the play queue. No I/O. |
| `crates/youtube` | YouTube Music as a catalog and as a source of audio streams. |
| `crates/audio` | Decode, two-deck output with gapless and crossfade, equaliser. |

## Run

limn is used by path and must sit next to this repository as `../lienzo`.

```sh
nix-shell shell.nix --run "cargo run -p dnmusic"
nix-shell shell.nix --run "cargo test --workspace"
```

`--search <text>`, `--play <n>` and `--screenshot <file.png> --after <ms>` drive the
window from the command line, for checking a build without touching it.

## Credits

`crates/audio` is adapted from [noFAYZ/zuno](https://github.com/noFAYZ/zuno)
(Apache-2.0, see `LICENSES/Apache-2.0.txt`). Each adapted file names its origin
and what changed.
