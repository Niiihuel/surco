package io.github.niiihuel.surco

import android.content.Intent
import android.os.Looper
import androidx.media3.common.C
import androidx.media3.common.MediaItem
import androidx.media3.common.MediaMetadata
import androidx.media3.common.Player
import androidx.media3.common.SimpleBasePlayer
import androidx.media3.common.SimpleBasePlayer.MediaItemData
import androidx.media3.common.SimpleBasePlayer.State
import androidx.media3.session.MediaSession
import androidx.media3.session.MediaSessionService
import com.google.common.util.concurrent.Futures
import com.google.common.util.concurrent.ListenableFuture

/**
 * The media session's player. It holds no playback: it shows what Rust published and hands
 * each control back to Rust, which decides and publishes again.
 */
class SurcoPlayer : SimpleBasePlayer(Looper.getMainLooper()) {
    /** Rebuilds what the session shows, after a new snapshot or cover. */
    fun refresh() = invalidateState()

    override fun getState(): State {
        val commands = Player.Commands.Builder()
            .addAll(
                Player.COMMAND_PLAY_PAUSE,
                Player.COMMAND_SEEK_TO_NEXT,
                Player.COMMAND_SEEK_TO_PREVIOUS,
                Player.COMMAND_SEEK_IN_CURRENT_MEDIA_ITEM,
                Player.COMMAND_GET_CURRENT_MEDIA_ITEM,
                Player.COMMAND_GET_METADATA,
            )
            .build()
        val now = SurcoMedia.now
        val track = now?.track
        if (now == null || track == null) {
            return State.Builder().setAvailableCommands(commands).setPlaybackState(Player.STATE_IDLE).build()
        }
        val metadata = MediaMetadata.Builder()
            .setTitle(track.title)
            .setArtist(track.artist)
            .setAlbumTitle(track.album)
        SurcoMedia.artwork?.takeIf { it.trackId == track.id }?.let {
            metadata.setArtworkData(it.bytes, MediaMetadata.PICTURE_TYPE_FRONT_COVER)
        }
        val media = metadata.build()
        val item = MediaItemData.Builder(track.id)
            .setMediaItem(MediaItem.Builder().setMediaId(track.id).setMediaMetadata(media).build())
            .setMediaMetadata(media)
            .setDurationUs(if (now.duration > 0) (now.duration * 1_000_000).toLong() else C.TIME_UNSET)
            .build()
        // The position advances on its own while it plays; Rust's next snapshot corrects it.
        return State.Builder()
            .setAvailableCommands(commands)
            .setPlaylist(listOf(item))
            .setPlaybackState(Player.STATE_READY)
            .setPlayWhenReady(now.rate > 0, Player.PLAY_WHEN_READY_CHANGE_REASON_REMOTE)
            .setIsLoading(false)
            .setContentPositionMs(
                SimpleBasePlayer.PositionSupplier.getExtrapolating((now.elapsed * 1000).toLong(), now.rate.toFloat()),
            )
            .build()
    }

    override fun handleSetPlayWhenReady(playWhenReady: Boolean): ListenableFuture<*> {
        SurcoMedia.send(if (playWhenReady) "play" else "pause")
        return Futures.immediateVoidFuture()
    }

    override fun handleSeek(mediaItemIndex: Int, positionMs: Long, seekCommand: Int): ListenableFuture<*> {
        when (seekCommand) {
            Player.COMMAND_SEEK_TO_NEXT -> SurcoMedia.send("next")
            Player.COMMAND_SEEK_TO_PREVIOUS -> SurcoMedia.send("previous")
            Player.COMMAND_SEEK_IN_CURRENT_MEDIA_ITEM -> SurcoMedia.send("seek", positionMs / 1000.0)
        }
        return Futures.immediateVoidFuture()
    }
}

/**
 * The foreground service that keeps Rust's playback alive with the screen off. Media3 posts the
 * media notification and holds the foreground for it while something plays.
 */
class SurcoMediaService : MediaSessionService() {
    private lateinit var player: SurcoPlayer
    private lateinit var session: MediaSession

    override fun onCreate() {
        super.onCreate()
        player = SurcoPlayer()
        session = MediaSession.Builder(this, player).build()
        // Started rather than bound, the service knows its session only once added: without it
        // Media3 never posts the notification, and Android kills the app for never calling startForeground.
        addSession(session)
        SurcoMedia.attach(this, player)
    }

    override fun onGetSession(controllerInfo: MediaSession.ControllerInfo): MediaSession = session

    /** A swiped-away app stops with nothing playing, and keeps the music going otherwise. */
    override fun onTaskRemoved(rootIntent: Intent?) {
        if (!player.isPlaying) stopSelf()
    }

    override fun onDestroy() {
        SurcoMedia.detach(player)
        session.release()
        player.release()
        super.onDestroy()
    }
}
