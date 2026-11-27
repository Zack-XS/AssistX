package app.assistant

import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import android.util.Base64
import java.security.KeyStore
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec

/**
 * Keeps secrets encrypted with keys that never leave the phone's Keystore.
 *
 * Two keys. The access token uses one that works any time (alarms and notices need it in the background). The
 * remembered-phone key, which can unlock the whole account on the server, uses a stricter one (security review #15):
 * it only works within [UNLOCK_WINDOW_SECS] of the person unlocking the phone (fingerprint, face or screen lock). A
 * stolen phone that stays locked cannot unlock the account. Phones with no screen lock fall back to the plain key.
 */
object Vault {
    private const val ALIAS = "assistant-token"
    private const val STRICT_ALIAS = "assistant-device-v2"
    /** How long after the phone is unlocked the strict key works. */
    const val UNLOCK_WINDOW_SECS = 15 * 60
    /** Sealed with the strict key: starts with this mark, so older values (plain key) still open. */
    private const val STRICT_MARK = "s1:"

    private fun key(alias: String = ALIAS, strict: Boolean = false): SecretKey {
        val ks = KeyStore.getInstance("AndroidKeyStore").apply { load(null) }
        (ks.getKey(alias, null) as? SecretKey)?.let { return it }
        val gen = KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, "AndroidKeyStore")
        val spec = KeyGenParameterSpec.Builder(alias, KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT)
            .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
            .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
            .setKeySize(256)
        if (strict) {
            spec.setUserAuthenticationRequired(true)
                .setUserAuthenticationParameters(UNLOCK_WINDOW_SECS, KeyProperties.AUTH_BIOMETRIC_STRONG or KeyProperties.AUTH_DEVICE_CREDENTIAL)
                .setInvalidatedByBiometricEnrollment(false)
        }
        gen.init(spec.build())
        return gen.generateKey()
    }

    /** Text in, base64 of (iv + ciphertext) out. */
    fun seal(plain: String): String {
        if (plain.isEmpty()) return ""
        val c = Cipher.getInstance("AES/GCM/NoPadding").apply { init(Cipher.ENCRYPT_MODE, key()) }
        return Base64.encodeToString(c.iv + c.doFinal(plain.toByteArray()), Base64.NO_WRAP)
    }

    /**
     * Seal with the strict key. Null when the phone has no screen lock or was not unlocked lately: the caller then
     * keeps what it has and tries again later.
     */
    fun sealStrict(plain: String): String? = try {
        val c = Cipher.getInstance("AES/GCM/NoPadding").apply { init(Cipher.ENCRYPT_MODE, key(STRICT_ALIAS, strict = true)) }
        STRICT_MARK + Base64.encodeToString(c.iv + c.doFinal(plain.toByteArray()), Base64.NO_WRAP)
    } catch (e: Exception) {
        null
    }

    fun isStrict(sealed: String?): Boolean = sealed?.startsWith(STRICT_MARK) == true

    /** Empty text if there is nothing stored or it cannot be read (after a restore, or a strict value while the phone was not unlocked lately). */
    fun open(sealed: String?): String {
        if (sealed.isNullOrEmpty()) return ""
        return try {
            val strict = isStrict(sealed)
            val raw = Base64.decode(if (strict) sealed.removePrefix(STRICT_MARK) else sealed, Base64.NO_WRAP)
            val k = if (strict) key(STRICT_ALIAS, strict = true) else key()
            val c = Cipher.getInstance("AES/GCM/NoPadding").apply { init(Cipher.DECRYPT_MODE, k, GCMParameterSpec(128, raw.copyOfRange(0, 12))) }
            String(c.doFinal(raw.copyOfRange(12, raw.size)))
        } catch (e: Exception) {
            ""
        }
    }
}
