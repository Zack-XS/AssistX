package app.assistant

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import org.json.JSONObject
import java.time.LocalDate
import java.time.LocalTime
import java.util.concurrent.Executors

/**
 * Tells the server the user woke up, once a day (POST /v1/signals {kind: "woke"}), which starts the
 * morning brief. Whichever comes first:
 * - the first unlock after the screen was off 4 hours or more, between 03:00 and 12:00;
 * - dismissing an alarm between 03:00 and 12:00;
 * - a Health Connect sleep session that ended this morning (seen when health numbers sync).
 * The server checks the same rule (src/signals.rs) and also sends the brief only once a day.
 * Screen events reach the app only while one of its services runs (Location keeps one running).
 */
object Woke {
    private const val KEY_OFF_AT = "woke_screen_off_at"
    private const val KEY_DAY = "woke_day"
    private const val NIGHT_OFF_MS = 4 * 3600_000L
    private val io = Executors.newSingleThreadExecutor()

    private fun prefs(ctx: Context) = ctx.getSharedPreferences("app", Context.MODE_PRIVATE)

    /** The wake rule (same as the server's `is_wake`). */
    fun isWake(source: String, at: LocalTime, offMs: Long?): Boolean {
        if (at.hour < 3 || at.hour >= 12) return false
        return when (source) {
            "unlock" -> offMs == null || offMs >= NIGHT_OFF_MS
            "alarm", "sleep", "manual" -> true
            else -> false
        }
    }

    fun screenOff(ctx: Context) {
        // Keep the first screen-off since the last unlock: a notification lighting the screen at night does not reset it.
        if (prefs(ctx).getLong(KEY_OFF_AT, 0L) == 0L) prefs(ctx).edit().putLong(KEY_OFF_AT, System.currentTimeMillis()).apply()
    }

    fun unlocked(ctx: Context) {
        val off = prefs(ctx).getLong(KEY_OFF_AT, 0L)
        if (off == 0L) return
        val offMs = System.currentTimeMillis() - off
        prefs(ctx).edit().remove(KEY_OFF_AT).apply()
        signal(ctx, "unlock", System.currentTimeMillis(), offMs)
    }

    fun alarmDismissed(ctx: Context) = signal(ctx, "alarm", System.currentTimeMillis(), null)

    /** A sleep session ended at `endMs` (from Health Connect). Only a fresh one this morning counts. */
    fun sleepEnded(ctx: Context, endMs: Long) {
        if (System.currentTimeMillis() - endMs > 3 * 3600_000L) return
        signal(ctx, "sleep", endMs, null)
    }

    private fun signal(ctx: Context, source: String, atMs: Long, offMs: Long?) {
        val today = LocalDate.now().toString()
        val at = java.time.Instant.ofEpochMilli(atMs).atZone(java.time.ZoneId.systemDefault())
        if (at.toLocalDate().toString() != today) return
        if (!isWake(source, at.toLocalTime(), offMs)) return
        if (prefs(ctx).getString(KEY_DAY, null) == today) return
        if (Alarms.token(ctx).isEmpty()) return
        val app = ctx.applicationContext
        io.execute {
            if (prefs(app).getString(KEY_DAY, null) == today) return@execute
            val body = JSONObject().put("kind", "woke").put("at", atMs / 1000).put("source", source)
            if (offMs != null) body.put("off_secs", offMs / 1000)
            val code = Alarms.http(app, "POST", "/v1/signals", body.toString()).first
            // 400 = the server will not take it (bad time): do not try again today either.
            if (code == 200 || code == 400) prefs(app).edit().putString(KEY_DAY, today).apply()
        }
    }

    /** Screen off and unlock, while a service of the app runs. */
    class ScreenReceiver : BroadcastReceiver() {
        override fun onReceive(ctx: Context, intent: Intent) {
            when (intent.action) {
                Intent.ACTION_SCREEN_OFF -> screenOff(ctx)
                Intent.ACTION_USER_PRESENT -> unlocked(ctx)
            }
        }
    }

    fun register(ctx: Context): BroadcastReceiver {
        val r = ScreenReceiver()
        val f = IntentFilter().apply { addAction(Intent.ACTION_SCREEN_OFF); addAction(Intent.ACTION_USER_PRESENT) }
        ctx.registerReceiver(r, f, Context.RECEIVER_NOT_EXPORTED)
        return r
    }
}
