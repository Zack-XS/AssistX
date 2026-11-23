package app.assistant

import android.annotation.SuppressLint
import android.app.PendingIntent
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.location.LocationManager
import org.json.JSONObject
import kotlin.concurrent.thread

/**
 * Arriving at and leaving a place, told by the phone itself (no Google services: Android's own
 * proximity alerts). The server says which places a reminder or task waits on (`GET
 * /v1/location/fences`); each gets a circle. When the phone crosses one, a single small message
 * goes up (`POST /v1/location/transitions`), so "when I get home" does not wait for the 15 minute
 * batch of location points. The 5 minute tracking stays as the backup.
 */
object Fences {
    private const val KEY = "fence_ids"
    private const val ACTION = "app.assistant.FENCE"
    const val EXTRA_PLACE = "place"
    /** An alert fires at once when the phone is already inside a new circle; that is not an arrival. */
    private const val SETTLE_MS = 90_000L

    private fun prefs(ctx: Context) = ctx.getSharedPreferences("app", Context.MODE_PRIVATE)
    private fun registeredAt(ctx: Context, id: String) = prefs(ctx).getLong("fence_at_$id", 0)

    private fun pending(ctx: Context, id: String): PendingIntent =
        PendingIntent.getBroadcast(
            ctx, id.hashCode(),
            Intent(ctx, FenceReceiver::class.java).setAction(ACTION).putExtra(EXTRA_PLACE, id),
            // The system adds "entering" to this intent, so it must be mutable (it stays explicit).
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_MUTABLE,
        )

    /** Make the circles match what the server wants watched. Runs on a background thread. */
    @SuppressLint("MissingPermission")
    fun sync(ctx: Context) {
        if (!LocationService.allowed(ctx) || !LocationService.background(ctx) || Alarms.token(ctx).isEmpty()) return
        val (code, text) = Alarms.http(ctx, "GET", "/v1/location/fences")
        if (code != 200) return
        val want = try {
            val a = JSONObject(text).optJSONArray("fences")
            (0 until (a?.length() ?: 0)).map { a!!.getJSONObject(it) }
        } catch (e: Exception) { return }
        val lm = ctx.getSystemService(LocationManager::class.java)
        val old = prefs(ctx).getStringSet(KEY, emptySet()).orEmpty()
        val keep = want.map { it.getString("id") }.toSet()
        for (id in old - keep) { try { lm.removeProximityAlert(pending(ctx, id)) } catch (e: Exception) { } ; prefs(ctx).edit().remove("fence_at_$id").apply() }
        for (f in want) {
            val id = f.getString("id")
            if (id in old) continue // already registered
            try {
                lm.addProximityAlert(f.getDouble("lat"), f.getDouble("lon"), maxOf(f.optDouble("radius", 100.0), 120.0).toFloat(), -1L, pending(ctx, id))
                prefs(ctx).edit().putLong("fence_at_$id", System.currentTimeMillis()).apply()
            } catch (e: Exception) { /* try again at the next sync */ return }
        }
        prefs(ctx).edit().putStringSet(KEY, keep).apply()
    }

    /** A circle was crossed: tell the server, once. */
    fun crossed(ctx: Context, id: String, entering: Boolean) {
        if (entering && System.currentTimeMillis() - registeredAt(ctx, id) < SETTLE_MS) return
        val body = JSONObject().put("place", id).put("what", if (entering) "arrive" else "leave").put("ts", System.currentTimeMillis() / 1000)
        for (i in 0 until 3) { // a few tries: the network may be waking up
            val code = Alarms.http(ctx, "POST", "/v1/location/transitions", body.toString()).first
            if (code == 200 || code == 400) return
            Thread.sleep(4000)
        }
    }
}

class FenceReceiver : BroadcastReceiver() {
    override fun onReceive(ctx: Context, intent: Intent) {
        val id = intent.getStringExtra(Fences.EXTRA_PLACE) ?: return
        val entering = intent.getBooleanExtra(LocationManager.KEY_PROXIMITY_ENTERING, true)
        val pr = goAsync()
        thread { try { Fences.crossed(ctx.applicationContext, id, entering) } finally { pr.finish() } }
    }
}
