package app.assistant

import android.content.Context
import android.graphics.Canvas
import android.graphics.Color
import android.graphics.Matrix
import android.graphics.Paint
import android.graphics.RectF
import android.graphics.SweepGradient
import android.view.View
import kotlin.math.max
import kotlin.math.sin

/**
 * The see-through edge glow of the voice screen, in one colour (white). A soft light runs round the
 * whole rim of the screen like a slow ring; it breathes when idle and swells and speeds up with
 * your voice, so you can see you are being heard. It fades in when the screen opens.
 * Feed it the recognizer's volume with `feed`.
 */
class VoiceGlow(ctx: Context) : View(ctx) {
    private val dp = resources.displayMetrics.density
    private val paint = Paint(Paint.ANTI_ALIAS_FLAG).apply { style = Paint.Style.STROKE }
    private val rect = RectF()
    private val matrix = Matrix()
    private var sweep: SweepGradient? = null
    private var smooth = 0f
    private var last = 0L
    private var t = 0f
    private var angle = 0f
    private var appear = 0f

    /** Raw loudness, 0 (quiet) to 1 (loud). Smoothed when drawn. */
    @Volatile var level = 0f
    /** The glow kept when nothing is heard (thinking, answering). */
    var idle = 0.15f

    /** True while the microphone is open: a soft ripple runs in from the edge every couple of seconds. */
    @Volatile var listening = false
    private var burst = 0f
    private var nextRipple = 0f
    private val ripples = ArrayList<Float>()

    /**
     * The moment you press to talk: the glow flares, spins up and sends two ripples in from the edge,
     * then settles over about a second and a half.
     */
    fun burst() {
        post {
            burst = 1f
            ripples.clear()
            ripples.add(0f); ripples.add(-0.18f); ripples.add(-0.36f)
            nextRipple = t + 2.6f
        }
    }

    /** The recognizer reports about -2..10 dB; map it to 0..1. */
    fun feed(rmsDb: Float) { level = ((rmsDb + 2f) / 12f).coerceIn(0f, 1f) }

    override fun onSizeChanged(w: Int, h: Int, ow: Int, oh: Int) {
        // Bright arcs and dim gaps, all white: the arcs are what seems to run round the edge.
        val dim = Color.argb(70, 255, 255, 255)
        val mid = Color.argb(150, 255, 255, 255)
        val hot = Color.argb(255, 255, 255, 255)
        sweep = SweepGradient(w / 2f, h / 2f, intArrayOf(dim, hot, mid, dim, mid, hot, dim, dim), floatArrayOf(0f, 0.12f, 0.25f, 0.45f, 0.6f, 0.78f, 0.9f, 1f))
    }

    override fun onDraw(c: Canvas) {
        val now = System.nanoTime()
        val dt = if (last == 0L) 0.016f else ((now - last) / 1e9f).coerceAtMost(0.05f)
        last = now
        t += dt
        appear = (appear + dt / 0.6f).coerceAtMost(1f)
        val ease = appear * appear * (3f - 2f * appear) // smooth fade-in
        burst = (burst - dt / 1.5f).coerceAtLeast(0f)
        val eb = burst * burst
        // While the microphone is open, a gentle ripple every few seconds keeps it alive.
        if (listening && t >= nextRipple) { ripples.add(0f); nextRipple = t + 2.6f }
        val target = max(max(level, idle + 0.05f * sin(t * 1.6f)), 0.85f * eb) // idle breathing, or the flare
        smooth += (target - smooth) * (1f - Math.pow(if (target > smooth) 0.0004 else 0.03, dt.toDouble()).toFloat()) // frame-rate independent, quick up, slow down
        val w = width.toFloat()
        val h = height.toFloat()
        val sw = sweep ?: return
        angle = (angle + dt * (22f + 160f * smooth + 520f * eb)) % 360f
        matrix.setRotate(angle, w / 2f, h / 2f)
        sw.setLocalMatrix(matrix)
        paint.shader = sw
        // Layers from the very edge inwards, each fainter: this makes the soft fall-off.
        val layers = 14
        val reach = (22f + 70f * smooth + 46f * eb) * dp
        val step = reach / layers
        val corner = 44f * dp
        paint.strokeWidth = step * 1.3f
        for (i in 0 until layers) {
            val f = 1f - i / layers.toFloat()
            paint.alpha = (255 * ease * f * f * (0.3f + 0.7f * smooth.coerceIn(0f, 1f))).toInt().coerceIn(0, 255)
            val inset = step * (i + 0.5f)
            rect.set(inset, inset, w - inset, h - inset)
            val r = max(4f * dp, corner - inset)
            c.drawRoundRect(rect, r, r, paint)
        }
        // Ripples: thin bright rings that run in from the edge and fade.
        val maxInset = 0.2f * minOf(w, h)
        val it = ripples.listIterator()
        while (it.hasNext()) {
            val p = it.next() + dt / 1.4f
            if (p >= 1f) { it.remove(); continue }
            it.set(p)
            if (p <= 0f) continue
            val e = 1f - (1f - p) * (1f - p) // fast at first, then slows
            val inset = 4f * dp + e * maxInset
            paint.strokeWidth = (2f + 5f * (1f - p)) * dp
            paint.alpha = (200f * ease * (1f - p) * (1f - p) * (0.45f + 0.55f * max(eb, 0.4f))).toInt().coerceIn(0, 255)
            rect.set(inset, inset, w - inset, h - inset)
            val r = corner + inset * 0.7f
            c.drawRoundRect(rect, r, r, paint)
        }
        postInvalidateOnAnimation()
    }
}
