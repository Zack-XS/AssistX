package app.assistant

import android.Manifest
import android.app.Activity
import android.content.Intent
import android.content.pm.PackageManager
import android.graphics.Color
import android.graphics.drawable.GradientDrawable
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.speech.RecognitionListener
import android.speech.RecognizerIntent
import android.speech.SpeechRecognizer
import android.speech.tts.TextToSpeech
import android.view.Gravity
import android.view.ViewGroup
import android.view.WindowManager
import android.widget.Button
import android.widget.LinearLayout
import android.widget.ScrollView
import android.widget.TextView
import org.json.JSONObject
import java.util.Locale
import java.util.concurrent.Executors

/**
 * The voice card. It opens when you hold the home or power button (with AssistX as the phone's
 * assistant), when you say "AssistX" (WakeService), or from the app. It listens at once, sends
 * what you said to your server, shows the answer and reads it out. Tap the mic to talk again.
 */
class AssistActivity : Activity() {
    private val main = Handler(Looper.getMainLooper())
    private val io = Executors.newSingleThreadExecutor()
    /** Fetching a voice must never wait behind `io`: `io` can be busy waiting for the user to answer a question out loud. */
    private val net = Executors.newCachedThreadPool()
    private var rec: SpeechRecognizer? = null
    private var tts: TextToSpeech? = null
    private lateinit var heard: TextView
    private lateinit var answer: TextView
    private lateinit var mic: Button
    private lateinit var glow: VoiceGlow
    private lateinit var top: LinearLayout
    private lateinit var rootView: android.view.View
    private var pulse: android.animation.ObjectAnimator? = null
    private var micPulse: android.animation.ObjectAnimator? = null

    /** The button's words. While it says "Listening" the button breathes gently. */
    private var micLabel: String = ""
        set(v) {
            field = v
            mic.text = v
            micPulse?.cancel(); micPulse = null
            if (v.startsWith("Listening")) {
                micPulse = android.animation.ObjectAnimator.ofPropertyValuesHolder(mic,
                    android.animation.PropertyValuesHolder.ofFloat("scaleX", 1f, 1.04f), android.animation.PropertyValuesHolder.ofFloat("scaleY", 1f, 1.1f)).apply {
                    duration = 700; repeatMode = android.animation.ValueAnimator.REVERSE; repeatCount = android.animation.ValueAnimator.INFINITE
                    interpolator = android.view.animation.AccelerateDecelerateInterpolator(); start()
                }
            } else { mic.animate().scaleX(1f).scaleY(1f).setDuration(160).start() }
        }

    /** A soft press: the button shrinks a little under the finger and springs back. No movement across the screen. */
    private fun softPress(v: android.view.View) {
        v.setOnTouchListener { view, e ->
            when (e.actionMasked) {
                android.view.MotionEvent.ACTION_DOWN -> view.animate().scaleX(0.95f).scaleY(0.95f).setDuration(90).start()
                android.view.MotionEvent.ACTION_UP, android.view.MotionEvent.ACTION_CANCEL ->
                    view.animate().scaleX(1f).scaleY(1f).setDuration(260).setInterpolator(android.view.animation.OvershootInterpolator(2.2f)).start()
            }
            false
        }
    }
    /** Someone is waiting for the next thing the user says (a question the assistant asked). */
    private var awaiting: ((String?) -> Unit)? = null

    /** What you said, in the middle. A change fades in and the box grows or shrinks smoothly. */
    private var heardText: String = ""
        set(v) {
            if (v == field) return
            field = v
            settle()
            heard.text = v
            heard.alpha = 0.55f
            heard.animate().alpha(1f).setStartDelay(0).setDuration(220).start()
        }

    /** The answer. "Thinking" breathes; a real answer appears word by word. It never moves. */
    private var answerText: String = ""
        set(v) {
            if (v == field) return
            field = v
            pulse?.cancel(); pulse = null
            settle()
            answer.text = v
            if (v.startsWith("Thinking")) {
                Sfx.startLoop(this) // a soft loop while it works; it stops the moment words arrive
                glow.idle = 0.38f // a slow, brighter glow while it works
                pulse = android.animation.ObjectAnimator.ofFloat(answer, "alpha", 1f, 0.35f).apply {
                    duration = 800; repeatMode = android.animation.ValueAnimator.REVERSE; repeatCount = android.animation.ValueAnimator.INFINITE
                    interpolator = android.view.animation.AccelerateDecelerateInterpolator(); start()
                }
            } else {
                Sfx.stopLoop()
                answer.alpha = 1f
                answer.translationY = 0f
                glow.idle = 0.22f
                reveal(v)
            }
        }

    private var revealAnim: android.animation.ValueAnimator? = null

    /** A word whose colour (and so its fade) can change after the text is laid out. */
    private class WordFade(var color: Int) : android.text.style.CharacterStyle() {
        override fun updateDrawState(tp: android.text.TextPaint) { tp.color = color }
    }

    /** The words appear one after another, each fading in softly. The text is laid out at once, so nothing jumps. */
    private fun reveal(text: String) {
        revealAnim?.cancel()
        val words = Regex("\\S+").findAll(text).map { it.range }.toList()
        if (words.isEmpty()) return
        val full = answer.currentTextColor
        val rgb = full and 0x00FFFFFF
        val peak = android.graphics.Color.alpha(full)
        val sp = android.text.SpannableString(text)
        val fades = words.map { r -> WordFade(rgb).also { sp.setSpan(it, r.first, r.last + 1, android.text.Spannable.SPAN_EXCLUSIVE_EXCLUSIVE) } }
        answer.text = sp
        val n = fades.size
        revealAnim = android.animation.ValueAnimator.ofFloat(0f, n + 3f).apply {
            duration = (n * 90L).coerceIn(500L, 3200L)
            interpolator = android.view.animation.LinearInterpolator()
            addUpdateListener { va ->
                val pos = va.animatedValue as Float
                for (i in 0 until n) {
                    val k = ((pos - i) / 3f).coerceIn(0f, 1f) // each word fades over three words' time
                    fades[i].color = (((peak * (k * k * (3f - 2f * k))).toInt()) shl 24) or rgb
                }
                answer.invalidate()
            }
            start()
        }
    }

    /** Let the layout change (a longer line, an answer arriving) glide instead of jump. */
    private fun settle() {
        val tr = android.transition.TransitionSet().setOrdering(android.transition.TransitionSet.ORDERING_TOGETHER)
            .addTransition(android.transition.ChangeBounds()).addTransition(android.transition.Fade())
            .setDuration(260).setInterpolator(android.view.animation.DecelerateInterpolator(1.4f))
        android.transition.TransitionManager.beginDelayedTransition(top, tr)
    }
    private var busy = false
    private var retries = 0
    private var attempt = -1
    private var ready = false

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setShowWhenLocked(true)
        setTurnScreenOn(true)
        window.setLayout(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.MATCH_PARENT)
        WakeService.pauseFor(this, true) // the wake word listener lets go of the microphone
        Sfx.play(this, "live_start")
        val dp = resources.displayMetrics.density
        // See-through: the screen behind shows through a soft dark veil, so text is white on every phone.
        val fg = Color.WHITE
        val sheet = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            gravity = Gravity.CENTER_HORIZONTAL
            setPadding((44 * dp).toInt(), (80 * dp).toInt(), (44 * dp).toInt(), (40 * dp).toInt())
        }
        heard = TextView(this).apply { textSize = 28f; gravity = Gravity.CENTER; setTextColor(fg); setShadowLayer(8f, 0f, 2f, 0x99000000.toInt()); text = "Listening\u2026"; contentDescription = "What you said" }
        answer = TextView(this).apply { textSize = 18f; gravity = Gravity.CENTER; setTextColor(0xE6FFFFFF.toInt()); setShadowLayer(8f, 0f, 2f, 0x99000000.toInt()); setPadding(0, (18 * dp).toInt(), 0, 0) }
        val scroll = ScrollView(this).apply { addView(answer) }
        mic = Button(this).apply {
            text = "Talk"
            isAllCaps = false
            textSize = 18f
            setTextColor(Color.WHITE)
            background = GradientDrawable().apply { cornerRadius = 40 * dp; setColor(0x40FFFFFF); setStroke((1 * dp).toInt(), 0x80FFFFFF.toInt()) }
            contentDescription = "Talk again"
            setOnClickListener { listen() }
        }
        val open = Button(this).apply {
            text = "Open app"
            isAllCaps = false
            setBackgroundColor(Color.TRANSPARENT)
            setTextColor(fg)
            setOnClickListener { startActivity(Intent(this@AssistActivity, MainActivity::class.java).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)); finish() }
        }
        val row = LinearLayout(this).apply { orientation = LinearLayout.HORIZONTAL; gravity = Gravity.CENTER_VERTICAL; setPadding(0, (16 * dp).toInt(), 0, 0) }
        // No Talk button: hold the assistant button (or say the wake word) to talk again.
        row.gravity = Gravity.CENTER
        row.addView(open)
        // Top: what you said, in the middle. Then the answer. Buttons at the bottom.
        top = LinearLayout(this).apply { orientation = LinearLayout.VERTICAL; gravity = Gravity.CENTER_HORIZONTAL }
        top.addView(heard, LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT))
        top.addView(scroll, LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT))
        val middle = android.widget.FrameLayout(this)
        middle.addView(top, android.widget.FrameLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT, Gravity.CENTER))
        sheet.addView(middle, LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, 0, 1f))
        sheet.addView(row)
        glow = VoiceGlow(this)
        val root = android.widget.FrameLayout(this).apply { setBackgroundColor(0x99000000.toInt()) }
        root.addView(glow, android.widget.FrameLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.MATCH_PARENT))
        root.addView(sheet, android.widget.FrameLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.MATCH_PARENT))
        setContentView(root)
        // Open smoothly: the veil and the words fade in and settle; the glow fades in on its own.
        rootView = root
        softPress(open)
        // The pieces fade in one after another (they only fade; nothing slides).
        listOf<android.view.View>(heard, scroll, row).forEachIndexed { i, v ->
            v.alpha = 0f
            v.animate().alpha(1f).setStartDelay(160L + 120L * i).setDuration(420).setInterpolator(android.view.animation.DecelerateInterpolator()).start()
        }
        root.alpha = 0f
        sheet.scaleX = 1.03f; sheet.scaleY = 1.03f
        root.animate().alpha(1f).setDuration(320).start()
        sheet.animate().scaleX(1f).scaleY(1f).setDuration(420).setInterpolator(android.view.animation.DecelerateInterpolator(1.5f)).start()
        tts = TextToSpeech(this) {
            if (it == TextToSpeech.SUCCESS) {
                tts?.language = Locale.getDefault()
                // After a spoken question, the microphone opens by itself so the user can just answer.
                tts?.setOnUtteranceProgressListener(object : android.speech.tts.UtteranceProgressListener() {
                    override fun onStart(id: String?) {}
                    @Deprecated("Deprecated in Java") override fun onError(id: String?) { if (id == LISTEN_AFTER) main.post { awaiting?.let { awaiting = null; it(null) } } }
                    override fun onDone(id: String?) { if (id == LISTEN_AFTER) main.post { if (!isFinishing) listen() } }
                })
            }
        }
        if (Alarms.token(this).isEmpty()) { heardText = "Open the app once and connect it to your server first."; mic.isEnabled = false; return }
        main.postDelayed({ listen() }, if (WakeService.active) WakeService.HANDOVER_MS else 0)
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        if (!busy) listen() // pressed again or said the wake word again: listen again
    }

    private fun listen() {
        if (checkSelfPermission(Manifest.permission.RECORD_AUDIO) != PackageManager.PERMISSION_GRANTED) {
            requestPermissions(arrayOf(Manifest.permission.RECORD_AUDIO), 31)
            return
        }
        stopSpeaking()
        rec?.destroy()
        if (attempt < 0) attempt = Speech.good(this).coerceIn(0, Speech.count(this) - 1)
        ready = false
        val r = Speech.make(this, attempt)
        rec = r
        // Never answers: try the next speech service.
        main.postDelayed({ if (rec === r && !ready && attempt + 1 < Speech.count(this)) { attempt++; listen() } }, 2500)
        heardText = "Listening\u2026"
        micLabel = "Listening\u2026"
        glow.idle = 0.15f
        glow.listening = true
        r.setRecognitionListener(object : RecognitionListener {
            override fun onPartialResults(b: Bundle?) { text(b)?.let { heardText = it } }
            override fun onResults(b: Bundle?) {
                micLabel = "Talk"
                glow.listening = false
                retries = 0
                val t = text(b)
                val cb = awaiting
                if (t.isNullOrBlank()) { heardText = "I did not catch that. Tap Talk."; if (cb != null) { awaiting = null; cb(null) }; return }
                heardText = t
                Speech.remember(this@AssistActivity, attempt)
                if (cb != null) { awaiting = null; cb(t); return } // an answer to a question the assistant asked
                ask(t)
            }
            override fun onError(error: Int) {
                micLabel = "Talk"
                glow.listening = false
                // The on-device recognizer failed: try the phone's normal speech service once.
                if ((!ready || Speech.tryNext(error)) && attempt + 1 < Speech.count(this@AssistActivity)) { attempt++; main.postDelayed({ listen() }, 300); return }
                // The phone's recognizer was still letting go of the wake word: try once more.
                if ((error == SpeechRecognizer.ERROR_RECOGNIZER_BUSY || error == SpeechRecognizer.ERROR_CLIENT) && retries++ < 2) {
                    main.postDelayed({ listen() }, 600); return
                }
                heardText = when (error) {
                    SpeechRecognizer.ERROR_NO_MATCH, SpeechRecognizer.ERROR_SPEECH_TIMEOUT -> "I did not hear anything. Tap Talk."
                    else -> "I cannot listen: " + Speech.why(error) + "."
                }
                awaiting?.let { awaiting = null; it(null) }
            }
            override fun onReadyForSpeech(p: Bundle?) { ready = true; glow.burst() } // the mic is open: flare
            override fun onBeginningOfSpeech() {}
            override fun onRmsChanged(rmsdB: Float) { glow.feed(rmsdB) }
            override fun onBufferReceived(buffer: ByteArray?) {}
            override fun onEndOfSpeech() { micLabel = "Thinking\u2026"; glow.level = 0f; glow.listening = false }
            override fun onEvent(eventType: Int, params: Bundle?) {}
            private fun text(b: Bundle?) = b?.getStringArrayList(SpeechRecognizer.RESULTS_RECOGNITION)?.firstOrNull()?.trim()
        })
        r.startListening(Speech.intent(this, true))
    }

    @Deprecated("Deprecated in Java")
    override fun onRequestPermissionsResult(requestCode: Int, permissions: Array<out String>, grantResults: IntArray) {
        super.onRequestPermissionsResult(requestCode, permissions, grantResults)
        if (grantResults.firstOrNull() == PackageManager.PERMISSION_GRANTED) listen() else heardText = "The microphone is off for this app."
    }

    /** Send what was said to the "Voice" chat and wait for the answer. */
    private fun ask(text: String) {
        busy = true
        answerText = "Thinking\u2026"
        mic.isEnabled = false
        io.execute {
            val (reply, listenAfter) = try { send(text) } catch (e: Exception) { "Something went wrong: ${e.message}" to false }
            main.post {
                busy = false
                mic.isEnabled = true
                micLabel = "Talk"
                answerText = reply
                speak(reply, listenAfter)
            }
        }
    }

    private var player: android.media.MediaPlayer? = null
    private var visualizer: android.media.audiofx.Visualizer? = null
    /** Remember that the server has no voice, so the next answers do not wait for it. */
    private var serverVoice = true

    /**
     * Read something out with the Kokoro voice on your server, and light the glow with the voice as it
     * plays. If the server has no voice, the phone's own is used. With `listenAfter`, the microphone
     * opens when the words end (live mode).
     */
    private fun speak(text: String, listenAfter: Boolean) {
        stopSpeaking()
        val clean = text.replace(Regex("[*_#`>]"), "").take(1500)
        if (!serverVoice) { phoneVoice(clean, listenAfter); return }
        net.execute {
            val (code, wav) = Alarms.httpBytes(this, "POST", "/v1/speech", JSONObject().put("text", clean).toString())
            main.post {
                if (isFinishing || isDestroyed) return@post
                if (code == 200 && wav.size > 64) play(wav, listenAfter)
                else {
                    if (code == 503 || code == 404) serverVoice = false // no voice on this server
                    phoneVoice(clean, listenAfter)
                }
            }
        }
    }

    private fun phoneVoice(text: String, listenAfter: Boolean) {
        tts?.speak(text, TextToSpeech.QUEUE_FLUSH, null, if (listenAfter) LISTEN_AFTER else "answer")
    }

    private fun play(wav: ByteArray, listenAfter: Boolean) {
        try {
            val f = java.io.File(cacheDir, "say.wav")
            f.writeBytes(wav)
            val mp = android.media.MediaPlayer()
            mp.setAudioAttributes(android.media.AudioAttributes.Builder()
                .setUsage(android.media.AudioAttributes.USAGE_ASSISTANT).setContentType(android.media.AudioAttributes.CONTENT_TYPE_SPEECH).build())
            mp.setDataSource(f.path)
            mp.setOnCompletionListener {
                glow.level = 0f
                stopSpeaking()
                if (listenAfter && !isFinishing) listen()
            }
            mp.setOnErrorListener { _, _, _ -> stopSpeaking(); if (listenAfter && !isFinishing) listen(); true }
            mp.prepare()
            player = mp
            // The glow follows the assistant's voice while it talks.
            try {
                val v = android.media.audiofx.Visualizer(mp.audioSessionId)
                v.captureSize = android.media.audiofx.Visualizer.getCaptureSizeRange()[0].coerceAtLeast(256)
                v.setDataCaptureListener(object : android.media.audiofx.Visualizer.OnDataCaptureListener {
                    override fun onWaveFormDataCapture(vis: android.media.audiofx.Visualizer?, w: ByteArray?, rate: Int) {
                        if (w == null) return
                        var sum = 0.0
                        for (b in w) { val d = ((b.toInt() and 0xFF) - 128) / 128.0; sum += d * d }
                        glow.level = (Math.sqrt(sum / w.size) * 4.0).toFloat().coerceIn(0f, 1f)
                    }
                    override fun onFftDataCapture(vis: android.media.audiofx.Visualizer?, fft: ByteArray?, rate: Int) {}
                }, android.media.audiofx.Visualizer.getMaxCaptureRate() / 2, true, false)
                v.enabled = true
                visualizer = v
            } catch (e: Exception) { /* no visualizer on this phone: the glow just breathes */ }
            mp.start()
        } catch (e: Exception) {
            stopSpeaking()
            if (listenAfter && !isFinishing) listen()
        }
    }

    private fun stopSpeaking() {
        try { visualizer?.enabled = false; visualizer?.release() } catch (e: Exception) {}
        visualizer = null
        try { player?.release() } catch (e: Exception) {}
        player = null
        tts?.stop()
    }

    /**
     * Send what was said as live speech (the server tells the model it is being spoken to). Returns the
     * answer and whether the assistant asked for the microphone to open for a reply.
     */
    private fun send(text: String): Pair<String, Boolean> {
        val prefs = getSharedPreferences("app", MODE_PRIVATE)
        var thread = prefs.getString("voice_thread", null)
        val msgId = java.util.UUID.randomUUID().toString() // a repeat of the same message is the same turn on the server
        /** A call that did not get through (network, a busy server) is tried again, up to three times. */
        fun call(method: String, path: String, body: String? = null): Pair<Int, String> {
            var r = Alarms.http(this, method, path, body)
            var n = 0
            while ((r.first == -1 || r.first == 429 || r.first in 502..504) && n < 3) { Thread.sleep(if (n == 0) 1200L else 3000L); r = Alarms.http(this, method, path, body); n++ }
            return r
        }
        fun post(): Pair<Int, String> = call("POST", "/v1/threads/$thread/messages", JSONObject().put("text", text).put("live", true).put("interrupt", true).put("client_msg_id", msgId).toString())
        var r = if (thread != null) post() else 404 to ""
        // First time, the chat was deleted (404), or it was archived from the chat list (409): start a new one.
        if (r.first == 404 || r.first == 409) {
            val (c, b) = call("POST", "/v1/threads", JSONObject().put("title", "Voice").toString())
            if (c != 201) return (if (c == -1) "I cannot reach your server. Check that you are on the same network." else "The server answered $c.") to false
            thread = JSONObject(b).getString("id")
            prefs.edit().putString("voice_thread", thread).apply()
            r = post()
        }
        if (r.first == -1) return "I cannot reach your server. Check that you are on the same network." to false
        if (r.first != 202) return "The server answered ${r.first}." to false
        val turn = JSONObject(r.second).optString("turn_id")
        val until = System.currentTimeMillis() + 300_000 // a cold local model can take a while to load
        var listen = false
        var tries = 0
        var lost = 0
        var finished = false
        while (System.currentTimeMillis() < until) {
            Thread.sleep(800)
            val (c, b) = Alarms.http(this, "GET", "/v1/turns/$turn")
            if (c != 200) { if (++lost > 25) return "I lost the connection to your server. Try again." to false; continue }
            lost = 0
            val t = JSONObject(b)
            when (t.optString("status")) {
                "done", "error", "cancelled" -> {
                    listen = t.optBoolean("listen"); finished = true
                    // The model could not be reached: say it plainly (the server has the reason).
                    if (t.optString("status") == "error") { val m = t.optJSONObject("error")?.optString("message").orEmpty(); if (m.isNotBlank() && m != "null") return m to false }
                    break
                }
                "waiting" -> {
                    // Waiting for a yes or an answer: ask out loud and listen, unless it must be a tap.
                    val a = pendingFor(turn) ?: continue
                    if (!a.optBoolean("voice_ok")) return "This one needs your tap on the screen. Open the app to answer." to false
                    if (++tries > 4) return "I did not get an answer. It is still waiting in the app." to false
                    if (!askAloud(a)) return "I did not hear an answer. It is still waiting in the app." to false
                }
            }
        }
        if (!finished) return "The model is taking too long. It may still be loading. Try again in a moment." to false
        // The newest page holds the newest messages: follow the cursor to the end.
        var cursor = ""
        var answer = ""
        while (true) {
            val (c, b) = Alarms.http(this, "GET", "/v1/threads/$thread/messages?limit=200" + (if (cursor.isEmpty()) "" else "&cursor=$cursor"))
            if (c != 200) return "The server answered $c." to false
            val page = JSONObject(b)
            val items = page.optJSONArray("items") ?: break
            for (i in 0 until items.length()) {
                val m = items.getJSONObject(i)
                if (m.optString("role") == "assistant" && m.optString("turn_id") == turn) {
                    val t = m.optJSONObject("body")?.optString("content").orEmpty()
                    if (t.isNotBlank() && t != "null") answer = t
                }
            }
            cursor = page.optString("next_cursor", "")
            if (cursor.isEmpty() || cursor == "null") break
        }
        return (if (answer.isNotEmpty()) answer else "No answer yet. Open the app to see it when it comes.") to (listen && answer.isNotEmpty())
    }

    /** The approval this turn is waiting on, if any. */
    private fun pendingFor(turn: String): JSONObject? {
        val (c, b) = Alarms.http(this, "GET", "/v1/approvals?status=pending&limit=50")
        if (c != 200) return null
        val items = JSONObject(b).optJSONArray("items") ?: return null
        for (i in 0 until items.length()) { val a = items.getJSONObject(i); if (a.optString("turn_id") == turn) return a }
        return null
    }

    /** Read the question out, open the microphone, and send the spoken answer. False if nothing was heard. */
    private fun askAloud(a: JSONObject): Boolean {
        val isQuestion = a.optString("kind") == "question"
        val q = a.optString("question")
        for (round in 0 until 2) {
            val got = java.util.concurrent.SynchronousQueue<String>()
            val none = "\u0000"
            main.post {
                answerText = q
                heardText = if (isQuestion) "Listening\u2026" else "Say yes or no\u2026"
                awaiting = { t -> Thread { try { got.offer(t ?: none, 5, java.util.concurrent.TimeUnit.SECONDS) } catch (e: Exception) {} }.start() }
                speak(if (round == 0) q else "Please say yes or no.", true)
            }
            val said = got.poll(90, java.util.concurrent.TimeUnit.SECONDS)
            if (said == null || said == none) { main.post { awaiting = null }; return false }
            val body = JSONObject().put("via", "voice")
            if (isQuestion) body.put("answer", "reply").put("text", said)
            else when (yesNo(said)) {
                true -> body.put("answer", "yes")
                false -> body.put("answer", "no")
                null -> continue // not a clear answer: ask once more
            }
            val (c, _) = Alarms.http(this, "POST", "/v1/approvals/" + a.optString("id"), body.toString())
            return c == 200 || c == 409 // 409: it was answered on the screen meanwhile
        }
        return false
    }

    /** True for a clear yes, false for a clear no, null for anything else. A no always wins. */
    private fun yesNo(said: String): Boolean? {
        val w = said.lowercase(Locale.ROOT).replace(Regex("[^a-z' ]"), " ").trim().split(Regex("\\s+")).toSet()
        val no = setOf("no", "nope", "don't", "dont", "cancel", "stop", "deny", "never", "negative")
        val yes = setOf("yes", "yeah", "yep", "yup", "sure", "ok", "okay", "approve", "confirm", "go", "do", "correct")
        if (w.any { it in no } || said.lowercase(Locale.ROOT).contains("do not")) return false
        if (w.any { it in yes }) return true
        return null
    }

    override fun onDestroy() {
        Sfx.stopLoop()
        Sfx.play(this, "live_end")
        pulse?.cancel()
        micPulse?.cancel()
        revealAnim?.cancel()
        stopSpeaking()
        rec?.destroy()
        tts?.shutdown()
        io.shutdownNow()
        net.shutdownNow()
        WakeService.pauseFor(this, false)
        super.onDestroy()
    }

    companion object {
        /** Utterance id: when these words end, open the microphone. */
        private const val LISTEN_AFTER = "listen_after"
    }
}
