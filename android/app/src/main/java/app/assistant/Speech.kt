package app.assistant

import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.os.Build
import android.speech.RecognitionService
import android.speech.RecognizerIntent
import android.speech.SpeechRecognizer
import java.util.Locale

/**
 * Speech to text on the phone. Audio is never sent to the Assistant server: only the text is.
 *
 * Phones pick a "voice input" app for `createSpeechRecognizer`. When another app becomes the
 * digital assistant (AssistX), Android may point that setting at an app with no speech service,
 * and the default recognizer then hears nothing. So this does not trust the default: it tries,
 * in order, the on-device recognizer, then each speech service installed on the phone by name
 * (Google's first), and the default last.
 */
object Speech {
    private val PREFERRED = listOf("com.google.android.googlequicksearchbox", "com.google.android.as", "com.google.android.tts")

    /** Every way this phone can do speech, best first. `null` means the on-device recognizer. */
    fun options(ctx: Context): List<ComponentName?> {
        val out = mutableListOf<ComponentName?>()
        if (Build.VERSION.SDK_INT >= 33 && SpeechRecognizer.isOnDeviceRecognitionAvailable(ctx)) out.add(null)
        val found = ctx.packageManager.queryIntentServices(Intent(RecognitionService.SERVICE_INTERFACE), 0)
            .mapNotNull { it.serviceInfo }.filter { it.packageName != ctx.packageName }
            .map { ComponentName(it.packageName, it.name) }
            .sortedBy { c -> PREFERRED.indexOf(c.packageName).let { if (it < 0) 99 else it } }
        out.addAll(found)
        return out
    }

    /** The recognizer for option `attempt` (see `options`); past the end, the phone's default. */
    fun make(ctx: Context, attempt: Int = 0): SpeechRecognizer {
        val opts = options(ctx)
        return try {
            when {
                attempt < opts.size && opts[attempt] == null -> SpeechRecognizer.createOnDeviceSpeechRecognizer(ctx)
                attempt < opts.size -> SpeechRecognizer.createSpeechRecognizer(ctx, opts[attempt])
                else -> SpeechRecognizer.createSpeechRecognizer(ctx)
            }
        } catch (e: Exception) {
            SpeechRecognizer.createSpeechRecognizer(ctx)
        }
    }

    /** How many options there are to try, the default included. */
    fun count(ctx: Context) = options(ctx).size + 1

    fun available(ctx: Context): Boolean = count(ctx) > 1 || SpeechRecognizer.isRecognitionAvailable(ctx)

    /** The request. The language is set: the on-device recognizer may hear nothing without it. */
    fun intent(ctx: Context, partial: Boolean): Intent = Intent(RecognizerIntent.ACTION_RECOGNIZE_SPEECH)
        .putExtra(RecognizerIntent.EXTRA_LANGUAGE_MODEL, RecognizerIntent.LANGUAGE_MODEL_FREE_FORM)
        .putExtra(RecognizerIntent.EXTRA_LANGUAGE, ctx.getSharedPreferences("app", Context.MODE_PRIVATE).getString("speech_lang", null) ?: Locale.getDefault().toLanguageTag())
        .putExtra(RecognizerIntent.EXTRA_PARTIAL_RESULTS, partial)
        .putExtra(RecognizerIntent.EXTRA_CALLING_PACKAGE, ctx.packageName)

    /** The phone's own speech window, for when no quiet recognizer will start. One phrase, then it closes. */
    fun windowIntent(ctx: Context): Intent = intent(ctx, false).putExtra(RecognizerIntent.EXTRA_PROMPT, "Speak now")

    /** The option that worked last time, so the next start goes straight to it. */
    fun good(ctx: Context): Int = ctx.getSharedPreferences("app", Context.MODE_PRIVATE).getInt("speech_ok", 0)
    fun remember(ctx: Context, attempt: Int) { ctx.getSharedPreferences("app", Context.MODE_PRIVATE).edit().putInt("speech_ok", attempt).apply() }

    fun why(error: Int): String = when (error) {
        SpeechRecognizer.ERROR_AUDIO -> "the microphone could not be used"
        SpeechRecognizer.ERROR_CLIENT -> "the speech service refused to start"
        SpeechRecognizer.ERROR_INSUFFICIENT_PERMISSIONS -> "the microphone is off for this app"
        SpeechRecognizer.ERROR_NETWORK, SpeechRecognizer.ERROR_NETWORK_TIMEOUT -> "the speech service has no network"
        SpeechRecognizer.ERROR_NO_MATCH -> "nothing was recognized"
        SpeechRecognizer.ERROR_RECOGNIZER_BUSY -> "another app is using speech recognition"
        SpeechRecognizer.ERROR_SERVER -> "the speech service had an error"
        SpeechRecognizer.ERROR_SPEECH_TIMEOUT -> "no speech was heard"
        11 -> "the speech service disconnected"
        12, 13 -> "this language is not downloaded for speech on the phone"
        NO_START -> "no speech service answered"
        else -> "speech error $error"
    }

    /** Not a real Android code: the recognizer never started (a broken voice-input setting). */
    const val NO_START = 900

    /** Errors after which the next option may work. */
    fun tryNext(error: Int) = error !in listOf(SpeechRecognizer.ERROR_NO_MATCH, SpeechRecognizer.ERROR_SPEECH_TIMEOUT, SpeechRecognizer.ERROR_INSUFFICIENT_PERMISSIONS, SpeechRecognizer.ERROR_RECOGNIZER_BUSY)
}
