package app.assistant

import android.content.Context
import android.media.AudioAttributes
import android.media.AudioManager
import android.media.MediaPlayer
import android.media.SoundPool

/**
 * Short sound effects (the files in the repo's "SOUND EFFECTS" folder, copied to res/raw): message sent, error,
 * approval asked, the voice screen opening and closing, and a soft loop while the voice screen waits for the answer.
 * They follow the phone's silent and vibrate modes, and one switch in Settings (General, Sound effects).
 */
object Sfx {
    private var pool: SoundPool? = null
    private val ids = HashMap<String, Int>()
    private var loop: MediaPlayer? = null
    private val NAMES = mapOf("send" to "sfx_send", "error" to "sfx_error", "approval" to "sfx_approval", "live_start" to "live_start", "live_end" to "live_end")

    private val attrs = AudioAttributes.Builder().setUsage(AudioAttributes.USAGE_ASSISTANCE_SONIFICATION).setContentType(AudioAttributes.CONTENT_TYPE_SONIFICATION).build()

    fun enabled(ctx: Context) = ctx.getSharedPreferences("app", Context.MODE_PRIVATE).getBoolean("sfx", true)
    fun setEnabled(ctx: Context, on: Boolean) { ctx.getSharedPreferences("app", Context.MODE_PRIVATE).edit().putBoolean("sfx", on).apply(); if (!on) stopLoop() }

    /** Loud enough to hear only when the phone is in normal mode (not silent, not vibrate). */
    private fun allowed(ctx: Context) = enabled(ctx) && ctx.getSystemService(AudioManager::class.java)?.ringerMode == AudioManager.RINGER_MODE_NORMAL

    @Synchronized private fun pool(ctx: Context): SoundPool {
        pool?.let { return it }
        val p = SoundPool.Builder().setMaxStreams(4).setAudioAttributes(attrs).build()
        for ((name, res) in NAMES) {
            val id = ctx.resources.getIdentifier(res, "raw", ctx.packageName)
            if (id != 0) ids[name] = p.load(ctx, id, 1)
        }
        pool = p
        return p
    }

    /** Play one effect by name: send, error, approval, live_start, live_end. */
    fun play(ctx: Context, name: String, volume: Float = 0.85f) {
        if (!allowed(ctx)) return
        try {
            val p = pool(ctx.applicationContext)
            ids[name]?.let { p.play(it, volume, volume, 1, 0, 1f) }
        } catch (e: Exception) { /* no sound is better than a crash */ }
    }

    /** A soft loop, for the voice screen while it works on the answer. */
    @Synchronized fun startLoop(ctx: Context, volume: Float = 0.35f) {
        if (!allowed(ctx) || loop != null) return
        try {
            val res = ctx.resources.getIdentifier("live_loop", "raw", ctx.packageName)
            if (res == 0) return
            loop = MediaPlayer.create(ctx.applicationContext, res, attrs, (ctx.getSystemService(AudioManager::class.java)).generateAudioSessionId())?.apply {
                isLooping = true
                setVolume(volume, volume)
                start()
            }
        } catch (e: Exception) { loop = null }
    }

    @Synchronized fun stopLoop() {
        try { loop?.stop() } catch (e: Exception) { }
        try { loop?.release() } catch (e: Exception) { }
        loop = null
    }
}
