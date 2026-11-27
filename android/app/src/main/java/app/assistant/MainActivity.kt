package app.assistant

import android.Manifest
import android.annotation.SuppressLint
import android.app.Activity
import android.app.AlertDialog
import android.content.Intent
import android.content.SharedPreferences
import android.content.pm.ApplicationInfo
import android.content.pm.PackageManager
import android.content.res.Configuration
import android.graphics.Color
import android.os.Build
import android.os.Bundle
import android.os.VibrationEffect
import android.os.VibratorManager
import android.speech.RecognitionListener
import android.speech.RecognizerIntent
import android.speech.SpeechRecognizer
import android.view.ViewGroup
import android.view.WindowInsets
import android.view.WindowInsetsController
import android.view.WindowManager
import android.webkit.JavascriptInterface
import android.webkit.JsPromptResult
import android.webkit.JsResult
import android.webkit.WebChromeClient
import android.webkit.WebResourceRequest
import android.webkit.WebResourceResponse
import android.webkit.WebSettings
import android.util.Base64
import android.webkit.WebView
import android.webkit.WebViewClient
import android.widget.EditText
import android.widget.FrameLayout
import org.json.JSONArray
import org.json.JSONObject

/**
 * Hosts the app's screens (the web page in assets/index.html) in a full-screen WebView and gives
 * it native things: the server address and token kept in the Keystore, microphone permission,
 * dictation, always-on listening (CaptureService), vibration, dialogs, and the Back button.
 */
class MainActivity : Activity() {
    private lateinit var web: WebView
    private lateinit var prefs: SharedPreferences
    private lateinit var root: FrameLayout
    private var dictation: SpeechRecognizer? = null
    private var wantDictation = false
    private var wantCapture = false
    private var tried = 0L
    private var resumed = false
    // The page says it started (NativeApp.pageReady). If it does not, a bad update broke it: the app
    // offers its own way out (update, reload) that does not need the page.
    @Volatile private var pageOk = false
    /** A tapped notification asked for this chat; opened once the page is ready. */
    private var pendingThread: String? = null

    private fun openPendingThread() {
        val t = pendingThread ?: return
        pendingThread = null
        // Only a plain chat id reaches the page.
        if (!Regex("^[A-Za-z0-9_:.-]{1,80}$").matches(t)) return
        // The page may still be connecting: try for a few seconds.
        js("(function(t){var n=0;(function go(){if(window.openThread&&typeof cfg!=='undefined'&&cfg.tok){openThread(t)}else if(n++<20){setTimeout(go,300)}})()})(${JSONObject.quote(t)})")
    }

    /** A home-screen shortcut asked for a screen (calendar, alarms, new-event, new-alarm); opened once the page is connected. */
    private var pendingOpen: String? = null

    private fun openPendingView() {
        val v = pendingOpen ?: return
        pendingOpen = null
        if (v !in OPEN_VIEWS) return
        js("(function(v){var n=0;(function go(){if(window.__open&&window.__ready){__open(v)}else if(n++<30){setTimeout(go,300)}})()})(${JSONObject.quote(v)})")
    }
    private var lastJsError = ""
    private var rescueShown = false
    private var forceInstall = false
    @Volatile private var oauthLink = ""
    private var barPx = intArrayOf(0, 0, 0, 0)

    /** Tell the page how big the bars are (top, right, bottom, left) in CSS pixels. */
    private fun pushInsets() {
        val d = resources.displayMetrics.density
        val (t, r, b, l) = barPx.map { it / d }
        web.evaluateJavascript(
            "(function(s){s.setProperty('--sat','${t}px');s.setProperty('--sar','${r}px');s.setProperty('--sab','${b}px');s.setProperty('--sal','${l}px')})(document.documentElement.style)", null)
    }

    @SuppressLint("SetJavaScriptEnabled")
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        prefs = getSharedPreferences("app", MODE_PRIVATE)
        takeDebugExtras(intent)
        takeLink(intent)
        takeShare(intent)
        PhoneActions.askContacts = { runOnUiThread { if (resumed) requestPermissions(arrayOf(Manifest.permission.READ_CONTACTS), REQ_SETUP) } }
        pendingThread = intent?.getStringExtra(EXTRA_THREAD)
        pendingOpen = intent?.getStringExtra(EXTRA_OPEN)

        // Full screen: draw under the status bar, the navigation bar and the camera cutout.
        window.setDecorFitsSystemWindows(false)
        window.attributes = window.attributes.also { it.layoutInDisplayCutoutMode = WindowManager.LayoutParams.LAYOUT_IN_DISPLAY_CUTOUT_MODE_ALWAYS }

        root = FrameLayout(this)
        web = WebView(this)
        root.addView(web, FrameLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.MATCH_PARENT))
        setContentView(root)
        // Keep the page clear of the camera cutout, the keyboard, and the bars when they are showing.
        root.setOnApplyWindowInsetsListener { v, insets ->
            // Keep room for the bars and the camera cutout even while they are hidden, so the page
            // never touches the top or bottom edge. The keyboard pushes the page up.
            val bars = insets.getInsetsIgnoringVisibility(WindowInsets.Type.systemBars() or WindowInsets.Type.displayCutout())
            val kb = insets.getInsets(WindowInsets.Type.ime()).bottom
            // The page draws under the bars (so a theme change covers them too) and gets their size
            // as CSS values. Only the keyboard is still handled here, by padding.
            val kbUp = kb > bars.bottom
            barPx = intArrayOf(bars.top, bars.right, if (kbUp) 0 else bars.bottom, bars.left)
            v.setPadding(0, 0, 0, if (kbUp) kb else 0)
            pushInsets()
            WindowInsets.CONSUMED
        }
        setBars(isNight())

        web.settings.apply {
            javaScriptEnabled = true
            domStorageEnabled = true
            // The page is served from https://appassets.androidplatform.net and talks to your own
            // server over plain http, so mixed content has to be allowed.
            mixedContentMode = WebSettings.MIXED_CONTENT_ALWAYS_ALLOW
            allowFileAccess = false
            allowContentAccess = false
            setSupportZoom(false)
            setSupportMultipleWindows(false)
        }
        web.overScrollMode = android.view.View.OVER_SCROLL_NEVER
        web.addJavascriptInterface(Bridge(), "NativeApp")
        web.webViewClient = object : WebViewClient() {
            override fun shouldInterceptRequest(view: WebView, request: WebResourceRequest): WebResourceResponse? {
                val u = request.url
                if (u.scheme != "https" || u.host != HOST) return null
                val path = u.path.orEmpty()
                if (path != "/" && path != "/index.html") {
                    return WebResourceResponse("text/plain", "utf-8", 404, "Not found", emptyMap(), "".byteInputStream())
                }
                return WebResourceResponse("text/html", "utf-8", 200, "OK", mapOf("Cache-Control" to "no-store", "Content-Security-Policy" to pageCsp), page.byteInputStream())
            }

            override fun onPageFinished(view: WebView, url: String) {
                pushInsets()
                web.postDelayed({ if (!pageOk) showRescue() }, 8000)
            }

            // The page never leaves its own address; links go to the phone's browser.
            override fun shouldOverrideUrlLoading(view: WebView, request: WebResourceRequest): Boolean {
                if (request.url.host == HOST) return false
                startActivity(Intent(Intent.ACTION_VIEW, request.url))
                return true
            }
        }
        web.webChromeClient = object : WebChromeClient() {
            override fun onShowFileChooser(view: WebView, callback: android.webkit.ValueCallback<Array<android.net.Uri>>, params: FileChooserParams): Boolean {
                pick?.onReceiveValue(null)
                pick = callback
                return try { startActivityForResult(params.createIntent(), REQ_FILE); true } catch (e: Exception) { pick = null; false }
            }

            override fun onConsoleMessage(m: android.webkit.ConsoleMessage): Boolean {
                if (m.messageLevel() == android.webkit.ConsoleMessage.MessageLevel.ERROR) lastJsError = "${m.message()} (line ${m.lineNumber()})"
                return false
            }

            override fun onJsAlert(view: WebView, url: String, message: String, result: JsResult): Boolean {
                AlertDialog.Builder(this@MainActivity).setMessage(message)
                    .setPositiveButton("OK") { _, _ -> result.confirm() }.setOnCancelListener { result.cancel() }.show()
                return true
            }

            override fun onJsConfirm(view: WebView, url: String, message: String, result: JsResult): Boolean {
                AlertDialog.Builder(this@MainActivity).setMessage(message)
                    .setPositiveButton("OK") { _, _ -> result.confirm() }
                    .setNegativeButton("Cancel") { _, _ -> result.cancel() }.setOnCancelListener { result.cancel() }.show()
                return true
            }

            override fun onJsPrompt(view: WebView, url: String, message: String, defaultValue: String?, result: JsPromptResult): Boolean {
                val input = EditText(this@MainActivity).apply { setText(defaultValue.orEmpty()) }
                AlertDialog.Builder(this@MainActivity).setMessage(message).setView(input)
                    .setPositiveButton("OK") { _, _ -> result.confirm(input.text.toString()) }
                    .setNegativeButton("Cancel") { _, _ -> result.cancel() }.setOnCancelListener { result.cancel() }.show()
                return true
            }
        }
        CaptureService.listener = { runOnUiThread { js("window.__captureChanged&&__captureChanged()") } }
        Updater.listener = {
            runOnUiThread {
                js("window.__updateChanged&&__updateChanged()")
                // Ready and switched on: install now, once per version, while the app is open.
                if (Updater.state == Updater.State.READY && (forceInstall || prefs.getBoolean("auto_update", true)) && tried != Updater.latestCode && resumed) {
                    tried = Updater.latestCode
                    Updater.install(this)
                }
            }
        }
        web.loadUrl("https://$HOST/index.html")
        askForMicrophoneOnce()
        web.postDelayed({ Updater.check(this, true) }, 4000)
        Alarms.ensurePeriodic(this)
        Alarms.syncAsync(this)
        LocationService.listener = { runOnUiThread { js("window.__locationChanged&&__locationChanged()") } }
        WakeService.listener = { runOnUiThread { js("window.__voiceChanged&&__voiceChanged()") } }
        // The wake word comes back when the app opens (Android does not let a microphone service start from the background).
        if (prefs.getBoolean(WakeService.KEY_ON, false) && hasMic() && !WakeService.active) try { startForegroundService(Intent(this, WakeService::class.java)) } catch (e: Exception) { }
        HealthSync.listener = { runOnUiThread { js("window.__healthChanged&&__healthChanged()") } }
        // The app is on screen, so it may start even without "Allow all the time".
        if (prefs.getBoolean(LocationService.KEY_ON, false) && LocationService.allowed(this) && !LocationService.active) startLocation()
    }

    // ---------- file picker (for uploading an alarm sound) ----------

    private var pick: android.webkit.ValueCallback<Array<android.net.Uri>>? = null

    @Deprecated("Deprecated in Java")
    override fun onActivityResult(requestCode: Int, resultCode: Int, data: Intent?) {
        super.onActivityResult(requestCode, resultCode, data)
        if (requestCode == REQ_CAMERA) { val u = shotUri; if (resultCode == RESULT_OK && u != null) readPhotoUri(u, false) { java.io.File(cacheDir, "photos/shot.jpg").delete() }; return }
        if (requestCode == REQ_PHOTO) { data?.data?.takeIf { resultCode == RESULT_OK }?.let { readPhotoUri(it, false) {} }; return }
        if (requestCode == REQ_HEALTH) { finishHealth(); return }
        if (requestCode == REQ_SPEECH) {
            // The phone's own speech window (last resort). Send what it heard to the page.
            val t = data?.getStringArrayListExtra(RecognizerIntent.EXTRA_RESULTS)?.firstOrNull()?.takeIf { it.isNotBlank() }
            if (resultCode == RESULT_OK && t != null) js("window.__dictation&&__dictation(${JSONObject.quote(t)},false)")
            return
        }
        if (requestCode != REQ_FILE) return
        pick?.onReceiveValue(WebChromeClient.FileChooserParams.parseResult(resultCode, data))
        pick = null
    }

    /** What the alarm page needs to know: can alarms be exact, light up the screen, and show at all. */
    private fun alarmStatus(): String {
        val am = getSystemService(android.app.AlarmManager::class.java)
        val nm = getSystemService(android.app.NotificationManager::class.java)
        val full = if (Build.VERSION.SDK_INT >= 34) nm.canUseFullScreenIntent() else true
        return JSONObject().put("exact", am.canScheduleExactAlarms()).put("fullscreen", full).put("notifications", nm.areNotificationsEnabled()).toString()
    }

    private fun openAlarmSettings(kind: String) {
        val uri = android.net.Uri.parse("package:$packageName")
        val i = when (kind) {
            "exact" -> Intent(android.provider.Settings.ACTION_REQUEST_SCHEDULE_EXACT_ALARM, uri)
            "fullscreen" -> if (Build.VERSION.SDK_INT >= 34) Intent(android.provider.Settings.ACTION_MANAGE_APP_USE_FULL_SCREEN_INTENT, uri) else null
            "notifications" -> if (Build.VERSION.SDK_INT >= 33 && checkSelfPermission(Manifest.permission.POST_NOTIFICATIONS) != PackageManager.PERMISSION_GRANTED) {
                requestPermissions(arrayOf(Manifest.permission.POST_NOTIFICATIONS), REQ_MIC); null
            } else Intent(android.provider.Settings.ACTION_APP_NOTIFICATION_SETTINGS).putExtra(android.provider.Settings.EXTRA_APP_PACKAGE, packageName)
            else -> null
        } ?: return
        try { startActivity(i) } catch (e: Exception) { startActivity(Intent(android.provider.Settings.ACTION_APPLICATION_DETAILS_SETTINGS, uri)) }
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        intent.getStringExtra(EXTRA_THREAD)?.let { pendingThread = it; if (pageOk) openPendingThread() }
        intent.getStringExtra(EXTRA_OPEN)?.let { pendingOpen = it; if (pageOk) openPendingView() }
        if (intent.action == PlaceWatch.ACTION_START) setCapture(true)
        if (takeDebugExtras(intent)) web.loadUrl("https://$HOST/index.html")
        if (takeLink(intent)) js("window.__oauthCheck&&__oauthCheck()")
        takeShare(intent)
    }

    /** The sign-in page sends the browser back to app.assistant://...; keep what it sent for the page to pick up. */
    private fun takeLink(i: Intent?): Boolean {
        val d = i?.data ?: return false
        if (d.scheme != "app.assistant") return false
        oauthLink = JSONObject().put("code", d.getQueryParameter("code") ?: "").put("state", d.getQueryParameter("state") ?: "").put("error", d.getQueryParameter("error") ?: "").toString()
        return true
    }

    // ---------- shared from other apps, and photos read on the phone ----------

    /** Something shared to the app (the share sheet): (kind, text), handed to the page once it is ready. */
    private var pendingShare: Pair<String, String>? = null
    private var shotUri: android.net.Uri? = null

    private fun takeShare(i: Intent?) {
        if (i?.action != Intent.ACTION_SEND) return
        val type = i.type ?: return
        if (type.startsWith("image/")) {
            @Suppress("DEPRECATION")
            val uri = (if (Build.VERSION.SDK_INT >= 33) i.getParcelableExtra(Intent.EXTRA_STREAM, android.net.Uri::class.java) else i.getParcelableExtra(Intent.EXTRA_STREAM) as? android.net.Uri) ?: return
            readPhotoUri(uri, true) {}
        } else {
            val t = listOfNotNull(i.getStringExtra(Intent.EXTRA_SUBJECT), i.getCharSequenceExtra(Intent.EXTRA_TEXT)?.toString()).joinToString("\n").trim()
            if (t.isEmpty()) return
            pendingShare = "text" to t.take(20000)
            if (pageOk) deliverShare()
        }
    }

    private fun deliverShare() {
        val s = pendingShare ?: return
        pendingShare = null
        js("(function(k,t){var n=0;(function go(){if(window.__shared&&typeof cfg!=='undefined'&&cfg.tok){__shared(k,t)}else if(n++<30){setTimeout(go,300)}})()})(${JSONObject.quote(s.first)},${JSONObject.quote(s.second)})")
    }

    /** Read the text in a photo here on the phone; only the text goes to the page. */
    private fun readPhotoUri(uri: android.net.Uri, shared: Boolean, after: () -> Unit) {
        android.widget.Toast.makeText(this, "Reading the photo...", android.widget.Toast.LENGTH_SHORT).show()
        PhotoText.read(this, uri) { t ->
            after()
            when {
                t == null -> js("alert('Could not read that photo.')")
                shared -> { pendingShare = "photo" to t.take(20000); if (pageOk) deliverShare() }
                else -> js("window.__photoText&&__photoText(${JSONObject.quote(t.take(20000))})")
            }
        }
    }

    /** The camera button: take a photo or pick one. */
    private fun choosePhoto() {
        AlertDialog.Builder(this).setTitle("Read text from a photo")
            .setItems(arrayOf("Take a photo", "Choose a photo")) { _, which ->
                try {
                    if (which == 0) {
                        val f = java.io.File(java.io.File(cacheDir, "photos").apply { mkdirs() }, "shot.jpg")
                        val u = androidx.core.content.FileProvider.getUriForFile(this, "$packageName.photos", f)
                        shotUri = u
                        startActivityForResult(Intent(android.provider.MediaStore.ACTION_IMAGE_CAPTURE).putExtra(android.provider.MediaStore.EXTRA_OUTPUT, u)
                            .addFlags(Intent.FLAG_GRANT_WRITE_URI_PERMISSION or Intent.FLAG_GRANT_READ_URI_PERMISSION), REQ_CAMERA)
                    } else {
                        val i = if (Build.VERSION.SDK_INT >= 33) Intent(android.provider.MediaStore.ACTION_PICK_IMAGES)
                            else Intent(Intent.ACTION_GET_CONTENT).setType("image/*").addCategory(Intent.CATEGORY_OPENABLE)
                        startActivityForResult(i, REQ_PHOTO)
                    }
                } catch (e: Exception) { js("alert('No camera or photo app found.')") }
            }.show()
    }

    override fun onResume() {
        super.onResume()
        resumed = true
        Notices.pageOnScreen = true
        js("window.__setupChanged&&__setupChanged()") // back from a system screen: refresh the checklist
        if (System.currentTimeMillis() - Updater.lastCheck > 30 * 60 * 1000) Updater.check(this, true)
        Alarms.syncAsync(this)
        HealthSync.syncSoon(this)
        PhoneActions.pollAsync(this)
        Thread { NotifyListener.refresh(this) }.start()
    }

    override fun onPause() {
        resumed = false
        Notices.pageOnScreen = false
        super.onPause()
    }

    override fun onWindowFocusChanged(hasFocus: Boolean) {
        super.onWindowFocusChanged(hasFocus)
        if (hasFocus) hideBars()
    }

    override fun onDestroy() {
        CaptureService.listener = null
        PhoneActions.askContacts = null
        Updater.listener = null
        dictation?.destroy()
        web.destroy()
        super.onDestroy()
    }

    // ---------- look: full screen ----------

    /** Hide the status and navigation bars. A swipe from the edge shows them for a moment. */
    private fun hideBars() {
        window.insetsController?.apply {
            hide(WindowInsets.Type.systemBars())
            systemBarsBehavior = WindowInsetsController.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE
        }
    }

    private fun isNight() = resources.configuration.uiMode and Configuration.UI_MODE_NIGHT_MASK == Configuration.UI_MODE_NIGHT_YES

    /** Window colour behind the page and dark or light system bar icons (for when the bars show). */
    private fun setBars(dark: Boolean) {
        window.decorView.setBackgroundColor(if (dark) Color.BLACK else Color.parseColor("#FAFAFA"))
        val mask = WindowInsetsController.APPEARANCE_LIGHT_STATUS_BARS or WindowInsetsController.APPEARANCE_LIGHT_NAVIGATION_BARS
        window.insetsController?.setSystemBarsAppearance(if (dark) 0 else mask, mask)
    }

    // ---------- back ----------

    @Deprecated("Back is handled by the page: close a sub page, then go to Home, then leave.")
    override fun onBackPressed() {
        web.evaluateJavascript(
            "(function(){try{if(!document.getElementById('subpage').hidden){closeSubSoft();return 'sub'}if(view!=='home'){show('home');return 'tab'}}catch(e){}return 'exit'})()"
        ) { r -> if (r == null || r.contains("exit")) finish() }
    }

    private fun js(code: String) = web.evaluateJavascript(code, null)

    /** The page did not start. Offer an update (the fix is usually a newer version) or a reload. */
    private fun showRescue() {
        if (rescueShown || isFinishing) return
        rescueShown = true
        val why = if (lastJsError.isEmpty()) "" else "\n\nError: $lastJsError"
        AlertDialog.Builder(this)
            .setTitle("The app did not start")
            .setMessage("Something in this version is broken. Getting the newest version usually fixes it.$why")
            .setPositiveButton("Update now") { _, _ ->
                forceInstall = true; tried = 0L; rescueShown = false
                android.widget.Toast.makeText(this, "Looking for an update\u2026", android.widget.Toast.LENGTH_SHORT).show()
                if (Updater.state == Updater.State.READY) Updater.install(this) else Updater.check(this, false)
                web.postDelayed({ if (!pageOk && Updater.state != Updater.State.DOWNLOADING && Updater.state != Updater.State.READY && Updater.state != Updater.State.INSTALLING) showRescue() }, 15000)
            }
            .setNeutralButton("Reload") { _, _ -> rescueShown = false; lastJsError = ""; web.reload() }
            .setNegativeButton("Close", null)
            .show()
    }

    // ---------- microphone ----------

    private fun hasMic() = checkSelfPermission(Manifest.permission.RECORD_AUDIO) == PackageManager.PERMISSION_GRANTED

    /** First launch: say why, then ask. The system asks again later if it is ever needed and was refused. */
    private fun askForMicrophoneOnce() {
        if (prefs.getBoolean("asked_mic", false)) return
        prefs.edit().putBoolean("asked_mic", true).apply()
        if (hasMic()) return
        AlertDialog.Builder(this)
            .setTitle("Microphone")
            .setMessage("The assistant can listen to you, to type what you say and, if you turn it on, to note what matters from your day. Speech is turned into text on this phone; the server gets text only. You choose when it listens.")
            .setPositiveButton("Continue") { _, _ ->
                val wanted = if (Build.VERSION.SDK_INT >= 33) arrayOf(Manifest.permission.RECORD_AUDIO, Manifest.permission.POST_NOTIFICATIONS) else arrayOf(Manifest.permission.RECORD_AUDIO)
                requestPermissions(wanted, REQ_MIC)
            }
            .setNegativeButton("Not now", null)
            .show()
    }

    @Deprecated("Deprecated in Java")
    override fun onRequestPermissionsResult(requestCode: Int, permissions: Array<out String>, grantResults: IntArray) {
        super.onRequestPermissionsResult(requestCode, permissions, grantResults)
        if (requestCode == REQ_LOC) {
            if (LocationService.allowed(this)) {
                startLocation()
                // "Allow all the time" lets it come back by itself after the phone restarts.
                if (!LocationService.background(this)) {
                    AlertDialog.Builder(this)
                        .setTitle("Keep it on after a restart")
                        .setMessage("Location works now. To start again by itself after the phone restarts, choose \"Allow all the time\" on the next screen.")
                        .setPositiveButton("Continue") { _, _ -> requestPermissions(arrayOf(Manifest.permission.ACCESS_BACKGROUND_LOCATION), REQ_BG) }
                        .setNegativeButton("Not now", null)
                        .show()
                }
            } else {
                js("alert('Location is off for this app. Allow it in the phone settings to use this.')")
            }
            js("window.__locationChanged&&__locationChanged()")
            return
        }
        if (requestCode == REQ_SETUP) { js("window.__setupChanged&&__setupChanged()"); return }
        if (requestCode == REQ_HEALTH) { finishHealth(); return }
        if (requestCode == REQ_BG) { js("window.__locationChanged&&__locationChanged()"); return }
        if (requestCode != REQ_MIC) return
        val granted = hasMic()
        if (granted && wantCapture) startCapture()
        if (!granted && (wantDictation || wantCapture)) js("alert('The microphone is off. Allow it for this app in the phone settings to use this.')")
        wantDictation = false
        wantCapture = false
        js("window.__captureChanged&&__captureChanged()")
    }

    // ---------- dictation ----------

    private val main = android.os.Handler(android.os.Looper.getMainLooper())
    private var held = false
    private var heard = ""

    /** Finger down on the mic: listen until the finger lifts, even through pauses in speech. */
    private fun holdStart() {
        if (dictation != null) return
        if (CaptureService.active) { js("alert('Turn off listening on this phone first.')"); return }
        if (!hasMic()) { wantDictation = true; requestPermissions(arrayOf(Manifest.permission.RECORD_AUDIO), REQ_MIC); return }
        held = true
        heard = ""
        if (WakeService.active) {
            // Let the wake word hand over the recognizer first, or this one fails as busy.
            WakeService.pauseFor(this, true)
            main.postDelayed({ if (held && dictation == null) startDictation() }, WakeService.HANDOVER_MS)
        } else {
            startDictation()
        }
    }

    /** Finger up: stop listening and keep what was heard. */
    private fun holdEnd() {
        held = false
        val r = dictation ?: return
        try { r.stopListening() } catch (e: Exception) { stopDictation(); return }
        // If the recognizer never answers, close it anyway.
        main.postDelayed({ if (!held && dictation === r) stopDictation() }, 2500)
    }

    private var dictAttempt = -1
    private var dictErrors = 0
    private var dictReady = false

    private fun listenOnce(r: SpeechRecognizer) {
        dictReady = false
        r.startListening(Speech.intent(this, true))
        // A recognizer that never answers (a broken voice-input setting): move to the next one.
        main.postDelayed({ if (held && dictation === r && !dictReady) nextRecognizer(r, Speech.NO_START) }, 4500)
    }

    /** This recognizer failed: try the next way the phone can do speech, or say why. */
    private fun nextRecognizer(r: SpeechRecognizer, error: Int) {
        if (dictation !== r) return
        r.destroy()
        dictation = null
        if (dictAttempt + 1 < Speech.count(this) && held) {
            dictAttempt++
            main.postDelayed({ if (held && dictation == null) startDictation() }, 300)
        } else {
            dictAttempt = -1
            stopDictation()
            // Last resort: the phone's own speech window. It works where the quiet recognizers refuse to start.
            val started = error != SpeechRecognizer.ERROR_INSUFFICIENT_PERMISSIONS && try {
                startActivityForResult(Speech.windowIntent(this), REQ_SPEECH); true
            } catch (e: Exception) { false }
            if (!started) android.widget.Toast.makeText(this, "Cannot turn speech into text: " + Speech.why(error) + " (code $error).", android.widget.Toast.LENGTH_LONG).show()
        }
    }

    private fun startDictation() {
        if (dictAttempt < 0) dictAttempt = Speech.good(this).coerceIn(0, Speech.count(this) - 1)
        val r = Speech.make(this, dictAttempt)
        dictation = r
        r.setRecognitionListener(object : RecognitionListener {
            override fun onPartialResults(b: android.os.Bundle?) = say(b)
            override fun onResults(b: android.os.Bundle?) {
                dictErrors = 0
                val t = text(b)
                if (t != null) { heard = (heard + " " + t).trim(); send(heard); Speech.remember(this@MainActivity, dictAttempt) }
                if (held && dictation === r) restart(r) else stopDictation()
            }
            override fun onError(error: Int) {
                if (dictation !== r) return
                if (!dictReady || Speech.tryNext(error)) { main.post { nextRecognizer(r, error) }; return }
                if (error == SpeechRecognizer.ERROR_INSUFFICIENT_PERMISSIONS || ++dictErrors > 30) {
                    android.widget.Toast.makeText(this@MainActivity, "Cannot listen: " + Speech.why(error) + ".", android.widget.Toast.LENGTH_LONG).show()
                    stopDictation(); return
                }
                if (held) restart(r) else stopDictation()
            }
            override fun onReadyForSpeech(params: android.os.Bundle?) { dictReady = true }
            override fun onBeginningOfSpeech() { dictReady = true }
            override fun onRmsChanged(rmsdB: Float) {}
            override fun onBufferReceived(buffer: ByteArray?) {}
            override fun onEndOfSpeech() {}
            override fun onEvent(eventType: Int, params: android.os.Bundle?) {}
            private fun text(b: android.os.Bundle?) = b?.getStringArrayList(SpeechRecognizer.RESULTS_RECOGNITION)?.firstOrNull()?.takeIf { it.isNotBlank() }
            private fun say(b: android.os.Bundle?) { val t = text(b) ?: return; send((heard + " " + t).trim()) }
            private fun send(t: String) = js("window.__dictation&&__dictation(${JSONObject.quote(t)},false)")
            private fun restart(r: SpeechRecognizer) = main.postDelayed({ if (held && dictation === r) try { listenOnce(r) } catch (e: Exception) { nextRecognizer(r, Speech.NO_START) } }, 150)
        })
        try { listenOnce(r) } catch (e: Exception) { nextRecognizer(r, Speech.NO_START); return }
        js("window.__dictationState&&__dictationState(true)")
    }

    private fun stopDictation() {
        held = false
        dictAttempt = -1
        dictation?.destroy()
        dictation = null
        WakeService.pauseFor(this, false)
        js("window.__dictationState&&__dictationState(false)")
    }

    // ---------- listening all day ----------

    /** Turning it on needs a yes to a plain warning first, once, and the microphone permission. */
    private fun setCapture(on: Boolean) {
        if (!on) {
            startService(Intent(this, CaptureService::class.java).setAction(CaptureService.ACTION_STOP))
            return
        }
        if (!prefs.getBoolean("capture_consent", false)) {
            AlertDialog.Builder(this)
                .setTitle("Listen on this phone")
                .setMessage("While this is on, the phone listens, turns speech into text, and sends the text to your server, which notes what matters and asks you before anything is kept.\n\nThe laws about recording people differ from place to place, and many need everyone's consent. Turn it on only where you are allowed to. A notification shows the whole time, and you can stop it any moment.")
                .setPositiveButton("I understand, turn on") { _, _ -> prefs.edit().putBoolean("capture_consent", true).apply(); setCapture(true) }
                .setNegativeButton("Cancel") { _, _ -> js("window.__captureChanged&&__captureChanged()") }
                .setOnCancelListener { js("window.__captureChanged&&__captureChanged()") }
                .show()
            return
        }
        if (!hasMic()) {
            wantCapture = true
            val wanted = if (Build.VERSION.SDK_INT >= 33) arrayOf(Manifest.permission.RECORD_AUDIO, Manifest.permission.POST_NOTIFICATIONS) else arrayOf(Manifest.permission.RECORD_AUDIO)
            requestPermissions(wanted, REQ_MIC)
            return
        }
        startCapture()
    }

    private fun startCapture() {
        stopDictation()
        startForegroundService(Intent(this, CaptureService::class.java))
    }

    // ---------- location ----------

    /** Turning it on needs a yes to a plain note first, once, and the location permission. */
    private fun setLocation(on: Boolean) {
        if (!on) {
            startService(Intent(this, LocationService::class.java).setAction(LocationService.ACTION_STOP))
            return
        }
        if (!prefs.getBoolean("location_consent", false)) {
            AlertDialog.Builder(this)
                .setTitle("Remember where you go")
                .setMessage("While this is on, the phone notes where it is about every 5 minutes and sends it to your own server. The assistant learns the places you stay at or visit often, and you can name them.\n\nIt uses Wi-Fi and cell towers more than GPS, and sends in batches, to be easy on the battery. A small notification shows while it is on. Turn it off any time.")
                .setPositiveButton("Turn on") { _, _ -> prefs.edit().putBoolean("location_consent", true).apply(); setLocation(true) }
                .setNegativeButton("Cancel") { _, _ -> js("window.__locationChanged&&__locationChanged()") }
                .setOnCancelListener { js("window.__locationChanged&&__locationChanged()") }
                .show()
            return
        }
        if (!LocationService.allowed(this)) {
            requestPermissions(arrayOf(Manifest.permission.ACCESS_FINE_LOCATION, Manifest.permission.ACCESS_COARSE_LOCATION), REQ_LOC)
            return
        }
        startLocation()
    }

    private fun setupFix(id: String) {
        val pkg = android.net.Uri.parse("package:$packageName")
        fun open(i: Intent) { try { startActivity(i) } catch (e: Exception) { startActivity(Intent(android.provider.Settings.ACTION_APPLICATION_DETAILS_SETTINGS, pkg)) } }
        when (id) {
            "notifications" -> requestPermissions(arrayOf(Manifest.permission.POST_NOTIFICATIONS), REQ_SETUP)
            "location" -> requestPermissions(arrayOf(Manifest.permission.ACCESS_FINE_LOCATION, Manifest.permission.ACCESS_COARSE_LOCATION), REQ_SETUP)
            "location_always" -> if (!LocationService.allowed(this)) requestPermissions(arrayOf(Manifest.permission.ACCESS_FINE_LOCATION, Manifest.permission.ACCESS_COARSE_LOCATION), REQ_SETUP)
                else requestPermissions(arrayOf(Manifest.permission.ACCESS_BACKGROUND_LOCATION), REQ_BG)
            "tracking" -> setLocation(true)
            "battery" -> open(Intent(android.provider.Settings.ACTION_REQUEST_IGNORE_BATTERY_OPTIMIZATIONS, pkg))
            "alarms" -> open(Intent(android.provider.Settings.ACTION_REQUEST_SCHEDULE_EXACT_ALARM, pkg))
            "mic" -> requestPermissions(arrayOf(Manifest.permission.RECORD_AUDIO), REQ_SETUP)
            "contacts" -> requestPermissions(arrayOf(Manifest.permission.READ_CONTACTS), REQ_SETUP)
            "installs" -> open(Intent(android.provider.Settings.ACTION_MANAGE_UNKNOWN_APP_SOURCES, pkg))
            "listener" -> open(Intent(android.provider.Settings.ACTION_NOTIFICATION_LISTENER_SETTINGS))
        }
    }

    private fun startLocation() {
        prefs.edit().putBoolean(LocationService.KEY_ON, true).apply()
        startForegroundService(Intent(this, LocationService::class.java))
    }

    // ---------- wake word ----------

    private fun setWake(on: Boolean) {
        if (!on) {
            startService(Intent(this, WakeService::class.java).setAction(WakeService.ACTION_STOP))
            return
        }
        if (!prefs.getBoolean("wake_consent", false)) {
            AlertDialog.Builder(this)
                .setTitle("Listen for \u201cAssistX\u201d")
                .setMessage("While this is on, the phone keeps its speech recognizer running and only checks for the word \u201cAssistX\u201d. What it hears is turned into text on the phone and thrown away; nothing is kept or sent until you say the word.\n\nThis uses more battery than the rest of the app. A notification shows while it is on.")
                .setPositiveButton("Turn on") { _, _ -> prefs.edit().putBoolean("wake_consent", true).apply(); setWake(true) }
                .setNegativeButton("Cancel") { _, _ -> js("window.__voiceChanged&&__voiceChanged()") }
                .show()
            return
        }
        if (!hasMic()) { requestPermissions(arrayOf(Manifest.permission.RECORD_AUDIO), REQ_MIC); return }
        if (CaptureService.active) { js("alert('Turn off listening on this phone first. It already uses the microphone.')"); return }
        prefs.edit().putBoolean(WakeService.KEY_ON, true).apply()
        startForegroundService(Intent(this, WakeService::class.java))
    }

    // ---------- health ----------

    /** Turning it on needs a plain note first, once, then Health Connect's own permission screen. */
    private fun setHealth(on: Boolean) {
        if (!on) { HealthSync.off(this); return }
        if (!HealthSync.available(this)) {
            js("alert('Health Connect is not available on this phone. Install or update it from the Play Store.')")
            return
        }
        if (!prefs.getBoolean("health_consent", false)) {
            AlertDialog.Builder(this)
                .setTitle("Share your body numbers")
                .setMessage("The app reads your steps, sleep, heart rate, weight and workouts from Health Connect, adds them up per day, and sends the daily totals to your own server. Raw readings stay on the phone. It sends when you open the app. You choose what to allow on the next screen, and you can turn this off any time.")
                .setPositiveButton("Continue") { _, _ -> prefs.edit().putBoolean("health_consent", true).apply(); setHealth(true) }
                .setNegativeButton("Cancel") { _, _ -> js("window.__healthChanged&&__healthChanged()") }
                .setOnCancelListener { js("window.__healthChanged&&__healthChanged()") }
                .show()
            return
        }
        Thread {
            val missing = HealthSync.missing(this).toTypedArray()
            runOnUiThread {
                when {
                    missing.isEmpty() -> finishHealth()
                    Build.VERSION.SDK_INT >= 34 -> requestPermissions(missing, REQ_HEALTH)
                    else -> try {
                        // Android 12 and 13: the Health Connect app asks.
                        val i = Intent("androidx.health.ACTION_REQUEST_PERMISSIONS")
                            .putExtra("androidx.health.connect.client.EXTRA_REQUEST_PERMISSIONS", missing)
                            .setPackage("com.google.android.apps.healthdata")
                        @Suppress("DEPRECATION") startActivityForResult(i, REQ_HEALTH)
                    } catch (e: Exception) { js("alert('Could not open Health Connect.')") }
                }
            }
        }.start()
    }

    /** After the permission screen: if anything was allowed, switch on and send now. */
    private fun finishHealth() {
        Thread {
            val granted = HealthSync.permissions.size - HealthSync.missing(this).size
            runOnUiThread {
                if (granted > 0) {
                    prefs.edit().putBoolean(HealthSync.KEY_ON, true).apply()
                    HealthSync.syncSoon(this, true)
                } else {
                    js("alert('Nothing was allowed. You can allow it in Health Connect, under App permissions.')")
                }
                js("window.__healthChanged&&__healthChanged()")
            }
        }.start()
    }

    // ---------- vibration ----------

    /**
     * One small feel for each kind of moment. On phones with a good motor these are crisp "primitives" (short,
     * tuned taps); on others the same moment falls back to a plain predefined effect.
     * tick: any tap · select: a tab or a choice · toggle: a switch · click: a main button · heavy: long press
     * success: done · warn: needs a look · error: it failed.
     */
    private fun haptic(kind: String) {
        val v = getSystemService(VibratorManager::class.java)?.defaultVibrator ?: return
        if (!v.hasVibrator()) return
        fun comp(vararg parts: Triple<Int, Float, Int>): VibrationEffect? {
            if (!v.areAllPrimitivesSupported(*parts.map { it.first }.toIntArray())) return null
            val c = VibrationEffect.startComposition()
            for ((prim, scale, delay) in parts) c.addPrimitive(prim, scale, delay)
            return c.compose()
        }
        val C = VibrationEffect.Composition.PRIMITIVE_CLICK
        val T = VibrationEffect.Composition.PRIMITIVE_TICK
        val L = VibrationEffect.Composition.PRIMITIVE_LOW_TICK
        val fx = when (kind) {
            "select" -> comp(Triple(L, 0.9f, 0)) ?: VibrationEffect.createPredefined(VibrationEffect.EFFECT_TICK)
            "toggle" -> comp(Triple(C, 0.6f, 0)) ?: VibrationEffect.createPredefined(VibrationEffect.EFFECT_CLICK)
            "click" -> comp(Triple(C, 0.8f, 0)) ?: VibrationEffect.createPredefined(VibrationEffect.EFFECT_CLICK)
            "heavy" -> comp(Triple(C, 1f, 0), Triple(T, 0.5f, 40)) ?: VibrationEffect.createPredefined(VibrationEffect.EFFECT_HEAVY_CLICK)
            "success" -> comp(Triple(T, 0.6f, 0), Triple(C, 0.9f, 70)) ?: VibrationEffect.createWaveform(longArrayOf(0, 14, 70, 14), -1)
            "warn" -> comp(Triple(C, 0.7f, 0), Triple(C, 0.7f, 110)) ?: VibrationEffect.createWaveform(longArrayOf(0, 20, 110, 20), -1)
            "error" -> comp(Triple(C, 1f, 0), Triple(C, 0.8f, 80), Triple(C, 0.6f, 80)) ?: VibrationEffect.createWaveform(longArrayOf(0, 30, 60, 30, 60, 30), -1)
            else -> comp(Triple(T, 0.45f, 0)) ?: VibrationEffect.createPredefined(VibrationEffect.EFFECT_TICK)
        }
        v.vibrate(fx)
    }

    // ---------- debug ----------

    /** Debug builds only: `adb shell am start ... --es server URL --es token TOKEN` skips typing on the phone. */
    private fun takeDebugExtras(i: Intent?): Boolean {
        if (applicationInfo.flags and ApplicationInfo.FLAG_DEBUGGABLE == 0 || i == null) return false
        val e = prefs.edit()
        var any = false
        i.getStringExtra("server")?.let { e.putString("base", it); any = true }
        i.getStringExtra("token")?.let { e.putString("tok", Vault.seal(it)); any = true }
        e.apply()
        return any
    }

    /** The page, read once. */
    private val page: String by lazy { assets.open("index.html").bufferedReader().use { it.readText() } }

    /**
     * Only the page's own script may run, named by its SHA-256 (security review #23). The page can read the token
     * through [Bridge.getConfig], so a slipped-in script must never run. Everything else (the server's address,
     * pictures, sounds) stays open, since the server address is the user's own choice.
     */
    private val pageCsp: String by lazy {
        val script = page.substringAfter("<script>", "").substringBefore("</script>")
        val hash = Base64.encodeToString(java.security.MessageDigest.getInstance("SHA-256").digest(script.toByteArray()), Base64.NO_WRAP)
        "default-src * data: blob:; script-src 'sha256-$hash'; style-src * 'unsafe-inline'; object-src 'none'; base-uri 'none'; form-action 'none'"
    }

    /** What the page may ask the app to do. Only the page from HOST can reach it (see shouldOverrideUrlLoading). */
    inner class Bridge {
        /** The page calls this once it has started, so the app knows this version works. */
        @JavascriptInterface
        fun pageReady() { pageOk = true; runOnUiThread { openPendingThread(); openPendingView(); deliverShare() } }

        @JavascriptInterface
        fun getConfig(): String = JSONObject()
            .put("base", prefs.getString("base", null) ?: BuildConfig.DEFAULT_SERVER)
            .put("tok", Vault.open(prefs.getString("tok", null)))
            .toString()

        @JavascriptInterface
        fun saveConfig(base: String, tok: String) {
            prefs.edit().putString("base", base).putString("tok", Vault.seal(tok)).apply()
        }

        /** A short sound effect: send, error, approval. Off when the phone is silent or Sound effects is off. */
        @JavascriptInterface
        fun sfx(name: String) {
            Sfx.play(this@MainActivity, name)
        }

        @JavascriptInterface
        fun sfxEnabled(): Boolean = Sfx.enabled(this@MainActivity)

        @JavascriptInterface
        fun setSfx(on: Boolean) {
            Sfx.setEnabled(this@MainActivity, on)
        }

        /** The device key from sign-in ("remember this phone"), kept sealed. */
        @JavascriptInterface
        fun saveDevice(json: String) {
            Unlock.save(this@MainActivity, json)
        }

        /** Unlock this account on the server with the remembered key (after a server restart). Waits for the answer. */
        @JavascriptInterface
        fun unlockNow(): Boolean = Unlock.ensure(this@MainActivity)

        /** Sign out for good on this phone: the server forgets the remembered key too. */
        @JavascriptInterface
        fun forgetDevice() {
            Unlock.forget(this@MainActivity)
        }

        @JavascriptInterface
        fun clearConfig() {
            prefs.edit().remove("tok").apply()
        }

        @JavascriptInterface
        fun setTheme(dark: Boolean) {
            runOnUiThread { setBars(dark) }
        }

        @JavascriptInterface
        fun haptic(kind: String) {
            runOnUiThread { this@MainActivity.haptic(kind) }
        }

        @JavascriptInterface
        fun postNotice(json: String) {
            Notices.postFromPage(this@MainActivity, json)
        }

        @JavascriptInterface
        fun hasDictation(): Boolean = Speech.available(this@MainActivity)

        @JavascriptInterface
        fun holdStart() {
            runOnUiThread { this@MainActivity.holdStart() }
        }

        @JavascriptInterface
        fun holdEnd() {
            runOnUiThread { this@MainActivity.holdEnd() }
        }

        @JavascriptInterface
        fun takeOauth(): String {
            val t = oauthLink
            oauthLink = ""
            return t
        }

        /** Open a sign-in page in the phone's browser. https only. */
        @JavascriptInterface
        fun openUrl(url: String) {
            if (url.startsWith("https://")) runOnUiThread { startActivity(Intent(Intent.ACTION_VIEW, android.net.Uri.parse(url))) }
        }

        /** Look for AssistX servers on the local network. Each one found calls `__lanFound(json)`; the end calls `__lanDone()`. */
        @JavascriptInterface
        fun scanLan() {
            LanScan.start(this@MainActivity,
                { s -> val q = JSONObject.quote(s.toString()); runOnUiThread { js("window.__lanFound&&__lanFound($q)") } },
                { runOnUiThread { js("window.__lanDone&&__lanDone()") } })
        }

        /** Share plain text (a list) through Android's share sheet. Text only, capped. */
        @JavascriptInterface
        fun shareText(title: String, text: String) {
            val send = Intent(Intent.ACTION_SEND).setType("text/plain")
                .putExtra(Intent.EXTRA_SUBJECT, title.take(120))
                .putExtra(Intent.EXTRA_TEXT, text.take(20000))
            runOnUiThread { startActivity(Intent.createChooser(send, title.take(120).ifBlank { null })) }
        }

        @JavascriptInterface
        fun updateInfo(): String = Updater.json(this@MainActivity, prefs.getBoolean("auto_update", true))

        @JavascriptInterface
        fun checkUpdate() {
            runOnUiThread { Updater.check(this@MainActivity, false) }
        }

        @JavascriptInterface
        fun installUpdate() {
            runOnUiThread { Updater.install(this@MainActivity) }
        }

        @JavascriptInterface
        fun setAutoUpdate(on: Boolean) {
            prefs.edit().putBoolean("auto_update", on).apply()
        }

        @JavascriptInterface
        fun captureState(): String = if (CaptureService.active) "on" else "off"

        @JavascriptInterface
        fun setCapture(on: Boolean) {
            runOnUiThread { this@MainActivity.setCapture(on) }
        }

        /** Copy the server's alarms and reminders into the phone's alarm clock now. */
        @JavascriptInterface
        fun syncAlarms() {
            Alarms.syncAsync(this@MainActivity)
        }

        @JavascriptInterface
        fun alarmStatus(): String = this@MainActivity.alarmStatus()

        @JavascriptInterface
        fun locationState(): String = JSONObject()
            .put("on", LocationService.active).put("allowed", LocationService.allowed(this@MainActivity))
            .put("background", LocationService.background(this@MainActivity)).put("last", LocationService.last ?: JSONObject.NULL).toString()

        /** What the first-run checklist needs to know: which permissions and settings are in place. */
        @JavascriptInterface
        fun setupState(): String {
            val a = this@MainActivity
            val rm = getSystemService(android.app.role.RoleManager::class.java)
            return JSONObject()
                .put("notifications", checkSelfPermission(Manifest.permission.POST_NOTIFICATIONS) == PackageManager.PERMISSION_GRANTED)
                .put("location", LocationService.allowed(a))
                .put("location_always", LocationService.background(a))
                .put("tracking", LocationService.active)
                .put("battery", getSystemService(android.os.PowerManager::class.java).isIgnoringBatteryOptimizations(packageName))
                .put("alarms", getSystemService(android.app.AlarmManager::class.java).canScheduleExactAlarms())
                .put("mic", hasMic())
                .put("installs", packageManager.canRequestPackageInstalls())
                .put("listener", NotifyListener.allowed(a))
                .put("contacts", checkSelfPermission(Manifest.permission.READ_CONTACTS) == PackageManager.PERMISSION_GRANTED)
                .put("assistant", try { rm.isRoleHeld(android.app.role.RoleManager.ROLE_ASSISTANT) } catch (e: Exception) { false })
                .toString()
        }

        /** The button next to a checklist item: ask for the permission or open the right system screen. */
        @JavascriptInterface
        fun setupFix(id: String) {
            runOnUiThread { this@MainActivity.setupFix(id) }
        }

        @JavascriptInterface
        fun setLocation(on: Boolean) {
            runOnUiThread { this@MainActivity.setLocation(on) }
        }

        /** Is AssistX the phone's assistant, is the wake word on, may it open over other apps? */
        @JavascriptInterface
        fun voiceState(): String {
            val rm = getSystemService(android.app.role.RoleManager::class.java)
            val isAssistant = try { rm.isRoleHeld(android.app.role.RoleManager.ROLE_ASSISTANT) } catch (e: Exception) { false }
            return JSONObject().put("assistant", isAssistant).put("wake", WakeService.active)
                .put("overlay", android.provider.Settings.canDrawOverlays(this@MainActivity)).toString()
        }

        /** Open the phone's "Digital assistant app" choice. */
        @JavascriptInterface
        fun openAssistantSettings() {
            runOnUiThread {
                val tries = listOf(Intent(android.provider.Settings.ACTION_VOICE_INPUT_SETTINGS), Intent(android.provider.Settings.ACTION_MANAGE_DEFAULT_APPS_SETTINGS))
                for (i in tries) { try { startActivity(i); return@runOnUiThread } catch (e: Exception) { } }
            }
        }

        @JavascriptInterface
        fun openOverlaySettings() {
            runOnUiThread { startActivity(Intent(android.provider.Settings.ACTION_MANAGE_OVERLAY_PERMISSION, android.net.Uri.parse("package:$packageName"))) }
        }

        @JavascriptInterface
        fun setWake(on: Boolean) {
            runOnUiThread { this@MainActivity.setWake(on) }
        }

        /** Open the voice card now (the same one the assistant button opens). */
        @JavascriptInterface
        fun notifyState(): String = JSONObject().put("allowed", NotifyListener.allowed(this@MainActivity)).put("watching", JSONArray(NotifyListener.watched(this@MainActivity).toList())).toString()

        @JavascriptInterface
        fun notifyApps(): String = NotifyListener.seenApps(this@MainActivity)

        @JavascriptInterface
        fun openNotifySettings() {
            runOnUiThread { startActivity(Intent(android.provider.Settings.ACTION_NOTIFICATION_LISTENER_SETTINGS)) }
        }

        /** An automation changed: ask the server again which apps to watch. */
        @JavascriptInterface
        fun syncNotify() {
            Thread { NotifyListener.refresh(this@MainActivity) }.start()
        }
        @JavascriptInterface
        fun talk() {
            runOnUiThread { startActivity(Intent(this@MainActivity, AssistActivity::class.java)) }
        }

        @JavascriptInterface
        fun healthState(): String = JSONObject()
            .put("available", HealthSync.available(this@MainActivity)).put("on", HealthSync.on(this@MainActivity))
            .put("last", HealthSync.last(this@MainActivity) ?: JSONObject.NULL).toString()

        @JavascriptInterface
        fun setHealth(on: Boolean) {
            runOnUiThread { this@MainActivity.setHealth(on) }
        }

        @JavascriptInterface
        fun syncHealth() {
            HealthSync.syncSoon(this@MainActivity, true)
        }

        @JavascriptInterface
        fun openAlarmSettings(kind: String) {
            runOnUiThread { this@MainActivity.openAlarmSettings(kind) }
        }

        /** The page heard `phone.action`: fetch and do the phone's waiting actions now. */
        @JavascriptInterface
        fun phoneActions() { PhoneActions.pollAsync(this@MainActivity) }

        @JavascriptInterface
        fun readPhoto() { runOnUiThread { choosePhoto() } }
    }

    companion object {
        const val HOST = "appassets.androidplatform.net"
        private const val REQ_MIC = 21
        private const val REQ_FILE = 22
        private const val REQ_LOC = 23
        private const val REQ_BG = 24
        private const val REQ_HEALTH = 25
        private const val REQ_SETUP = 26
        private const val REQ_SPEECH = 26
        private const val REQ_CAMERA = 27
        private const val REQ_PHOTO = 28
        const val EXTRA_THREAD = "thread_id"
        const val EXTRA_OPEN = "open"
        val OPEN_VIEWS = setOf("calendar", "alarms", "new-event", "new-alarm", "new-chat")
    }
}
