# backend: what the player, the downloads, the lyrics and the updates tell when they fail.

backend-playback-interrupted = Playback was interrupted by another audio session.
backend-output-open-failed = Couldn't open the audio output: { $error }
backend-output-both-failed = Couldn't use the chosen output ({ $error }) or the default one ({ $fallback }). Check the device's connection and choose an output again.
backend-output-default-failed = Couldn't use the default output: { $error }. Check the system's sound settings and choose an output again.
backend-engine-unavailable = The audio engine isn't available: { $message }
backend-lyrics-start-failed = Couldn't start the lyrics service. Reopen Surco.
backend-artist-not-found = We couldn't find { $name } in the catalog.
backend-playlist-empty = We couldn't get any of the songs in that playlist.
backend-update-title = A new version of Surco is available
backend-update-body = Version { $version } is ready. Open Surco to update.
backend-update-no-program = This version has no program for this system.
backend-update-no-sum = The version doesn't say how to verify its program.
backend-update-mismatch = The download isn't the program the version published.
backend-update-no-location = Couldn't tell where the program is installed.
backend-update-replace-failed = Couldn't replace the program: { $error }
backend-no-audio = YouTube didn't offer audio for this track.
backend-download-cut = The download stopped before it finished.
backend-save-failed = Couldn't save to disk: { $error }
backend-cover = Cover art
backend-lyrics-limited = The lyrics service hit its limit. Wait { $seconds } { $seconds ->
        [one] second
       *[other] seconds
    } and try again.
backend-lyrics-youtube-failed = We couldn't load the lyrics from YouTube Music. Try again.
backend-lyrics-timeout = The lyrics service took too long. Try again.
backend-lyrics-unreachable = We couldn't reach the lyrics service. Check your connection and try again.
backend-lyrics-unavailable = The lyrics service isn't available (HTTP { $status }). Try again.
backend-lyrics-unreadable = The lyrics service sent a response we couldn't read. Try again.
backend-translation-unreadable = The translation could not be read.
backend-pictures = Pictures
