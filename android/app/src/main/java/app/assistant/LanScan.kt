package app.assistant

import android.content.Context
import android.net.ConnectivityManager
import android.net.NetworkCapabilities
import org.json.JSONObject
import java.net.HttpURLConnection
import java.net.Inet4Address
import java.net.URL
import java.util.concurrent.Executors
import java.util.concurrent.TimeUnit

/**
 * Looks for AssistX servers on the phone's own Wi-Fi or Ethernet network: every address of the /24 around the phone,
 * port 8790, `GET /v1/health`. A server counts only if its answer says `"name": "AssistX"` (the handshake).
 * Nothing is sent but that one request; nothing leaves the local network.
 */
object LanScan {
    const val PORT = 8790
    @Volatile private var running = false

    /** Calls [found] (from a worker thread) for each server, then [done]. Does nothing if a scan is already running. */
    fun start(ctx: Context, found: (JSONObject) -> Unit, done: () -> Unit) {
        if (running) return
        val prefix = localPrefix(ctx) ?: return done()
        running = true
        Thread {
            val pool = Executors.newFixedThreadPool(32)
            for (i in 1..254) pool.execute { probe("$prefix.$i")?.let(found) }
            pool.shutdown()
            pool.awaitTermination(20, TimeUnit.SECONDS)
            pool.shutdownNow()
            running = false
            done()
        }.start()
    }

    /** "192.168.1" for a phone at 192.168.1.23, only on Wi-Fi or Ethernet (never mobile data or a VPN). */
    private fun localPrefix(ctx: Context): String? {
        val cm = ctx.getSystemService(ConnectivityManager::class.java) ?: return null
        val net = cm.activeNetwork ?: return null
        val caps = cm.getNetworkCapabilities(net) ?: return null
        if (!caps.hasTransport(NetworkCapabilities.TRANSPORT_WIFI) && !caps.hasTransport(NetworkCapabilities.TRANSPORT_ETHERNET)) return null
        val ip = cm.getLinkProperties(net)?.linkAddresses?.map { it.address }?.filterIsInstance<Inet4Address>()?.firstOrNull { it.isSiteLocalAddress } ?: return null
        return ip.hostAddress?.substringBeforeLast('.')
    }

    private fun probe(host: String): JSONObject? = try {
        val c = URL("http://$host:$PORT/v1/health").openConnection() as HttpURLConnection
        c.connectTimeout = 600
        c.readTimeout = 1500
        c.instanceFollowRedirects = false
        try {
            if (c.responseCode != 200) null
            else {
                val body = c.inputStream.bufferedReader().use { r -> CharArray(4096).let { b -> String(b, 0, maxOf(0, r.read(b))) } }
                val j = JSONObject(body)
                if (j.optString("name") != "AssistX") null
                else JSONObject().put("base", "http://$host:$PORT").put("host", host).put("version", j.optString("version").take(40)).put("setup", j.optBoolean("setup"))
            }
        } finally { c.disconnect() }
    } catch (e: Exception) { null }
}
