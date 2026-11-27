//! Time-based one-time codes (TOTP, RFC 6238) for the admin login (security review #20): six digits from an
//! authenticator app, a new one every 30 seconds, SHA-1 as every app expects. The secret is 20 random bytes,
//! shown once as base32 and as an `otpauth://` link, and kept only inside the encrypted `server.db`.

use hmac::{Hmac, Mac};
use sha1::Sha1;

const STEP: i64 = 30;
const B32: &[u8; 32] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";

/// A new secret as base32 (32 letters, 160 bits).
pub fn new_secret() -> String {
    let k = crate::vault::random_key();
    encode(&k[..20])
}

pub fn encode(b: &[u8]) -> String {
    let mut out = String::new();
    let (mut buf, mut bits) = (0u32, 0u32);
    for &x in b {
        buf = (buf << 8) | x as u32;
        bits += 8;
        while bits >= 5 {
            out.push(B32[((buf >> (bits - 5)) & 31) as usize] as char);
            bits -= 5;
        }
    }
    if bits > 0 {
        out.push(B32[((buf << (5 - bits)) & 31) as usize] as char);
    }
    out
}

pub fn decode(s: &str) -> Option<Vec<u8>> {
    let mut out = vec![];
    let (mut buf, mut bits) = (0u32, 0u32);
    for c in s.chars().filter(|c| c.is_ascii_alphanumeric()) {
        let v = B32.iter().position(|b| *b as char == c.to_ascii_uppercase())? as u32;
        buf = (buf << 5) | v;
        bits += 5;
        if bits >= 8 {
            out.push((buf >> (bits - 8)) as u8);
            bits -= 8;
        }
    }
    Some(out)
}

/// The code for one 30-second step.
pub fn code_at(secret: &[u8], step: i64) -> String {
    let Ok(mut m) = Hmac::<Sha1>::new_from_slice(secret) else { return String::new() };
    m.update(&(step as u64).to_be_bytes());
    let h = m.finalize().into_bytes();
    let o = (h[19] & 0xf) as usize;
    let n = (u32::from_be_bytes([h[o], h[o + 1], h[o + 2], h[o + 3]]) & 0x7fff_ffff) % 1_000_000;
    format!("{n:06}")
}

/// The step a right code belongs to (this one or one either side, for clock drift), or `None`.
pub fn check(secret_b32: &str, code: &str, now: i64) -> Option<i64> {
    let secret = decode(secret_b32)?;
    let code: String = code.chars().filter(|c| c.is_ascii_digit()).collect();
    if code.len() != 6 {
        return None;
    }
    let step = now / STEP;
    (step - 1..=step + 1).find(|s| crate::hub_admin::same(&code_at(&secret, *s), &code))
}

/// The link an authenticator app reads.
pub fn uri(secret_b32: &str, account: &str) -> String {
    let enc = |s: &str| s.chars().map(|c| if c.is_ascii_alphanumeric() { c.to_string() } else { format!("%{:02X}", c as u32) }).collect::<String>();
    format!("otpauth://totp/Server%20admin:{}?secret={secret_b32}&issuer=Server%20admin&digits=6&period=30", enc(account))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rfc_6238_sha1_vectors() {
        // RFC 6238 appendix B, SHA-1, the 8-digit values cut to their last 6.
        let k = b"12345678901234567890";
        assert_eq!(code_at(k, 59 / 30), "287082");
        assert_eq!(code_at(k, 1_111_111_109 / 30), "081804");
        assert_eq!(code_at(k, 1_234_567_890 / 30), "005924");
        assert_eq!(code_at(k, 20_000_000_000 / 30), "353130");
    }

    #[test]
    fn base32_and_check() {
        let k = b"12345678901234567890";
        let s = encode(k);
        assert_eq!(s, "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ");
        assert_eq!(decode(&s).unwrap(), k.to_vec());
        assert_eq!(check(&s, "287 082", 59), Some(1));
        assert_eq!(check(&s, "287082", 59 + 30), Some(1), "one step late still works");
        assert!(check(&s, "287082", 59 + 120).is_none(), "an old code is refused");
        assert!(check(&s, "12345", 59).is_none());
        assert_eq!(new_secret().len(), 32);
    }
}
