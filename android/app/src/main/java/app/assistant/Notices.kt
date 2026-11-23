package app.assistant

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import org.json.JSONObject

/**
 * What the assistant tells you unasked (`assistant.notice`), as real phone notifications, also
 * while the app is closed. No Google services: the app asks your server for new notices with the
 * saved cursor on the same light timer as the alarm sync (about every 15 minutes, batched by Android
 * to save battery), and whenever the app opens.
 *
 * normal: the default channel, with sound. silent: a quiet channel, no sound or buzz.
 * urgent: high importance (pops up). Tapping one opens its chat.
 */
object Notices {
    private const val CH_NORMAL = "notices_friendly"
    private const val CH_SILENT = "notices_quiet"
    private const val CH_URGENT = "notices_urgent"
    private const val KEY_CURSOR = "notice_cursor"
    private const val KEY_POSTED = "notice_posted"
    /** The app page is on screen: it shows notices itself, so none pops up twice. */
    @Volatile var pageOnScreen = false

    /** Android fixes a channel's sound when it is made, so the friendly sound lives on a new channel id. */
    fun friendly(ctx: Context): android.net.Uri = android.net.Uri.parse("android.resource://${ctx.packageName}/raw/notice_friendly")

    private fun channels(ctx: Context, nm: NotificationManager) {
        nm.deleteNotificationChannel("notices") // the old one, with the phone's plain sound
        nm.createNotificationChannel(NotificationChannel(CH_NORMAL, "Assistant", NotificationManager.IMPORTANCE_DEFAULT).apply {
            setSound(friendly(ctx), android.media.AudioAttributes.Builder().setUsage(android.media.AudioAttributes.USAGE_NOTIFICATION).setContentType(android.media.AudioAttributes.CONTENT_TYPE_SONIFICATION).build())
        })
        nm.createNotificationChannel(NotificationChannel(CH_SILENT, "Assistant (quiet)", NotificationManager.IMPORTANCE_LOW).apply {
            setSound(null, null); enableVibration(false)
        })
        nm.createNotificationChannel(NotificationChannel(CH_URGENT, "Assistant (urgent)", NotificationManager.IMPORTANCE_HIGH))
    }

    /** Ask the server for new notices and post them. Runs on a background thread. */
    fun poll(ctx: Context) {
        if (Alarms.token(ctx).isEmpty()) return
        val prefs = ctx.getSharedPreferences("app", Context.MODE_PRIVATE)
        val cursor = prefs.getLong(KEY_CURSOR, -1)
        var after = if (cursor < 0) 0 else cursor
        val nm = ctx.getSystemService(NotificationManager::class.java)
        channels(ctx, nm)
        var newest = after
        val posted = prefs.getStringSet(KEY_POSTED, emptySet())!!.toMutableSet()
        for (page in 0 until 10) { // at most 200 at once
            val (code, text) = Alarms.http(ctx, "GET", "/v1/notices?limit=20&cursor=$after")
            if (code != 200) break
            val items = JSONObject(text).optJSONArray("items") ?: break
            if (items.length() == 0) break
            for (i in 0 until items.length()) {
                val n = items.getJSONObject(i)
                newest = maxOf(newest, n.optLong("n"))
                // First run: only remember where we are; do not post the past.
                // The page and this check keep separate cursors; the posted list keeps one notice from showing twice.
                if (cursor < 0 || !n.optBoolean("delivered", true) || pageOnScreen || posted.contains(n.optLong("n").toString())) continue
                post(ctx, nm, n)
                posted.add(n.optLong("n").toString())
            }
            after = newest
            if (items.length() < 20) break
        }
        val keep = if (posted.size > 300) posted.sortedBy { it.toLong() }.takeLast(200).toSet() else posted
        prefs.edit().putStringSet(KEY_POSTED, keep).apply()
        if (newest != cursor) prefs.edit().putLong(KEY_CURSOR, newest).apply()
    }

    /** The page found a new notice while on screen: show it as a real phone notification (no chat line). */
    fun postFromPage(ctx: Context, json: String) {
        val n = try { JSONObject(json) } catch (e: Exception) { return }
        val nm = ctx.getSystemService(NotificationManager::class.java)
        channels(ctx, nm)
        val prefs = ctx.getSharedPreferences("app", Context.MODE_PRIVATE)
        val posted = prefs.getStringSet(KEY_POSTED, emptySet())!!.toMutableSet()
        if (!posted.add(n.optLong("n").toString())) return // the background check already showed it
        prefs.edit().putStringSet(KEY_POSTED, if (posted.size > 300) posted.sortedBy { it.toLongOrNull() ?: 0 }.takeLast(200).toSet() else posted).apply()
        post(ctx, nm, n)
    }

    private fun post(ctx: Context, nm: NotificationManager, n: JSONObject) {
        val silent = n.optBoolean("silent")
        val urgent = n.optString("priority") == "urgent"
        val ch = when { urgent -> CH_URGENT; silent -> CH_SILENT; else -> CH_NORMAL }
        val text = n.optString("text")
        val title = n.optString("title").takeIf { it.isNotBlank() && it != "null" } ?: "Assistant"
        val thread = n.optString("thread_id").takeIf { it.isNotBlank() && it != "null" }
        val open = Intent(ctx, MainActivity::class.java).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_SINGLE_TOP)
        if (thread != null) open.putExtra(MainActivity.EXTRA_THREAD, thread)
        val id = n.optLong("n").toInt()
        val pi = PendingIntent.getActivity(ctx, 1000 + (id and 0xFFFF), open, PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT)
        val b = Notification.Builder(ctx, ch)
            .setSmallIcon(android.R.drawable.ic_dialog_info)
            .setContentTitle(title)
            .setContentText(text)
            .setStyle(Notification.BigTextStyle().bigText(text))
            .setContentIntent(pi)
            .setAutoCancel(true)
            .setCategory(if (urgent) Notification.CATEGORY_ALARM else Notification.CATEGORY_MESSAGE)
        if (silent) b.setOnlyAlertOnce(true)
        // A reminder's notice: Snooze (same words again in 10 minutes) and Done.
        if (n.optString("job_id").let { it.isNotBlank() && it != "null" }) {
            fun act(code: String, label: String): Notification.Action {
                val i = Intent(ctx, NoticeAction::class.java).setAction(code).putExtra(NoticeAction.EXTRA_N, n.optLong("n")).putExtra(NoticeAction.EXTRA_ID, id)
                val p = PendingIntent.getBroadcast(ctx, (id and 0xFFFF) * 4 + (if (code == NoticeAction.SNOOZE) 1 else 2), i, PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT)
                return Notification.Action.Builder(null, label, p).build()
            }
            b.addAction(act(NoticeAction.SNOOZE, "Snooze 10 min")).addAction(act(NoticeAction.DONE, "Done"))
        } else {
            // Anything else the assistant decided to tell you: say if it was not worth it. This goes into the review pack.
            val i = Intent(ctx, NoticeAction::class.java).setAction(NoticeAction.NOT_USEFUL).putExtra(NoticeAction.EXTRA_N, n.optLong("n")).putExtra(NoticeAction.EXTRA_ID, id)
            val p = PendingIntent.getBroadcast(ctx, (id and 0xFFFF) * 4 + 3, i, PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT)
            b.addAction(Notification.Action.Builder(null, "Not useful", p).build())
        }
        try { nm.notify("notice", id, b.build()) } catch (e: SecurityException) { /* notifications are off for this app */ }
    }
}

/** The buttons on a reminder's notification. Done just clears it; Snooze asks the server for the same words in 10 minutes. */
class NoticeAction : android.content.BroadcastReceiver() {
    override fun onReceive(ctx: Context, intent: Intent) {
        val nm = ctx.getSystemService(NotificationManager::class.java)
        nm.cancel("notice", intent.getIntExtra(EXTRA_ID, 0))
        val n = intent.getLongExtra(EXTRA_N, 0)
        if (intent.action == NOT_USEFUL) {
            val pr = goAsync()
            kotlin.concurrent.thread {
                try { Alarms.http(ctx.applicationContext, "POST", "/v1/notices/$n/feedback", org.json.JSONObject().put("rating", "not_useful").toString()) } finally { pr.finish() }
            }
            return
        }
        if (intent.action != SNOOZE) return
        val pr = goAsync()
        kotlin.concurrent.thread {
            try {
                val code = Alarms.http(ctx.applicationContext, "POST", "/v1/notices/$n/snooze", org.json.JSONObject().put("minutes", 10).toString()).first
                // Copy the new reminder into the phone's alarm clock now, so it comes on time.
                if (code == 200) Alarms.syncAsync(ctx.applicationContext)
            } finally { pr.finish() }
        }
    }

    companion object {
        const val SNOOZE = "app.assistant.NOTICE_SNOOZE"
        const val DONE = "app.assistant.NOTICE_DONE"
        const val NOT_USEFUL = "app.assistant.NOTICE_NOT_USEFUL"
        const val EXTRA_N = "n"
        const val EXTRA_ID = "id"
    }
}
