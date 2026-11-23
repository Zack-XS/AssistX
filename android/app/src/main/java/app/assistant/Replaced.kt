package app.assistant

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import java.io.File

/** After an update the app is stopped. Say so, and offer a tap to open it again. */
class Replaced : BroadcastReceiver() {
    override fun onReceive(c: Context, i: Intent) {
        if (i.action != Intent.ACTION_MY_PACKAGE_REPLACED) return
        File(c.cacheDir, "update.apk").delete()
        val nm = c.getSystemService(NotificationManager::class.java)
        nm.createNotificationChannel(NotificationChannel("updates", "App updates", NotificationManager.IMPORTANCE_LOW))
        val open = PendingIntent.getActivity(c, 2, Intent(c, MainActivity::class.java).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK), PendingIntent.FLAG_IMMUTABLE)
        val name = try { c.packageManager.getPackageInfo(c.packageName, 0).versionName.orEmpty() } catch (e: Exception) { "" }
        nm.notify(8, Notification.Builder(c, "updates")
            .setSmallIcon(android.R.drawable.stat_sys_download_done)
            .setContentTitle("Assistant updated")
            .setContentText("Now on version $name. Tap to open. If listening was on, turn it on again.")
            .setContentIntent(open)
            .setAutoCancel(true)
            .build())
    }
}
