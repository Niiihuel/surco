package io.github.niiihuel.surco

import android.Manifest
import android.annotation.SuppressLint
import android.app.Activity
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.ClipData
import android.content.ClipboardManager
import android.content.ContentResolver
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.net.Uri
import android.os.Build
import android.os.Looper
import android.util.Base64
import android.util.Log
import androidx.activity.result.ActivityResultLauncher
import androidx.activity.result.PickVisualMediaRequest
import androidx.activity.result.contract.ActivityResultContracts
import androidx.core.app.NotificationCompat
import androidx.core.app.NotificationManagerCompat
import androidx.core.content.ContextCompat
import com.google.mlkit.vision.barcode.common.Barcode
import com.google.mlkit.vision.codescanner.GmsBarcodeScannerOptions
import com.google.mlkit.vision.codescanner.GmsBarcodeScanning
import org.json.JSONObject
import java.io.ByteArrayOutputStream
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicReference

/**
 * The phone's everyday services for the Rust app: the clipboard, the share sheet, notifications,
 * the photo picker, the code reader, and leaving the app on Back. Every call is answered on the main thread,
 * where the platform wants these; [handle] is called from any thread.
 */
object SurcoSystem {
    private const val TAG = "Surco"
    private const val CHANNEL = "surco-updates"
    private const val PHOTO_BYTES_MAX = 16 * 1024 * 1024

    private var channelMade = false
    private var notificationsAsked = false
    private var notificationsLauncher: ActivityResultLauncher<String>? = null
    private var photoLauncher: ActivityResultLauncher<PickVisualMediaRequest>? = null
    private var nextNotificationId = 1
    private var scanning = false

    /** Runs the `system.*` [kind] with its JSON body, and answers with JSON, or null for an unknown kind. */
    fun handle(kind: String, json: String): String? = when (kind) {
        "system.copy" -> copyText(JSONObject(json).getString("text"))
        "system.paste" -> pasteText()
        "system.share" -> shareText(JSONObject(json).getString("title"), JSONObject(json).getString("text"))
        "system.notify" -> postNotification(JSONObject(json).getString("title"), JSONObject(json).getString("body"))
        "system.pick_photo" -> pickPhoto()
        "system.scan" -> scanCode()
        "system.leave" -> leave()
        else -> null
    }

    /**
     * Runs [block] on the main thread and waits for its answer, up to two seconds. Null when
     * there is no activity on screen or the main thread did not answer in time.
     */
    private fun onMain(block: (Activity) -> String?): String? {
        val activity = SurcoBridge.activity ?: return null
        fun run(): String? = try {
            block(activity)
        } catch (error: Exception) {
            Log.e(TAG, "system call", error)
            answer(false, error.toString())
        }
        // Waiting here on the main thread would never return: the block would not run.
        if (Looper.myLooper() == Looper.getMainLooper()) return run()
        val latch = CountDownLatch(1)
        val result = AtomicReference<String?>()
        activity.runOnUiThread {
            result.set(run())
            latch.countDown()
        }
        latch.await(2, TimeUnit.SECONDS)
        return result.get()
    }

    private fun answer(ok: Boolean, error: String? = null): String =
        JSONObject().put("ok", ok).apply { if (error != null) put("error", error) }.toString()

    private fun copyText(text: String): String? = onMain { activity ->
        val clipboard = activity.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
        clipboard.setPrimaryClip(ClipData.newPlainText("Surco", text))
        answer(true)
    }

    private fun pasteText(): String? = onMain { activity ->
        val clipboard = activity.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
        val text = clipboard.primaryClip?.takeIf { it.itemCount > 0 }?.getItemAt(0)
            ?.coerceToText(activity)?.toString()?.takeIf { it.isNotEmpty() }
        JSONObject().put("text", text ?: JSONObject.NULL).toString()
    }

    private fun shareText(title: String, text: String): String? = onMain { activity ->
        val send = Intent(Intent.ACTION_SEND).setType("text/plain").putExtra(Intent.EXTRA_TEXT, text)
        activity.startActivity(Intent.createChooser(send, title))
        answer(true)
    }

    @SuppressLint("MissingPermission") // Checked below on Android 13 and later; older versions need no permission.
    private fun postNotification(title: String, body: String): String? = onMain { activity ->
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU &&
            ContextCompat.checkSelfPermission(activity, Manifest.permission.POST_NOTIFICATIONS) != PackageManager.PERMISSION_GRANTED
        ) {
            askForNotifications(activity)
            return@onMain answer(false, "Notifications are not allowed")
        }
        if (!channelMade && Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            val channel = NotificationChannel(CHANNEL, "Updates", NotificationManager.IMPORTANCE_DEFAULT)
            activity.getSystemService(NotificationManager::class.java).createNotificationChannel(channel)
            channelMade = true
        }
        val launch = activity.packageManager.getLaunchIntentForPackage(activity.packageName)
        val tap = launch?.let { PendingIntent.getActivity(activity, 0, it, PendingIntent.FLAG_IMMUTABLE) }
        val notification = NotificationCompat.Builder(activity, CHANNEL)
            .setSmallIcon(activity.applicationInfo.icon)
            .setContentTitle(title)
            .setContentText(body)
            .setContentIntent(tap)
            .setAutoCancel(true)
            .build()
        NotificationManagerCompat.from(activity).notify(nextNotificationId++, notification)
        answer(true)
    }

    /** Asks once; the notification that needed the answer is dropped, not held for a yes. */
    private fun askForNotifications(activity: Activity) {
        if (notificationsAsked) return
        notificationsAsked = true
        val launcher = activity.activityResultRegistry.register(
            "surco-notifications", ActivityResultContracts.RequestPermission(),
        ) {
            notificationsLauncher?.unregister()
            notificationsLauncher = null
        }
        notificationsLauncher = launcher
        launcher.launch(Manifest.permission.POST_NOTIFICATIONS)
    }

    private fun pickPhoto(): String? = onMain { activity ->
        // A picker already open answers for this request too; a second one would clash on its key.
        if (photoLauncher == null) {
            val launcher = activity.activityResultRegistry.register(
                "surco-photo", ActivityResultContracts.PickVisualMedia(),
            ) { uri ->
                photoLauncher?.unregister()
                photoLauncher = null
                readPhoto(activity.contentResolver, uri)
            }
            photoLauncher = launcher
            launcher.launch(PickVisualMediaRequest(ActivityResultContracts.PickVisualMedia.ImageOnly))
        }
        answer(true)
    }

    /** Reads the picked picture off the main thread, then tells Rust; a cancelled pick sends null. */
    private fun readPhoto(resolver: ContentResolver, uri: Uri?) {
        Thread {
            val bytes = uri?.let { readCapped(resolver, it) }
            val encoded = bytes?.let { Base64.encodeToString(it, Base64.NO_WRAP) }
            emit("system.photo", JSONObject().put("bytes", encoded ?: JSONObject.NULL).toString())
        }.start()
    }

    /** The picture's bytes, or null when it cannot be read or is over the cap. */
    private fun readCapped(resolver: ContentResolver, uri: Uri): ByteArray? {
        return try {
            resolver.openInputStream(uri)?.use { input ->
                val out = ByteArrayOutputStream()
                val chunk = ByteArray(64 * 1024)
                while (true) {
                    val count = input.read(chunk)
                    if (count < 0) break
                    if (out.size() + count > PHOTO_BYTES_MAX) return null
                    out.write(chunk, 0, count)
                }
                out.toByteArray()
            }
        } catch (error: Exception) {
            Log.e(TAG, "photo read", error)
            null
        }
    }

    /** Opens Google's code reader for a QR code; the code's text comes later as `system.scanned`. */
    private fun scanCode(): String? = onMain { activity ->
        // The scan already running delivers its result; a second reader on top would hide it.
        if (scanning) return@onMain answer(true)
        val options = GmsBarcodeScannerOptions.Builder()
            .setBarcodeFormats(Barcode.FORMAT_QR_CODE)
            .build()
        val scan = try {
            GmsBarcodeScanning.getClient(activity, options).startScan()
        } catch (error: Exception) {
            // A reader that never opened must not keep the next one from opening.
            Log.e(TAG, "code scan", error)
            return@onMain answer(false, error.toString())
        }
        scanning = true
        scan
            .addOnSuccessListener { barcode ->
                scanning = false
                emit("system.scanned", JSONObject().put("text", barcode.rawValue ?: JSONObject.NULL).toString())
            }
            .addOnCanceledListener {
                scanning = false
                emit("system.scanned", JSONObject().put("text", JSONObject.NULL).toString())
            }
            .addOnFailureListener { error ->
                scanning = false
                Log.e(TAG, "code scan", error)
                emit("system.scanned", JSONObject().put("text", JSONObject.NULL).toString())
            }
        answer(true)
    }

    private fun leave(): String? = onMain { activity ->
        activity.moveTaskToBack(true)
        answer(true)
    }

    /** Tells Rust of [kind]; a Rust side that is not loaded yet must not crash the picker. */
    private fun emit(kind: String, json: String) {
        try {
            SurcoBridge.nativeEvent(kind, json)
        } catch (error: UnsatisfiedLinkError) {
            Log.e(TAG, "no Rust side for $kind", error)
        }
    }
}
