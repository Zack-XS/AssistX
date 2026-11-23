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
import android.location.Location
import android.location.LocationListener
import android.location.LocationManager
import android.location.LocationRequest
import android.os.Handler
import android.os.IBinder
import android.os.Looper
import org.json.JSONArray
import org.json.JSONObject
import java.io.File
import java.util.concurrent.Executors

/**
 * Where you go (docs/10). While it is on, the phone notes its place about every 5 minutes and
 * sends the points to your server, which works out visits and names places.
 *
 * Kind to the battery:
 * - "balanced" accuracy: Wi-Fi and cell towers, GPS only when the system already has it.
 * - The system may hold points and hand them over in a batch every 15 minutes, so the phone can
 *   stay asleep in between.
 * - Points are sent together, at most every 15 minutes (one short request), not one by one.
 * - Points are kept even when the phone does not move: the server needs them to see a stay.
 */
class LocationService : Service() {
    private val main = Handler(Looper.getMainLooper())
    private val io = Executors.newSingleThreadExecutor()
    private var lm: LocationManager? = null
    private var lastSent = 0L

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        if (intent?.action == ACTION_STOP) {
            prefs(this).edit().putBoolean(KEY_ON, false).apply()
            stopSelf()
            return START_NOT_STICKY
        }
        if (!allowed(this)) { stopSelf(); return START_NOT_STICKY }
        startForeground(NOTE_ID, notification(), ServiceInfo.FOREGROUND_SERVICE_TYPE_LOCATION)
        if (lm == null) start()
        active = true
        listener?.invoke()
        return START_STICKY // the system brings it back if it has to stop it
    }

    @Suppress("MissingPermission")
    private fun start() {
        val m = getSystemService(LocationManager::class.java)
        lm = m
        val provider = listOf(LocationManager.FUSED_PROVIDER, LocationManager.NETWORK_PROVIDER, LocationManager.GPS_PROVIDER).firstOrNull { m.allProviders.contains(it) } ?: return
        chosen = provider
        request(m, provider, EVERY_MS)
        // One point now, from what the phone already knows (costs nothing).
        m.getLastKnownLocation(provider)?.let { keep(listOf(it)) }
        main.postDelayed(flushTick, BATCH_MS)
        io.execute { try { Fences.sync(this) } catch (e: Exception) { } } // circles for reminders that wait on a place
        NoticeStream.start(this) // notices and place changes at once
        if (screen == null) screen = try { Woke.register(this) } catch (e: Exception) { null } // first unlock after the night: the morning brief
    }

    private var screen: android.content.BroadcastReceiver? = null
    private var chosen: String? = null
    private var every = EVERY_MS
    private var lastPoint: Location? = null
    private var still = 0

    @Suppress("MissingPermission")
    private fun request(m: LocationManager, provider: String, ms: Long) {
        every = ms
        val req = LocationRequest.Builder(ms)
            .setQuality(LocationRequest.QUALITY_BALANCED_POWER_ACCURACY)
            .setMinUpdateIntervalMillis(ms - 60_000)
            .setMaxUpdateDelayMillis(BATCH_MS)
            .build()
        m.requestLocationUpdates(provider, req, io, got)
    }

    /**
     * Staying put (three points within 150 m) means the next ones can come every 15 minutes
     * instead of 5; the first move puts it back at once. Places you arrive at or leave are caught
     * by the phone's own geofences (Fences), so a slower beat does not delay those.
     */
    private fun adapt(l: Location) {
        val m = lm ?: return
        val p = chosen ?: return
        val prev = lastPoint
        lastPoint = l
        if (prev != null && prev.distanceTo(l) < 150f) still++ else still = 0
        val want = if (still >= 3) SLOW_MS else EVERY_MS
        if (want != every) {
            m.removeUpdates(got)
            request(m, p, want)
        }
    }

    private val got = object : LocationListener {
        override fun onLocationChanged(location: Location) = keep(listOf(location))
        override fun onLocationChanged(locations: MutableList<Location>) = keep(locations)
        override fun onProviderEnabled(provider: String) {}
        override fun onProviderDisabled(provider: String) {}
    }

    /** Save points to a small file first, so nothing is lost if the network or the app goes away. */
    private fun keep(list: List<Location>) {
        val f = queueFile(this)
        val lines = list.joinToString("") { l ->
            JSONObject().put("ts", l.time / 1000).put("lat", l.latitude).put("lon", l.longitude).put("acc", if (l.hasAccuracy()) l.accuracy.toDouble() else JSONObject.NULL).toString() + "\n"
        }
        synchronized(LOCK) {
            f.appendText(lines)
            // Keep at most about two weeks of points if the server stays out of reach.
            if (f.length() > 600_000) f.writeText(f.readLines().takeLast(4000).joinToString("\n", postfix = "\n"))
        }
        last = list.lastOrNull()?.let { "%.4f, %.4f".format(it.latitude, it.longitude) } ?: last
        list.lastOrNull()?.let { adapt(it) }
        if (System.currentTimeMillis() - lastSent > BATCH_MS) io.execute { send(this) }
    }

    private val flushTick = object : Runnable {
        override fun run() {
            io.execute { send(this@LocationService) }
            main.postDelayed(this, BATCH_MS)
        }
    }

    private fun send(ctx: Context) {
        lastSent = System.currentTimeMillis()
        sendQueue(ctx)
        try { PlaceWatch.check(ctx) } catch (e: Exception) { }
        try { Fences.sync(ctx) } catch (e: Exception) { }
    }

    /** Swiped away from recents: come back at once. (A force stop in Settings cannot be undone by any app.) */
    override fun onTaskRemoved(rootIntent: Intent?) {
        if (prefs(this).getBoolean(KEY_ON, true)) {
            val pi = PendingIntent.getForegroundService(this, 22, Intent(this, LocationService::class.java), PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT)
            getSystemService(android.app.AlarmManager::class.java).set(android.app.AlarmManager.RTC, System.currentTimeMillis() + 1500, pi)
        }
        super.onTaskRemoved(rootIntent)
    }

    override fun onDestroy() {
        main.removeCallbacksAndMessages(null)
        NoticeStream.stop()
        screen?.let { try { unregisterReceiver(it) } catch (e: Exception) { } }
        screen = null
        lm?.removeUpdates(got)
        lm = null
        io.execute { sendQueue(this) } // one last try
        io.shutdown()
        active = false
        listener?.invoke()
        super.onDestroy()
    }

    private fun notification(): Notification {
        val nm = getSystemService(NotificationManager::class.java)
        nm.createNotificationChannel(NotificationChannel(CHANNEL, "Location", NotificationManager.IMPORTANCE_MIN).apply { setShowBadge(false) })
        val stop = PendingIntent.getService(this, 20, Intent(this, LocationService::class.java).setAction(ACTION_STOP), PendingIntent.FLAG_IMMUTABLE)
        val open = PendingIntent.getActivity(this, 21, Intent(this, MainActivity::class.java), PendingIntent.FLAG_IMMUTABLE)
        return Notification.Builder(this, CHANNEL)
            .setSmallIcon(android.R.drawable.ic_menu_mylocation)
            .setContentTitle("Started, tracking")
            .setContentText("Your places go to your own server, every 5 to 15 minutes.")
            .setContentIntent(open)
            .setOngoing(true)
            .addAction(Notification.Action.Builder(Icon.createWithResource(this, android.R.drawable.ic_menu_close_clear_cancel), "Turn off", stop).build())
            .build()
    }

    companion object {
        const val ACTION_STOP = "app.assistant.STOP_LOCATION"
        const val KEY_ON = "location_on"
        private const val CHANNEL = "location"
        private const val NOTE_ID = 8
        private const val EVERY_MS = 5 * 60_000L
        private const val BATCH_MS = 15 * 60_000L
        private const val SLOW_MS = 15 * 60_000L
        private val LOCK = Any()

        @Volatile var active = false
        @Volatile var last: String? = null
        @Volatile var listener: (() -> Unit)? = null

        fun prefs(ctx: Context) = ctx.getSharedPreferences("app", Context.MODE_PRIVATE)
        private fun queueFile(ctx: Context) = File(ctx.filesDir, "location-queue.jsonl")

        fun allowed(ctx: Context) = ctx.checkSelfPermission(Manifest.permission.ACCESS_FINE_LOCATION) == PackageManager.PERMISSION_GRANTED ||
            ctx.checkSelfPermission(Manifest.permission.ACCESS_COARSE_LOCATION) == PackageManager.PERMISSION_GRANTED
        fun background(ctx: Context) = ctx.checkSelfPermission(Manifest.permission.ACCESS_BACKGROUND_LOCATION) == PackageManager.PERMISSION_GRANTED

        /** Start again after a restart or an update, if it was on. */
        fun resume(ctx: Context) {
            if (prefs(ctx).getBoolean(KEY_ON, true) && allowed(ctx) && background(ctx)) {
                try { ctx.startForegroundService(Intent(ctx, LocationService::class.java)) } catch (e: Exception) { }
            }
        }

        /** Send what is waiting, in one request. Points the server took are removed. */
        fun sendQueue(ctx: Context) {
            val f = queueFile(ctx)
            val lines = synchronized(LOCK) { if (f.exists()) f.readLines().filter { it.isNotBlank() } else emptyList() }
            if (lines.isEmpty() || Alarms.token(ctx).isEmpty()) return
            for (part in lines.chunked(1500)) {
                val arr = JSONArray()
                part.forEach { try { arr.put(JSONObject(it)) } catch (e: Exception) { } }
                val code = Alarms.http(ctx, "POST", "/v1/location/points", JSONObject().put("points", arr).toString()).first
                if (code != 200 && code != 400) return // try again later; 400 = bad points, drop them
                synchronized(LOCK) {
                    val now = if (f.exists()) f.readLines().filter { it.isNotBlank() } else emptyList()
                    f.writeText(now.drop(part.size).joinToString("") { "$it\n" })
                }
            }
        }
    }
}
