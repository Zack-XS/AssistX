package app.assistant

import android.app.Activity
import android.content.Intent
import android.graphics.Color
import android.graphics.Typeface
import android.graphics.drawable.GradientDrawable
import android.os.Bundle
import android.view.Gravity
import android.view.ViewGroup
import android.widget.Button
import android.widget.LinearLayout
import android.widget.TextView
import org.json.JSONObject
import java.time.LocalTime
import java.time.format.DateTimeFormatter

/** The full-screen card while an alarm rings: the time, the label, Snooze and Stop. Shows over the lock screen. */
class RingActivity : Activity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setShowWhenLocked(true)
        setTurnScreenOn(true)
        val now = RingService.current?.let { JSONObject(it) }
        if (now == null) { finish(); return }
        val dp = resources.displayMetrics.density
        fun pill(label: String, solid: Boolean, go: () -> Unit) = Button(this).apply {
            text = label
            textSize = 20f
            isAllCaps = false
            setTextColor(if (solid) Color.parseColor("#0A0A0A") else Color.parseColor("#EDEDED"))
            background = GradientDrawable().apply {
                cornerRadius = 40 * dp
                // Design tokens (dark): ink #EDEDED, raised #1A1A1A, line #262626.
                setColor(if (solid) Color.parseColor("#EDEDED") else Color.parseColor("#1A1A1A"))
                if (!solid) setStroke((1 * dp).toInt().coerceAtLeast(1), Color.parseColor("#262626"))
            }
            setPadding((24 * dp).toInt(), (18 * dp).toInt(), (24 * dp).toInt(), (18 * dp).toInt())
            contentDescription = label
            setOnClickListener { go() }
        }
        val box = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            gravity = Gravity.CENTER
            setBackgroundColor(Color.BLACK)
            setPadding((32 * dp).toInt(), 0, (32 * dp).toInt(), 0)
        }
        box.addView(TextView(this).apply {
            text = LocalTime.now().format(DateTimeFormatter.ofPattern("H:mm"))
            textSize = 84f
            setTextColor(Color.parseColor("#EDEDED"))
            typeface = Typeface.create("sans-serif-light", Typeface.NORMAL)
            gravity = Gravity.CENTER
        })
        box.addView(TextView(this).apply {
            text = now.optString("text", "Alarm")
            textSize = 24f
            setTextColor(Color.parseColor("#8F8F8F"))
            gravity = Gravity.CENTER
            setPadding(0, (8 * dp).toInt(), 0, (56 * dp).toInt())
        })
        val lp = { LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT).apply { topMargin = (12 * dp).toInt() } }
        box.addView(pill("Snooze ${RingService.SNOOZE_MIN} min", false) { send(RingService.ACTION_SNOOZE) }, lp())
        box.addView(pill("Stop", true) { send(RingService.ACTION_STOP) }, lp())
        setContentView(box)
        RingService.listener = { runOnUiThread { if (RingService.current == null) finish() } }
    }

    private fun send(action: String) {
        startService(Intent(this, RingService::class.java).setAction(action))
        finish()
    }

    override fun onDestroy() {
        RingService.listener = null
        super.onDestroy()
    }
}
