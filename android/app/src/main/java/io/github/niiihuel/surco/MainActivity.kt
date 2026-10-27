package io.github.niiihuel.surco

import android.os.Bundle
import android.util.Log
import androidx.activity.OnBackPressedCallback
import androidx.core.view.ViewCompat
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat
import com.google.androidgamesdk.GameActivity
import org.json.JSONObject

/** The activity the Rust loop runs in; it must not be recreated, so the manifest lists its config changes. */
class MainActivity : GameActivity() {

    companion object {
        init {
            System.loadLibrary("surco_app")
        }
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        // Before super: GameActivity starts Rust's thread there, which asks for both at once.
        SurcoBridge.attach(this)
        SurcoBridge.launchArgs = intent.getStringArrayExtra("surco.args")
        super.onCreate(savedInstanceState)
        WindowCompat.setDecorFitsSystemWindows(window, false)

        ViewCompat.setOnApplyWindowInsetsListener(window.decorView) { _, insets ->
            val bars = insets.getInsets(WindowInsetsCompat.Type.systemBars() or WindowInsetsCompat.Type.displayCutout())
            val ime = insets.getInsets(WindowInsetsCompat.Type.ime())
            // Rust lays out in dp, so the pixels are converted here.
            val density = resources.displayMetrics.density
            val json = JSONObject()
                .put("top", (bars.top / density).toDouble())
                .put("bottom", (bars.bottom / density).toDouble())
                .put("left", (bars.left / density).toDouble())
                .put("right", (bars.right / density).toDouble())
                .put("ime", (ime.bottom / density).toDouble())
            send("window.insets", json.toString())
            insets
        }
        ViewCompat.requestApplyInsets(window.decorView)

        onBackPressedDispatcher.addCallback(this, object : OnBackPressedCallback(true) {
            override fun handleOnBackPressed() {
                send("window.back", "{}")
            }
        })
    }

    private fun send(kind: String, json: String) {
        try {
            SurcoBridge.nativeEvent(kind, json)
        } catch (error: UnsatisfiedLinkError) {
            Log.e("Surco", "no native bridge for $kind", error)
        }
    }
}
