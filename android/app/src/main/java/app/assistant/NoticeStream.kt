package app.assistant

import android.content.Context
import java.net.HttpURLConnection
import java.net.URL
import kotlin.concurrent.thread

/**
 * One held-open connection to the server (`GET /v1/events`, server-sent events), so notices and
 * place changes arrive at once instead of waiting for the 15 minute check. It lives and dies with
 * the location service (a foreground service, so Android lets it keep running). Idle, it costs a
 * few small keep-alive lines a minute. If it drops, it reconnects with a growing wait; the old
 * 15 minute check stays as the backup, and the cursor keeps anything from showing twice.
 *
 * `assistant.notice`  -> ask for new notices now (Notices.poll)
 * `fences.changed`    -> a reminder waits on a new place: watch it now (Fences.sync)
 * `app.updated`       -> a new app version was published: check now, so the update banner shows at once
 * `phone.action`      -> the assistant asked this phone to do something: fetch it now (PhoneActions)
 */
object NoticeStream {
    @Volatile private var running = false
    @Volatile private var conn: HttpURLConnection? = null
    private var worker: Thread? = null

    fun start(ctx: Context) {
        if (running) return
        running = true
        val app = ctx.applicationContext
        worker = thread(name = "notice-stream", isDaemon = true) {
            var wait = 2_000L
            while (running) {
                try {
                    if (Alarms.token(app).isEmpty()) { Thread.sleep(30_000); continue }
                    val c = URL(Alarms.base(app) + "/v1/events").openConnection() as HttpURLConnection
                    conn = c
                    c.connectTimeout = 10_000
                    c.readTimeout = 120_000 // the server sends a keep-alive far more often than this
                    c.setRequestProperty("Authorization", "Bearer " + Alarms.token(app))
                    c.setRequestProperty("Accept", "text/event-stream")
                    if (c.responseCode == 423) Unlock.ensure(app) // the server restarted: unlock, then the next try works
                    if (c.responseCode != 200) throw RuntimeException("status ${c.responseCode}")
                    wait = 2_000L
                    // Anything that came while we were away.
                    try { Notices.poll(app); Fences.sync(app) } catch (e: Exception) { }
                    var kind = ""
                    c.inputStream.bufferedReader().use { r ->
                        while (running) {
                            val line = r.readLine() ?: break
                            when {
                                line.startsWith("event:") -> kind = line.substring(6).trim()
                                line.isEmpty() -> {
                                    when (kind) {
                                        "assistant.notice" -> try { Notices.poll(app) } catch (e: Exception) { }
                                        "app.updated" -> try { Updater.check(app, true) } catch (e: Exception) { }
                                        "fences.changed" -> try { Fences.sync(app) } catch (e: Exception) { }
                                        "phone.action" -> PhoneActions.pollAsync(app)
                                    }
                                    kind = ""
                                }
                            }
                        }
                    }
                } catch (e: Exception) {
                    // offline, server restarting, or stopped: wait, then try again
                } finally {
                    try { conn?.disconnect() } catch (e: Exception) { }
                    conn = null
                }
                if (running) try { Thread.sleep(wait) } catch (e: InterruptedException) { }
                wait = minOf(wait * 2, 5 * 60_000L)
            }
        }
    }

    fun stop() {
        running = false
        try { conn?.disconnect() } catch (e: Exception) { }
        worker?.interrupt()
        worker = null
    }
}
