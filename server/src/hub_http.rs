//! The front door for accounts (docs/15): server routes under `/v1/server/` (they need the server
//! token), and everything else goes to the one account named in the token (`<id>.<secret>`).
//! A locked account answers 423 `account_locked`.

use crate::hub::Hub;
use axum::extract::{Path, Request, State};
use axum::middleware::{self, Next};
use axum::http::{HeaderValue, Method, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use serde_json::{Value, json};
use std::sync::Arc;
use tower::ServiceExt;

type H = Arc<Hub>;

fn bad(code: StatusCode, c: &str, msg: &str) -> Response {
    (code, Json(json!({ "error": { "code": c, "message": msg } }))).into_response()
}

pub fn router(hub: H) -> Router {
    Router::new()
        .route("/v1/health", get(|State(h): State<H>| async move { Json(json!({ "ok": true, "name": "AssistX", "accounts": true, "version": env!("CARGO_PKG_VERSION"), "setup": h.list().is_empty() })) }))
        .route("/v1/server/info", get(info))
        .route("/v1/server/accounts", get(accounts).post(invite))
        .route("/v1/server/accounts/{id}", delete(remove))
        .route("/v1/server/accounts/{id}/lock", post(lock))
        .route("/v1/server/signup", post(signup))
        .route("/v1/server/signup/confirm", post(confirm))
        .route("/v1/server/login", post(login))
        .route("/v1/server/recover", post(recover))
        .route("/v1/server/passphrase", post(passphrase))
        .route("/v1/server/accounts/{id}/revoke-devices", post(revoke_devices))
        .route("/v1/server/accounts/{id}/disable", post(disable))
        .route("/v1/server/accounts/{id}/enable", post(enable))
        .route("/v1/server/accounts/{id}/invite-code", post(invite_code))
        .route("/v1/server/accounts/{id}/devices", get(devices))
        .route("/v1/server/accounts/{id}/devices/{device}", delete(revoke_device))
        .route("/v1/server/overview", get(overview))
        .route("/v1/server/log", get(admin_log))
        .route("/v1/server/unlock", post(unlock_device))
        .route("/v1/server/unlock/forget", post(forget_device))
        .route("/v1/server/admin/status", get(admin_status))
        .route("/v1/server/admin/setup", post(admin_setup))
        .route("/v1/server/admin/login", post(admin_login))
        .route("/v1/server/admin/logout", post(admin_logout))
        .route("/v1/server/admin/password", post(admin_password))
        .route("/admin", get(admin_page))
        .fallback(dispatch)
        .layer(middleware::from_fn_with_state(hub.clone(), cross_origin))
        .with_state(hub)
}

/// For every route, the health answer included: check Host and Origin, answer the browser's pre-check (OPTIONS)
/// with 204, and add the cross-origin headers the app's page needs to read any answer.
async fn cross_origin(State(hub): State<H>, req: Request, next: Next) -> Response {
    let origin = match gate(&hub, &req) {
        Ok(o) => o,
        Err(r) => return r,
    };
    if req.method() == Method::OPTIONS {
        return with_cors(StatusCode::NO_CONTENT.into_response(), origin);
    }
    with_cors(next.run(req).await, origin)
}

/// Host and Origin checks (an account's own router repeats them for its routes). Gives back the Origin to echo.
fn gate(hub: &Hub, req: &Request) -> Result<Option<HeaderValue>, Response> {
    let host_ok = req.headers().get(header::HOST).and_then(|h| h.to_str().ok()).is_some_and(|h| hub.cfg.host_ok(h));
    if !host_ok {
        return Err(bad(StatusCode::FORBIDDEN, "forbidden", "Wrong Host header."));
    }
    let origin = req.headers().get(header::ORIGIN).and_then(|h| h.to_str().ok()).map(String::from);
    if let Some(o) = &origin {
        // The server's own page (the admin dashboard at /admin) calls it from its own origin: no cross-origin headers.
        if same_origin(req, o) {
            return Ok(None);
        }
        if !hub.cfg.allowed_origins.iter().any(|a| a == o) {
            return Err(bad(StatusCode::FORBIDDEN, "forbidden", "Origin not allowed."));
        }
    }
    Ok(origin.and_then(|o| HeaderValue::from_str(&o).ok()))
}

/// The Origin is this server itself: the scheme plus the Host that was just checked.
fn same_origin(req: &Request, origin: &str) -> bool {
    let host = req.headers().get(header::HOST).and_then(|h| h.to_str().ok()).unwrap_or("");
    !host.is_empty() && origin.strip_prefix("http://").or_else(|| origin.strip_prefix("https://")) == Some(host)
}

/// `GET /admin`: the server's admin dashboard (docs/architecture/accounts.md). The page holds no data; every call it
/// makes needs the server password, which the admin types in and the page keeps only in memory.
async fn admin_page() -> Response {
    (
        [
            (header::CONTENT_TYPE, "text/html; charset=utf-8"),
            (header::CACHE_CONTROL, "no-store"),
            (header::X_FRAME_OPTIONS, "DENY"),
            (header::REFERRER_POLICY, "no-referrer"),
            (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
            (
                header::CONTENT_SECURITY_POLICY,
                admin_csp(),
            ),
        ],
        include_str!("../web/admin.html"),
    )
        .into_response()
}

/// The admin page's policy: only its own script runs, named by its SHA-256 (security review #27), never any
/// inline script an attacker could slip in. Styles stay inline (they cannot run code).
fn admin_csp() -> &'static str {
    static CSP: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    CSP.get_or_init(|| {
        let page = include_str!("../web/admin.html");
        let script = page.split_once("<script>").and_then(|(_, rest)| rest.split_once("</script>")).map(|(s, _)| s).unwrap_or("");
        let digest = crate::vault::from_hex(&crate::util::sha256_hex(script.as_bytes())).unwrap_or_default();
        format!(
            "default-src 'none'; script-src 'sha256-{}'; style-src 'unsafe-inline'; connect-src 'self'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'",
            crate::util::base64(&digest)
        )
    })
}

fn with_cors(mut r: Response, origin: Option<HeaderValue>) -> Response {
    if let Some(o) = origin {
        let h = r.headers_mut();
        h.insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, o);
        h.insert(header::ACCESS_CONTROL_ALLOW_HEADERS, HeaderValue::from_static("authorization, content-type, last-event-id, x-admin-session"));
        h.insert(header::ACCESS_CONTROL_ALLOW_METHODS, HeaderValue::from_static("GET, POST, PATCH, PUT, DELETE"));
        h.insert(header::VARY, HeaderValue::from_static("Origin"));
    }
    r
}

/// Who is calling: the peer address when the server provides it (the real server does), else one shared "local" name.
/// Behind a proxy on this same computer (for example `tailscale serve`), every caller would be 127.0.0.1 and one
/// person's typos would slow down everyone (security review #18). With `AGENT_TRUST_PROXY=1`, and only when the peer
/// is loopback, the first `X-Forwarded-For` address is used instead.
fn caller(req: &Request) -> String {
    let peer = req.extensions().get::<axum::extract::ConnectInfo<std::net::SocketAddr>>().map(|c| c.0.ip());
    if peer.is_some_and(|ip| ip.is_loopback()) && std::env::var("AGENT_TRUST_PROXY").is_ok_and(|v| v == "1")
        && let Some(f) = req.headers().get("x-forwarded-for").and_then(|h| h.to_str().ok()).and_then(|v| v.split(',').next())
        && let Ok(ip) = f.trim().parse::<std::net::IpAddr>()
    {
        return ip.to_string();
    }
    peer.map(|ip| ip.to_string()).unwrap_or_else(|| "local".into())
}

/// 429 with how long to wait.
fn slow_down(wait: i64) -> Response {
    let mut r = bad(StatusCode::TOO_MANY_REQUESTS, "rate_limited", &format!("Too many wrong tries. Wait {wait} seconds and try again."));
    if let Ok(v) = HeaderValue::from_str(&wait.to_string()) {
        r.headers_mut().insert(header::RETRY_AFTER, v);
    }
    r
}

fn bearer(req: &Request) -> Option<String> {
    req.headers().get(header::AUTHORIZATION).and_then(|h| h.to_str().ok()).and_then(|h| h.strip_prefix("Bearer ")).map(String::from)
}

type Out = Result<Value, (StatusCode, String)>;

/// A server route: host and origin checks, the server token, then the work off the async threads (Argon2 is heavy).
async fn serve<F>(hub: H, req: Request, f: F) -> Response
where
    F: FnOnce(Arc<Hub>, Value) -> Out + Send + 'static,
{
    serve_as(hub, req, false, f).await
}

/// An admin route: everything `serve` checks, plus a live admin session (`x-admin-session`, see `hub_admin.rs`).
async fn admin<F>(hub: H, req: Request, f: F) -> Response
where
    F: FnOnce(Arc<Hub>, Value) -> Out + Send + 'static,
{
    serve_as(hub, req, true, f).await
}

async fn serve_as<F>(hub: H, req: Request, need_admin: bool, f: F) -> Response
where
    F: FnOnce(Arc<Hub>, Value) -> Out + Send + 'static,
{
    let origin = match gate(&hub, &req) {
        Ok(o) => o,
        Err(r) => return r,
    };
    if req.method() == Method::OPTIONS {
        return with_cors(StatusCode::NO_CONTENT.into_response(), origin);
    }
    let who = caller(&req);
    if let Some(wait) = hub.blocked_for(&who) {
        return with_cors(slow_down(wait), origin);
    }
    let token = bearer(&req);
    if !token.as_deref().is_some_and(|t| hub.server_token_ok(t)) {
        if token.is_some() {
            hub.token_missed(&who); // a wrong token counts; no token at all does not
        }
        return with_cors(bad(StatusCode::UNAUTHORIZED, "unauthorized", "Missing or wrong server token."), origin);
    }
    if need_admin {
        let session = req.headers().get("x-admin-session").and_then(|h| h.to_str().ok()).map(String::from);
        if !session.as_deref().is_some_and(|t| hub.admin_session_ok(t)) {
            if session.is_some() {
                hub.token_missed(&who); // a guessed or ended session counts as a wrong token
            }
            return with_cors(bad(StatusCode::UNAUTHORIZED, "admin_login", "Sign in with the admin login first."), origin);
        }
    }
    hub.token_ok_from(&who);
    let body = match axum::body::to_bytes(req.into_body(), 64 * 1024).await {
        Ok(b) => serde_json::from_slice::<Value>(&b).unwrap_or(Value::Null),
        Err(_) => Value::Null,
    };
    let r = tokio::task::spawn_blocking(move || {
        crate::hub::CALLER.with(|c| *c.borrow_mut() = who);
        let out = f(hub, body);
        crate::hub::CALLER.with(|c| c.borrow_mut().clear());
        out
    }).await;
    with_cors(
        match r {
            Ok(Ok(v)) => Json(v).into_response(),
            Ok(Err((code, m))) => bad(code, "invalid", &m),
            Err(_) => bad(StatusCode::INTERNAL_SERVER_ERROR, "internal", "Something went wrong."),
        },
        origin,
    )
}

fn s<'a>(v: &'a Value, k: &str) -> &'a str {
    v[k].as_str().unwrap_or("")
}

fn invalid(e: String) -> (StatusCode, String) {
    (StatusCode::BAD_REQUEST, e)
}

async fn info(State(h): State<H>, req: Request) -> Response {
    serve(h, req, |h, _| Ok(json!({ "ok": true, "accounts": h.list().len(), "version": env!("CARGO_PKG_VERSION") }))).await
}
async fn accounts(State(h): State<H>, req: Request) -> Response {
    admin(h, req, |h, _| Ok(json!({ "items": h.list() }))).await
}
async fn invite(State(h): State<H>, req: Request) -> Response {
    admin(h, req, |h, b| h.invite(s(&b, "username")).map_err(invalid)).await
}
async fn remove(State(h): State<H>, Path(id): Path<String>, req: Request) -> Response {
    admin(h, req, move |h, _| h.delete(&id).map(|_| json!({ "deleted": id })).map_err(invalid)).await
}
async fn lock(State(h): State<H>, Path(id): Path<String>, req: Request) -> Response {
    admin(h, req, move |h, _| {
        let locked = h.lock(&id);
        if locked {
            h.log_admin("lock", &id, "");
        }
        Ok(json!({ "locked": locked }))
    }).await
}
async fn disable(State(h): State<H>, Path(id): Path<String>, req: Request) -> Response {
    admin(h, req, move |h, _| h.set_enabled(&id, false).map_err(invalid)).await
}
async fn enable(State(h): State<H>, Path(id): Path<String>, req: Request) -> Response {
    admin(h, req, move |h, _| h.set_enabled(&id, true).map_err(invalid)).await
}
async fn invite_code(State(h): State<H>, Path(id): Path<String>, req: Request) -> Response {
    admin(h, req, move |h, _| h.new_invite_code(&id).map_err(invalid)).await
}
async fn devices(State(h): State<H>, Path(id): Path<String>, req: Request) -> Response {
    admin(h, req, move |h, _| Ok(json!({ "items": h.devices(&id) }))).await
}
async fn revoke_device(State(h): State<H>, Path((id, device)): Path<(String, String)>, req: Request) -> Response {
    admin(h, req, move |h, _| h.revoke_device(&id, &device).map_err(invalid)).await
}
async fn overview(State(h): State<H>, req: Request) -> Response {
    admin(h, req, |h, _| Ok(h.overview())).await
}
async fn admin_log(State(h): State<H>, req: Request) -> Response {
    let limit = req.uri().query().and_then(|q| q.split('&').find_map(|kv| kv.strip_prefix("limit="))).and_then(|n| n.parse().ok()).unwrap_or(50);
    admin(h, req, move |h, _| Ok(json!({ "items": h.admin_log(limit) }))).await
}
async fn admin_status(State(h): State<H>, req: Request) -> Response {
    serve(h, req, |h, _| Ok(json!({ "admin": h.admin_exists() }))).await
}
async fn admin_setup(State(h): State<H>, req: Request) -> Response {
    serve(h, req, |h, b| h.admin_setup(s(&b, "setup_code"), s(&b, "username"), s(&b, "password")).map_err(|e| (StatusCode::UNAUTHORIZED, e))).await
}
async fn admin_login(State(h): State<H>, req: Request) -> Response {
    serve(h, req, |h, b| h.admin_login(s(&b, "username"), s(&b, "password")).map_err(|e| (StatusCode::UNAUTHORIZED, e))).await
}
async fn admin_logout(State(h): State<H>, req: Request) -> Response {
    let t = req.headers().get("x-admin-session").and_then(|h| h.to_str().ok()).unwrap_or("").to_string();
    admin(h, req, move |h, _| { h.admin_logout(&t); Ok(json!({ "signed_out": true })) }).await
}
async fn admin_password(State(h): State<H>, req: Request) -> Response {
    admin(h, req, |h, b| h.admin_change_password(s(&b, "old"), s(&b, "new")).map_err(invalid)).await
}
async fn signup(State(h): State<H>, req: Request) -> Response {
    serve(h, req, |h, b| h.signup(s(&b, "username"), s(&b, "passphrase"), s(&b, "invite_code")).map_err(invalid)).await
}
async fn confirm(State(h): State<H>, req: Request) -> Response {
    serve(h, req, |h, b| h.confirm(s(&b, "username"), s(&b, "passphrase"), s(&b, "tail"), s(&b, "label"), b["enroll"] == true).map_err(invalid)).await
}
async fn login(State(h): State<H>, req: Request) -> Response {
    serve(h, req, |h, b| h.login(s(&b, "username"), s(&b, "passphrase"), s(&b, "label"), b["enroll"] == true).map_err(|e| (StatusCode::UNAUTHORIZED, e))).await
}
async fn recover(State(h): State<H>, req: Request) -> Response {
    serve(h, req, |h, b| h.recover(s(&b, "username"), s(&b, "recovery_key"), s(&b, "new_passphrase")).map_err(invalid)).await
}
async fn passphrase(State(h): State<H>, req: Request) -> Response {
    serve(h, req, |h, b| h.change_passphrase(s(&b, "username"), s(&b, "old"), s(&b, "new")).map_err(invalid)).await
}

async fn revoke_devices(State(h): State<H>, Path(id): Path<String>, req: Request) -> Response {
    admin(h, req, move |h, _| {
        let n = h.revoke_devices(&id);
        h.log_admin("revoke_devices", &id, &n.to_string());
        Ok(json!({ "revoked": n }))
    }).await
}

/// A remembered phone asks to unlock its account (after a server restart) or to be forgotten. The phone's device key
/// is the proof, a random 256-bit secret only it holds, so no server password is needed. Wrong keys are slowed down.
async fn device_call(hub: H, req: Request, forget: bool) -> Response {
    let origin = match gate(&hub, &req) {
        Ok(o) => o,
        Err(r) => return r,
    };
    if req.method() == Method::OPTIONS {
        return with_cors(StatusCode::NO_CONTENT.into_response(), origin);
    }
    let who = caller(&req);
    if let Some(wait) = hub.blocked_for(&who) {
        return with_cors(slow_down(wait), origin);
    }
    let body = match axum::body::to_bytes(req.into_body(), 4096).await {
        Ok(b) => serde_json::from_slice::<Value>(&b).unwrap_or(Value::Null),
        Err(_) => Value::Null,
    };
    let h2 = hub.clone();
    let r = tokio::task::spawn_blocking(move || if forget { h2.forget_device(s(&body, "device_id"), s(&body, "key")) } else { h2.unlock_device(s(&body, "device_id"), s(&body, "key")) }).await;
    with_cors(
        match r {
            Ok(Ok(v)) => {
                hub.token_ok_from(&who);
                Json(v).into_response()
            }
            Ok(Err(m)) => {
                hub.token_missed(&who);
                bad(StatusCode::UNAUTHORIZED, "unauthorized", &m)
            }
            Err(_) => bad(StatusCode::INTERNAL_SERVER_ERROR, "internal", "Something went wrong."),
        },
        origin,
    )
}
async fn unlock_device(State(h): State<H>, req: Request) -> Response {
    device_call(h, req, false).await
}
async fn forget_device(State(h): State<H>, req: Request) -> Response {
    device_call(h, req, true).await
}

/// Everything else belongs to one account.
async fn dispatch(State(hub): State<H>, mut req: Request) -> Response {
    let origin = match gate(&hub, &req) {
        Ok(o) => o,
        Err(r) => return r,
    };
    if req.method() == Method::OPTIONS {
        return with_cors(StatusCode::NO_CONTENT.into_response(), origin);
    }
    let who = caller(&req);
    if let Some(wait) = hub.blocked_for(&who) {
        return with_cors(slow_down(wait), origin);
    }
    let token = bearer(&req);
    // The app's update files are the server's, not an account's. The server token may fetch them, so a phone that is
    // signed out, or still holds the old one-owner token (which is the server token), can still update itself.
    if req.method() == Method::GET && matches!(req.uri().path(), "/v1/app/version" | "/v1/app/apk") && token.as_deref().is_some_and(|t| hub.server_token_ok(t)) {
        let path = req.uri().path().to_string();
        let dir = hub.cfg.data_dir.join("app");
        let r = tokio::task::spawn_blocking(move || app_files(&dir, &path)).await.unwrap_or_else(|_| bad(StatusCode::INTERNAL_SERVER_ERROR, "internal", "Something went wrong."));
        return with_cors(r, origin);
    }
    let Some((acct, secret)) = token.as_deref().and_then(|t| t.split_once('.')) else {
        return with_cors(bad(StatusCode::UNAUTHORIZED, "unauthorized", "Sign in to an account first."), origin);
    };
    let router = hub.routers.read().unwrap().get(acct).cloned();
    let Some(router) = router else {
        let r = match hub.status_of(acct).as_deref() {
            Some("active") => bad(StatusCode::LOCKED, "account_locked", "This account is locked. Sign in again to unlock it."),
            Some("disabled") => bad(StatusCode::FORBIDDEN, "account_disabled", "This account is switched off. Ask the server's admin."),
            _ => {
                hub.token_missed(&who); // an account that does not exist
                bad(StatusCode::UNAUTHORIZED, "unauthorized", "Missing or wrong token.")
            }
        };
        return with_cors(r, origin);
    };
    hub.touch(acct);
    if let Ok(v) = HeaderValue::from_str(&format!("Bearer {secret}")) {
        req.headers_mut().insert(header::AUTHORIZATION, v);
    }
    let r = match router.oneshot(req).await {
        Ok(r) => r,
        Err(e) => match e {},
    };
    if r.status() == StatusCode::UNAUTHORIZED {
        hub.token_missed(&who); // the account is real but the secret is wrong
    }
    r
}

/// `GET /v1/app/version` and `/v1/app/apk` from `<server dir>/app/` (what `assistantx-app-release` publishes).
fn app_files(dir: &std::path::Path, path: &str) -> Response {
    let (Ok(meta), Ok(apk)) = (std::fs::read_to_string(dir.join("version.json")), std::fs::read(dir.join("assistant.apk"))) else {
        return bad(StatusCode::NOT_FOUND, "not_found", "App update not found.");
    };
    if path == "/v1/app/apk" {
        return ([(header::CONTENT_TYPE, "application/vnd.android.package-archive")], apk).into_response();
    }
    let m: Value = serde_json::from_str(&meta).unwrap_or(Value::Null);
    let history: Value = std::fs::read_to_string(dir.join("changelog.json")).ok().and_then(|s| serde_json::from_str::<Vec<Value>>(&s).ok())
        .map(|mut v| { v.sort_by_key(|x| std::cmp::Reverse(x["versionCode"].as_i64().unwrap_or(0))); v.truncate(15); Value::Array(v) }).unwrap_or(Value::Array(vec![]));
    Json(json!({ "versionCode": m["versionCode"], "versionName": m["versionName"], "notes": m["notes"], "history": history, "size": apk.len(), "sha256": crate::util::sha256_hex(&apk) })).into_response()
}
