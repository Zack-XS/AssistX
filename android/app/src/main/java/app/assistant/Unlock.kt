package app.assistant

import android.content.Context
import org.json.JSONObject
import java.net.HttpURLConnection
import java.net.URL

/**
 * "Remember this phone" (docs/15). When you sign in, the server gives this phone a random device key, once. It is
 * kept here sealed by the Android keystore, like the account token, and never stored on the server. If the server
 * restarts it forgets every account's key; any request then answers 423. This phone sends the device key, the
 * account unlocks by itself, and the request is tried again, so nobody types a passphrase. Signing out forgets it.
 */
object Unlock {
    private const val KEY = "device"
    private val lock = Any()
    @Volatile private var lastTry = 0L
    @Volatile private var lastOk = false

    private fun prefs(ctx: Context) = ctx.getSharedPreferences("app", Context.MODE_PRIVATE)

    fun save(ctx: Context, deviceJson: String) {
        prefs(ctx).edit().putString(KEY, Vault.seal(deviceJson)).apply()
    }

    private fun creds(ctx: Context): JSONObject? = try {
        val s = Vault.open(prefs(ctx).getString(KEY, null))
        if (s.isEmpty()) null else JSONObject(s)
    } catch (e: Exception) { null }

    private fun post(ctx: Context, path: String, c: JSONObject): Int = try {
        val u = URL(Alarms.base(ctx) + path).openConnection() as HttpURLConnection
        u.requestMethod = "POST"
        u.connectTimeout = 8000
        u.readTimeout = 15000
        u.doOutput = true
        u.setRequestProperty("Content-Type", "application/json")
        u.outputStream.use { it.write(JSONObject().put("device_id", c.optString("id")).put("key", c.optString("key")).toString().toByteArray()) }
        val code = u.responseCode
        u.disconnect()
        code
    } catch (e: Exception) { -1 }

    /** Unlock this phone's account on the server. True when it is unlocked now. Calls close together share one answer. */
    fun ensure(ctx: Context): Boolean = synchronized(lock) {
        if (System.currentTimeMillis() - lastTry < 4000) return lastOk
        val c = creds(ctx) ?: return false
        lastTry = System.currentTimeMillis()
        lastOk = post(ctx, "/v1/server/unlock", c) == 200
        lastOk
    }

    /** Sign out for good: the server forgets this phone's key, and so does the phone. */
    fun forget(ctx: Context) {
        creds(ctx)?.let { post(ctx, "/v1/server/unlock/forget", it) }
        prefs(ctx).edit().remove(KEY).apply()
    }
}
