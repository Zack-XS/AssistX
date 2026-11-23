package app.assistant

import android.app.Notification
import android.content.ComponentName
import android.content.Context
import android.provider.Settings
import android.service.notification.NotificationListenerService
import android.service.notification.StatusBarNotification
import org.json.JSONArray
import org.json.JSONObject
import java.util.concurrent.Executors

/**
 * Passes notifications to your server, so an automation can start on one ("when Gmail
 * notifies, check the mail"). Only apps that have such an automation are sent: the server tells
 * the phone which ones (GET /v1/automations/watch). Everything else stays on the phone; only the
 * app's name (no text) is remembered here, so the Automation page can offer it as a choice.
 * Separately, every notification is matched on the phone for parcels, bills and plans (`Detect`);
 * only a small item (kind, title, date, amount, app) is sent, and codes are never looked at.
 */
class NotifyListener : NotificationListenerService() {
    private val io = Executors.newSingleThreadExecutor()

    override fun onListenerConnected() {
        io.execute { refresh(this) }
    }

    override fun onNotificationPosted(sbn: StatusBarNotification) {
        if (sbn.packageName == packageName || sbn.isOngoing) return
        val n = sbn.notification ?: return
        if (n.flags and Notification.FLAG_GROUP_SUMMARY != 0) return // the bundle header repeats its children
        val label = appName(this, sbn.packageName)
        remember(this, sbn.packageName, label)
        val ex = n.extras
        val title = ex.getCharSequence(Notification.EXTRA_TITLE)?.toString().orEmpty()
        val text = (ex.getCharSequence(Notification.EXTRA_BIG_TEXT) ?: ex.getCharSequence(Notification.EXTRA_TEXT))?.toString().orEmpty()
        if (System.currentTimeMillis() - lastRefresh > 15 * 60_000) { lastRefresh = System.currentTimeMillis(); io.execute { refresh(this) } }
        if (title.isBlank() && text.isBlank()) return
        // Parcels, bills, plans: matched here, only a small item leaves the phone; codes are never looked at.
        val kinds = Detect.kinds(this)
        if (kinds.isNotEmpty() && Alarms.token(this).isNotEmpty()) {
            Detect.match(label, title, text, kinds)?.let { item -> io.execute { Detect.send(this, item) } }
        }
        if (sbn.packageName !in watched(this)) return
        val body = JSONObject().put("app", sbn.packageName).put("app_name", label).put("title", title.take(200)).put("text", text.take(2000)).toString()
        io.execute {
            Alarms.http(this, "POST", "/v1/automations/notify", body)
            if (System.currentTimeMillis() - lastRefresh > 15 * 60_000) refresh(this)
        }
    }

    override fun onDestroy() {
        io.shutdown()
        super.onDestroy()
    }

    companion object {
        @Volatile private var lastRefresh = 0L

        private fun prefs(ctx: Context) = ctx.getSharedPreferences("notify", Context.MODE_PRIVATE)

        fun allowed(ctx: Context): Boolean {
            val flat = Settings.Secure.getString(ctx.contentResolver, "enabled_notification_listeners").orEmpty()
            return flat.split(':').any { ComponentName.unflattenFromString(it)?.packageName == ctx.packageName }
        }

        fun watched(ctx: Context): Set<String> = prefs(ctx).getStringSet("watch", emptySet()) ?: emptySet()

        /** Ask the server which apps start an automation. Runs off the main thread. */
        fun refresh(ctx: Context) {
            lastRefresh = System.currentTimeMillis()
            if (Alarms.token(ctx).isEmpty()) return
            Detect.refresh(ctx)
            val (code, body) = Alarms.http(ctx, "GET", "/v1/automations/watch")
            if (code != 200) return
            val arr = JSONObject(body).optJSONArray("apps") ?: JSONArray()
            prefs(ctx).edit().putStringSet("watch", (0 until arr.length()).map { arr.getString(it) }.toSet()).apply()
        }

        private fun appName(ctx: Context, pkg: String): String = try {
            ctx.packageManager.getApplicationLabel(ctx.packageManager.getApplicationInfo(pkg, 0)).toString()
        } catch (e: Exception) { pkg }

        /** App names only (never the text), so the page can offer apps that send notifications. */
        private fun remember(ctx: Context, pkg: String, name: String) {
            val p = prefs(ctx)
            val seen = JSONObject(p.getString("seen", "{}") ?: "{}")
            if (seen.optString(pkg) == name) return
            seen.put(pkg, name)
            p.edit().putString("seen", seen.toString()).apply()
        }

        fun seenApps(ctx: Context): String {
            val seen = JSONObject(prefs(ctx).getString("seen", "{}") ?: "{}")
            val out = JSONArray()
            seen.keys().forEach { out.put(JSONObject().put("app", it).put("name", seen.optString(it))) }
            return out.toString()
        }
    }
}
