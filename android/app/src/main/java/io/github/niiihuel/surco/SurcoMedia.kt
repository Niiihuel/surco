package io.github.niiihuel.surco

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.media.AudioAttributes
import android.media.AudioFocusRequest
import android.media.AudioManager
import android.os.Handler
import android.os.Looper
import android.util.Base64
import android.util.Log
import androidx.core.content.ContextCompat
import org.json.JSONObject

/** The track Rust publishes, as the session shows it. */
internal class Track(val id: String, val title: String, val artist: String, val album: String?)

/** What Rust publishes: the track, if any, its length and position in seconds, and 1.0 while it plays. */
internal class Now(val track: Track?, val duration: Double, val elapsed: Double, val rate: Double)

/** The cover of the track with [trackId]. */
internal class Artwork(val trackId: String, val bytes: ByteArray)

/**
 * Android's view of what Rust plays: the media session's notification and lock screen, the
 * service that keeps the music going with the screen off, and the buttons and focus that come
 * back to Rust.
 *
 * Rust calls [handle] from its own thread. Everything it changes runs on the main thread.
 */
object SurcoMedia {
    internal var now: Now? = null
    internal var artwork: Artwork? = null
    private var context: Context? = null
    private var player: SurcoPlayer? = null
    private var focus: AudioFocusRequest? = null
    private var receiving = false
    /** Whether the last focus loss paused the music, so that regaining focus may resume it. */
    private var pausedByFocus = false
    private val main = Handler(Looper.getMainLooper())

    /** Answers a `media.*` call from Rust with "{}", or null for a kind this does not know. */
    fun handle(kind: String, json: String): String? {
        // Parsed on Rust's thread, so a body that cannot be read fails only its own call.
        when (kind) {
            "media.publish" -> {
                val next = parse(JSONObject(json))
                val activity = SurcoBridge.activity?.applicationContext
                main.post { publish(next, activity) }
            }
            "media.artwork" -> {
                val body = JSONObject(json)
                val cover = Artwork(body.getString("track_id"), Base64.decode(body.getString("bytes"), Base64.DEFAULT))
                main.post {
                    artwork = cover
                    player?.refresh()
                }
            }
            "media.stop" -> main.post { stop() }
            else -> return null
        }
        return "{}"
    }

    /** The service has made its player, which now shows what plays. */
    internal fun attach(service: Context, player: SurcoPlayer) {
        context = service.applicationContext
        this.player = player
        player.refresh()
    }

    /** The service is going: its player is no longer the one to show, and focus is let go. */
    internal fun detach(player: SurcoPlayer) {
        if (this.player === player) this.player = null
        release()
    }

    /** Tells Rust of a key, or of a pause that focus or an unplugged output asks for. */
    internal fun send(key: String, seconds: Double? = null) {
        val body = JSONObject().put("key", key)
        if (seconds != null) body.put("seconds", seconds)
        try {
            SurcoBridge.nativeEvent("media.key", body.toString())
        } catch (error: UnsatisfiedLinkError) {
            // The native library is gone, so no one is there to hear it.
            Log.w("Surco", "media.key $key", error)
        }
    }

    private fun publish(next: Now, activity: Context?) {
        now = next
        activity?.let { context = it.applicationContext }
        val app = context
        // Only once something plays: a foreground service must show its notification within
        // seconds, and Media3 only does for a playing session, so a paused start would be killed.
        // Android refuses one started from the background; then the music plays without it.
        if (next.track != null && next.rate > 0 && player == null && app != null) {
            try {
                ContextCompat.startForegroundService(app, Intent(app, SurcoMediaService::class.java))
            } catch (error: IllegalStateException) {
                Log.w("Surco", "Now Playing could not start its service", error)
            }
        }
        playing(next.rate > 0)
        player?.refresh()
    }

    private fun stop() {
        now = null
        artwork = null
        release()
        player?.refresh()
        context?.let { it.stopService(Intent(it, SurcoMediaService::class.java)) }
    }

    /** Holds focus and the unplug signal while something plays, and lets go of them when nothing does. */
    private fun playing(playing: Boolean) {
        val app = context ?: return
        val audio = app.getSystemService(AudioManager::class.java)
        if (playing) {
            pausedByFocus = false
            if (!receiving) {
                ContextCompat.registerReceiver(
                    app, becomingNoisy, IntentFilter(AudioManager.ACTION_AUDIO_BECOMING_NOISY),
                    ContextCompat.RECEIVER_NOT_EXPORTED,
                )
                receiving = true
            }
            if (focus == null) {
                focus = AudioFocusRequest.Builder(AudioManager.AUDIOFOCUS_GAIN)
                    .setAudioAttributes(
                        AudioAttributes.Builder()
                            .setUsage(AudioAttributes.USAGE_MEDIA)
                            .setContentType(AudioAttributes.CONTENT_TYPE_MUSIC)
                            .build(),
                    )
                    .setOnAudioFocusChangeListener(focusChanged, main)
                    .build()
                    .also { audio.requestAudioFocus(it) }
            }
        } else {
            if (receiving) {
                app.unregisterReceiver(becomingNoisy)
                receiving = false
            }
            // A focus loss keeps the focus, since its return is what resumes the music.
            if (!pausedByFocus) {
                focus?.let { audio.abandonAudioFocusRequest(it) }
                focus = null
            }
        }
    }

    private fun release() {
        pausedByFocus = false
        playing(false)
    }

    private val focusChanged = AudioManager.OnAudioFocusChangeListener { change ->
        when (change) {
            AudioManager.AUDIOFOCUS_GAIN -> if (pausedByFocus) {
                pausedByFocus = false
                send("play")
            }
            AudioManager.AUDIOFOCUS_LOSS -> {
                release()
                send("pause")
            }
            AudioManager.AUDIOFOCUS_LOSS_TRANSIENT, AudioManager.AUDIOFOCUS_LOSS_TRANSIENT_CAN_DUCK -> {
                // Only a pause that focus caused is undone when focus returns.
                val playingNow = (now?.rate ?: 0.0) > 0
                pausedByFocus = playingNow
                if (playingNow) send("pause")
            }
            else -> Unit
        }
    }

    private val becomingNoisy = object : BroadcastReceiver() {
        override fun onReceive(app: Context, intent: Intent) = send("pause")
    }

    private fun parse(json: JSONObject): Now {
        val track = json.optJSONObject("track")?.let {
            Track(it.getString("id"), it.getString("title"), it.getString("artist"), if (it.isNull("album")) null else it.getString("album"))
        }
        return Now(track, json.getDouble("duration"), json.getDouble("elapsed"), json.getDouble("rate"))
    }
}
