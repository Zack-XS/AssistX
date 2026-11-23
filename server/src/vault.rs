//! Keys for accounts (docs/15). An account's data key is 32 random bytes. It is never stored as it
//! is: only wrapped (encrypted) once by a key made from the passphrase (Argon2id) and once by the
//! recovery key shown at sign-up. Lose both and the data is gone for good.

use argon2::{Algorithm, Argon2, Params, Version};
use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};

/// A random 256-bit key, as bytes.
pub fn random_key() -> [u8; 32] {
    let hex = crate::util::random_hex(32);
    let mut k = [0u8; 32];
    for (i, b) in k.iter_mut().enumerate() {
        *b = u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).unwrap_or(0);
    }
    k
}

pub fn to_hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

pub fn from_hex(s: &str) -> Option<Vec<u8>> {
    (s.len() % 2 == 0).then_some(())?;
    (0..s.len() / 2).map(|i| u8::from_str_radix(s.get(i * 2..i * 2 + 2)?, 16).ok()).collect()
}

/// Argon2id: 64 MiB, 3 passes, 1 lane. About a third of a second on a small server.
pub fn kdf(passphrase: &str, salt: &[u8]) -> Result<[u8; 32], String> {
    let params = Params::new(64 * 1024, 3, 1, Some(32)).map_err(|e| e.to_string())?;
    let mut out = [0u8; 32];
    Argon2::new(Algorithm::Argon2id, Version::V0x13, params).hash_password_into(passphrase.as_bytes(), salt, &mut out).map_err(|e| e.to_string())?;
    Ok(out)
}

/// Encrypt `secret` under `key`. Output: 12 byte nonce then ciphertext with its tag, as hex. `aad` ties it to its owner.
pub fn wrap(key: &[u8; 32], secret: &[u8], aad: &str) -> Result<String, String> {
    let nonce_bytes = random_key();
    let nonce = Nonce::from_slice(&nonce_bytes[..12]);
    let ct = ChaCha20Poly1305::new(Key::from_slice(key)).encrypt(nonce, Payload { msg: secret, aad: aad.as_bytes() }).map_err(|_| "could not encrypt".to_string())?;
    let mut out = nonce_bytes[..12].to_vec();
    out.extend(ct);
    Ok(to_hex(&out))
}

/// The reverse. Any wrong key, wrong owner or damaged text fails the same way.
pub fn unwrap(key: &[u8; 32], wrapped: &str, aad: &str) -> Result<Vec<u8>, String> {
    let raw = from_hex(wrapped).filter(|r| r.len() > 12 + 16).ok_or("wrong")?;
    ChaCha20Poly1305::new(Key::from_slice(key)).decrypt(Nonce::from_slice(&raw[..12]), Payload { msg: &raw[12..], aad: aad.as_bytes() }).map_err(|_| "wrong".to_string())
}

const B32: &[u8; 32] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";

/// The recovery key as people see it: 52 letters and digits in 13 groups of 4. It is the 256 bit key itself.
pub fn recovery_text(key: &[u8; 32]) -> String {
    let mut bits = String::new();
    for b in key {
        bits.push_str(&format!("{b:08b}"));
    }
    while bits.len() % 5 != 0 {
        bits.push('0');
    }
    let chars: Vec<char> = (0..bits.len() / 5).map(|i| B32[usize::from_str_radix(&bits[i * 5..i * 5 + 5], 2).unwrap_or(0)] as char).collect();
    chars.chunks(4).map(|c| c.iter().collect::<String>()).collect::<Vec<_>>().join("-")
}

/// Read a recovery key back (dashes, spaces and case do not matter).
pub fn recovery_parse(text: &str) -> Option<[u8; 32]> {
    let mut bits = String::new();
    for c in text.chars().filter(|c| c.is_ascii_alphanumeric()) {
        let v = B32.iter().position(|b| *b as char == c.to_ascii_uppercase())?;
        bits.push_str(&format!("{v:05b}"));
    }
    (bits.len() >= 256).then_some(())?;
    let mut k = [0u8; 32];
    for (i, b) in k.iter_mut().enumerate() {
        *b = u8::from_str_radix(&bits[i * 8..i * 8 + 8], 2).ok()?;
    }
    Some(k)
}

/// The last two groups of a recovery key, upper case, for the "did you really save it" check.
pub fn recovery_tail(text: &str) -> String {
    let clean: Vec<char> = text.chars().filter(|c| c.is_ascii_alphanumeric()).map(|c| c.to_ascii_uppercase()).collect();
    clean[clean.len().saturating_sub(8)..].iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrap_unwrap_and_wrong_keys() {
        let (k, other, secret) = (random_key(), random_key(), random_key());
        let w = wrap(&k, &secret, "acc1").unwrap();
        assert_eq!(unwrap(&k, &w, "acc1").unwrap(), secret.to_vec());
        assert!(unwrap(&other, &w, "acc1").is_err(), "wrong key");
        assert!(unwrap(&k, &w, "acc2").is_err(), "wrong owner");
        let mut bad = w.clone();
        bad.replace_range(30..32, if &w[30..32] == "00" { "01" } else { "00" });
        assert!(unwrap(&k, &bad, "acc1").is_err(), "damaged");
        assert_ne!(wrap(&k, &secret, "acc1").unwrap(), w, "a fresh nonce each time");
    }

    #[test]
    fn recovery_text_round_trips() {
        let k = random_key();
        let t = recovery_text(&k);
        assert_eq!(t.len(), 52 + 12, "{t}");
        assert_eq!(recovery_parse(&t.to_lowercase().replace('-', " ")).unwrap(), k);
        assert!(recovery_parse("NOT-A-KEY").is_none());
        assert_eq!(recovery_tail(&t).len(), 8);
    }

    #[test]
    fn the_passphrase_key_depends_on_both_inputs() {
        let a = kdf("correct horse battery", b"salt-salt-salt-1").unwrap();
        assert_eq!(a, kdf("correct horse battery", b"salt-salt-salt-1").unwrap());
        assert_ne!(a, kdf("correct horse battery!", b"salt-salt-salt-1").unwrap());
        assert_ne!(a, kdf("correct horse battery", b"salt-salt-salt-2").unwrap());
    }
}
