//! Sign-in for plugins that act on one of your accounts (docs/03). Spotify today.
//!
//! Authorization Code with PKCE, so no client secret is needed: you make a free app on the
//! provider's developer site, paste its Client ID, and approve once. The core keeps the tokens
//! as secrets named `<PROVIDER>_*` and refreshes the short-lived access token itself. A plugin
//! never sees the refresh token or the Client ID, and the model never sees any of them.

use crate::{App, store, util};
use rusqlite::{OptionalExtension, params};
use serde_json::{Value, json};
use std::time::Duration;

pub struct Provider {
    pub id: &'static str,
    /// Prefix of the secret names: `SPOTIFY_CLIENT_ID`, `SPOTIFY_REFRESH_TOKEN`, ...
    pub prefix: &'static str,
    pub scopes: &'static str,
    /// Extra sign-in address parameters (Google needs these to hand out a refresh token).
    pub extra: &'static [(&'static str, &'static str)],
    /// Whether the provider also wants a client secret (Google does, even with PKCE).
    pub secret: bool,
}

const PROVIDERS: &[Provider] = &[Provider {
    id: "spotify",
    prefix: "SPOTIFY",
    scopes: "user-read-playback-state user-modify-playback-state user-read-currently-playing user-library-read user-library-modify user-top-read user-read-recently-played playlist-read-private",
    extra: &[],
    secret: false,
},
Provider {
    id: "google-health",
    prefix: "GOOGLE",
    scopes: "https://www.googleapis.com/auth/googlehealth.activity_and_fitness.readonly https://www.googleapis.com/auth/googlehealth.health_metrics_and_measurements.readonly https://www.googleapis.com/auth/googlehealth.sleep.readonly https://www.googleapis.com/auth/googlehealth.nutrition.readonly https://www.googleapis.com/auth/googlehealth.profile.readonly",
    extra: &[("access_type", "offline"), ("prompt", "consent")],
    secret: true,
},
// Gmail (plugin `email`). Its own tokens (`GMAIL_*`), so it signs in and out apart from Google Health;
// the same Google Cloud client may be used for both.
Provider {
    id: "gmail",
    prefix: "GMAIL",
    scopes: "https://www.googleapis.com/auth/gmail.readonly https://www.googleapis.com/auth/gmail.compose https://www.googleapis.com/auth/gmail.send",
    extra: &[("access_type", "offline"), ("prompt", "consent")],
    secret: true,
}];

pub fn provider(id: &str) -> Option<&'static Provider> {
    PROVIDERS.iter().find(|p| p.id == id)
}

fn name(p: &Provider, what: &str) -> String {
    format!("{}_{what}", p.prefix)
}

fn get(app: &App, p: &Provider, what: &str) -> Option<String> {
    store::secret_get(app, &name(p, what))
}

fn put(app: &App, p: &Provider, what: &str, value: &str) {
    store::secret_set(app, &name(p, what), value);
}

// ---------------- small encoders ----------------

/// Percent-encode everything but the unreserved characters (RFC 3986).
pub fn enc(s: &str) -> String {
    s.bytes().map(|b| if b.is_ascii_alphanumeric() || b"-._~".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") }).collect()
}

pub fn form(pairs: &[(&str, &str)]) -> String {
    pairs.iter().map(|(k, v)| format!("{}={}", enc(k), enc(v))).collect::<Vec<_>>().join("&")
}

/// URL-safe base64 without padding.
fn b64url(bytes: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::new();
    for c in bytes.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        if c.len() > 1 { out.push(T[(n >> 6) as usize & 63] as char); }
        if c.len() > 2 { out.push(T[n as usize & 63] as char); }
    }
    out
}

fn sha256_bytes(s: &str) -> Vec<u8> {
    let hex = util::sha256_hex(s.as_bytes());
    (0..hex.len()).step_by(2).filter_map(|i| u8::from_str_radix(&hex[i..i + 2], 16).ok()).collect()
}

// ---------------- sign-in ----------------

/// Where the provider may send the browser back to: the app's own link, a loopback address for
/// testing on this computer, or an https address.
fn redirect_ok(uri: &str) -> bool {
    uri.len() <= 200 && (uri.starts_with("app.assistant://") || uri.starts_with("http://127.0.0.1") || uri.starts_with("https://")) && !uri.contains(char::is_whitespace)
}

/// Step 1. Remember a one-time proof, and return the address to open in the browser.
pub fn start(app: &App, id: &str, client_id: &str, redirect_uri: &str, client_secret: &str) -> Result<Value, String> {
    let p = provider(id).ok_or("unknown provider")?;
    if !(8..=120).contains(&client_id.len()) || !client_id.chars().all(|c| c.is_ascii_alphanumeric() || "-._".contains(c)) {
        return Err("The Client ID is 32 letters and digits, copied from the provider's developer dashboard.".into());
    }
    if !redirect_ok(redirect_uri) {
        return Err("redirect_uri must be app.assistant://..., http://127.0.0.1..., or https://...".into());
    }
    let verifier = util::random_hex(40);
    let state = util::random_hex(16);
    let challenge = b64url(&sha256_bytes(&verifier));
    {
        let c = app.db.lock();
        let _ = c.execute("DELETE FROM oauth_pending WHERE created < ?1", [util::unix_now() - 600]);
        let _ = c.execute(
            "INSERT INTO oauth_pending (state, provider, verifier, redirect_uri, client_id, created) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![state, p.id, verifier, redirect_uri, client_id, util::unix_now()],
        );
    }
    put(app, p, "CLIENT_ID", client_id);
    if p.secret {
        if client_secret.is_empty() || client_secret.len() > 200 || client_secret.contains(char::is_whitespace) {
            return Err("This provider also needs the Client Secret from its developer dashboard.".into());
        }
        put(app, p, "CLIENT_SECRET", client_secret);
    }
    let mut pairs: Vec<(&str, &str)> = vec![
        ("response_type", "code"), ("client_id", client_id), ("scope", p.scopes), ("redirect_uri", redirect_uri),
        ("state", &state), ("code_challenge_method", "S256"), ("code_challenge", &challenge),
        ];
    pairs.extend_from_slice(p.extra);
    let q = form(&pairs);
    Ok(json!({ "url": format!("{}?{q}", auth_url(app, p)), "state": state }))
}

fn is_google(p: &Provider) -> bool {
    matches!(p.id, "google-health" | "gmail")
}

/// Where the browser goes to approve.
fn auth_url(app: &App, p: &Provider) -> String {
    if is_google(p) { app.cfg.google_auth_url.clone() } else { format!("{}/authorize", app.cfg.oauth_base) }
}

fn token_url(app: &App, p: &Provider) -> String {
    if is_google(p) { app.cfg.google_token_url.clone() } else { format!("{}/api/token", app.cfg.oauth_base) }
}

fn post_form(app: &App, p: &Provider, mut body: String) -> Result<Value, String> {
    if p.secret {
        if let Some(s) = get(app, p, "CLIENT_SECRET") {
            body.push_str(&format!("&client_secret={}", enc(&s)));
        }
    }
    let agent: ureq::Agent = ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(15))).http_status_as_error(false).build().into();
    let mut resp = agent
        .post(&token_url(app, p))
        .header("Content-Type", "application/x-www-form-urlencoded")
        .send(body)
        .map_err(|e| format!("Could not reach the provider: {e}"))?;
    let status = resp.status().as_u16();
    let text = resp.body_mut().read_to_string().unwrap_or_default();
    let v: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
    if status >= 400 {
        let why = v["error_description"].as_str().or(v["error"].as_str()).unwrap_or("it refused");
        return Err(format!("The provider answered {status}: {why}"));
    }
    Ok(v)
}

fn store_tokens(app: &App, p: &Provider, v: &Value) -> Result<(), String> {
    let access = v["access_token"].as_str().ok_or("The provider sent no access token.")?;
    put(app, p, "ACCESS_TOKEN", access);
    put(app, p, "ACCESS_EXPIRES", &(util::unix_now() + v["expires_in"].as_i64().unwrap_or(3600)).to_string());
    if let Some(r) = v["refresh_token"].as_str().filter(|r| !r.is_empty()) {
        put(app, p, "REFRESH_TOKEN", r);
    }
    // What the user actually allowed. A refresh may leave it out; then the old list still holds.
    if let Some(s) = v["scope"].as_str().filter(|s| !s.trim().is_empty()) {
        put(app, p, "SCOPES", s.trim());
    }
    Ok(())
}

/// Scopes asked for but not (yet) allowed. Empty when the provider never said what it granted.
pub fn missing_scopes(app: &App, id: &str) -> Vec<String> {
    let Some(p) = provider(id) else { return vec![] };
    let Some(have) = get(app, p, "SCOPES") else { return vec![] };
    let have: Vec<&str> = have.split_whitespace().collect();
    p.scopes.split_whitespace().filter(|s| !have.contains(s)).map(String::from).collect()
}

/// Step 2. The browser came back with a code; trade it for tokens. Blocking.
pub fn finish(app: &App, id: &str, code: &str, state: &str) -> Result<Value, String> {
    let p = provider(id).ok_or("unknown provider")?;
    let row: Option<(String, String, String)> = {
        let c = app.db.lock();
        let r = c
            .query_row("SELECT verifier, redirect_uri, client_id FROM oauth_pending WHERE state = ?1 AND provider = ?2 AND created > ?3", params![state, p.id, util::unix_now() - 600], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .optional()
            .ok()
            .flatten();
        let _ = c.execute("DELETE FROM oauth_pending WHERE state = ?1", [state]); // one use only
        r
    };
    let (verifier, redirect_uri, client_id) = row.ok_or("This sign-in link is not valid or has expired. Start again.")?;
    let v = post_form(app, p, form(&[("grant_type", "authorization_code"), ("code", code), ("redirect_uri", &redirect_uri), ("client_id", &client_id), ("code_verifier", &verifier)]))?;
    store_tokens(app, p, &v)?;
    if get(app, p, "REFRESH_TOKEN").is_none() {
        return Err("The provider sent no refresh token.".into());
    }
    Ok(json!({ "connected": true }))
}

/// An access token that is good for at least a minute. Refreshes it when needed. Blocking.
pub fn fresh_token(app: &App, id: &str) -> Result<String, String> {
    let p = provider(id).ok_or("unknown provider")?;
    let expires: i64 = get(app, p, "ACCESS_EXPIRES").and_then(|v| v.parse().ok()).unwrap_or(0);
    if let Some(t) = get(app, p, "ACCESS_TOKEN").filter(|_| expires > util::unix_now() + 60) {
        return Ok(t);
    }
    let (Some(refresh), Some(client_id)) = (get(app, p, "REFRESH_TOKEN"), get(app, p, "CLIENT_ID")) else {
        return Err(format!("{} is not connected. Connect it in Settings, Integrations.", p.id));
    };
    let v = post_form(app, p, form(&[("grant_type", "refresh_token"), ("refresh_token", &refresh), ("client_id", &client_id)]))
        .map_err(|e| format!("{e}. Connect {} again in Settings, Integrations.", p.id))?;
    store_tokens(app, p, &v)?;
    get(app, p, "ACCESS_TOKEN").ok_or_else(|| "no access token".into())
}

pub fn status(app: &App, id: &str) -> Result<Value, String> {
    let p = provider(id).ok_or("unknown provider")?;
    Ok(json!({
        "provider": p.id,
        "connected": get(app, p, "REFRESH_TOKEN").is_some(),
        "client_id": get(app, p, "CLIENT_ID"),
        "scopes": p.scopes,
        "missing_scopes": missing_scopes(app, id),
        "needs_secret": p.secret,
    }))
}

pub fn disconnect(app: &App, id: &str) -> Result<Value, String> {
    let p = provider(id).ok_or("unknown provider")?;
    for what in ["ACCESS_TOKEN", "ACCESS_EXPIRES", "REFRESH_TOKEN", "SCOPES", "CLIENT_SECRET"] {
        store::secret_delete(app, &name(p, what));
    }
    Ok(json!({ "connected": false }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn challenge_matches_the_rfc_example() {
        // RFC 7636 appendix B.
        let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
        assert_eq!(b64url(&sha256_bytes(verifier)), "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM");
    }

    #[test]
    fn form_encoding_escapes_what_it_must() {
        assert_eq!(form(&[("a", "b c"), ("r", "app.assistant://spotify?x=1")]), "a=b%20c&r=app.assistant%3A%2F%2Fspotify%3Fx%3D1");
    }

    #[test]
    fn gmail_signs_in_with_google_and_its_own_secrets() {
        let p = provider("gmail").expect("gmail provider");
        assert!(p.secret && is_google(p) && p.prefix == "GMAIL");
        for s in ["gmail.readonly", "gmail.compose", "gmail.send"] {
            assert!(p.scopes.split_whitespace().any(|x| x.ends_with(s)), "{s}");
        }
        assert!(p.extra.contains(&("access_type", "offline")));
    }

    #[test]
    fn only_known_redirects_are_allowed() {
        assert!(redirect_ok("app.assistant://spotify") && redirect_ok("http://127.0.0.1:8000/") && redirect_ok("https://example.com/cb"));
        assert!(!redirect_ok("http://example.com/cb") && !redirect_ok("javascript:alert(1)") && !redirect_ok("app.assistant://a b"));
    }
}
