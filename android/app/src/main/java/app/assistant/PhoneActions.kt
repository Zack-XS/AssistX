package app.assistant

import android.Manifest
import android.app.ActivityManager
import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.net.Uri
import android.provider.AlarmClock
import android.provider.ContactsContract
import org.json.JSONObject
import java.util.concurrent.Executors

/**
 * Things the assistant asks this phone to do (the server's `phone` plugin). The server queues
 * them; the app takes them with `GET /v1/phone/actions` when the event stream says `phone.action`,
 * when the page sees that event, and when the app opens, then reports each one back.
 *
 * Each action only opens a screen: the dialer with the number, the messages app with the words,
 * an app, the clock app's timer or alarm, or directions. Nothing is dialled or sent by itself.
 * Contact names become numbers here, on the phone; the address book never goes to the server.
 *
 * Android does not let an app open screens from the background, so when the app is not in front
 * (and may not draw over other apps) the action waits as a high-priority notification: tap it.
 */
object PhoneActions {
    private val io = Executors.newSingleThreadExecutor()
    private const val CH = "phone_actions"

    /** The app page asks for Contacts when a name could not be looked up (set by MainActivity). */
    @Volatile var askContacts: (() -> Unit)? = null

    fun pollAsync(ctx: Context) {
        val app = ctx.applicationContext
        io.execute { try { poll(app) } catch (e: Exception) { } }
    }

    /** Take waiting actions and do them. Runs on a background thread. */
    fun poll(ctx: Context) {
        if (Alarms.token(ctx).isEmpty()) return
        val (code, text) = Alarms.http(ctx, "GET", "/v1/phone/actions")
        if (code != 200) return
        val items = try { JSONObject(text).optJSONArray("items") } catch (e: Exception) { null } ?: return
        for (i in 0 until items.length()) {
            val a = items.getJSONObject(i)
            val out = try { run(ctx, a.optString("tool"), a.optJSONObject("args") ?: JSONObject()) } catch (e: Exception) {
                JSONObject().put("ok", false).put("error", e.message ?: "it did not work")
            }
            Alarms.http(ctx, "POST", "/v1/phone/actions/" + Uri.encode(a.optString("id")), out.toString())
        }
    }

    private fun fail(why: String) = JSONObject().put("ok", false).put("error", why)

    private fun run(ctx: Context, tool: String, a: JSONObject): JSONObject {
        var contact: String? = null
        val (intent, title) = when (tool) {
            "call", "text" -> {
                val to = a.optString("to").trim()
                val number = if (to.any { it.isLetter() }) {
                    if (ctx.checkSelfPermission(Manifest.permission.READ_CONTACTS) != PackageManager.PERMISSION_GRANTED) {
                        askContacts?.invoke()
                        return fail("Contacts are not allowed for the app on the phone. Allow Contacts in the app's setup list, or give the number.")
                    }
                    val hit = lookup(ctx, to) ?: return fail("No contact on the phone matches \"$to\". Ask for the number.")
                    contact = hit.first
                    hit.second
                } else to
                if (number.count { it.isDigit() } < 3) return fail("\"$to\" is not a phone number")
                if (tool == "call") Intent(Intent.ACTION_DIAL, Uri.fromParts("tel", number, null)) to "Call ${contact ?: number}"
                else Intent(Intent.ACTION_SENDTO, Uri.parse("smsto:" + Uri.encode(number))).putExtra("sms_body", a.optString("message")) to "Text ${contact ?: number}"
            }
            "open_app" -> {
                val name = a.optString("name").trim()
                val (label, i) = findApp(ctx, name) ?: return fail("No app called \"$name\" on the phone.")
                i to "Open $label"
            }
            "set_timer" -> {
                val secs = a.optInt("seconds")
                val i = Intent(AlarmClock.ACTION_SET_TIMER).putExtra(AlarmClock.EXTRA_LENGTH, secs).putExtra(AlarmClock.EXTRA_SKIP_UI, true)
                a.optString("label").takeIf { it.isNotBlank() && it != "null" }?.let { i.putExtra(AlarmClock.EXTRA_MESSAGE, it) }
                i to "Timer: " + (if (secs >= 60) "${secs / 60} min" else "$secs s")
            }
            "set_phone_alarm" -> {
                val h = a.optInt("hour"); val m = a.optInt("minute")
                val i = Intent(AlarmClock.ACTION_SET_ALARM).putExtra(AlarmClock.EXTRA_HOUR, h).putExtra(AlarmClock.EXTRA_MINUTES, m).putExtra(AlarmClock.EXTRA_SKIP_UI, true)
                val days = a.optJSONArray("days")
                if (days != null && days.length() > 0) i.putExtra(AlarmClock.EXTRA_DAYS, ArrayList((0 until days.length()).map { days.getInt(it) }))
                a.optString("label").takeIf { it.isNotBlank() && it != "null" }?.let { i.putExtra(AlarmClock.EXTRA_MESSAGE, it) }
                i to "Alarm at %02d:%02d".format(h, m)
            }
            "navigate_to" -> {
                val dest = a.optString("destination").trim()
                val mode = when (a.optString("mode")) { "walking" -> "w"; "bicycling" -> "b"; "transit" -> "r"; else -> "d" }
                // A navigation app that takes a travel mode, else any map app (geo:).
                val nav = Intent(Intent.ACTION_VIEW, Uri.parse("google.navigation:q=" + Uri.encode(dest) + "&mode=" + mode))
                val i = if (nav.resolveActivity(ctx.packageManager) != null) nav else Intent(Intent.ACTION_VIEW, Uri.parse("geo:0,0?q=" + Uri.encode(dest)))
                i to "Directions to $dest"
            }
            else -> return fail("this app does not know '$tool' yet; update the app")
        }
        if (intent.resolveActivity(ctx.packageManager) == null) return fail("No app on the phone can do that.")
        intent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
        val out = JSONObject().put("ok", true)
        contact?.let { out.put("contact", it) }
        if (canOpenNow(ctx)) {
            try { ctx.startActivity(intent); return out.put("shown", "screen") } catch (e: Exception) { /* fall back to a notification */ }
        }
        notify(ctx, intent, title)
        return out.put("shown", "notification")
    }

    /** The app is on screen (or may draw over other apps), so Android lets it open a screen now. */
    private fun canOpenNow(ctx: Context): Boolean {
        val me = ActivityManager.RunningAppProcessInfo().also { ActivityManager.getMyMemoryState(it) }
        return me.importance <= ActivityManager.RunningAppProcessInfo.IMPORTANCE_FOREGROUND || android.provider.Settings.canDrawOverlays(ctx)
    }

    private fun notify(ctx: Context, intent: Intent, title: String) {
        val nm = ctx.getSystemService(NotificationManager::class.java)
        nm.createNotificationChannel(NotificationChannel(CH, "Phone actions", NotificationManager.IMPORTANCE_HIGH))
        val id = (System.currentTimeMillis() and 0xFFFFFF).toInt()
        val pi = PendingIntent.getActivity(ctx, 5000 + (id and 0xFFFF), intent, PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT)
        val n = Notification.Builder(ctx, CH)
            .setSmallIcon(android.R.drawable.ic_dialog_info)
            .setContentTitle(title)
            .setContentText("Tap to open")
            .setContentIntent(pi)
            .setAutoCancel(true)
            .setCategory(Notification.CATEGORY_REMINDER)
            .build()
        try { nm.notify("phone_action", id, n) } catch (e: SecurityException) { }
    }

    /** A contact by name: (display name, number). An exact name wins, then a mobile number. */
    private fun lookup(ctx: Context, name: String): Pair<String, String>? {
        val uri = Uri.withAppendedPath(ContactsContract.CommonDataKinds.Phone.CONTENT_FILTER_URI, Uri.encode(name))
        val cols = arrayOf(ContactsContract.CommonDataKinds.Phone.DISPLAY_NAME, ContactsContract.CommonDataKinds.Phone.NUMBER,
            ContactsContract.CommonDataKinds.Phone.TYPE, ContactsContract.CommonDataKinds.Phone.IS_SUPER_PRIMARY)
        val hits = mutableListOf<Triple<String, String, Int>>()
        ctx.contentResolver.query(uri, cols, null, null, null)?.use { c ->
            while (c.moveToNext() && hits.size < 50) {
                val n = c.getString(0) ?: continue
                val num = c.getString(1) ?: continue
                var score = 0
                if (n.equals(name, true)) score += 4
                if (c.getInt(3) != 0) score += 2
                if (c.getInt(2) == ContactsContract.CommonDataKinds.Phone.TYPE_MOBILE) score += 1
                hits.add(Triple(n, num, score))
            }
        }
        return hits.maxByOrNull { it.third }?.let { it.first to it.second }
    }

    /** A launchable app by its name: the same name first, then one that starts with it, then one that holds it. */
    private fun findApp(ctx: Context, name: String): Pair<String, Intent>? {
        val pm = ctx.packageManager
        val apps = pm.queryIntentActivities(Intent(Intent.ACTION_MAIN).addCategory(Intent.CATEGORY_LAUNCHER), 0)
            .map { it.loadLabel(pm).toString() to it.activityInfo.packageName }
        val n = name.lowercase()
        val pick = apps.firstOrNull { it.first.lowercase() == n } ?: apps.firstOrNull { it.first.lowercase().startsWith(n) }
            ?: apps.firstOrNull { it.first.lowercase().contains(n) } ?: return null
        val i = pm.getLaunchIntentForPackage(pick.second) ?: return null
        return pick.first to i
    }
}
