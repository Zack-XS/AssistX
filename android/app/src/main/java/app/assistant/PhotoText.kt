package app.assistant

import android.content.Context
import android.net.Uri
import com.google.mlkit.vision.common.InputImage
import com.google.mlkit.vision.text.TextRecognition
import com.google.mlkit.vision.text.latin.TextRecognizerOptions

/**
 * Reads the text in a photo on the phone (ML Kit text recognition, the model is inside the app:
 * no Google services, no network). Only the text goes on to the chat, marked `[photo text]`;
 * the picture itself never leaves the phone.
 */
object PhotoText {
    /** Calls back on the main thread with the text, or null when it could not be read. */
    fun read(ctx: Context, uri: Uri, done: (String?) -> Unit) {
        val img = try { InputImage.fromFilePath(ctx, uri) } catch (e: Exception) { done(null); return }
        val rec = TextRecognition.getClient(TextRecognizerOptions.DEFAULT_OPTIONS)
        rec.process(img)
            .addOnSuccessListener { r -> done(r.textBlocks.joinToString("\n") { it.text }.trim()) }
            .addOnFailureListener { done(null) }
            .addOnCompleteListener { rec.close() }
    }
}
