package app.assistant

import android.Manifest
import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.content.pm.ServiceInfo
import android.graphics.drawable.Icon
import android.os.Bundle
import android.os.Handler
import android.os.IBinder
import android.os.Looper
import android.provider.Settings
import android.speech.RecognitionListener
import android.speech.RecognizerIntent
import android.speech.SpeechRecognizer

/**
 * Listens for "AssistX" while it is on. Speech is turned into text on the phone, checked for
 * the wake word, and thrown away: nothing is kept or sent. On the wake word it opens the voice
 * card. A notification shows the whole time, as Android requires for the microphone.
 *
 * There is no low-power wake-word chip open to normal apps, so this keeps the phone's speech
 * recognizer running: it costs more battery than the rest of the app.
 */
class WakeService : Service() {
    private val main = Handler(Looper.getMainLooper())
    private var rec: SpeechRecognizer? = null
    private var running = false

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        when (intent?.action) {
            ACTION_STOP -> { prefs(this).edit().putBoolean(KEY_ON, false).apply(); stopSelf(); return START_NOT_STICKY }
        }
        if (checkSelfPermission(Manifest.permission.RECORD_AUDIO) != PackageManager.PERMISSION_GRANTED) { stopSelf(); return START_NOT_STICKY }
        startForeground(NOTE_ID, notification(), ServiceInfo.FOREGROUND_SERVICE_TYPE_MICROPHONE)
        instance = this
        if (!running) { running = true; active = true; listen() }
        listener?.invoke()
        return START_STICKY
    }

    private fun stopListening() {
        main.removeCallbacksAndMessages(null)
        try { rec?.cancel() } catch (e: Exception) { }
        rec?.destroy()
        rec = null
    }

    private fun listen() {
        // While someone else has the microphone, wait to be handed it back (pauseFor).
        if (!running || paused) return
        if (CaptureService.active) { main.postDelayed({ listen() }, 5000); return }
        val r = rec ?: Speech.make(this, Speech.good(this)).also { it.setRecognitionListener(hear); rec = it }
        try {
            r.startListening(Speech.intent(this, true))
        } catch (e: Exception) { again(2000) }
    }

    private fun again(ms: Long) { if (running && !paused) main.postDelayed({ listen() }, ms) }

    private val hear = object : RecognitionListener {
        private fun check(b: Bundle?): Boolean {
            val all = b?.getStringArrayList(SpeechRecognizer.RESULTS_RECOGNITION).orEmpty()
            if (all.none { isWakeWord(it) }) return false
            stopListening()
            wake()
            return true
        }
        override fun onPartialResults(b: Bundle?) { check(b) }
        override fun onResults(b: Bundle?) { if (!check(b)) again(150) }
        override fun onError(error: Int) {
            when (error) {
                SpeechRecognizer.ERROR_INSUFFICIENT_PERMISSIONS -> stopSelf()
                // Another app (or part of this one) has the recognizer: step back for longer.
                SpeechRecognizer.ERROR_RECOGNIZER_BUSY -> { stopListening(); again(5000) }
                SpeechRecognizer.ERROR_NO_MATCH, SpeechRecognizer.ERROR_SPEECH_TIMEOUT -> again(150)
                else -> { stopListening(); again(1500) }
            }
        }
        override fun onReadyForSpeech(p: Bundle?) {}
        override fun onBeginningOfSpeech() {}
        override fun onRmsChanged(rmsdB: Float) {}
        override fun onBufferReceived(buffer: ByteArray?) {}
        override fun onEndOfSpeech() {}
        override fun onEvent(eventType: Int, params: Bundle?) {}
    }

    /** Open the voice card. From the background that needs "Display over other apps"; without it, a tap-to-open alert. */
    private fun wake() {
        val i = Intent(this, AssistActivity::class.java).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_SINGLE_TOP)
        if (Settings.canDrawOverlays(this)) {
            startActivity(i)
        } else {
            val nm = getSystemService(NotificationManager::class.java)
            nm.createNotificationChannel(NotificationChannel(CH_WAKE, "AssistX heard you", NotificationManager.IMPORTANCE_HIGH))
            val pi = PendingIntent.getActivity(this, 41, i, PendingIntent.FLAG_IMMUTABLE)
            nm.notify(NOTE_WAKE, Notification.Builder(this, CH_WAKE).setSmallIcon(android.R.drawable.ic_btn_speak_now)
                .setContentTitle("AssistX is listening").setContentText("Tap to talk").setFullScreenIntent(pi, true).setContentIntent(pi).setAutoCancel(true).build())
        }
        // Keep the microphone free for the voice card; it hands it back when it closes
        // (or after a while, if the card was never opened).
        paused = true
        main.postDelayed({ if (paused) { paused = false; listen() } }, if (Settings.canDrawOverlays(this)) 3 * 60_000L else 45_000L)
    }

    override fun onDestroy() {
        running = false
        active = false
        instance = null
        paused = false
        stopListening()
        listener?.invoke()
        super.onDestroy()
    }

    private fun notification(): Notification {
        val nm = getSystemService(NotificationManager::class.java)
        nm.createNotificationChannel(NotificationChannel(CHANNEL, "Wake word", NotificationManager.IMPORTANCE_MIN))
        val stop = PendingIntent.getService(this, 40, Intent(this, WakeService::class.java).setAction(ACTION_STOP), PendingIntent.FLAG_IMMUTABLE)
        return Notification.Builder(this, CHANNEL)
            .setSmallIcon(android.R.drawable.ic_btn_speak_now)
            .setContentTitle("Say \u201cAssistX\u201d")
            .setContentText("Listening for the wake word on this phone. Nothing is kept or sent until you say it.")
            .setOngoing(true)
            .addAction(Notification.Action.Builder(Icon.createWithResource(this, android.R.drawable.ic_menu_close_clear_cancel), "Turn off", stop).build())
            .build()
    }

    companion object {
        const val ACTION_STOP = "app.assistant.WAKE_STOP"
        const val KEY_ON = "wake_on"
        private const val CHANNEL = "wake"
        private const val CH_WAKE = "wake_heard"
        private const val NOTE_ID = 11
        private const val NOTE_WAKE = 12
        @Volatile var active = false
        @Volatile var paused = false
        @Volatile var listener: (() -> Unit)? = null

        fun prefs(ctx: Context) = ctx.getSharedPreferences("app", Context.MODE_PRIVATE)

        /** "AssistX" as the recognizer may write it: assist x, assist ex, assistax, assists... */
        fun isWakeWord(text: String): Boolean {
            val t = text.lowercase().replace(Regex("[^a-z ]"), " ").replace(Regex("\\s+"), " ").trim()
            val joined = t.replace(" ", "")
            return listOf("assistx", "assistex", "assistax", "assistix", "assisteks", "assistics").any { joined.contains(it) } ||
                Regex("\\bassist (x|ex|axe|acts|ax|eggs|next)\\b").containsMatchIn(t) || Regex("\\bassists\\b").containsMatchIn(t) && t.split(" ").size <= 3
        }

        /** The voice card, dictation or capture needs the microphone: let go of it now (not later,
         *  or their recognizer finds this one still running and fails), then take it back. */
        fun pauseFor(ctx: Context, pause: Boolean) {
            paused = pause
            val s = instance ?: return
            if (pause) {
                s.stopListening()
                // Back by itself if nobody hands it back (an app closed in a way that skipped its cleanup).
                s.main.postDelayed({ if (paused) { paused = false; s.listen() } }, 3 * 60_000L)
            } else {
                s.main.removeCallbacksAndMessages(null)
                s.main.postDelayed({ s.listen() }, 1200)
            }
        }

        /** Wait a moment after letting go, so the phone's recognizer is free for the next user. */
        const val HANDOVER_MS = 350L
        @Volatile private var instance: WakeService? = null
    }
}
