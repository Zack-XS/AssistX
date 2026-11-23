package app.assistant

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Intent
import android.content.pm.ServiceInfo
import android.graphics.drawable.Icon
import android.media.AudioAttributes
import android.media.MediaPlayer
import android.media.RingtoneManager
import android.net.Uri
import android.os.Handler
import android.os.IBinder
import android.os.Looper
import android.os.PowerManager
import android.os.VibrationEffect
import android.os.VibratorManager
import org.json.JSONObject
import java.util.concurrent.Executors

/**
 * Rings an alarm: the chosen sound on the alarm volume, getting louder, with vibration and a
 * full-screen card (RingActivity) until Stop or Snooze, or 10 minutes. Stop and Snooze are sent
 * to the server too, so every device stops and the server does not call it missed.
 */
class RingService : Service() {
    private val main = Handler(Looper.getMainLooper())
    private val io = Executors.newSingleThreadExecutor()
    private var player: MediaPlayer? = null
    private var wake: PowerManager.WakeLock? = null
    private var jobId = ""
    private var text = ""
    private var sound = "default"
    private var volume = 0.15f

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        when (intent?.action) {
            ACTION_STOP -> { finish("dismiss"); return START_NOT_STICKY }
            ACTION_SNOOZE -> { finish("snooze"); return START_NOT_STICKY }
        }
        quiet()
        jobId = intent?.getStringExtra("id") ?: ""
        text = intent?.getStringExtra("text") ?: "Alarm"
        sound = intent?.getStringExtra("sound") ?: "default"
        startForeground(NOTE_ID, notification(), ServiceInfo.FOREGROUND_SERVICE_TYPE_MEDIA_PLAYBACK)
        wake = getSystemService(PowerManager::class.java).newWakeLock(PowerManager.PARTIAL_WAKE_LOCK, "assistant:ring").also { it.acquire(RING_MS + 5000) }
        current = JSONObject().put("id", jobId).put("text", text).toString()
        play()
        try { startActivity(Intent(this, RingActivity::class.java).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)) } catch (e: Exception) { }
        main.postDelayed({ finish("missed") }, RING_MS)
        listener?.invoke()
        return START_NOT_STICKY
    }

    private fun soundUri(): Uri {
        val custom = Alarms.soundFile(this, sound)
        if (custom.exists() && custom.length() > 0) return Uri.fromFile(custom)
        if (sound == "default") return Uri.parse("android.resource://$packageName/raw/alarm_repeat") // the alarm's own loop
        val type = when (sound) { "chime", "gentle", "beep" -> RingtoneManager.TYPE_NOTIFICATION; else -> RingtoneManager.TYPE_ALARM }
        return RingtoneManager.getDefaultUri(type) ?: RingtoneManager.getDefaultUri(RingtoneManager.TYPE_RINGTONE)
    }

    private fun play() {
        try {
            player = MediaPlayer().apply {
                setAudioAttributes(AudioAttributes.Builder().setUsage(AudioAttributes.USAGE_ALARM).setContentType(AudioAttributes.CONTENT_TYPE_SONIFICATION).build())
                setDataSource(this@RingService, soundUri())
                isLooping = true
                setVolume(volume, volume)
                prepare()
                start()
            }
        } catch (e: Exception) {
            player = null // no sound file worked; vibration and the card still show
        }
        main.post(louder)
        getSystemService(VibratorManager::class.java)?.defaultVibrator?.vibrate(
            VibrationEffect.createWaveform(longArrayOf(0, 600, 400, 600, 1400), 0),
            android.os.VibrationAttributes.createForUsage(android.os.VibrationAttributes.USAGE_ALARM))
    }

    /** Start soft and get louder over about 30 seconds. "gentle" stays softer. */
    private val louder = object : Runnable {
        override fun run() {
            val top = if (sound == "gentle") 0.6f else 1f
            volume = (volume + 0.03f).coerceAtMost(top)
            try { player?.setVolume(volume, volume) } catch (e: Exception) { }
            if (volume < top) main.postDelayed(this, 1000)
        }
    }

    private fun quiet() {
        main.removeCallbacksAndMessages(null)
        try { player?.stop() } catch (e: Exception) { }
        player?.release()
        player = null
        getSystemService(VibratorManager::class.java)?.defaultVibrator?.cancel()
        volume = 0.15f
    }

    /** Stop ringing and tell the server. The server may not have its ring yet: try again once. */
    private fun finish(how: String) {
        quiet()
        val id = jobId
        val label = text
        current = null
        listener?.invoke()
        if (how == "missed") Alarms.notify(this, id, "Missed alarm: $label", "alarm")
        if (how == "dismiss") Woke.alarmDismissed(applicationContext) // a morning alarm turned off: the morning brief
        if (id.isNotEmpty() && how != "missed") {
            val ctx = applicationContext
            io.execute {
                val path = if (how == "snooze") "/v1/alarms/snooze" else "/v1/alarms/dismiss"
                val body = JSONObject().put("id", id).put("minutes", SNOOZE_MIN).toString()
                var code = Alarms.http(ctx, "POST", path, body).first
                if (code == 409) { Thread.sleep(20_000); code = Alarms.http(ctx, "POST", path, body).first }
                if (how == "snooze") {
                    if (code == 200) Alarms.syncAsync(ctx) // the server made the snoozed alarm; copy it
                    else Alarms.schedule(ctx, "snooze-$id", System.currentTimeMillis() + SNOOZE_MIN * 60_000L, label, "alarm", true, sound)
                }
            }
        }
        wake?.let { if (it.isHeld) it.release() }
        stopForeground(STOP_FOREGROUND_REMOVE)
        stopSelf()
    }

    override fun onDestroy() {
        quiet()
        io.shutdown()
        super.onDestroy()
    }

    private fun notification(): Notification {
        val nm = getSystemService(NotificationManager::class.java)
        nm.createNotificationChannel(NotificationChannel(CHANNEL, "Alarms", NotificationManager.IMPORTANCE_HIGH).apply { setSound(null, null); enableVibration(false) })
        val flags = PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT
        val stop = PendingIntent.getService(this, 10, Intent(this, RingService::class.java).setAction(ACTION_STOP), flags)
        val snooze = PendingIntent.getService(this, 11, Intent(this, RingService::class.java).setAction(ACTION_SNOOZE), flags)
        val full = PendingIntent.getActivity(this, 12, Intent(this, RingActivity::class.java), flags)
        return Notification.Builder(this, CHANNEL)
            .setSmallIcon(android.R.drawable.ic_lock_idle_alarm)
            .setContentTitle(text)
            .setContentText("Alarm")
            .setCategory(Notification.CATEGORY_ALARM)
            .setOngoing(true)
            .setFullScreenIntent(full, true)
            .setContentIntent(full)
            .addAction(Notification.Action.Builder(Icon.createWithResource(this, android.R.drawable.ic_media_pause), "Snooze $SNOOZE_MIN min", snooze).build())
            .addAction(Notification.Action.Builder(Icon.createWithResource(this, android.R.drawable.ic_menu_close_clear_cancel), "Stop", stop).build())
            .build()
    }

    companion object {
        const val ACTION_STOP = "app.assistant.RING_STOP"
        const val ACTION_SNOOZE = "app.assistant.RING_SNOOZE"
        private const val CHANNEL = "alarms"
        private const val NOTE_ID = 9
        private const val RING_MS = 10 * 60_000L
        const val SNOOZE_MIN = 9

        /** The alarm ringing now ({id, text}), or null. */
        @Volatile var current: String? = null
        /** Called when ringing starts or stops, so the card can close. */
        @Volatile var listener: (() -> Unit)? = null
    }
}
