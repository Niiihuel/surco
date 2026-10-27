package io.github.niiihuel.surco

import android.app.Activity
import android.util.Log
import java.lang.ref.WeakReference

/**
 * The one JNI door each way between the Rust app and this shell.
 *
 * Rust calls [call] with a kind such as `media.publish` and a JSON body, and has a JSON answer
 * back, or null. The shell tells Rust with [nativeEvent]. Its package is fixed: Rust finds it
 * by name, whatever the app's id is.
 */
object SurcoBridge {
    @Volatile private var current: WeakReference<Activity>? = null

    /** The activity on screen, for what needs one: the clipboard, sharing, the photo picker. */
    val activity: Activity? get() = current?.get()

    fun attach(activity: Activity) {
        current = WeakReference(activity)
    }

    @JvmStatic
    fun call(kind: String, json: String): String? = try {
        when (kind.substringBefore('.')) {
            "media" -> SurcoMedia.handle(kind, json)
            "system" -> SurcoSystem.handle(kind, json)
            else -> null
        }
    } catch (error: Throwable) {
        // Nothing thrown may cross into Rust: the call answers nothing instead.
        Log.e("Surco", "call $kind", error)
        null
    }

    /** Tells Rust of `kind`, with its JSON body. */
    @JvmStatic
    external fun nativeEvent(kind: String, json: String)
}
