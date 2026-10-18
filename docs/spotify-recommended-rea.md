# Playlist recommendations reference

The reference is the locally installed Spotify desktop client, `1.2.90.451`.
REA extracted `Apps/xpui.spa` and read its JavaScript without running it, logging in
or calling Spotify. The logic is in `1470.js` (module `48630`) and
`xpui-routes-playlist.js`. Surco keeps its own components, palette and wording.

## What Spotify does

- **Request:** `PlaylistAPI.getRecommendedTracks(playlistUri, skipIds, count)`. The
  server picks the seeds from the playlist. `skipIds` are every track already fetched
  for it. The native binary names `playlistextender/ft/v2/extend-playlist`.
- **Shelf:** 10 shown. A hidden queue behind them refills when it holds fewer than 20,
  asking for at most 20 more and holding at most 30.
- **Refresh:** no request of its own. It drops the 10 shown and the next 10 move up;
  the refill then fetches more, skipping everything seen. An answer short of what was
  asked clears the skip list, so songs may come round again.
- **Add:** appends the track to the end of the playlist and takes it off the shelf; the
  next one in the queue takes its place. Removing a song from the playlist also takes
  it out of the recommendations.
- **When:** any playlist that takes tracks. With no tracks the subtitle says it is
  based on your listening. Nothing renders until there is a recommendation; there is
  no loading or error state.
- **Layout:** "Recommended" over "Based on what's in this playlist". Rows are cover
  with title and artist, the album, and an "Add" pill. Refresh sits below the list.

## What Surco does

YouTube Music has no playlist extender, so the seeds are chosen here: five of the
playlist's songs, its latest first and others on each later round, or the songs the
user plays most for an empty one. Their radios are blended by `discover::blend`, without
the playlist's songs or any already offered, one artist at most twice. Shelf, refresh,
add, refill and the coming round again follow Spotify's numbers above
(`SUGGESTED_SHOWN`, `SUGGESTED_LOW` in `state.rs`).
