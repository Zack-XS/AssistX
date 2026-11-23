package app.assistant

import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import android.content.pm.PackageInstaller
import android.net.ConnectivityManager
import android.net.NetworkCapabilities
import android.net.Uri
import android.provider.Settings
import org.json.JSONObject
import java.io.File
import java.net.HttpURLConnection
import java.net.URL
import java.security.MessageDigest
import java.util.concurrent.Executors

/**
 * Keeps the app up to date from your own server (docs/09). It asks `GET /v1/app/version`; when a
 * higher version is published it downloads `GET /v1/app/apk`, checks the checksum and that the
 * file is this app, then installs it through the system installer. After the first confirmed
 * update, later ones install without a prompt. Android only accepts an update signed with the
 * same key as the installed app.
 */
object Updater {
    enum class State { IDLE, CHECKING, UP_TO_DATE, AVAILABLE, DOWNLOADING, READY, INSTALLING, ERROR }

    @Volatile var state = State.IDLE
    @Volatile var latestCode = 0L
    @Volatile var latestName = ""
    @Volatile var notes = ""
    /** Recent versions and what changed, newest first (JSON array text from the server). */
    @Volatile var history = "[]"
    @Volatile var message = ""
    @Volatile var lastCheck = 0L
    @Volatile var listener: (() -> Unit)? = null

    private val io = Executors.newSingleThreadExecutor()
    private var sha = ""

    fun installedCode(ctx: Context): Long = ctx.packageManager.getPackageInfo(ctx.packageName, 0).longVersionCode
    fun installedName(ctx: Context): String = ctx.packageManager.getPackageInfo(ctx.packageName, 0).versionName.orEmpty()

    private fun set(s: State, msg: String = "") {
        state = s
        message = msg
        listener?.invoke()
    }

    private fun apkFile(ctx: Context) = File(ctx.cacheDir, "update.apk")

    fun json(ctx: Context, auto: Boolean): String = JSONObject()
        .put("state", state.name.lowercase())
        .put("installed", JSONObject().put("code", installedCode(ctx)).put("name", installedName(ctx)))
        .put("latest", JSONObject().put("code", latestCode).put("name", latestName).put("notes", notes))
        .put("history", try { org.json.JSONArray(history) } catch (e: Exception) { org.json.JSONArray() })
        .put("message", message)
        .put("auto", auto)
        .toString()

    /** Ask the server. `auto` checks do not download over a metered (mobile data) connection. */
    fun check(ctx: Context, auto: Boolean) {
        if (state == State.CHECKING || state == State.DOWNLOADING || state == State.INSTALLING) return
        val app = ctx.applicationContext
        val prefs = app.getSharedPreferences("app", Context.MODE_PRIVATE)
        val base = prefs.getString("base", null) ?: BuildConfig.DEFAULT_SERVER
        val token = Vault.open(prefs.getString("tok", null))
        if (token.isEmpty()) return
        lastCheck = System.currentTimeMillis()
        set(State.CHECKING)
        io.execute {
            try {
                val (code, body) = get("$base/v1/app/version", token)
                if (code == 404) { set(State.UP_TO_DATE, "No update has been published."); return@execute }
                if (code != 200) { set(State.ERROR, "The server answered $code."); return@execute }
                val v = JSONObject(body)
                latestCode = v.optLong("versionCode")
                latestName = v.optString("versionName")
                notes = v.optString("notes")
                history = (v.optJSONArray("history") ?: org.json.JSONArray()).toString()
                sha = v.optString("sha256")
                if (latestCode <= installedCode(app)) { set(State.UP_TO_DATE); return@execute }
                if (auto && metered(app)) { set(State.AVAILABLE, "Waiting for Wi-Fi. Tap Check for updates to download now."); return@execute }
                download(app, base, token, v.optLong("size"))
            } catch (e: Exception) {
                set(State.ERROR, "Could not reach the server.")
            }
        }
    }

    private fun download(app: Context, base: String, token: String, size: Long) {
        set(State.DOWNLOADING)
        val file = apkFile(app)
        val c = URL("$base/v1/app/apk").openConnection() as HttpURLConnection
        c.setRequestProperty("Authorization", "Bearer $token")
        c.connectTimeout = 8000
        c.readTimeout = 30000
        if (c.responseCode != 200) { set(State.ERROR, "The download failed (${c.responseCode})."); return }
        val md = MessageDigest.getInstance("SHA-256")
        c.inputStream.use { i -> file.outputStream().use { o ->
            val buf = ByteArray(64 * 1024)
            while (true) { val n = i.read(buf); if (n < 0) break; md.update(buf, 0, n); o.write(buf, 0, n) }
        } }
        c.disconnect()
        val got = md.digest().joinToString("") { "%02x".format(it) }
        if (got != sha || (size > 0 && file.length() != size)) { file.delete(); set(State.ERROR, "The download did not match its checksum."); return }
        // It must be this app, at the version the server said.
        val info = app.packageManager.getPackageArchiveInfo(file.path, 0)
        if (info == null || info.packageName != app.packageName || info.longVersionCode != latestCode) { file.delete(); set(State.ERROR, "The file is not this app."); return }
        set(State.READY)
    }

    /** Hand the downloaded file to the system installer. */
    fun install(ctx: Context) {
        if (state != State.READY) return
        val app = ctx.applicationContext
        if (!app.packageManager.canRequestPackageInstalls()) {
            set(State.READY, "Allow installing from this app, then tap Install again.")
            app.startActivity(Intent(Settings.ACTION_MANAGE_UNKNOWN_APP_SOURCES, Uri.parse("package:${app.packageName}")).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK))
            return
        }
        val file = apkFile(app)
        if (!file.exists()) { set(State.ERROR, "The downloaded file is gone. Check again."); return }
        set(State.INSTALLING)
        io.execute {
            try {
                val pi = app.packageManager.packageInstaller
                val params = PackageInstaller.SessionParams(PackageInstaller.SessionParams.MODE_FULL_INSTALL).apply {
                    setAppPackageName(app.packageName)
                    setRequireUserAction(PackageInstaller.SessionParams.USER_ACTION_NOT_REQUIRED) // honoured after the first confirmed update
                }
                val id = pi.createSession(params)
                pi.openSession(id).use { s ->
                    file.inputStream().use { i -> s.openWrite("assistant.apk", 0, file.length()).use { o -> i.copyTo(o); s.fsync(o) } }
                    val back = PendingIntent.getBroadcast(app, id, Intent(app, InstallResult::class.java).setPackage(app.packageName), PendingIntent.FLAG_MUTABLE)
                    s.commit(back.intentSender)
                }
            } catch (e: Exception) {
                set(State.ERROR, "The install did not start: ${e.message}")
            }
        }
    }

    fun failed(msg: String?) = set(State.ERROR, msg ?: "The install did not finish.")

    private fun metered(ctx: Context): Boolean {
        val cm = ctx.getSystemService(ConnectivityManager::class.java)
        val caps = cm.getNetworkCapabilities(cm.activeNetwork) ?: return false
        return !caps.hasCapability(NetworkCapabilities.NET_CAPABILITY_NOT_METERED)
    }

    private fun get(url: String, token: String): Pair<Int, String> {
        val c = URL(url).openConnection() as HttpURLConnection
        c.setRequestProperty("Authorization", "Bearer $token")
        c.connectTimeout = 8000
        c.readTimeout = 15000
        val code = c.responseCode
        val text = (if (code < 400) c.inputStream else c.errorStream)?.bufferedReader()?.use { it.readText() }.orEmpty()
        c.disconnect()
        return code to text
    }
}
