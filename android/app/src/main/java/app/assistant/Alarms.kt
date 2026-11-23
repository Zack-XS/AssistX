package app.assistant

import android.app.AlarmManager
import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.media.AudioAttributes
import android.net.Uri
import android.os.SystemClock
import org.json.JSONObject
import java.io.File
import java.net.HttpURLConnection
import java.net.URL
import java.time.Instant
import java.util.concurrent.Executors

/**
 * Alarms and reminders on this phone (docs/01 Jobs). The server keeps the jobs; the phone copies
 * the coming ones into Android's alarm clock, so they ring on time even when the app is closed,
 * the screen is off, or the network is gone. A sync runs when the app opens, after each turn,
 * every 15 minutes, after a restart, and each time an alarm fires (for the next repeat).
 */
object Alarms {
    private val io = Executors.newSingleThreadExecutor()
    const val ACTION_FIRE = "app.assistant.ALARM_FIRE"
    const val ACTION_SYNC = "app.assistant.ALARM_SYNC"
    const val CH_REMIND = "reminders_friendly"
    private val BUILTIN = setOf("default", "chime", "bell", "beep", "gentle", "radar")

    fun base(ctx: Context): String = ctx.getSharedPreferences("app", Context.MODE_PRIVATE).getString("base", null) ?: BuildConfig.DEFAULT_SERVER
    fun token(ctx: Context): String = Vault.open(ctx.getSharedPreferences("app", Context.MODE_PRIVATE).getString("tok", null))

    /** One request. Returns the status code and body (-1 when the server cannot be reached). */
    fun http(ctx: Context, method: String, path: String, body: String? = null): Pair<Int, String> {
        val r = httpOnce(ctx, method, path, body)
        // 423: the server restarted and forgot the account's key. This phone is remembered, so it unlocks and tries again.
        return if (r.first == 423 && Unlock.ensure(ctx)) httpOnce(ctx, method, path, body) else r
    }

    private fun httpOnce(ctx: Context, method: String, path: String, body: String? = null): Pair<Int, String> = try {
        val c = URL(base(ctx) + path).openConnection() as HttpURLConnection
        c.requestMethod = method
        c.connectTimeout = 8000
        c.readTimeout = 15000
        c.setRequestProperty("Authorization", "Bearer " + token(ctx))
        if (body != null) {
            c.doOutput = true
            c.setRequestProperty("Content-Type", "application/json")
            c.outputStream.use { it.write(body.toByteArray()) }
        }
        val code = c.responseCode
        val text = (if (code < 400) c.inputStream else c.errorStream)?.bufferedReader()?.use { it.readText() } ?: ""
        c.disconnect()
        code to text
    } catch (e: Exception) {
        -1 to ""
    }

    /** Like `http`, but the answer is raw bytes (audio). */
    fun httpBytes(ctx: Context, method: String, path: String, body: String? = null): Pair<Int, ByteArray> = try {
        val c = URL(base(ctx) + path).openConnection() as HttpURLConnection
        c.requestMethod = method
        c.connectTimeout = 8000
        c.readTimeout = 70000 // the voice may need a few seconds for a long answer
        c.setRequestProperty("Authorization", "Bearer " + token(ctx))
        if (body != null) {
            c.doOutput = true
            c.setRequestProperty("Content-Type", "application/json")
            c.outputStream.use { it.write(body.toByteArray()) }
        }
        val code = c.responseCode
        val bytes = (if (code < 400) c.inputStream else c.errorStream)?.use { it.readBytes() } ?: ByteArray(0)
        c.disconnect()
        code to bytes
    } catch (e: Exception) {
        -1 to ByteArray(0)
    }

    fun syncAsync(ctx: Context, done: (() -> Unit)? = null) {
        val app = ctx.applicationContext
        io.execute {
            try { sync(app) } catch (e: Exception) { }
            try { Notices.poll(app) } catch (e: Exception) { }
            done?.invoke()
        }
    }

    /** Copy every coming alarm and reminder from the server into the phone's alarm clock. */
    private fun sync(ctx: Context) {
        if (token(ctx).isEmpty()) return
        val (code, text) = http(ctx, "GET", "/v1/jobs")
        if (code != 200) return // offline: keep what is already set
        val items = JSONObject(text).optJSONArray("items") ?: return
        val prefs = ctx.getSharedPreferences("alarms", Context.MODE_PRIVATE)
        val before = prefs.getStringSet("ids", emptySet()) ?: emptySet()
        val now = System.currentTimeMillis()
        val seen = HashSet<String>()
        for (i in 0 until items.length()) {
            val j = items.getJSONObject(i)
            val kind = j.optString("kind")
            if (kind !in setOf("alarm", "reminder", "event") || j.optString("status") != "active") continue
            val at = try { Instant.parse(j.optString("next_run")).toEpochMilli() } catch (e: Exception) { continue }
            if (at < now - 60_000) continue
            val id = j.getString("id")
            val sound = j.optString("sound").takeIf { it.isNotEmpty() && it != "null" } ?: "default"
            val ring = kind == "alarm" || j.optBoolean("ring")
            val label = j.optString("text")
            schedule(ctx, id, at, label, kind, ring, sound)
            if (sound !in BUILTIN) fetchSound(ctx, sound)
            seen.add(id)
        }
        for (gone in before - seen) cancel(ctx, gone)
        prefs.edit().putStringSet("ids", seen).apply()
    }

    private fun fireIntent(ctx: Context, id: String) = Intent(ctx, AlarmReceiver::class.java).setAction(ACTION_FIRE).setData(Uri.parse("job:$id"))

    fun schedule(ctx: Context, id: String, at: Long, text: String, kind: String, ring: Boolean, sound: String) {
        val i = fireIntent(ctx, id).putExtra("id", id).putExtra("text", text).putExtra("kind", kind).putExtra("ring", ring).putExtra("sound", sound).putExtra("at", at)
        val pi = PendingIntent.getBroadcast(ctx, id.hashCode(), i, PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT)
        val am = ctx.getSystemService(AlarmManager::class.java)
        val open = PendingIntent.getActivity(ctx, 2, Intent(ctx, MainActivity::class.java), PendingIntent.FLAG_IMMUTABLE)
        when {
            // The alarm clock kind wakes the phone exactly, even in deep sleep, and shows the clock icon.
            ring && am.canScheduleExactAlarms() -> am.setAlarmClock(AlarmManager.AlarmClockInfo(at, open), pi)
            am.canScheduleExactAlarms() -> am.setExactAndAllowWhileIdle(AlarmManager.RTC_WAKEUP, at, pi)
            else -> am.setAndAllowWhileIdle(AlarmManager.RTC_WAKEUP, at, pi) // may be a few minutes late
        }
    }

    fun cancel(ctx: Context, id: String) {
        val pi = PendingIntent.getBroadcast(ctx, id.hashCode(), fireIntent(ctx, id), PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_NO_CREATE) ?: return
        ctx.getSystemService(AlarmManager::class.java).cancel(pi)
        pi.cancel()
    }

    /**
     * The next sync in about 15 minutes, so changes made elsewhere and held notices reach the
     * phone. A repeating inexact alarm is put off for hours while the phone sleeps (Doze), which
     * is why notices used to come only after the app was opened. This one is allowed while idle
     * and re-arms itself each time it fires (AlarmReceiver); it is one tiny request per run.
     */
    fun ensurePeriodic(ctx: Context) {
        val pi = PendingIntent.getBroadcast(ctx, 77, Intent(ctx, AlarmReceiver::class.java).setAction(ACTION_SYNC), PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT)
        val am = ctx.getSystemService(AlarmManager::class.java)
        am.cancel(pi) // an older version's repeating alarm shares this id
        am.setAndAllowWhileIdle(AlarmManager.ELAPSED_REALTIME_WAKEUP, SystemClock.elapsedRealtime() + AlarmManager.INTERVAL_FIFTEEN_MINUTES, pi)
    }

    fun soundFile(ctx: Context, name: String) = File(File(ctx.filesDir, "sounds"), name)

    /** Keep a copy of a custom ringtone on the phone, so it rings without the network. */
    private fun fetchSound(ctx: Context, name: String) {
        val f = soundFile(ctx, name)
        if (f.exists() && f.length() > 0) return
        try {
            val c = URL(base(ctx) + "/v1/sounds/" + Uri.encode(name)).openConnection() as HttpURLConnection
            c.setRequestProperty("Authorization", "Bearer " + token(ctx))
            c.connectTimeout = 8000
            c.readTimeout = 20000
            if (c.responseCode == 200) {
                f.parentFile?.mkdirs()
                val tmp = File(f.path + ".part")
                c.inputStream.use { inp -> tmp.outputStream().use { inp.copyTo(it) } }
                tmp.renameTo(f)
            }
            c.disconnect()
        } catch (e: Exception) { }
    }

    /** A quiet reminder: one notification, no ringing. */
    fun notify(ctx: Context, id: String, text: String, kind: String) {
        val nm = ctx.getSystemService(NotificationManager::class.java)
        nm.deleteNotificationChannel("reminders") // the old one, with the phone's plain sound
        nm.createNotificationChannel(NotificationChannel(CH_REMIND, "Reminders", NotificationManager.IMPORTANCE_HIGH).apply {
            setSound(Notices.friendly(ctx), AudioAttributes.Builder().setUsage(AudioAttributes.USAGE_NOTIFICATION).setContentType(AudioAttributes.CONTENT_TYPE_SONIFICATION).build())
        })
        val open = PendingIntent.getActivity(ctx, 3, Intent(ctx, MainActivity::class.java), PendingIntent.FLAG_IMMUTABLE)
        val n = Notification.Builder(ctx, CH_REMIND)
            .setSmallIcon(android.R.drawable.ic_popup_reminder)
            .setContentTitle(if (kind == "event") "Coming up" else "Reminder")
            .setContentText(text)
            .setStyle(Notification.BigTextStyle().bigText(text))
            .setCategory(Notification.CATEGORY_REMINDER)
            .setContentIntent(open)
            .setAutoCancel(true)
            .build()
        nm.notify(id.hashCode(), n)
    }
}

/** An alarm or reminder is due (or it is time to sync). */
class AlarmReceiver : BroadcastReceiver() {
    override fun onReceive(ctx: Context, intent: Intent) {
        when (intent.action) {
            Alarms.ACTION_FIRE -> {
                val id = intent.getStringExtra("id") ?: return
                val text = intent.getStringExtra("text") ?: "Alarm"
                val kind = intent.getStringExtra("kind") ?: "alarm"
                // Too late to ring (the phone was off): just say it was missed.
                val late = System.currentTimeMillis() - intent.getLongExtra("at", 0) > 30 * 60_000
                if (intent.getBooleanExtra("ring", false) && !late) {
                    ctx.startForegroundService(Intent(ctx, RingService::class.java).putExtras(intent))
                } else {
                    Alarms.notify(ctx, id, if (late && kind == "alarm") "Missed alarm: $text" else text, kind)
                }
            }
            Intent.ACTION_BOOT_COMPLETED, Intent.ACTION_MY_PACKAGE_REPLACED -> { Alarms.ensurePeriodic(ctx); LocationService.resume(ctx) }
        }
        if (intent.action == Alarms.ACTION_SYNC) Alarms.ensurePeriodic(ctx) // the next check
        // Set the next repeat, and pick up changes made elsewhere.
        val p = goAsync()
        Alarms.syncAsync(ctx) { p.finish() }
    }
}
