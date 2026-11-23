package app.assistant

import android.Manifest
import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Intent
import android.content.pm.PackageManager
import android.content.pm.ServiceInfo
import android.graphics.drawable.Icon
import android.os.Bundle
import android.os.Handler
import android.os.IBinder
import android.os.Looper
import android.speech.RecognitionListener
import android.speech.RecognizerIntent
import android.speech.SpeechRecognizer
import org.json.JSONArray
import org.json.JSONObject
import java.net.HttpURLConnection
import java.net.URL
import java.util.concurrent.Executors

/**
 * Listens while it is switched on, turns speech into text on the phone, and uploads the text to
 * the server in small batches (docs/08). A notification stays on screen the whole time, with a
 * Stop button. The server decides what to do with the text; it never gets audio.
 *
 * The phone cannot tell whose voice it hears yet, so every chunk is sent as speaker "unknown".
 */
class CaptureService : Service() {
    private val main = Handler(Looper.getMainLooper())
    private val io = Executors.newSingleThreadExecutor()
    private val queue = ArrayList<JSONObject>()
    private var rec: SpeechRecognizer? = null
    private var running = false
    private var heardFrom = 0L
    private var pausedUntil = 0L

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        if (intent?.action == ACTION_STOP) {
            stopSelf()
            return START_NOT_STICKY
        }
        if (checkSelfPermission(Manifest.permission.RECORD_AUDIO) != PackageManager.PERMISSION_GRANTED) {
            stopSelf()
            return START_NOT_STICKY
        }
        startForeground(NOTE_ID, notification(), ServiceInfo.FOREGROUND_SERVICE_TYPE_MICROPHONE)
        if (!running) {
            running = true
            active = true
            WakeService.pauseFor(this, true) // all-day capture takes the one recognizer
            listener?.invoke()
            listen()
            main.postDelayed(flushTick, FLUSH_EVERY_MS)
        }
        return START_NOT_STICKY // Android does not let a microphone service restart itself from the background.
    }

    override fun onDestroy() {
        running = false
        active = false
        main.removeCallbacksAndMessages(null)
        rec?.destroy()
        rec = null
        WakeService.pauseFor(this, false) // the wake word gets the recognizer back
        send(ArrayList(queue)) // one last try with what is left
        io.shutdown()
        listener?.invoke()
        super.onDestroy()
    }

    // ---------- listening ----------

    private fun listen() {
        if (!running) return
        val wait = pausedUntil - System.currentTimeMillis()
        if (wait > 0) { main.postDelayed({ listen() }, wait); return }
        val r = rec ?: Speech.make(this, Speech.good(this)).also { it.setRecognitionListener(hear); rec = it }
        heardFrom = System.currentTimeMillis() / 1000
        val i = Intent(RecognizerIntent.ACTION_RECOGNIZE_SPEECH)
            .putExtra(RecognizerIntent.EXTRA_LANGUAGE_MODEL, RecognizerIntent.LANGUAGE_MODEL_FREE_FORM)
            .putExtra(RecognizerIntent.EXTRA_PARTIAL_RESULTS, false)
            .putExtra(RecognizerIntent.EXTRA_PREFER_OFFLINE, true)
            .putExtra(RecognizerIntent.EXTRA_CALLING_PACKAGE, packageName)
        try { r.startListening(i) } catch (e: Exception) { again(1500) }
    }

    private fun again(ms: Long) {
        if (running) main.postDelayed({ listen() }, ms.coerceAtLeast(250))
    }

    private val hear = object : RecognitionListener {
        override fun onResults(results: Bundle?) {
            val text = results?.getStringArrayList(SpeechRecognizer.RESULTS_RECOGNITION)?.firstOrNull()?.trim().orEmpty()
            val conf = results?.getFloatArray(SpeechRecognizer.CONFIDENCE_SCORES)?.firstOrNull()
            if (text.isNotEmpty()) {
                val c = JSONObject().put("started", heardFrom).put("ended", System.currentTimeMillis() / 1000)
                    .put("speaker", "unknown").put("text", text.take(3900)).put("place", PlaceWatch.here)
                if (conf != null && conf >= 0f) c.put("stt_confidence", conf.toDouble())
                queue.add(c)
                if (queue.size > MAX_QUEUE) queue.removeAt(0)
                if (queue.size >= 5) flushNow()
            }
            again(250)
        }

        override fun onError(error: Int) {
            when (error) {
                SpeechRecognizer.ERROR_INSUFFICIENT_PERMISSIONS -> stopSelf()
                SpeechRecognizer.ERROR_RECOGNIZER_BUSY -> { rec?.destroy(); rec = null; again(1000) }
                SpeechRecognizer.ERROR_NO_MATCH, SpeechRecognizer.ERROR_SPEECH_TIMEOUT -> again(250)
                SpeechRecognizer.ERROR_NETWORK, SpeechRecognizer.ERROR_NETWORK_TIMEOUT, SpeechRecognizer.ERROR_SERVER -> again(4000)
                else -> { rec?.destroy(); rec = null; again(1500) }
            }
        }

        override fun onReadyForSpeech(params: Bundle?) {}
        override fun onBeginningOfSpeech() {}
        override fun onRmsChanged(rmsdB: Float) {}
        override fun onBufferReceived(buffer: ByteArray?) {}
        override fun onEndOfSpeech() {}
        override fun onPartialResults(partialResults: Bundle?) {}
        override fun onEvent(eventType: Int, params: Bundle?) {}
    }

    // ---------- upload ----------

    private val flushTick = object : Runnable {
        override fun run() {
            flushNow()
            if (running) main.postDelayed(this, FLUSH_EVERY_MS)
        }
    }

    private fun flushNow() {
        if (queue.isEmpty()) return
        val batch = ArrayList(queue)
        queue.clear()
        send(batch)
    }

    /** Uploads on a background thread. A failed upload puts the chunks back, up to a limit. */
    private fun send(batch: List<JSONObject>) {
        if (batch.isEmpty() || io.isShutdown) return
        val prefs = getSharedPreferences("app", MODE_PRIVATE)
        val base = prefs.getString("base", null) ?: BuildConfig.DEFAULT_SERVER
        val token = Vault.open(prefs.getString("tok", null))
        if (token.isEmpty()) return
        try {
            io.execute {
                val code = post("$base/v1/ambient/chunks", token, JSONObject().put("chunks", JSONArray(batch)).toString())
                main.post {
                    when (code) {
                        200 -> {}
                        409 -> pausedUntil = System.currentTimeMillis() + 60_000 // server says not listening: wait, drop this batch
                        400, 413 -> {}                                              // bad data: do not resend
                        401, 403 -> stopSelf()                                      // token no longer accepted
                        else -> { queue.addAll(0, batch); while (queue.size > MAX_QUEUE) queue.removeAt(0) }
                    }
                }
            }
        } catch (e: Exception) { /* service is closing */ }
    }

    private fun post(url: String, token: String, body: String): Int = try {
        val c = URL(url).openConnection() as HttpURLConnection
        c.requestMethod = "POST"
        c.connectTimeout = 8000
        c.readTimeout = 15000
        c.doOutput = true
        c.setRequestProperty("Authorization", "Bearer $token")
        c.setRequestProperty("Content-Type", "application/json")
        c.outputStream.use { it.write(body.toByteArray()) }
        val code = c.responseCode
        c.disconnect()
        code
    } catch (e: Exception) {
        -1
    }

    // ---------- notification ----------

    private fun notification(): Notification {
        val nm = getSystemService(NotificationManager::class.java)
        nm.createNotificationChannel(NotificationChannel(CHANNEL, "Listening", NotificationManager.IMPORTANCE_LOW))
        val stop = PendingIntent.getService(this, 0, Intent(this, CaptureService::class.java).setAction(ACTION_STOP), PendingIntent.FLAG_IMMUTABLE)
        val open = PendingIntent.getActivity(this, 1, Intent(this, MainActivity::class.java), PendingIntent.FLAG_IMMUTABLE)
        return Notification.Builder(this, CHANNEL)
            .setSmallIcon(android.R.drawable.ic_btn_speak_now)
            .setContentTitle("Listening")
            .setContentText("Speech becomes text on this phone and is sent to your server.")
            .setContentIntent(open)
            .setOngoing(true)
            .addAction(Notification.Action.Builder(Icon.createWithResource(this, android.R.drawable.ic_media_pause), "Stop", stop).build())
            .build()
    }

    companion object {
        const val ACTION_STOP = "app.assistant.STOP_CAPTURE"
        private const val CHANNEL = "capture"
        private const val NOTE_ID = 7
        private const val FLUSH_EVERY_MS = 15_000L
        private const val MAX_QUEUE = 300

        /** True while the service is listening. */
        @Volatile var active = false

        /** Called when it starts or stops, so the page can update. */
        @Volatile var listener: (() -> Unit)? = null
    }
}
