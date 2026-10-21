# Words several parts of the app share.

common-artist = Artist
common-artists = Artists
common-album = Album
common-albums = Albums
common-song = Song
common-songs-heading = Songs
common-top-result = Top result
common-songs = { $count ->
    [one] 1 song
   *[other] { $count } songs
}
common-playlist = Playlist
common-cancel = Cancel
common-close = Close
common-delete = Delete
common-remove = Remove
common-clear = Clear
common-retry = Retry
common-play = Play
common-pause = Pause
common-options = Options
common-song-options = Song options
common-share-song = Share song
common-like = Like
common-liked-songs = Liked Songs
common-downloads = Downloads
common-settings = Settings
common-equalizer = Equalizer
common-library = Your Library
common-search = Search
common-search-placeholder = What do you want to listen to?
common-back = Back
common-all = All
common-follow = Follow
common-following = Following
common-link-copied = Link copied
common-no-results = No results
common-next-song = Next song
common-create-playlist = Create playlist
common-import = Import
common-creator = Creator
common-code = Code
common-title = Title
common-minutes = { $minutes } min
common-hours-minutes = { $hours } h { $minutes } min
common-language = Language
common-language-detail = The app's words, in this language.
common-recommended = Recommended
common-recommended-detail = Based on what's in this playlist
common-recommended-listening = Based on your listening
common-recommended-add = Add
common-recommended-refresh = Refresh
common-recommended-add-title = Add { $title } to the playlist
common-refresh = Refresh
common-patches = Release notes
common-patches-detail = What each version of Surco brought
common-patches-current = This version
common-patches-date = { $month }/{ $day }/{ $year }
common-patches-failed = We couldn't get the release notes
common-watch-video = Watch the video
common-translate-lyrics = Translate the lyrics
common-original-lyrics = Show the original lyrics
common-welcome-title = Bring your music. We'll find you more.
common-welcome-detail = Your playlists from Spotify, Apple Music or YouTube Music, in Surco in a few simple steps.
common-welcome-import = IMPORT NOW
common-settings-playback = Playback
common-settings-playback-about = Autoplay • Crossfade
common-settings-audio = Audio quality
common-settings-audio-about = Quality • Normalize • Equalizer
common-settings-devices = Connections and devices
common-settings-devices-about = Audio output • Bluetooth
common-settings-display = Display and language
common-settings-display-about = Motion • Videos • Language
common-settings-library = Library
common-settings-library-about = Order • View • Import music • Scrobbling
common-settings-about = About Surco
common-settings-about-about = Version { $version } • Release notes
common-settings-storage = Storage and privacy
common-settings-storage-about = Downloads • Recent
common-settings-transitions = Transitions between songs
common-settings-controls = Playback controls
common-settings-streaming = Streaming quality
common-settings-sound = Sound
common-settings-your-library = Your library
common-settings-bring = Bring music in
common-clear-recent-searches = Clear recent searches

## First run
common-onboarding-welcome = Welcome to Surco
common-onboarding-welcome-detail = Your music, on your computer. No servers and no cloud accounts.
common-onboarding-fresh = Start fresh
common-onboarding-bring = I already have Surco on another device
common-onboarding-name = What should we call you?
common-onboarding-name-detail = It shows on your profile. You can change it whenever you like.
common-onboarding-name-placeholder = Your name
common-onboarding-genres = What do you like to listen to?
common-onboarding-genres-detail = Choose the genres you want. They help us show you artists.
common-onboarding-artists = Choose 3 or more artists you like.
common-onboarding-artists-detail = Your Home is made from them. Picking one brings others like them.
common-onboarding-search = Search artists
common-onboarding-left = { $count ->
    [one] 1 to go
   *[other] { $count } to go
}
common-onboarding-chosen = { $count ->
    [one] 1 chosen
   *[other] { $count } chosen
}
common-onboarding-next = Next
common-onboarding-ready = Done
common-onboarding-skip = Skip
common-onboarding-done = You're all set, { $name }!
common-onboarding-done-detail = We made your Home from what you chose.
common-onboarding-home = Go to Home
common-onboarding-step = Go to step { $step }
common-onboarding-taste = Choose artists again
common-onboarding-taste-detail = The genres and artists your Home is made from.
common-scan-code = Scan code
common-onboarding-bring-title = Bring your account
common-onboarding-bring-detail = On the other device, open Settings › Move to another device › Send.
common-onboarding-bring-type = Or type the address it shows:

## SponsorBlock
common-sponsorblock-title = Skip what is not music
common-sponsorblock-detail = In videos, the parts people marked on SponsorBlock.
common-sponsorblock-footer = Segments come from sponsor.ajay.app (CC BY-NC-SA 4.0). Only part of the video's hash is sent, not which one plays.
common-sponsorblock-music-offtopic = Non-music sections
common-sponsorblock-sponsor = Sponsors
common-sponsorblock-selfpromo = Self-promotion
common-sponsorblock-interaction = Like and subscribe reminders
common-sponsorblock-intro = Intros
common-sponsorblock-outro = Endcards and credits
common-sponsorblock-preview = Previews and recaps
common-sponsorblock-filler = Filler and jokes
common-sponsorblock-segment = A section
common-sponsorblock-skipped = Skipped: { $what }
common-skip-silence = Skip silence
common-skip-silence-detail = Passes by the silence each song opens and closes with.

## Replay
common-replay = Your Replay
common-replay-see = See your Replay
common-replay-month = This month
common-replay-year = This year
common-replay-all = All time
common-replay-empty = Nothing to tell yet
common-replay-empty-detail = Listen for a while: a song counts after 30 seconds.
common-replay-minutes = { $count ->
    [one] minute listened
   *[other] minutes listened
}
common-replay-plays = { $count ->
    [one] play
   *[other] plays
}
common-replay-songs = { $count ->
    [one] song
   *[other] songs
}
common-replay-artists = { $count ->
    [one] artist
   *[other] artists
}
common-replay-plays-of = { $count ->
    [one] 1 play
   *[other] { $count } plays
}
common-replay-song-heard = { $plays } × · { $minutes } min
common-replay-minutes-title = Your minutes listened
common-replay-minutes-days = { $count ->
    [one] That's { $count } day.
   *[other] That's { $count } days.
}
common-replay-minutes-hours = { $count ->
    [one] That's an hour.
   *[other] That's { $count } hours.
}
common-replay-top-artist = Your top artist
common-replay-streak-title = Days in a row
common-replay-story = Watch your Replay
common-replay-intro =
    You listened.
    We counted.
common-replay-everyone-else = Everyone else
common-replay-top-songs = What you played most
common-replay-top-artists = Your artists
common-replay-when = When you listen
common-replay-night = Night owl: your music comes alive after dark.
common-replay-morning = Early bird: you start the day with a soundtrack.
common-replay-day = Your music keeps you company all day.
common-replay-evening = Evenings are when you press play.
common-replay-streak = days in a row with music
common-replay-busiest = Your busiest day · { $minutes } min
common-replay-on-repeat = On repeat · { $times } times in one day
common-replay-days = { $count ->
    [one] { $count } day
   *[other] { $count } days
}
common-replay-hours = { $count ->
    [one] Over an hour
   *[other] Over { $count } hours
}

## Scrobbling
common-scrobble = Scrobbling
common-scrobble-footer = What you listen to goes to your account once you heard half the song or four minutes. Keys stay on this device only and do not travel when you move your account.
common-scrobble-listenbrainz-detail = Paste your token from listenbrainz.org/settings.
common-scrobble-lastfm-detail = With your own API key from last.fm/api. The password is used once and not kept.
common-scrobble-token = User token
common-scrobble-key = API key
common-scrobble-secret = Shared secret
common-scrobble-user = Username
common-scrobble-password = Password
common-scrobble-connect = Connect
common-scrobble-connecting = Connecting…
common-scrobble-sign-out = Sign out
common-scrobble-signed = Signed in as { $user }
common-scrobble-signed-waiting = Signed in as { $user } · { $count ->
    [one] 1 listen to send
   *[other] { $count } listens to send
}
common-scrobble-failed = Could not connect

## Recognize
common-recognize-title = What's this song?
common-recognize-offer = Listens to what plays nearby and tells you what it is.
common-recognize-detail = Listens through the microphone and asks Shazam. Only a fingerprint of the sound is sent, never the recording.
common-recognize-start = Listen
common-recognize-stop = Stop listening
common-recognize-idle = Tap to listen
common-recognize-listening = Listening… { $seconds } s
common-recognize-found = Found it!
common-recognize-unknown = We couldn't tell. Try closer to the music.
common-recognize-failed = Couldn't listen
common-recognize-search = Find in Surco
common-recognize-lately = Found lately

## Together
common-together = Listen together
common-together-detail = A room with a code: what plays on the host plays for everyone, in step, each on their own device.
common-together-host = Start a room
common-together-host-detail = You are the host: what you play here, everyone who comes in with the code hears.
common-together-host-go = Start room
common-together-join = Join a room
common-together-join-detail = Type the code you were given.
common-together-join-go = Join
common-together-bad-code = That is not a room's code. It is twelve characters, like K7QX-M2PA-9DTE.
common-together-failed = Could not enter the room.
common-together-connecting = Connecting…
common-together-waiting = Waiting for the host…
common-together-hosting = You are the host · { $relays } relays
common-together-live = Listening with the host · { $relays } relays
common-together-lost = The host has not been heard from for a while…
common-together-ended = The room ended.
common-together-copy = Copy code
common-together-open = Guests control it too
common-together-open-detail = They can play, pause, seek and skip, not only suggest songs.
common-together-close = Close the room
common-together-leave = Leave the room
common-together-members = In the room
common-together-host-tag = host
common-together-chat = Chat
common-together-chat-empty = Nobody said anything yet.
common-together-chat-placeholder = Say something
common-together-send = Send
common-together-joined = { $name } came in.
common-together-suggested = { $name } suggested “{ $title }”.
common-together-guest-suggest = What you choose is suggested to the host for their queue.
common-together-guest-open = The host lets you play, pause and skip songs for everyone.
