package app.assistant

import android.content.Context
import org.json.JSONArray
import org.json.JSONObject

/**
 * Parcels, bills and plans, found in phone notifications ON THE PHONE. Only a small item goes to
 * the server (`POST /v1/detected`): kind, a short title, date, amount, a tracking number, the app's
 * name. Never the whole text. Codes and sign-in messages (one-time passwords, verification, 2FA)
 * are always ignored, before anything else. Each kind can be turned off in Settings, Noticing
 * (`GET /v1/detected/settings`, read here every 15 minutes).
 */
object Detect {
    private fun prefs(ctx: Context) = ctx.getSharedPreferences("detect", Context.MODE_PRIVATE)

    private val SECRET = Regex(
        "\\b(otp|one[- ]time|passcode|2fa|two[- ]factor|verification|verify|security code|auth(entication)? code|log[- ]?in code|sign[- ]?in code|confirmation code|access code|pin code|m?tan)\\b|do not share|don'?t share|never share",
        RegexOption.IGNORE_CASE,
    )
    private val CODE_NEAR_DIGITS = Regex("code", RegexOption.IGNORE_CASE)
    private val DIGITS_4_8 = Regex("(?<!\\d)\\d{4,8}(?!\\d)")

    /** A code or a login message: never looked at further. */
    fun isSecret(s: String): Boolean = SECRET.containsMatchIn(s) || (CODE_NEAR_DIGITS.containsMatchIn(s) && DIGITS_4_8.containsMatchIn(s))

    private val PARCEL = listOf(
        "out for delivery" to "Out for delivery", "was delivered" to "Delivered", "has been delivered" to "Delivered", "been delivered" to "Delivered",
        "delivered today" to "Delivered", "arriving today" to "Arriving today", "arriving tomorrow" to "Arriving tomorrow", "delivery attempt" to "Delivery attempt",
        "has shipped" to "Shipped", "has been shipped" to "Shipped", "was dispatched" to "Dispatched", "has been dispatched" to "Dispatched",
        "on its way" to "On its way", "ready for pickup" to "Ready for pickup", "ready for collection" to "Ready for collection",
    )
    private val TRACKING = Regex("\\b(1Z[0-9A-Z]{16}|[A-Z]{2}\\d{9}[A-Z]{2}|JJD\\d{12,20}|\\d{12,22})\\b")
    private val BILL = Regex("amount due|payment due|minimum payment|your bill|bill is (ready|due|available)|invoice|statement is (ready|available)|direct debit|due (on|by|date)", RegexOption.IGNORE_CASE)
    private val AMOUNT = Regex("([$€£¥₹]\\s?\\d[\\d,]*(?:[.,]\\d{2})?|\\d[\\d,]*(?:[.,]\\d{2})?\\s?(?:USD|EUR|GBP|CHF|CAD|AUD|SEK|NOK|DKK|kr))")
    private val DUE = Regex("due (?:on |by |date:? )?([A-Za-z]{3,9}\\.? \\d{1,2}|\\d{1,2} [A-Za-z]{3,9}|\\d{1,2}[./-]\\d{1,2}(?:[./-]\\d{2,4})?|\\d{4}-\\d{2}-\\d{2}|today|tomorrow)", RegexOption.IGNORE_CASE)
    private val PLAN = Regex("invited you|invitation:|you'?re invited|event invitation|see you (at|on|tomorrow|tonight|later at)|meet (at|you at)|let'?s meet|dinner at|lunch at|drinks at|coffee at|are you free", RegexOption.IGNORE_CASE)
    private val TIME = Regex("\\b(?:at|@)\\s?(\\d{1,2}(?::\\d{2})?\\s?(?:am|pm)?)\\b", RegexOption.IGNORE_CASE)
    private val DAY = Regex("\\b(today|tonight|tomorrow|monday|tuesday|wednesday|thursday|friday|saturday|sunday)\\b", RegexOption.IGNORE_CASE)

    /** The sentence around a match, cut short. */
    private fun around(text: String, at: Int, max: Int = 80): String {
        val start = text.lastIndexOfAny(charArrayOf('.', '!', '?', '\n'), at).let { if (it < 0) 0 else it + 1 }
        val end = text.indexOfAny(charArrayOf('.', '!', '?', '\n'), at).let { if (it < 0) text.length else it }
        return text.substring(start, end).trim().take(max)
    }

    /** One item or null. Pure, so it can be checked without a phone. */
    fun match(app: String, title: String, text: String, kinds: Set<String>): JSONObject? {
        val all = "$title\n$text"
        if (all.isBlank() || isSecret(all)) return null
        val low = all.lowercase()
        if ("parcel" in kinds) {
            PARCEL.firstOrNull { low.contains(it.first) }?.let { (_, status) ->
                val item = JSONObject().put("kind", "parcel").put("title", status).put("app", app.take(60))
                TRACKING.find(all)?.let { item.put("ref", it.value) }
                DAY.find(all)?.let { item.put("date", it.value.lowercase()) }
                return item
            }
        }
        if ("bill" in kinds) {
            BILL.find(all)?.let {
                val from = title.ifBlank { app }.take(40)
                val item = JSONObject().put("kind", "bill").put("title", "Bill: $from").put("app", app.take(60))
                AMOUNT.find(all)?.let { a -> item.put("amount", a.value.trim()) }
                DUE.find(all)?.let { d -> item.put("date", d.groupValues[1]) }
                return item
            }
        }
        if ("plan" in kinds) {
            PLAN.find(text.ifBlank { title })?.let { m ->
                val src = text.ifBlank { title }
                val who = title.take(30)
                val line = around(src, m.range.first)
                val item = JSONObject().put("kind", "plan").put("title", if (who.isNotBlank()) "$who: $line" else line).put("app", app.take(60))
                val whenParts = listOfNotNull(DAY.find(src)?.value?.lowercase(), TIME.find(src)?.groupValues?.get(1))
                if (whenParts.isNotEmpty()) item.put("date", whenParts.joinToString(" "))
                return item
            }
        }
        return null
    }

    /** Kinds turned on (all until the server says otherwise). */
    fun kinds(ctx: Context): Set<String> = prefs(ctx).getStringSet("kinds", setOf("parcel", "bill", "plan")) ?: emptySet()

    /** Read the switches from the server. Off the main thread. */
    fun refresh(ctx: Context) {
        val (code, body) = Alarms.http(ctx, "GET", "/v1/detected/settings")
        if (code != 200) return
        val s = try { JSONObject(body) } catch (e: Exception) { return }
        val on = listOf("parcel", "bill", "plan").filter { s.optBoolean(it, true) }.toSet()
        prefs(ctx).edit().putStringSet("kinds", on).apply()
    }

    /** Send one item, unless the same was sent lately (notifications are often updated in place). */
    fun send(ctx: Context, item: JSONObject) {
        val key = item.optString("kind") + "|" + item.optString("title") + "|" + item.optString("date")
        val p = prefs(ctx)
        val recent = JSONArray(p.getString("recent", "[]") ?: "[]")
        for (i in 0 until recent.length()) if (recent.optString(i) == key) return
        val (code, _) = Alarms.http(ctx, "POST", "/v1/detected", JSONObject().put("items", JSONArray().put(item)).toString())
        if (code != 200) return
        val keep = JSONArray().put(key)
        for (i in 0 until minOf(recent.length(), 49)) keep.put(recent.optString(i))
        p.edit().putString("recent", keep.toString()).apply()
    }
}
