//! Accounts (docs/15): sign-up with a recovery key, sign-in, one assistant per account, locking,
//! recovery, and nothing readable on disk. Uses the real router, called directly.

use agent_core::config::Config;
use agent_core::hub::Hub;
use agent_core::hub_http;
use axum::Router;
use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use serde_json::{Value, json};
use std::path::PathBuf;
use std::sync::Arc;
use tower::ServiceExt;

fn template(name: &str) -> Config {
    let dir: PathBuf = std::env::temp_dir().join(format!("acct-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    // The same shape the other tests use; only paths matter here.
    let mut cfg = Config::from_env();
    cfg.port = 9999;
    cfg.bind = "127.0.0.1".into();
    cfg.allowed_hosts = vec![];
    cfg.allowed_origins = vec!["https://appassets.androidplatform.net".into()];
    cfg.data_dir = dir.clone();
    cfg.server_dir = dir.clone();
    cfg.plugins_dir = root.join("plugins");
    cfg.prompts_dir = root.join("prompts");
    cfg.model.kind = "mock".into();
    cfg.sandbox = "off".into();
    cfg.runner_exe = PathBuf::from(env!("CARGO_BIN_EXE_agent-core"));
    cfg
}

struct T {
    router: Router,
    hub: Arc<Hub>,
    dir: PathBuf,
    server: String,
    /// An admin session, sent with every server-token call (the admin routes need it, the others ignore it).
    session: String,
}

impl T {
    fn new(name: &str) -> T {
        let cfg = template(name);
        let dir = cfg.data_dir.clone();
        let hub = Hub::open(cfg, Box::new(|_| Ok(Arc::new(agent_core::model::Mock::new(&[]))))).unwrap();
        let server = format!("srv_{}", "a".repeat(40));
        std::fs::write(dir.join("owner.token"), &server).unwrap(); // the old owner token becomes the server token
        assert!(hub.ensure_server_token().unwrap().is_none());
        Hub::set_admin_password(&dir, "boss", "the boss password").unwrap();
        let session = hub.admin_login("boss", "the boss password").unwrap()["session"].as_str().unwrap().to_string();
        T { router: hub_http::router(hub.clone()), hub, dir, server, session }
    }

    async fn call(&self, method: &str, path: &str, token: Option<&str>, body: Option<Value>) -> (StatusCode, Value) {
        let mut r = Request::builder().method(method).uri(path).header("host", "127.0.0.1:9999");
        if let Some(t) = token {
            r = r.header("authorization", format!("Bearer {t}"));
            if t == self.server {
                r = r.header("x-admin-session", &self.session);
            }
        }
        let req = r.header("content-type", "application/json").body(Body::from(body.map(|b| b.to_string()).unwrap_or_default())).unwrap();
        let resp = self.router.clone().oneshot(req).await.unwrap();
        let status = resp.status();
        let bytes = to_bytes(resp.into_body(), 1 << 20).await.unwrap();
        (status, serde_json::from_slice(&bytes).unwrap_or(Value::Null))
    }

    async fn srv(&self, path: &str, body: Value) -> (StatusCode, Value) {
        self.call("POST", path, Some(&self.server), Some(body)).await
    }

    /// Invite, sign up and confirm. Returns (token, recovery key).
    async fn make(&self, user: &str, pass: &str) -> (String, String) {
        let (c, v) = self.srv("/v1/server/accounts", json!({ "username": user })).await;
        assert_eq!(c, StatusCode::OK, "{v}");
        let code = v["invite_code"].as_str().unwrap().to_string();
        let (c, v) = self.srv("/v1/server/signup", json!({ "username": user, "passphrase": pass, "invite_code": code })).await;
        assert_eq!(c, StatusCode::OK, "{v}");
        let key = v["recovery_key"].as_str().unwrap().to_string();
        let tail: String = key.chars().filter(|c| c.is_ascii_alphanumeric()).rev().take(8).collect::<Vec<_>>().into_iter().rev().collect();
        let (c, v) = self.srv("/v1/server/signup/confirm", json!({ "username": user, "passphrase": pass, "tail": tail })).await;
        assert_eq!(c, StatusCode::OK, "{v}");
        (v["token"].as_str().unwrap().to_string(), key)
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn accounts_are_separate_encrypted_and_recoverable() {
    let t = T::new("main");

    // The server token is the outer gate: nothing under /v1/server works without it, and an account token is not it.
    assert_eq!(t.call("GET", "/v1/server/accounts", None, None).await.0, StatusCode::UNAUTHORIZED);
    assert_eq!(t.call("GET", "/v1/server/accounts", Some("nope"), None).await.0, StatusCode::UNAUTHORIZED);
    assert_eq!(t.call("GET", "/v1/health", None, None).await.0, StatusCode::OK);

    // Sign-up needs an invite, a long passphrase, and the recovery key typed back.
    assert_eq!(t.srv("/v1/server/signup", json!({ "username": "ghost", "passphrase": "a long enough phrase" })).await.0, StatusCode::BAD_REQUEST);
    let (_, inv) = t.srv("/v1/server/accounts", json!({ "username": "alice" })).await;
    let code = inv["invite_code"].as_str().unwrap().to_string();
    assert_eq!(t.srv("/v1/server/accounts", json!({ "username": "alice" })).await.0, StatusCode::BAD_REQUEST, "taken");
    assert_eq!(t.srv("/v1/server/signup", json!({ "username": "alice", "passphrase": "short", "invite_code": code })).await.0, StatusCode::BAD_REQUEST);
    let (_, v) = t.srv("/v1/server/signup", json!({ "username": "alice", "passphrase": "alice has a long passphrase", "invite_code": code.to_lowercase().replace('-', " ") })).await;
    assert!(v["recovery_key"].as_str().is_some_and(|k| k.len() == 64) && v["warning"].as_str().unwrap().contains("gone for good"), "{v}");
    let (c, _) = t.srv("/v1/server/signup/confirm", json!({ "username": "alice", "passphrase": "alice has a long passphrase", "tail": "WRONG123" })).await;
    assert_eq!(c, StatusCode::BAD_REQUEST, "a wrong tail does not activate the account");
    drop(v);

    let (bob_tok, _) = t.make("bob", "bob has a long passphrase too").await; // an account made the whole way
    let (c, _) = t.call("POST", "/v1/threads", Some(&bob_tok), Some(json!({}))).await;
    assert_eq!(c, StatusCode::CREATED);

    // A second account in full: its own world.
    let (carol, _) = t.make("carol", "carol keeps her own phrase").await;
    let (_, list) = t.call("GET", "/v1/threads", Some(&carol), None).await;
    assert_eq!(list["items"].as_array().map(|a| a.len()), Some(0), "carol sees none of bob's chats: {list}");
    let (_, list) = t.call("GET", "/v1/threads", Some(&bob_tok), None).await;
    assert_eq!(list["items"].as_array().map(|a| a.len()), Some(1));
    // Bob's id with carol's secret, or the reverse, gets nowhere.
    let (bid, _) = bob_tok.split_once('.').unwrap();
    let (_, csec) = carol.split_once('.').unwrap();
    assert_eq!(t.call("GET", "/v1/threads", Some(&format!("{bid}.{csec}")), None).await.0, StatusCode::UNAUTHORIZED);
    // An account token does not open the server routes.
    assert_eq!(t.call("GET", "/v1/server/accounts", Some(&carol), None).await.0, StatusCode::UNAUTHORIZED);

    // Wrong passphrase: the same plain answer, and five misses lock the name for a while.
    for _ in 0..5 {
        let (c, v) = t.srv("/v1/server/login", json!({ "username": "carol", "passphrase": "not the phrase at all" })).await;
        assert_eq!((c, v["error"]["message"].as_str()), (StatusCode::UNAUTHORIZED, Some("wrong username or passphrase")));
    }
    let (_, v) = t.srv("/v1/server/login", json!({ "username": "carol", "passphrase": "carol keeps her own phrase" })).await;
    assert!(v["error"]["message"].as_str().unwrap().contains("too many wrong tries"), "{v}");

    // Locking forgets the key: the token answers 423 until the person signs in again, and the data is still there.
    let bid = bid.to_string();
    assert_eq!(t.call("POST", &format!("/v1/server/accounts/{bid}/lock"), Some(&t.server), Some(json!({}))).await.0, StatusCode::OK);
    assert_eq!(t.call("GET", "/v1/threads", Some(&bob_tok), None).await.0, StatusCode::LOCKED);
    let (c, v) = t.srv("/v1/server/login", json!({ "username": "bob", "passphrase": "bob has a long passphrase too" })).await;
    assert_eq!(c, StatusCode::OK, "{v}");
    let again = v["token"].as_str().unwrap().to_string();
    let (_, list) = t.call("GET", "/v1/threads", Some(&again), None).await;
    assert_eq!(list["items"].as_array().map(|a| a.len()), Some(1), "bob's chat survived the lock");

    // The app's update files belong to the server: the server token (also an old phone's saved token) can fetch them.
    std::fs::create_dir_all(t.dir.join("app")).unwrap();
    std::fs::write(t.dir.join("app").join("version.json"), r#"{"versionCode": 7, "versionName": "0.1.7", "notes": "n"}"#).unwrap();
    std::fs::write(t.dir.join("app").join("assistant.apk"), b"apk bytes").unwrap();
    let (c, v) = t.call("GET", "/v1/app/version", Some(&t.server), None).await;
    assert_eq!((c, v["versionCode"].as_i64(), v["size"].as_i64()), (StatusCode::OK, Some(7), Some(9)), "{v}");
    assert_eq!(t.call("GET", "/v1/app/version", Some("nope"), None).await.0, StatusCode::UNAUTHORIZED);
    assert_eq!(t.call("GET", "/v1/app/version", Some(&again), None).await.0, StatusCode::OK, "an account token works too");

    // On disk: the account database is not a plain SQLite file, and the server database holds no secrets.
    let acct_db = t.dir.join("accounts").join(&bid).join("core.db");
    let head = std::fs::read(&acct_db).unwrap();
    assert!(!head.starts_with(b"SQLite format 3"), "the account database must be encrypted");
    let server_db = std::fs::read(t.dir.join("server.db")).unwrap();
    let text = String::from_utf8_lossy(&server_db);
    assert!(!text.contains("long passphrase") && !text.contains(&t.server), "no passphrase and no server token in server.db");
    // Each account has its own folders and plugin copies.
    assert!(t.dir.join("accounts").join(&bid).join("plugins").join("scheduler").join("plugin.yaml").exists());
    assert!(t.hub.list().iter().any(|a| a["username"] == "bob" && a["unlocked"] == true));
}

#[tokio::test(flavor = "multi_thread")]
async fn recovery_key_replaces_a_lost_passphrase() {
    let t = T::new("recover");
    let (tok, key) = t.make("dana", "dana remembers this phrase").await;
    let (id, _) = tok.split_once('.').unwrap();
    t.hub.lock(id);
    // The recovery key, in any case and spacing, opens the account and sets a new passphrase.
    let loose = key.to_lowercase().replace('-', " ");
    let (c, v) = t.srv("/v1/server/recover", json!({ "username": "dana", "recovery_key": loose, "new_passphrase": "dana picked a new phrase" })).await;
    assert_eq!(c, StatusCode::OK, "{v}");
    assert_eq!(t.srv("/v1/server/login", json!({ "username": "dana", "passphrase": "dana remembers this phrase" })).await.0, StatusCode::UNAUTHORIZED, "the old passphrase is gone");
    assert_eq!(t.srv("/v1/server/login", json!({ "username": "dana", "passphrase": "dana picked a new phrase" })).await.0, StatusCode::OK);
    // A wrong recovery key does not.
    let (c, _) = t.srv("/v1/server/recover", json!({ "username": "dana", "recovery_key": "AAAA-AAAA-AAAA-AAAA-AAAA-AAAA-AAAA-AAAA-AAAA-AAAA-AAAA-AAAA-AAAA", "new_passphrase": "another new long phrase" })).await;
    assert_eq!(c, StatusCode::BAD_REQUEST);
    // Changing the passphrase needs the old one.
    assert_eq!(t.srv("/v1/server/passphrase", json!({ "username": "dana", "old": "wrong old phrase here", "new": "dana picked again later" })).await.0, StatusCode::BAD_REQUEST);
    assert_eq!(t.srv("/v1/server/passphrase", json!({ "username": "dana", "old": "dana picked a new phrase", "new": "dana picked again later" })).await.0, StatusCode::OK);
    // Removing an account removes its folder for good.
    assert_eq!(t.call("DELETE", &format!("/v1/server/accounts/{id}"), Some(&t.server), None).await.0, StatusCode::OK);
    assert!(!t.dir.join("accounts").join(id).exists());
}

#[tokio::test(flavor = "multi_thread")]
async fn the_server_password_can_be_changed_from_the_command_line() {
    let t = T::new("pw");
    assert_eq!(t.call("GET", "/v1/server/info", Some(&t.server), None).await.0, StatusCode::OK);
    assert!(Hub::set_server_password(&t.dir, "short").is_err(), "25 characters at least");
    assert!(Hub::set_server_password(&t.dir, "twenty-four characters!!").is_err(), "25 characters at least");
    Hub::set_server_password(&t.dir, "a new password of mine, long").unwrap();
    // The server reads the hash on every call, so the change is live at once and the old one is dead.
    assert_eq!(t.call("GET", "/v1/server/info", Some(&t.server), None).await.0, StatusCode::UNAUTHORIZED);
    assert_eq!(t.call("GET", "/v1/server/info", Some("a new password of mine, long"), None).await.0, StatusCode::OK);
}

#[tokio::test(flavor = "multi_thread")]
async fn repeated_wrong_tokens_are_slowed_down_and_a_right_one_is_not_punished_early() {
    let t = T::new("limit");
    // A few misses are fine, and a right token in between clears the count.
    for _ in 0..4 {
        assert_eq!(t.call("GET", "/v1/server/accounts", Some("wrong"), None).await.0, StatusCode::UNAUTHORIZED);
    }
    assert_eq!(t.call("GET", "/v1/server/accounts", Some(&t.server), None).await.0, StatusCode::OK);
    for _ in 0..4 {
        assert_eq!(t.call("GET", "/v1/server/accounts", Some("wrong"), None).await.0, StatusCode::UNAUTHORIZED);
    }
    // No token at all is not a guess and does not count.
    assert_eq!(t.call("GET", "/v1/server/accounts", None, None).await.0, StatusCode::UNAUTHORIZED);
    // The fifth miss in a row locks the caller out: even the right token waits, and the answer says how long.
    assert_eq!(t.call("GET", "/v1/server/accounts", Some("wrong"), None).await.0, StatusCode::UNAUTHORIZED);
    let (c, v) = t.call("GET", "/v1/server/accounts", Some(&t.server), None).await;
    assert_eq!(c, StatusCode::TOO_MANY_REQUESTS, "{v}");
    assert_eq!(v["error"]["code"], "rate_limited");
    // Guessing account tokens counts too, and the account routes are shut for that caller as well.
    assert_eq!(t.call("GET", "/v1/threads", Some("abc.def"), None).await.0, StatusCode::TOO_MANY_REQUESTS);
    // Each lock is longer than the last (30 s, then 60 s, ...): check the numbers, not the clock.
    assert!(t.hub.blocked_for("local").is_some_and(|s| (1..=30).contains(&s)));
}

#[tokio::test(flavor = "multi_thread")]
async fn the_apps_page_may_read_every_answer_and_its_pre_check_works() {
    let t = T::new("cors");
    let origin = "https://appassets.androidplatform.net";
    let send = |method: &str, path: &str, extra: &[(&str, &str)]| {
        let mut r = Request::builder().method(method).uri(path).header("host", "127.0.0.1:9999").header("origin", origin);
        for (k, v) in extra {
            r = r.header(*k, *v);
        }
        t.router.clone().oneshot(r.body(Body::empty()).unwrap())
    };
    // The health answer carries the headers (the first thing the page asks), and so do refusals.
    let r = send("GET", "/v1/health", &[]).await.unwrap();
    assert_eq!((r.status(), r.headers().get("access-control-allow-origin").and_then(|h| h.to_str().ok())), (StatusCode::OK, Some(origin)));
    let r = send("GET", "/v1/server/info", &[]).await.unwrap();
    assert_eq!((r.status(), r.headers().get("access-control-allow-origin").is_some()), (StatusCode::UNAUTHORIZED, true));
    // The browser's pre-check for a request with an Authorization header, on a server route and on an account route.
    for path in ["/v1/server/login", "/v1/server/info", "/v1/threads"] {
        let r = send("OPTIONS", path, &[("access-control-request-method", "POST"), ("access-control-request-headers", "authorization")]).await.unwrap();
        assert_eq!(r.status(), StatusCode::NO_CONTENT, "{path}");
        assert!(r.headers().get("access-control-allow-headers").and_then(|h| h.to_str().ok()).is_some_and(|h| h.contains("authorization")), "{path}");
    }
    // A page from anywhere else is refused.
    let r = t.router.clone().oneshot(Request::builder().uri("/v1/health").header("host", "127.0.0.1:9999").header("origin", "https://evil.example").body(Body::empty()).unwrap()).await.unwrap();
    assert_eq!(r.status(), StatusCode::FORBIDDEN);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_remembered_phone_unlocks_its_account_after_a_restart_without_a_passphrase() {
    let t = T::new("remember");
    let pass = "erin keeps this long phrase";
    let (_, inv) = t.srv("/v1/server/accounts", json!({ "username": "erin" })).await;
    let (_, v) = t.srv("/v1/server/signup", json!({ "username": "erin", "passphrase": pass, "invite_code": inv["invite_code"] })).await;
    let key = v["recovery_key"].as_str().unwrap().to_string();
    let tail: String = key.chars().filter(|c| c.is_ascii_alphanumeric()).rev().take(8).collect::<Vec<_>>().into_iter().rev().collect();
    // Signing in asks to remember this phone and is handed its device key, once.
    let (c, v) = t.srv("/v1/server/signup/confirm", json!({ "username": "erin", "passphrase": pass, "tail": tail, "label": "Pixel", "enroll": true })).await;
    assert_eq!(c, StatusCode::OK, "{v}");
    let (tok, dev_id, dev_key) = (v["token"].as_str().unwrap().to_string(), v["device"]["id"].as_str().unwrap().to_string(), v["device"]["key"].as_str().unwrap().to_string());
    assert_eq!(dev_key.len(), 64);
    let (_, made) = t.call("POST", "/v1/threads", Some(&tok), Some(json!({}))).await;
    assert!(made["id"].is_string());

    // The server's database holds the data key only wrapped: the device key itself is nowhere on the server.
    let server_db = String::from_utf8_lossy(&std::fs::read(t.dir.join("server.db")).unwrap()).to_string();
    assert!(!server_db.contains(&dev_key), "the device key must stay on the phone");

    // A restart: a new Hub on the same folder. The account is locked until something unlocks it.
    let t2 = {
        let cfg = t.hub.cfg.clone(); // the same settings and folder, a fresh process (nothing unlocked)
        let hub = Hub::open(cfg, Box::new(|_| Ok(Arc::new(agent_core::model::Mock::new(&[]))))).unwrap();
        let session = hub.admin_login("boss", "the boss password").unwrap()["session"].as_str().unwrap().to_string(); // sessions end with a restart
        T { router: hub_http::router(hub.clone()), hub, dir: t.dir.clone(), server: t.server.clone(), session }
    };
    assert_eq!(t2.call("GET", "/v1/threads", Some(&tok), None).await.0, StatusCode::LOCKED);
    // A wrong device key does nothing; the right one unlocks, and the same token works again with the data intact.
    let post = |body: Value| t2.call("POST", "/v1/server/unlock", None, Some(body));
    assert_eq!(post(json!({ "device_id": dev_id, "key": "0".repeat(64) })).await.0, StatusCode::UNAUTHORIZED);
    assert_eq!(post(json!({ "device_id": "nope", "key": dev_key })).await.0, StatusCode::UNAUTHORIZED);
    assert_eq!(post(json!({ "device_id": dev_id, "key": dev_key })).await.0, StatusCode::OK);
    let (c, list) = t2.call("GET", "/v1/threads", Some(&tok), None).await;
    assert_eq!((c, list["items"].as_array().map(|a| a.len())), (StatusCode::OK, Some(1)), "{list}");

    // Signing out for good on the phone makes that key useless; so does a recovery.
    t2.hub.lock(tok.split_once('.').unwrap().0);
    assert_eq!(t2.call("POST", "/v1/server/unlock/forget", None, Some(json!({ "device_id": dev_id, "key": dev_key }))).await.0, StatusCode::OK);
    assert_eq!(post(json!({ "device_id": dev_id, "key": dev_key })).await.0, StatusCode::UNAUTHORIZED, "forgotten");
    let (_, v) = t2.srv("/v1/server/login", json!({ "username": "erin", "passphrase": pass, "enroll": true })).await;
    let (id2, key2) = (v["device"]["id"].as_str().unwrap().to_string(), v["device"]["key"].as_str().unwrap().to_string());
    t2.hub.lock(tok.split_once('.').unwrap().0);
    t2.srv("/v1/server/recover", json!({ "username": "erin", "recovery_key": key, "new_passphrase": "erin picked a new phrase" })).await;
    assert_eq!(post(json!({ "device_id": id2, "key": key2 })).await.0, StatusCode::UNAUTHORIZED, "a recovery forgets every remembered phone");
}

#[tokio::test(flavor = "multi_thread")]
async fn invite_codes_switch_off_phones_and_the_admin_dashboard() {
    let t = T::new("admin");
    let get = |p: &'static str| t.call("GET", p, Some(&t.server), None);

    // Sign-up needs the invite code: the server password and the name are not enough.
    let (_, inv) = t.srv("/v1/server/accounts", json!({ "username": "dana" })).await;
    let (id, code) = (inv["id"].as_str().unwrap().to_string(), inv["invite_code"].as_str().unwrap().to_string());
    assert_eq!(code.len(), 14, "three groups of four: {code}");
    for bad in [json!({ "username": "dana", "passphrase": "dana has a long phrase" }), json!({ "username": "dana", "passphrase": "dana has a long phrase", "invite_code": "AAAA-BBBB-CCCC" })] {
        let (c, v) = t.srv("/v1/server/signup", bad).await;
        assert_eq!(c, StatusCode::BAD_REQUEST);
        assert!(v["error"]["message"].as_str().unwrap().contains("invite code"), "{v}");
    }
    // A new code makes the old one useless.
    let (c, v) = t.srv(&format!("/v1/server/accounts/{id}/invite-code"), json!({})).await;
    assert_eq!(c, StatusCode::OK, "{v}");
    let code2 = v["invite_code"].as_str().unwrap().to_string();
    assert_ne!(code, code2);
    assert_eq!(t.srv("/v1/server/signup", json!({ "username": "dana", "passphrase": "dana has a long phrase", "invite_code": code })).await.0, StatusCode::BAD_REQUEST);
    assert_eq!(t.srv("/v1/server/signup", json!({ "username": "dana", "passphrase": "dana has a long phrase", "invite_code": code2 })).await.0, StatusCode::OK);

    // A full account, signed in on a remembered phone.
    let (tok, _) = t.make("eve", "eve has a long phrase").await;
    let eid = tok.split_once('.').unwrap().0.to_string();
    assert_eq!(t.srv(&format!("/v1/server/accounts/{eid}/invite-code"), json!({})).await.0, StatusCode::BAD_REQUEST, "only before set-up");
    let (_, v) = t.srv("/v1/server/login", json!({ "username": "eve", "passphrase": "eve has a long phrase", "enroll": true, "label": "Pixel" })).await;
    let dev = v["device"]["id"].as_str().unwrap().to_string();
    let (_, d) = t.call("GET", &format!("/v1/server/accounts/{eid}/devices"), Some(&t.server), None).await;
    assert_eq!(d["items"][0]["id"], dev.as_str());
    assert_eq!(d["items"][0]["label"], "Pixel");
    assert_eq!(t.call("DELETE", &format!("/v1/server/accounts/{eid}/devices/{dev}"), Some(&t.server), None).await.0, StatusCode::OK);
    assert_eq!(t.call("DELETE", &format!("/v1/server/accounts/{eid}/devices/{dev}"), Some(&t.server), None).await.0, StatusCode::BAD_REQUEST, "already gone");

    // Switched off: the token gets 403, sign-in is refused, the data stays; switched on, sign-in works again.
    assert_eq!(t.srv(&format!("/v1/server/accounts/{eid}/disable"), json!({})).await.0, StatusCode::OK);
    let (c, v) = t.call("GET", "/v1/threads", Some(&tok), None).await;
    assert_eq!((c, v["error"]["code"].as_str()), (StatusCode::FORBIDDEN, Some("account_disabled")));
    assert_eq!(t.srv("/v1/server/login", json!({ "username": "eve", "passphrase": "eve has a long phrase" })).await.0, StatusCode::UNAUTHORIZED);
    assert_eq!(t.srv(&format!("/v1/server/accounts/{eid}/disable"), json!({})).await.0, StatusCode::BAD_REQUEST, "already off");
    assert_eq!(t.srv(&format!("/v1/server/accounts/{eid}/enable"), json!({})).await.0, StatusCode::OK);
    assert_eq!(t.srv("/v1/server/login", json!({ "username": "eve", "passphrase": "eve has a long phrase" })).await.0, StatusCode::OK);

    // The dashboard numbers and the admin log.
    let (c, o) = get("/v1/server/overview").await;
    assert_eq!(c, StatusCode::OK);
    assert_eq!(o["accounts"]["total"], 2);
    assert_eq!(o["accounts"]["active"], 1);
    assert_eq!(o["accounts"]["pending"], 1);
    assert!(o["disk_bytes"].as_u64().unwrap() > 0, "{o}");
    let (_, list) = get("/v1/server/accounts").await;
    let eve = list["items"].as_array().unwrap().iter().find(|a| a["username"] == "eve").unwrap().clone();
    assert!(eve["disk_bytes"].as_u64().unwrap() > 0 && eve["invite_expires"].is_null(), "{eve}");
    let (_, log) = get("/v1/server/log").await;
    let actions: Vec<&str> = log["items"].as_array().unwrap().iter().map(|l| l["action"].as_str().unwrap()).collect();
    for a in ["invite", "invite_code", "signup", "revoke_device", "disable", "enable"] {
        assert!(actions.contains(&a), "{a} missing from {actions:?}");
    }
    assert_eq!(log["items"][0]["action"], "enable", "newest first");
    assert_eq!(log["items"][0]["username"], "eve");
    let db = String::from_utf8_lossy(&std::fs::read(t.dir.join("server.db")).unwrap()).to_string();
    assert!(!db.contains(&code2), "invite codes are kept only as a hash");

    // None of it without the server password.
    for p in ["/v1/server/overview", "/v1/server/log"] {
        assert_eq!(t.call("GET", p, Some(&tok), None).await.0, StatusCode::UNAUTHORIZED);
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn the_admin_dashboard_is_a_page_on_the_server_and_may_call_its_own_routes() {
    let t = T::new("adminpage");
    let get = |uri: &str, extra: &[(&str, &str)]| {
        let mut r = Request::builder().uri(uri).header("host", "127.0.0.1:9999");
        for (k, v) in extra {
            r = r.header(*k, *v);
        }
        t.router.clone().oneshot(r.body(Body::empty()).unwrap())
    };
    // The page: HTML, never cached, never framed, and it holds no data.
    let r = get("/admin", &[]).await.unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    let hv = |n: &str| r.headers().get(n).and_then(|h| h.to_str().ok()).unwrap_or("").to_string();
    assert!(hv("content-type").starts_with("text/html"));
    assert_eq!((hv("cache-control"), hv("x-frame-options")), ("no-store".into(), "DENY".into()));
    assert!(hv("content-security-policy").contains("connect-src 'self'") && hv("content-security-policy").contains("frame-ancestors 'none'"));
    assert!(hv("content-security-policy").contains("script-src 'sha256-") && !hv("content-security-policy").contains("script-src 'unsafe-inline'"), "#27: only the page's own script runs");
    let body = String::from_utf8(to_bytes(r.into_body(), 1 << 20).await.unwrap().to_vec()).unwrap();
    assert!(body.contains("Server admin") && body.contains("/v1/server/overview"));
    // Its own calls carry this server's origin: let through, with no cross-origin headers. The password is still needed.
    let own = [("origin", "http://127.0.0.1:9999")];
    let r = get("/v1/server/accounts", &own).await.unwrap();
    assert_eq!((r.status(), r.headers().get("access-control-allow-origin").is_some()), (StatusCode::UNAUTHORIZED, false));
    let auth = format!("Bearer {}", t.server);
    let r = get("/v1/server/overview", &[own[0], ("authorization", auth.as_str()), ("x-admin-session", t.session.as_str())]).await.unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    // Another host's page, or this port on another name, is still refused.
    for o in ["http://127.0.0.1:9998", "https://evil.example", "http://127.0.0.1:9999.evil.example"] {
        assert_eq!(get("/v1/server/accounts", &[("origin", o)]).await.unwrap().status(), StatusCode::FORBIDDEN, "{o}");
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn managing_accounts_needs_the_admin_login_as_well_as_the_server_access_code() {
    let t = T::new("adminlogin");
    let (router, server) = (t.router.clone(), t.server.clone());
    let send = |method: &str, path: &str, session: Option<&str>, body: Value| {
        let router = router.clone();
        let mut r = Request::builder().method(method).uri(path).header("host", "127.0.0.1:9999").header("authorization", format!("Bearer {server}")).header("content-type", "application/json");
        if let Some(s) = session {
            r = r.header("x-admin-session", s);
        }
        let req = r.body(Body::from(body.to_string())).unwrap();
        async move {
            let resp = router.oneshot(req).await.unwrap();
            let st = resp.status();
            (st, serde_json::from_slice::<Value>(&to_bytes(resp.into_body(), 1 << 20).await.unwrap()).unwrap_or(Value::Null))
        }
    };
    // The code alone: people can still sign up and sign in, but cannot see or change accounts.
    for (m, p) in [("GET", "/v1/server/accounts"), ("GET", "/v1/server/overview"), ("GET", "/v1/server/log"), ("POST", "/v1/server/accounts")] {
        let (c, v) = send(m, p, None, json!({ "username": "eve" })).await;
        assert_eq!((c, v["error"]["code"].as_str()), (StatusCode::UNAUTHORIZED, Some("admin_login")), "{p}");
    }
    assert_eq!(send("GET", "/v1/server/info", None, json!({})).await.0, StatusCode::OK);
    assert_eq!(send("GET", "/v1/server/admin/status", None, json!({})).await.1["admin"], true);
    // A made-up session does not work; a wrong password does not give one.
    assert_eq!(send("GET", "/v1/server/accounts", Some("adm_nope"), json!({})).await.0, StatusCode::UNAUTHORIZED);
    assert_eq!(send("POST", "/v1/server/admin/login", None, json!({ "username": "boss", "password": "wrong password!!" })).await.0, StatusCode::UNAUTHORIZED);
    // The right login gives a session that works, until sign-out.
    let (c, v) = send("POST", "/v1/server/admin/login", None, json!({ "username": "boss", "password": "the boss password" })).await;
    assert_eq!(c, StatusCode::OK);
    let s = v["session"].as_str().unwrap().to_string();
    assert_eq!(send("GET", "/v1/server/accounts", Some(&s), json!({})).await.0, StatusCode::OK);
    // The session alone, without the code, is not enough.
    let r = t.router.clone().oneshot(Request::builder().uri("/v1/server/accounts").header("host", "127.0.0.1:9999").header("x-admin-session", &s).body(Body::empty()).unwrap()).await.unwrap();
    assert_eq!(r.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(send("POST", "/v1/server/admin/logout", Some(&s), json!({})).await.0, StatusCode::OK);
    assert_eq!(send("GET", "/v1/server/accounts", Some(&s), json!({})).await.0, StatusCode::UNAUTHORIZED);
    // Changing the admin password ends the other sessions and needs the current one.
    let (_, v) = send("POST", "/v1/server/admin/login", None, json!({ "username": "boss", "password": "the boss password" })).await;
    let s = v["session"].as_str().unwrap().to_string();
    assert_eq!(send("POST", "/v1/server/admin/password", Some(&s), json!({ "old": "not it at all", "new": "a brand new password" })).await.0, StatusCode::BAD_REQUEST);
    let (c, v) = send("POST", "/v1/server/admin/password", Some(&s), json!({ "old": "the boss password", "new": "a brand new password" })).await;
    assert_eq!(c, StatusCode::OK);
    assert_eq!(send("GET", "/v1/server/accounts", Some(&t.session), json!({})).await.0, StatusCode::UNAUTHORIZED, "old sessions end");
    assert_eq!(send("GET", "/v1/server/accounts", Some(v["session"].as_str().unwrap()), json!({})).await.0, StatusCode::OK);
}

#[tokio::test(flavor = "multi_thread")]
async fn the_first_admin_login_is_made_with_the_one_time_setup_code() {
    let cfg = template("adminsetup");
    let dir = cfg.data_dir.clone();
    let hub = Hub::open(cfg, Box::new(|_| Ok(Arc::new(agent_core::model::Mock::new(&[]))))).unwrap();
    assert!(!hub.admin_exists());
    let (code, path) = hub.ensure_admin_setup().unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap().trim(), code);
    assert!(hub.admin_setup("AAAA-BBBB-CCCC", "boss", "the boss password").is_err(), "wrong code");
    assert!(hub.admin_setup(&code, "boss", "short").is_err(), "short password");
    let v = hub.admin_setup(&code.to_lowercase(), "boss", "the boss password").unwrap();
    assert!(v["session"].as_str().unwrap().starts_with("adm_"));
    assert!(!path.exists(), "the code file is removed once used");
    assert!(hub.admin_setup(&code, "eve", "eve's own password").is_err(), "only once");
    assert!(hub.ensure_admin_setup().is_none());
    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test(flavor = "multi_thread")]
async fn security_review_fixes_hold() {
    let t = T::new("secfix");
    // #16: the server token is kept as Argon2id, never as a fast SHA-256.
    assert!(!String::from_utf8_lossy(&std::fs::read(t.dir.join("server.db")).unwrap()).starts_with("SQLite format 3"), "#10: server.db is encrypted");
    let c = agent_core::db::open_server(&t.dir.join("server.db")).unwrap();
    let stored: Vec<String> = c.prepare("SELECT key FROM meta WHERE key LIKE 'token_%'").unwrap().query_map([], |r| r.get(0)).unwrap().flatten().collect();
    assert_eq!(stored, vec!["token_argon".to_string()], "only the slow hash is stored");
    drop(c);

    // #12: a new passphrase ends every sign-in token at once.
    let (tok, _) = t.make("dana", "dana's first passphrase").await;
    assert_eq!(t.call("GET", "/v1/threads", Some(&tok), None).await.0, StatusCode::OK);
    let (c, v) = t.srv("/v1/server/passphrase", json!({ "username": "dana", "old": "dana's first passphrase", "new": "dana's second passphrase" })).await;
    assert_eq!(c, StatusCode::OK, "{v}");
    assert_eq!(t.call("GET", "/v1/threads", Some(&tok), None).await.0, StatusCode::UNAUTHORIZED, "the old sign-in is over");

    // #3, #4: plain copies left by older versions are moved into the database and deleted on unlock.
    let id = tok.split_once('.').unwrap().0.to_string();
    let acct = t.dir.join("accounts").join(&id);
    std::fs::create_dir_all(acct.join("memory/People")).unwrap();
    std::fs::write(acct.join("memory/core.md"), "Dana likes tea.\n").unwrap();
    std::fs::write(acct.join("memory/People/x.md"), "secret").unwrap();
    std::fs::create_dir_all(acct.join("reports")).unwrap();
    std::fs::write(acct.join("reports/rep_old.json"), json!({ "id": "rep_old", "created": "2026-01-01T00:00:00Z", "auto": false, "thread": {} }).to_string()).unwrap();
    std::fs::create_dir_all(acct.join("evals")).unwrap();
    std::fs::write(acct.join("evals/2026-01-01-0000.json"), json!({ "id": "2026-01-01-0000", "started": "x" }).to_string()).unwrap();
    assert!(t.hub.lock(&id));
    let (c, v) = t.srv("/v1/server/login", json!({ "username": "dana", "passphrase": "dana's second passphrase" })).await;
    assert_eq!(c, StatusCode::OK, "{v}");
    let tok = v["token"].as_str().unwrap().to_string();
    assert!(!acct.join("memory").exists(), "no plain memory folder");
    assert!(!acct.join("reports").exists(), "no plain reports folder");
    assert!(!acct.join("evals").exists(), "no plain model-test folder");
    // #1: an account used just now is not idle; one past the limit is locked and its key leaves memory.
    assert!(t.hub.lock_idle(3600).is_empty(), "just used");
    assert_eq!(t.hub.lock_idle(-1), vec![id.clone()]);
    assert_eq!(t.call("GET", "/v1/threads", Some(&tok), None).await.0, StatusCode::LOCKED);
    let (_, v) = t.srv("/v1/server/login", json!({ "username": "dana", "passphrase": "dana's second passphrase" })).await;
    let tok = v["token"].as_str().unwrap().to_string();
    let (_, runs) = t.call("GET", "/v1/evals", Some(&tok), None).await;
    assert_eq!(runs["items"][0]["id"], "2026-01-01-0000", "the old run moved into the database");
    let (_, reps) = t.call("GET", "/v1/reports", Some(&tok), None).await;
    assert_eq!(reps["items"][0]["id"], "rep_old", "the old report moved into the database");
}

#[tokio::test(flavor = "multi_thread")]
async fn the_admin_login_can_ask_for_an_authenticator_code() {
    // Security review #20.
    let t = T::new("totp");
    let (c, v) = t.srv("/v1/server/admin/totp", json!({ "action": "start" })).await;
    assert_eq!(c, StatusCode::OK, "{v}");
    let secret = v["secret"].as_str().unwrap().to_string();
    assert!(v["uri"].as_str().unwrap().starts_with("otpauth://totp/"));
    let now = || std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64;
    let code = |step: i64| agent_core::totp::code_at(&agent_core::totp::decode(&secret).unwrap(), step);
    let (c, _) = t.srv("/v1/server/admin/totp", json!({ "action": "confirm", "code": "000000" })).await;
    assert_ne!(c, StatusCode::OK, "a wrong code does not turn it on");
    let s0 = now() / 30;
    let (c, v) = t.srv("/v1/server/admin/totp", json!({ "action": "confirm", "code": code(s0) })).await;
    assert_eq!(c, StatusCode::OK, "{v}");
    let (_, st) = t.call("GET", "/v1/server/admin/status", Some(&t.server), None).await;
    assert_eq!(st["code"], true);
    let login = |code: String| t.call("POST", "/v1/server/admin/login", Some(&t.server), Some(json!({ "username": "boss", "password": "the boss password", "code": code })));
    assert_eq!(login(String::new()).await.0, StatusCode::UNAUTHORIZED, "the password alone is not enough");
    assert_eq!(login(code(s0)).await.0, StatusCode::UNAUTHORIZED, "the code used to switch it on is spent");
    assert_eq!(login(code(s0 + 1)).await.0, StatusCode::OK, "the next code works");
    assert_eq!(login(code(s0 + 1)).await.0, StatusCode::UNAUTHORIZED, "a code works once");
    // The server's own shell can always reset the login, codes included.
    Hub::set_admin_password(&t.dir, "boss", "the boss password").unwrap();
    assert_eq!(login(String::new()).await.0, StatusCode::OK);
}
