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
        .route("/v1/health", get(|State(h): State<H>| async move { Json(json!({ "ok": true, "accounts": true, "version": env!("CARGO_PKG_VERSION"), "setup": h.list().is_empty() })) }))
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
        .route("/v1/server/unlock", post(unlock_device))
        .route("/v1/server/unlock/forget", post(forget_device))
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
    if let Some(o) = &origin
        && !hub.cfg.allowed_origins.iter().any(|a| a == o)
    {
        return Err(bad(StatusCode::FORBIDDEN, "forbidden", "Origin not allowed."));
    }
    Ok(origin.and_then(|o| HeaderValue::from_str(&o).ok()))
}

fn with_cors(mut r: Response, origin: Option<HeaderValue>) -> Response {
    if let Some(o) = origin {
        let h = r.headers_mut();
        h.insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, o);
        h.insert(header::ACCESS_CONTROL_ALLOW_HEADERS, HeaderValue::from_static("authorization, content-type, last-event-id"));
        h.insert(header::ACCESS_CONTROL_ALLOW_METHODS, HeaderValue::from_static("GET, POST, PATCH, PUT, DELETE"));
        h.insert(header::VARY, HeaderValue::from_static("Origin"));
    }
    r
}

/// Who is calling: the peer address when the server provides it (the real server does), else one shared "local" name.
fn caller(req: &Request) -> String {
    req.extensions().get::<axum::extract::ConnectInfo<std::net::SocketAddr>>().map(|c| c.0.ip().to_string()).unwrap_or_else(|| "local".into())
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
    hub.token_ok_from(&who);
    let body = match axum::body::to_bytes(req.into_body(), 64 * 1024).await {
        Ok(b) => serde_json::from_slice::<Value>(&b).unwrap_or(Value::Null),
        Err(_) => Value::Null,
    };
    let r = tokio::task::spawn_blocking(move || f(hub, body)).await;
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
    serve(h, req, |h, _| Ok(json!({ "items": h.list() }))).await
}
async fn invite(State(h): State<H>, req: Request) -> Response {
    serve(h, req, |h, b| h.invite(s(&b, "username")).map_err(invalid)).await
}
async fn remove(State(h): State<H>, Path(id): Path<String>, req: Request) -> Response {
    serve(h, req, move |h, _| h.delete(&id).map(|_| json!({ "deleted": id })).map_err(invalid)).await
}
async fn lock(State(h): State<H>, Path(id): Path<String>, req: Request) -> Response {
    serve(h, req, move |h, _| Ok(json!({ "locked": h.lock(&id) }))).await
}
async fn signup(State(h): State<H>, req: Request) -> Response {
    serve(h, req, |h, b| h.signup(s(&b, "username"), s(&b, "passphrase")).map_err(invalid)).await
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
    serve(h, req, move |h, _| Ok(json!({ "revoked": h.revoke_devices(&id) }))).await
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
        let locked = hub.is_active(acct);
        if !locked {
            hub.token_missed(&who); // an account that does not exist
        }
        return with_cors(
            if locked { bad(StatusCode::LOCKED, "account_locked", "This account is locked. Sign in again to unlock it.") } else { bad(StatusCode::UNAUTHORIZED, "unauthorized", "Missing or wrong token.") },
            origin,
        );
    };
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
