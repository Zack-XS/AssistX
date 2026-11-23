package app.assistant

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import org.json.JSONObject

/**
 * Listening is place by place. After each location upload this asks the server where the phone is
 * and which places the user chose for listening (`listen_places`, docs/08).
 * - At a listening place and not listening: a notification, one tap to start (Android does not
 *   let a microphone service start by itself from the background).
 * - Listening anywhere else: it is stopped.
 */
object PlaceWatch {
    private const val CHANNEL = "place"
    private const val NOTE_ID = 9
    const val ACTION_START = "app.assistant.START_CAPTURE_HERE"

    /** Lower-case name of the place the phone is at, or "" (also used to tag uploaded chunks). */
    @Volatile var here = ""

    fun check(ctx: Context) {
        val now = Alarms.http(ctx, "GET", "/v1/location/now")
        if (now.first != 200) return
        here = try { JSONObject(now.second).optJSONObject("place")?.optString("name").orEmpty().trim().lowercase() } catch (e: Exception) { "" }
        val set = Alarms.http(ctx, "GET", "/v1/ambient/settings")
        if (set.first != 200) return
        val places = try {
            val a = JSONObject(set.second).optJSONArray("listen_places")
            (0 until (a?.length() ?: 0)).map { a!!.getString(it).lowercase() }
        } catch (e: Exception) { emptyList() }
        val nm = ctx.getSystemService(NotificationManager::class.java)
        val ok = here.isNotEmpty() && here in places
        if (!ok && CaptureService.active) {
            ctx.startService(Intent(ctx, CaptureService::class.java).setAction(CaptureService.ACTION_STOP))
        }
        if (ok && !CaptureService.active) {
            nm.createNotificationChannel(NotificationChannel(CHANNEL, "Listening places", NotificationManager.IMPORTANCE_DEFAULT))
            val tap = PendingIntent.getActivity(ctx, 30, Intent(ctx, MainActivity::class.java).setAction(ACTION_START).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_SINGLE_TOP), PendingIntent.FLAG_IMMUTABLE)
            nm.notify(NOTE_ID, Notification.Builder(ctx, CHANNEL)
                .setSmallIcon(android.R.drawable.ic_btn_speak_now)
                .setContentTitle("You are at $here")
                .setContentText("Tap to start listening here. It stops when you leave.")
                .setContentIntent(tap)
                .setAutoCancel(true)
                .build())
        } else {
            nm.cancel(NOTE_ID)
        }
    }
}
