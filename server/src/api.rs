//! HTTP API, `/v1` (docs/01-api.md). JSON for commands, SSE for events.

use crate::auth::{self, Caller, Scope};
use crate::engine::{self, AnswerError};
use crate::events::{self, Event};
use crate::{App, gate, util};
use axum::extract::{Path, Query, Request, State};
use axum::http::{HeaderMap, HeaderValue, Method, StatusCode, header};
use axum::middleware::{self, Next};
use axum::response::sse::{Event as SseEvent, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Extension, Json, Router};
use futures_util::stream::{self, Stream};
use rusqlite::{OptionalExtension, params};
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::VecDeque;
use std::convert::Infallible;
use std::sync::Arc;
use tokio::sync::broadcast;

type AppState = Arc<App>;

// ---------------- errors ----------------

pub struct ApiError(StatusCode, &'static str, String);

impl ApiError {
    fn new(s: StatusCode, code: &'static str, msg: impl Into<String>) -> Self {
        ApiError(s, code, msg.into())
    }
    fn not_found(what: &str) -> Self {
        Self::new(StatusCode::NOT_FOUND, "not_found", format!("{what} not found."))
    }
    fn invalid(msg: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, "invalid_input", msg)
    }
    fn scope() -> Self {
        Self::new(StatusCode::FORBIDDEN, "forbidden_scope", "This needs an owner token.")
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.0, Json(json!({ "error": { "code": self.1, "message": self.2 } }))).into_response()
    }
}

type ApiResult = Result<Response, ApiError>;

fn ok(v: Value) -> ApiResult {
    Ok(Json(v).into_response())
}

fn need_owner(c: &Caller) -> Result<(), ApiError> {
    if c.scope == Scope::Owner { Ok(()) } else { Err(ApiError::scope()) }
}

// ---------------- router + guard ----------------

pub fn router(app: AppState) -> Router {
    Router::new()
        .route("/v1/health", get(health))
        .route("/v1/threads", post(create_thread).get(list_threads))
        .route("/v1/threads/{id}", get(get_thread).patch(patch_thread).delete(archive_thread))
        .route("/v1/threads/{id}/messages", get(list_messages).post(send_message))
        .route("/v1/threads/{id}/cancel", post(cancel_turn))
        .route("/v1/threads/{id}/report", post(report_thread))
        .route("/v1/reports", get(list_reports))
        .route("/v1/reports/{id}", get(get_report))
        .route("/v1/evals", get(list_evals))
        .route("/v1/evals/run", post(run_evals))
        .route("/v1/evals/{id}", get(get_eval))
        .route("/v1/threads/{id}/events", get(thread_events))
        .route("/v1/events", get(all_events))
        .route("/v1/turns/{id}", get(get_turn))
        .route("/v1/approvals", get(list_approvals))
        .route("/v1/approvals/{id}", get(get_approval).post(answer_approval))
        .route("/v1/actions", get(list_actions))
        .route("/v1/actions/{id}", get(get_action))
        .route("/v1/actions/{id}/undo", post(undo_action))
        .route("/v1/plugins", get(list_plugins))
        .route("/v1/plugins/install", post(install_plugin))
        .route("/v1/plugins/{id}", get(get_plugin).delete(uninstall_plugin))
        .route("/v1/plugins/{id}/versions", get(plugin_versions))
        .route("/v1/plugins/{id}/diff", get(plugin_diff))
        .route("/v1/plugins/{id}/approve", post(approve_plugin))
        .route("/v1/plugins/{id}/reject", post(reject_plugin))
        .route("/v1/plugins/{id}/disable", post(disable_plugin))
        .route("/v1/plugins/{id}/rollback", post(rollback_plugin))
        .route("/v1/memory", get(search_memory).post(add_memory))
        .route("/v1/memory/core", get(memory_core))
        .route("/v1/memory/folders", get(memory_folders).post(make_memory_folder))
        .route("/v1/memory/graph", get(memory_graph))
        .route("/v1/memory/clear", post(clear_memory))
        .route("/v1/memory/index", get(memory_index))
        .route("/v1/memory/{id}", get(get_memory).patch(edit_memory).delete(erase_memory))
        .route("/v1/memory/{id}/history", get(memory_history))
        .route("/v1/memory/{id}/forget", post(forget_memory))
        .route("/v1/recipes", get(list_recipes))
        .route("/v1/recipes/{id}", axum::routing::delete(delete_recipe))
        .route("/v1/oauth/{provider}", get(oauth_status).delete(oauth_disconnect))
        .route("/v1/oauth/{provider}/start", post(oauth_start))
        .route("/v1/oauth/{provider}/finish", post(oauth_finish))
        .route("/v1/app/version", get(app_version))
        .route("/v1/app/apk", get(app_apk))
        .route("/v1/ambient/chunks", post(ambient_upload).get(ambient_chunks).delete(ambient_forget))
        .route("/v1/ambient/status", get(ambient_status))
        .route("/v1/ambient/settings", get(ambient_get_settings).patch(ambient_set_settings))
        .route("/v1/ambient/pause", post(ambient_pause))
        .route("/v1/ambient/resume", post(ambient_resume))
        .route("/v1/ambient/process", post(ambient_process))
        .route("/v1/ambient/picks", get(ambient_picks))
        .route("/v1/ambient/picks/{id}", post(ambient_pick_act))
        .route("/v1/dreaming/run", post(run_dreaming))
        .route("/v1/dreaming/runs", get(dreaming_runs))
        .route("/v1/dreaming/runs/{id}", get(dreaming_run))
        .route("/v1/jobs", get(list_jobs).post(create_job))
        .route("/v1/jobs/{id}", axum::routing::delete(cancel_job).patch(update_job).get(get_job))
        .route("/v1/jobs/{id}/pause", post(pause_job))
        .route("/v1/jobs/{id}/resume", post(resume_job))
        .route("/v1/jobs/{id}/run", post(run_job))
        .route("/v1/automations/notify", post(automation_notify))
        .route("/v1/automations/watch", get(automation_watch))
        .route("/v1/alarms/ringing", get(alarms_ringing))
        .route("/v1/alarms/snooze", post(snooze_alarm))
        .route("/v1/alarms/dismiss", post(dismiss_alarm))
        .route("/v1/notices", get(list_notices))
        .route("/v1/notices/{n}/snooze", post(snooze_notice))
        .route("/v1/notices/{n}/feedback", post(notice_feedback))
        .route("/v1/audit/review", get(audit_review))
        .route("/v1/power", get(get_power).put(set_power).post(set_power))
        .route("/v1/audit/turns/{id}", get(audit_turn))
        .route("/v1/speech", get(speech_status).post(speak))
        .route("/v1/status", get(server_status))
        .route("/v1/usage", get(get_usage))
        .route("/v1/security", get(get_security))
        .route("/v1/sounds", get(list_sounds))
        .route("/v1/sounds/{name}", get(get_sound).put(put_sound).delete(delete_sound).layer(axum::extract::DefaultBodyLimit::max(crate::sounds::MAX_BYTES + 1024)))
        .route("/v1/calendar/events", get(list_events).post(add_event))
        .route("/v1/calendar/events/{id}", get(get_event).patch(update_event).delete(delete_event))
        .route("/v1/calendar/events/{id}/skip", post(skip_event_day))
        .route("/v1/jobs/{id}/skip", post(skip_job_day))
        .route("/v1/location/points", post(location_upload).get(location_points))
        .route("/v1/location/now", get(location_now))
        .route("/v1/location/fences", get(location_fences))
        .route("/v1/location/transitions", post(location_transition))
        .route("/v1/health/days", post(health_upload).get(health_days))
        .route("/v1/location/visits", get(location_visits))
        .route("/v1/places", get(list_places).post(make_place))
        .route("/v1/places/{id}", get(get_place).patch(rename_place).delete(forget_place))
        .route("/v1/nodes", get(list_nodes).post(add_node))
        .route("/v1/models", get(get_models).patch(patch_models))
        .route("/v1/nodes/work/poll", post(node_poll))
        .route("/v1/nodes/work/{id}", post(node_answer))
        .route("/v1/phone/actions", get(phone_actions))
        .route("/v1/phone/actions/{id}", post(phone_action_answer))
        .route("/v1/secrets", get(list_secrets))
        .route("/v1/secrets/{name}", axum::routing::put(set_secret).delete(delete_secret))
        .route("/v1/triggers/{source}", post(trigger))
        .route("/v1/export", get(export))
        .route("/v1/config", get(get_config).patch(patch_config))
        .route("/v1/tokens", get(list_tokens).post(create_token))
        .route("/v1/tokens/{id}", axum::routing::delete(revoke_token))
        .merge(crate::lists::routes())
        .merge(crate::detect::routes())
        .merge(crate::signals::router())
        .fallback(|| async { ApiError::not_found("Route") })
        .layer(middleware::from_fn_with_state(app.clone(), guard))
        .with_state(app)
}

/// Host check (DNS rebinding), Origin allowlist (CSRF), bearer token on every route but health.
async fn guard(State(app): State<AppState>, mut req: Request, next: Next) -> Response {
    let headers = req.headers().clone();
    let host_ok = headers.get(header::HOST).and_then(|h| h.to_str().ok()).is_some_and(|h| app.cfg.host_ok(h));
    if !host_ok {
        return ApiError::new(StatusCode::FORBIDDEN, "forbidden", "Wrong Host header.").into_response();
    }
    let origin = headers.get(header::ORIGIN).and_then(|h| h.to_str().ok()).map(String::from);
    if let Some(o) = &origin
        && !app.cfg.allowed_origins.iter().any(|a| a == o)
    {
        return ApiError::new(StatusCode::FORBIDDEN, "forbidden", "Origin not allowed.").into_response();
    }
    let cors = |mut r: Response| {
        if let Some(o) = &origin {
            let h = r.headers_mut();
            if let Ok(v) = HeaderValue::from_str(o) {
                h.insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, v);
            }
            h.insert(header::ACCESS_CONTROL_ALLOW_HEADERS, HeaderValue::from_static("authorization, content-type, last-event-id"));
            h.insert(header::ACCESS_CONTROL_ALLOW_METHODS, HeaderValue::from_static("GET, POST, PATCH, PUT, DELETE"));
            h.insert(header::VARY, HeaderValue::from_static("Origin"));
        }
        r
    };
    if req.method() == Method::OPTIONS {
        return cors(StatusCode::NO_CONTENT.into_response());
    }
    if req.uri().path() != "/v1/health" {
        let token = headers.get(header::AUTHORIZATION).and_then(|h| h.to_str().ok()).and_then(|h| h.strip_prefix("Bearer "));
        let Some(caller) = token.and_then(|t| auth::check_token(&app.db, t)) else {
            return cors(ApiError::new(StatusCode::UNAUTHORIZED, "unauthorized", "Missing or wrong token.").into_response());
        };
        // Trigger tokens may only reach /v1/triggers/{source} (not built yet).
        if caller.scope == Scope::Trigger && !req.uri().path().starts_with("/v1/triggers/") {
            return cors(ApiError::scope().into_response());
        }
        // A device token (the phone's recorder) may upload text and read capture state, nothing else.
        let (method, path) = (req.method().clone(), req.uri().path().to_string());
        let device_ok = (method == Method::POST && matches!(path.as_str(), "/v1/ambient/chunks" | "/v1/location/points" | "/v1/location/transitions" | "/v1/health/days" | "/v1/automations/notify" | "/v1/signals"))
            || (method == Method::GET && matches!(path.as_str(), "/v1/automations/watch" | "/v1/location/fences" | "/v1/ambient/status" | "/v1/ambient/settings" | "/v1/app/version" | "/v1/app/apk"))
            || (method == Method::GET && path == "/v1/phone/actions") || (method == Method::POST && path.starts_with("/v1/phone/actions/"))
            || (method == Method::POST && path == "/v1/detected") || (method == Method::GET && path == "/v1/detected/settings");
        if caller.scope == Scope::Device && !device_ok {
            return cors(ApiError::scope().into_response());
        }
        // A node token (a computer running the node program) may only fetch work and answer it.
        if caller.scope == Scope::Node && !path.starts_with("/v1/nodes/work") {
            return cors(ApiError::scope().into_response());
        }
        if !crate::power::is_on(&app) && !crate::power::allowed_while_off(method.as_str(), &path) {
            return cors(ApiError::new(StatusCode::SERVICE_UNAVAILABLE, "power_off", "AssistX is switched off. Turn it on in Settings.").into_response());
        }
        let ua = headers.get(header::USER_AGENT).and_then(|h| h.to_str().ok()).unwrap_or("");
        auth::touch_token(&app.db, &caller.token_id, ua);
        req.extensions_mut().insert(caller);
    }
    cors(next.run(req).await)
}

// ---------------- paging ----------------

#[derive(Deserialize)]
struct Page {
    limit: Option<i64>,
    cursor: Option<String>,
    status: Option<String>,
    thread_id: Option<String>,
    plugin: Option<String>,
}

impl Page {
    fn limit(&self) -> i64 {
        self.limit.unwrap_or(50).clamp(1, 200)
    }
    fn after(&self) -> i64 {
        self.cursor.as_deref().and_then(|c| c.parse().ok()).unwrap_or(0)
    }
}

/// Wrap rows as `{items, next_cursor}` where the cursor is the last row's `n`.
fn paged(rows: Vec<(i64, Value)>, limit: i64) -> Value {
    let next = if rows.len() as i64 == limit { rows.last().map(|r| r.0.to_string()) } else { None };
    json!({ "items": rows.into_iter().map(|r| r.1).collect::<Vec<_>>(), "next_cursor": next })
}

// ---------------- health, config, plugins ----------------

async fn health(State(app): State<AppState>) -> ApiResult {
    let model = if app.is_routed() { format!("routed:{}", crate::models::settings(&app)["mode"].as_str().unwrap_or("auto")) } else { app.model.name() };
    ok(json!({ "ok": true, "version": env!("CARGO_PKG_VERSION"), "model": model }))
}

async fn get_models(State(app): State<AppState>) -> ApiResult {
    ok(crate::models::describe(&app))
}

/// Switch models: `{"mode": "auto" | profile, "cloud": "nim" | "gemini" | "ovh", "cloud_upgrade": "ask" | "auto" | "never", "set": {profile: {model}}}`. Owner only.
async fn patch_models(State(app): State<AppState>, Extension(c): Extension<Caller>, Json(b): Json<Value>) -> ApiResult {
    need_owner(&c)?;
    crate::models::update(&app, &b).map(|v| Json(v).into_response()).map_err(ApiError::invalid)
}

async fn get_config(State(app): State<AppState>) -> ApiResult {
    let mut v = serde_json::to_value(&app.cfg).unwrap_or(Value::Null);
    v["model"]["api_key_set"] = json!(app.cfg.model.api_key.is_some());
    v["model"]["local"] = json!(app.cfg.model.is_local());
    v["plugin_problems"] = json!(app.manifest_problems());
    let autonomy: Vec<Value> = {
        let c = app.db.lock();
        let mut st = c.prepare("SELECT plugin, tool, level, spend_cap FROM user_autonomy ORDER BY plugin, tool").expect("prepare");
        st.query_map([], |r| Ok(json!({ "plugin": r.get::<_, String>(0)?, "tool": r.get::<_, String>(1)?, "level": r.get::<_, String>(2)?, "spend_cap": r.get::<_, Option<f64>>(3)? })))
            .map(|x| x.flatten().collect()).unwrap_or_default()
    };
    v["autonomy"] = json!(autonomy);
    ok(v)
}

/// Owner only. Today: per-tool autonomy (docs/01 Autonomy, step 2 "user config").
/// `{"autonomy": [{"plugin": "files", "tool": "write", "level": "auto"}]}`; `level: null` clears.
async fn patch_config(State(app): State<AppState>, Extension(c): Extension<Caller>, Json(b): Json<Value>) -> ApiResult {
    need_owner(&c)?;
    let obj = b.as_object().ok_or_else(|| ApiError::invalid("body must be an object"))?;
    if let Some(k) = obj.keys().find(|k| *k != "autonomy") {
        return Err(ApiError::invalid(format!("'{k}' cannot be changed live yet; set it in the environment")));
    }
    for item in b["autonomy"].as_array().into_iter().flatten() {
        let plugin = item["plugin"].as_str().unwrap_or_default();
        let tool = item["tool"].as_str().unwrap_or_default();
        let m = app.manifest(plugin).ok_or_else(|| ApiError::invalid(format!("no plugin '{plugin}'")))?;
        let decl = m.tool(tool).ok_or_else(|| ApiError::invalid(format!("no tool '{plugin}.{tool}'")))?;
        if item["level"].is_null() {
            let _ = app.db.lock().execute("DELETE FROM user_autonomy WHERE plugin = ?1 AND tool = ?2", [plugin, tool]);
            continue;
        }
        let level = item["level"].as_str().and_then(gate::Autonomy::parse).ok_or_else(|| ApiError::invalid("level must be auto, ask, never or null"))?;
        let cap = item["spend_cap"].as_f64();
        if level == gate::Autonomy::Auto && decl.tags.iter().any(|t| t == "spends") && cap.is_none() {
            return Err(ApiError::invalid("a `spends` tool can be auto only with a spend_cap"));
        }
        if level == gate::Autonomy::Auto && decl.tags.iter().any(|t| t == gate::UNRESTRICTED) {
            return Err(ApiError::invalid("an `unrestricted` tool (full command access) always asks; it cannot be auto"));
        }
        if level == gate::Autonomy::Auto && gate::always_asks(decl) {
            return Err(ApiError::invalid("this tool always asks (like sending mail); it cannot be auto"));
        }
        let lv = serde_json::to_value(level).ok().and_then(|v| v.as_str().map(String::from)).unwrap_or_default();
        let _ = app.db.lock().execute("INSERT OR REPLACE INTO user_autonomy (plugin, tool, level, spend_cap) VALUES (?1, ?2, ?3, ?4)", params![plugin, tool, lv, cap]);
    }
    get_config(State(app)).await
}

fn plugin_json(app: &App, id: &str) -> Option<Value> {
    let m = app.manifest(id)?;
    let actions = app.registry.actions(id);
    let backend = crate::runner::backend(app, &m);
    let tools: Vec<Value> = m.tools.iter().map(|t| {
        let a = actions.get(t.name.as_str());
        json!({
            "name": t.name, "tags": t.tags, "requested": t.autonomy.as_deref().unwrap_or("ask"),
            "effective": engine::effective_level(app, id, Some(t)), "risk": gate::risk(Some(t)),
            "undoable": t.undo.is_some(), "desc": a.map(|a| a.desc), "params": a.map(|a| &a.params),
            "implemented": match &backend { Some(crate::runner::Backend::Rust) => a.is_some(), Some(_) => true, None => false },
        })
    }).collect();
    let state = crate::store::state_json(app, id);
    Some(json!({
        "id": m.id, "version": m.version, "summary": m.summary, "needs": m.needs,
        "status": state["status"], "usable": app.plugin_usable(id), "provenance": state["provenance"], "created_by": state["created_by"],
        "has_draft": crate::store::draft_dir(app, id).join("plugin.yaml").exists(), "tools": tools,
    }))
}

async fn list_plugins(State(app): State<AppState>) -> ApiResult {
    let items: Vec<Value> = app.manifests().keys().filter_map(|id| plugin_json(&app, id)).collect();
    ok(json!({ "items": items, "problems": app.manifest_problems() }))
}

// ---------------- tokens ----------------

async fn list_tokens(State(app): State<AppState>, Extension(c): Extension<Caller>) -> ApiResult {
    need_owner(&c)?;
    ok(json!({ "items": auth::list_tokens(&app.db) }))
}

#[derive(Deserialize)]
struct NewToken {
    scope: String,
    #[serde(default)]
    label: String,
}

async fn create_token(State(app): State<AppState>, Extension(c): Extension<Caller>, Json(b): Json<NewToken>) -> ApiResult {
    need_owner(&c)?;
    let scope = Scope::parse(&b.scope).ok_or_else(|| ApiError::invalid("scope must be client, owner, trigger, device or node"))?;
    let (id, token) = auth::create_token(&app.db, scope, &util::clean_line(&b.label, 12));
    Ok((StatusCode::CREATED, Json(json!({ "id": id, "scope": scope, "token": token, "note": "Shown once. Only a hash is stored." }))).into_response())
}

async fn revoke_token(State(app): State<AppState>, Extension(c): Extension<Caller>, Path(id): Path<String>) -> ApiResult {
    need_owner(&c)?;
    if id == c.token_id {
        return Err(ApiError::new(StatusCode::CONFLICT, "conflict", "You cannot revoke the token you are using."));
    }
    if auth::revoke_token(&app.db, &id) { ok(json!({ "ok": true })) } else { Err(ApiError::not_found("Token")) }
}

// ---------------- threads ----------------

fn thread_json(app: &App, id: &str) -> Option<Value> {
    app.db.lock().query_row(
        "SELECT id, title, kind, created, archived, model FROM threads WHERE id = ?1", [id],
        |r| Ok(json!({ "id": r.get::<_, String>(0)?, "title": r.get::<_, String>(1)?, "kind": r.get::<_, String>(2)?,
                       "created": r.get::<_, String>(3)?, "archived": r.get::<_, i64>(4)? != 0, "model": r.get::<_, Option<String>>(5)? })),
    ).optional().ok().flatten()
}

#[derive(Deserialize, Default)]
struct NewThread {
    #[serde(default)]
    title: String,
    /// Who answers in this chat; empty = the default.
    #[serde(default)]
    model: Option<String>,
}

async fn create_thread(State(app): State<AppState>, Extension(c): Extension<Caller>, body: Option<Json<NewThread>>) -> ApiResult {
    let b = body.map(|b| b.0).unwrap_or_default();
    let title = util::clean_line(&b.title, 20);
    let model = check_chat_model(&app, &c, b.model.as_deref())?;
    let id = util::new_id("thr");
    app.db.lock().execute("INSERT INTO threads (id, title, created, model) VALUES (?1, ?2, ?3, ?4)", params![id, title, util::now_iso(), model])
        .map_err(|e| ApiError::invalid(e.to_string()))?;
    Ok((StatusCode::CREATED, Json(thread_json(&app, &id).unwrap_or(Value::Null))).into_response())
}

async fn list_threads(State(app): State<AppState>, Query(p): Query<Page>) -> ApiResult {
    let rows: Vec<(i64, Value)> = {
        let c = app.db.lock();
        let mut st = c.prepare("SELECT rowid, id, title, kind, created, archived, model FROM threads WHERE rowid > ?1 ORDER BY rowid LIMIT ?2").expect("prepare");
        st.query_map(params![p.after(), p.limit()], |r| Ok((r.get(0)?, json!({
            "id": r.get::<_, String>(1)?, "title": r.get::<_, String>(2)?, "kind": r.get::<_, String>(3)?,
            "created": r.get::<_, String>(4)?, "archived": r.get::<_, i64>(5)? != 0, "model": r.get::<_, Option<String>>(6)? }))))
            .map(|x| x.flatten().collect()).unwrap_or_default()
    };
    ok(paged(rows, p.limit()))
}

async fn get_thread(State(app): State<AppState>, Path(id): Path<String>) -> ApiResult {
    let mut v = thread_json(&app, &id).ok_or_else(|| ApiError::not_found("Thread"))?;
    // A long chat's summary (src/summary.rs): the model sees it instead of the older messages.
    v["summary"] = crate::summary::get(&app.db, &id).map(|s| crate::summary::to_json(&s)).unwrap_or(Value::Null);
    Ok(Json(v).into_response())
}

/// A chat's own model. Empty or null = follow the default. A cloud model sends the chat to that
/// service, so only the owner may pin one (Auto (Mix) still asks first).
fn check_chat_model(app: &App, c: &Caller, m: Option<&str>) -> Result<Option<String>, ApiError> {
    let Some(m) = m.map(str::trim).filter(|m| !m.is_empty() && *m != "default") else { return Ok(None) };
    let m = if m == "auto" { "auto-mix" } else { m };
    if !crate::models::valid_mode(app, m) {
        return Err(ApiError::invalid("model is default, auto-local, auto-mix, auto-smart or a profile id from GET /v1/models"));
    }
    if crate::models::get(app, m).is_some_and(|p| !p.local) {
        need_owner(c)?;
    }
    Ok(Some(m.to_string()))
}

/// PATCH /v1/threads/{id}: `{model}` (or `{title}`).
async fn patch_thread(State(app): State<AppState>, Extension(c): Extension<Caller>, Path(id): Path<String>, Json(b): Json<Value>) -> ApiResult {
    let t = thread_json(&app, &id).ok_or_else(|| ApiError::not_found("Thread"))?;
    if t["kind"] != "user" {
        return Err(ApiError::invalid("only your own chats can be changed"));
    }
    if b.get("model").is_some() {
        let model = check_chat_model(&app, &c, b["model"].as_str())?;
        let _ = app.db.lock().execute("UPDATE threads SET model = ?1 WHERE id = ?2", params![model, id]);
    }
    if let Some(title) = b["title"].as_str() {
        let _ = app.db.lock().execute("UPDATE threads SET title = ?1 WHERE id = ?2", params![util::clean_line(title, 20), id]);
    }
    ok(thread_json(&app, &id).unwrap_or(Value::Null))
}

/// Archive only. The raw log is kept (docs/01).
async fn archive_thread(State(app): State<AppState>, Path(id): Path<String>) -> ApiResult {
    let n = app.db.lock().execute("UPDATE threads SET archived = 1 WHERE id = ?1", [&id]).unwrap_or(0);
    if n == 0 { return Err(ApiError::not_found("Thread")); }
    app.cancel_running(&id);
    ok(json!({ "ok": true }))
}

async fn list_messages(State(app): State<AppState>, Path(id): Path<String>, Query(p): Query<Page>) -> ApiResult {
    thread_json(&app, &id).ok_or_else(|| ApiError::not_found("Thread"))?;
    let rows: Vec<(i64, Value)> = {
        let c = app.db.lock();
        let mut st = c.prepare("SELECT n, id, turn_id, role, trust, body, created, model FROM messages WHERE thread_id = ?1 AND n > ?2 ORDER BY n LIMIT ?3").expect("prepare");
        st.query_map(params![id, p.after(), p.limit()], |r| {
            let body: String = r.get(5)?;
            Ok((r.get(0)?, json!({ "id": r.get::<_, String>(1)?, "turn_id": r.get::<_, Option<String>>(2)?,
                "role": r.get::<_, String>(3)?, "trust": r.get::<_, String>(4)?,
                "body": serde_json::from_str::<Value>(&body).unwrap_or(Value::Null), "created": r.get::<_, String>(6)?,
                "model": r.get::<_, Option<String>>(7)? })))
        }).map(|x| x.flatten().collect()).unwrap_or_default()
    };
    ok(paged(rows, p.limit()))
}

#[derive(Deserialize)]
struct NewMessage {
    text: String,
    #[serde(default)]
    interrupt: bool,
    client_msg_id: Option<String>,
    /// The user is talking out loud (the voice screen): the model is told, its words are spoken, and it may ask to listen.
    #[serde(default)]
    live: bool,
}

async fn send_message(State(app): State<AppState>, Extension(c): Extension<Caller>, Path(id): Path<String>, Json(b): Json<NewMessage>) -> ApiResult {
    let t = thread_json(&app, &id).ok_or_else(|| ApiError::not_found("Thread"))?;
    if t["archived"] == true {
        return Err(ApiError::new(StatusCode::CONFLICT, "conflict", "This thread is archived."));
    }
    if b.text.trim().is_empty() || b.text.len() > 100_000 {
        return Err(ApiError::invalid("text must be 1 to 100000 bytes"));
    }
    if !app.rate_ok(&c.token_id, app.cfg.limits.messages_per_minute) {
        return Err(ApiError::new(StatusCode::TOO_MANY_REQUESTS, "rate_limited", "Too many messages. Wait a minute."));
    }
    // Same client_msg_id = same turn (safe retries).
    if let Some(cid) = &b.client_msg_id {
        let existing: Option<String> = app.db.lock()
            .query_row("SELECT id FROM turns WHERE thread_id = ?1 AND client_msg_id = ?2", params![id, cid], |r| r.get(0))
            .optional().ok().flatten();
        if let Some(turn) = existing {
            return Ok((StatusCode::ACCEPTED, Json(json!({ "turn_id": turn, "queued": false, "duplicate": true }))).into_response());
        }
    }
    if b.interrupt {
        app.cancel_running(&id);
    }
    let turn = util::new_id("turn");
    app.db.lock().execute(
        "INSERT INTO turns (id, thread_id, source, status, client_msg_id, input, created) VALUES (?1, ?2, 'message', 'queued', ?3, ?4, ?5)",
        params![turn, id, b.client_msg_id, json!({ "text": b.text, "live": b.live }).to_string(), util::now_iso()],
    ).map_err(|e| ApiError::invalid(e.to_string()))?;
    let queued = app.enqueue(&id, &turn);
    if queued {
        app.feed.emit(Some(&id), Some(&turn), "turn.queued", json!({}));
    }
    Ok((StatusCode::ACCEPTED, Json(json!({ "turn_id": turn, "queued": queued }))).into_response())
}

/// "Send log": save everything about this chat to `data/reports/` for review (src/reports.rs).
async fn report_thread(State(app): State<AppState>, Extension(c): Extension<Caller>, Path(id): Path<String>, body: Option<Json<Value>>) -> ApiResult {
    let note = body.as_ref().and_then(|b| b["note"].as_str()).unwrap_or("").to_string();
    match crate::reports::create(&app, &id, &note, &c.token_id) {
        Ok(v) => Ok((StatusCode::CREATED, Json(v)).into_response()),
        Err(_) => Err(ApiError::not_found("Thread")),
    }
}

/// Reports hold whole chats with tool results, so only the owner may read them back.
async fn list_reports(State(app): State<AppState>, Extension(c): Extension<Caller>, Query(q): Query<std::collections::HashMap<String, String>>) -> ApiResult {
    need_owner(&c)?;
    let only_auto = q.get("auto").is_some_and(|v| v == "1" || v == "true");
    ok(json!({ "items": crate::reports::list(&app, only_auto) }))
}

async fn get_report(State(app): State<AppState>, Extension(c): Extension<Caller>, Path(id): Path<String>) -> ApiResult {
    need_owner(&c)?;
    crate::reports::get(&app, &id).map(|v| Json(v).into_response()).ok_or_else(|| ApiError::not_found("Report"))
}

/// Model tests (src/eval.rs, docs/12). Owner only: they run real turns, and cloud ones cost money.
async fn run_evals(State(app): State<AppState>, Extension(c): Extension<Caller>, body: Option<Json<Value>>) -> ApiResult {
    need_owner(&c)?;
    let mut opts: crate::eval::Opts = body.map(|b| serde_json::from_value(b.0).unwrap_or_default()).unwrap_or_default();
    opts.cases = None; // only the shipped cases from the API
    match crate::eval::start(&app, opts) {
        Ok(plan) => Ok((StatusCode::ACCEPTED, Json(plan)).into_response()),
        Err(e) => Err(ApiError::new(StatusCode::CONFLICT, "conflict", e)),
    }
}

async fn list_evals(State(app): State<AppState>, Extension(c): Extension<Caller>) -> ApiResult {
    need_owner(&c)?;
    ok(crate::eval::list(&app))
}

async fn get_eval(State(app): State<AppState>, Extension(c): Extension<Caller>, Path(id): Path<String>) -> ApiResult {
    need_owner(&c)?;
    crate::eval::get(&app, &id).map(|v| Json(v).into_response()).ok_or_else(|| ApiError::not_found("Model test"))
}

async fn cancel_turn(State(app): State<AppState>, Path(id): Path<String>) -> ApiResult {
    thread_json(&app, &id).ok_or_else(|| ApiError::not_found("Thread"))?;
    ok(json!({ "cancelled": app.cancel_running(&id) }))
}

async fn get_turn(State(app): State<AppState>, Path(id): Path<String>) -> ApiResult {
    app.db.lock().query_row(
        "SELECT id, thread_id, source, status, created, finished, error_code, error_message, tokens_used, COALESCE(json_extract(input, '$.listen'), 0) FROM turns WHERE id = ?1", [&id],
        |r| Ok(json!({ "id": r.get::<_, String>(0)?, "thread_id": r.get::<_, String>(1)?, "source": r.get::<_, String>(2)?,
            "status": r.get::<_, String>(3)?, "created": r.get::<_, String>(4)?, "finished": r.get::<_, Option<String>>(5)?,
            "error": r.get::<_, Option<String>>(6)?.map(|c| json!({ "code": c })).map(|mut e| { e["message"] = json!(r.get::<_, Option<String>>(7).ok().flatten()); e }),
            "tokens_used": r.get::<_, i64>(8)?, "listen": r.get::<_, i64>(9)? != 0 })),
    ).optional().ok().flatten().map(|v| Json(v).into_response()).ok_or_else(|| ApiError::not_found("Turn"))
}

// ---------------- approvals ----------------

async fn list_approvals(State(app): State<AppState>, Query(p): Query<Page>) -> ApiResult {
    let ids: Vec<(i64, String)> = {
        let c = app.db.lock();
        let mut st = c.prepare("SELECT rowid, id FROM approvals WHERE (?1 IS NULL OR status = ?1) AND rowid > ?2 ORDER BY rowid LIMIT ?3").expect("prepare");
        st.query_map(params![p.status, p.after(), p.limit()], |r| Ok((r.get(0)?, r.get(1)?))).map(|x| x.flatten().collect()).unwrap_or_default()
    };
    let rows = ids.into_iter().filter_map(|(n, id)| engine::approval_json(&app, &id).map(|v| (n, v))).collect();
    ok(paged(rows, p.limit()))
}

async fn get_approval(State(app): State<AppState>, Path(id): Path<String>) -> ApiResult {
    engine::approval_json(&app, &id).map(|v| Json(v).into_response()).ok_or_else(|| ApiError::not_found("Approval"))
}

async fn answer_approval(State(app): State<AppState>, Extension(c): Extension<Caller>, Path(id): Path<String>, Json(b): Json<Value>) -> ApiResult {
    let a = engine::approval_json(&app, &id).ok_or_else(|| ApiError::not_found("Approval"))?;
    // High risk, and anything that installs or changes a plugin, needs the owner (docs/01 Scopes).
    if a["risk"] == "high" || a["kind"] == "plugin_install" || a["kind"] == "plugin_update" {
        need_owner(&c)?;
    }
    match engine::answer_approval(&app, &id, &b, &c.token_id) {
        Ok(v) => ok(v),
        Err(AnswerError::NotFound) => Err(ApiError::not_found("Approval")),
        Err(AnswerError::Conflict) => Err(ApiError::new(StatusCode::CONFLICT, "conflict", "This approval was already answered.")),
        Err(AnswerError::Expired) => Err(ApiError::new(StatusCode::CONFLICT, "approval_expired", "That request timed out, so nothing was run.")),
        Err(AnswerError::Invalid(m)) => Err(ApiError::invalid(m)),
    }
}

// ---------------- actions ----------------

fn action_row(r: &rusqlite::Row) -> rusqlite::Result<(i64, Value)> {
    let args: String = r.get(6)?;
    let result: Option<String> = r.get(7)?;
    Ok((r.get(0)?, json!({
        "id": r.get::<_, String>(1)?, "thread_id": r.get::<_, String>(2)?, "turn_id": r.get::<_, Option<String>>(3)?,
        "plugin": r.get::<_, String>(4)?, "tool": r.get::<_, String>(5)?,
        "args": serde_json::from_str::<Value>(&args).unwrap_or(Value::Null),
        "result": result.and_then(|s| serde_json::from_str::<Value>(&s).ok()),
        "ok": r.get::<_, Option<bool>>(8)?, "outcome": r.get::<_, String>(9)?, "decision": r.get::<_, String>(10)?,
        "approval_id": r.get::<_, Option<String>>(11)?, "undoable": r.get::<_, i64>(12)? != 0, "created": r.get::<_, String>(13)?,
    })))
}

const ACTION_COLS: &str = "n, id, thread_id, turn_id, plugin, tool, args, result, ok, outcome, decision, approval_id, undoable, created";

async fn list_actions(State(app): State<AppState>, Query(p): Query<Page>) -> ApiResult {
    let rows: Vec<(i64, Value)> = {
        let c = app.db.lock();
        let mut st = c.prepare(&format!(
            "SELECT {ACTION_COLS} FROM actions WHERE (?1 IS NULL OR thread_id = ?1) AND (?2 IS NULL OR plugin = ?2) AND n > ?3 ORDER BY n LIMIT ?4"
        )).expect("prepare");
        st.query_map(params![p.thread_id, p.plugin, p.after(), p.limit()], action_row).map(|x| x.flatten().collect()).unwrap_or_default()
    };
    ok(paged(rows, p.limit()))
}

async fn get_action(State(app): State<AppState>, Path(id): Path<String>) -> ApiResult {
    app.db.lock().query_row(&format!("SELECT {ACTION_COLS} FROM actions WHERE id = ?1"), [&id], action_row)
        .optional().ok().flatten().map(|v| Json(v.1).into_response()).ok_or_else(|| ApiError::not_found("Action"))
}

// ---------------- events (SSE) ----------------

async fn all_events(State(app): State<AppState>, headers: HeaderMap) -> impl IntoResponse {
    sse(app, headers, None)
}

async fn thread_events(State(app): State<AppState>, headers: HeaderMap, Path(id): Path<String>) -> ApiResult {
    thread_json(&app, &id).ok_or_else(|| ApiError::not_found("Thread"))?;
    Ok(sse(app, headers, Some(id)).into_response())
}

struct SseState {
    replay: VecDeque<Event>,
    rx: broadcast::Receiver<Arc<Event>>,
    last: i64,
    thread: Option<String>,
}

/// Replay from `Last-Event-ID`, then live. Subscribe first so nothing falls in the gap.
fn sse(app: AppState, headers: HeaderMap, thread: Option<String>) -> Sse<impl Stream<Item = Result<SseEvent, Infallible>>> {
    let rx = app.feed.subscribe();
    let last_hdr = headers.get("last-event-id").and_then(|v| v.to_str().ok()).and_then(events::parse_event_id);
    let last = last_hdr.unwrap_or(0);
    // Replay only when the client says where it stopped. A fresh client gets live events only.
    let replay: VecDeque<Event> = match last_hdr { Some(n) => app.feed.replay(n, thread.as_deref(), 10_000).into(), None => VecDeque::new() };
    let st = SseState { replay, rx, last, thread };
    let s = stream::unfold(st, |mut st| async move {
        loop {
            let ev = if let Some(e) = st.replay.pop_front() {
                e
            } else {
                match st.rx.recv().await {
                    Ok(e) => (*e).clone(),
                    Err(broadcast::error::RecvError::Lagged(_)) => return None, // client reconnects with Last-Event-ID
                    Err(broadcast::error::RecvError::Closed) => return None,
                }
            };
            if ev.n <= st.last || (st.thread.is_some() && ev.thread_id != st.thread) {
                continue;
            }
            st.last = ev.n;
            let data = serde_json::to_string(&ev).unwrap_or_default();
            let out = SseEvent::default().id(ev.id.clone()).event(ev.kind.clone()).data(data);
            return Some((Ok(out), st));
        }
    });
    Sse::new(s).keep_alive(KeepAlive::new().interval(std::time::Duration::from_secs(55))) // a line every 15 s (the default) kept every phone radio awake; the phone waits 120 s
}

// ---------------- undo, plugins lifecycle ----------------

async fn undo_action(State(app): State<AppState>, Extension(c): Extension<Caller>, Path(id): Path<String>) -> ApiResult {
    need_owner(&c)?;
    crate::export::undo(&app, &id).await.map(|v| Json(v).into_response()).map_err(|e| ApiError::new(StatusCode::CONFLICT, "conflict", e))
}

#[derive(Deserialize)]
struct Install {
    path: Option<String>,
    url: Option<String>,
}

/// Arrives as a draft and needs approval at `high` risk (docs/03, Sharing).
async fn install_plugin(State(app): State<AppState>, Extension(c): Extension<Caller>, Json(b): Json<Install>) -> ApiResult {
    need_owner(&c)?;
    if b.url.is_some() {
        return Err(ApiError::invalid("install by URL is not built yet; clone it and install by path"));
    }
    let path = b.path.ok_or_else(|| ApiError::invalid("give a path"))?;
    let id = crate::store::install_from_path(&app, &path).map_err(ApiError::invalid)?;
    let thread = "system:install";
    app.ensure_thread(thread, "system", "plugin installs");
    let apr = crate::store::submit(&app, thread, "install", &id, &[], true, None, None).map_err(ApiError::invalid)?;
    Ok((StatusCode::ACCEPTED, Json(json!({ "plugin": id, "status": "draft", "approval_id": apr }))).into_response())
}

async fn get_plugin(State(app): State<AppState>, Path(id): Path<String>) -> ApiResult {
    plugin_json(&app, &id).map(|v| Json(v).into_response()).ok_or_else(|| ApiError::not_found("Plugin"))
}

async fn plugin_versions(State(app): State<AppState>, Path(id): Path<String>) -> ApiResult {
    ok(crate::store::versions(&app, &id))
}

#[derive(Deserialize)]
struct DiffQ {
    from: Option<String>,
    to: Option<String>,
}

async fn plugin_diff(State(app): State<AppState>, Path(id): Path<String>, Query(q): Query<DiffQ>) -> ApiResult {
    crate::store::diff(&app, &id, q.from.as_deref(), q.to.as_deref()).map(|v| Json(v).into_response()).map_err(ApiError::invalid)
}

async fn approve_plugin(State(app): State<AppState>, Extension(c): Extension<Caller>, Path(id): Path<String>) -> ApiResult {
    need_owner(&c)?;
    // Answer any pending card for this draft too, so the two paths agree.
    let pending: Vec<String> = {
        let conn = app.db.lock();
        let mut st = conn.prepare("SELECT id FROM approvals WHERE status = 'pending' AND kind IN ('plugin_install', 'plugin_update') AND json_extract(details, '$.plugin') = ?1").expect("prepare");
        st.query_map([&id], |r| r.get(0)).map(|x| x.flatten().collect()).unwrap_or_default()
    };
    if let Some(apr) = pending.first() {
        return match engine::answer_approval(&app, apr, &json!({ "answer": "yes" }), &c.token_id) {
            Ok(_) => plugin_json(&app, &id).map(|v| Json(v).into_response()).ok_or_else(|| ApiError::not_found("Plugin")),
            Err(AnswerError::Invalid(m)) => Err(ApiError::invalid(m)),
            Err(_) => Err(ApiError::new(StatusCode::CONFLICT, "conflict", "Could not approve.")),
        };
    }
    crate::store::approve(&app, &id, None, "human").map(|v| Json(v).into_response()).map_err(ApiError::invalid)
}

async fn reject_plugin(State(app): State<AppState>, Extension(c): Extension<Caller>, Path(id): Path<String>) -> ApiResult {
    need_owner(&c)?;
    ok(json!({ "rejected": crate::store::reject(&app, &id) }))
}

async fn disable_plugin(State(app): State<AppState>, Extension(c): Extension<Caller>, Path(id): Path<String>) -> ApiResult {
    need_owner(&c)?;
    if crate::store::disable(&app, &id) { ok(json!({ "id": id, "status": "disabled" })) } else { Err(ApiError::not_found("Plugin")) }
}

#[derive(Deserialize)]
struct Rollback {
    version: String,
}

async fn rollback_plugin(State(app): State<AppState>, Extension(c): Extension<Caller>, Path(id): Path<String>, Json(b): Json<Rollback>) -> ApiResult {
    need_owner(&c)?;
    crate::store::rollback(&app, &id, &b.version).map(|v| Json(v).into_response()).map_err(ApiError::invalid)
}

#[derive(Deserialize)]
struct Uninstall {
    remove_state: Option<bool>,
}

async fn uninstall_plugin(State(app): State<AppState>, Extension(c): Extension<Caller>, Path(id): Path<String>, Query(q): Query<Uninstall>) -> ApiResult {
    need_owner(&c)?;
    if crate::store::uninstall(&app, &id, q.remove_state.unwrap_or(false)) { ok(json!({ "uninstalled": id })) } else { Err(ApiError::not_found("Plugin")) }
}

// ---------------- memory ----------------

#[derive(Deserialize)]
struct MemQ {
    q: Option<String>,
    limit: Option<i64>,
}

async fn search_memory(State(app): State<AppState>, Query(q): Query<MemQ>) -> ApiResult {
    let limit = q.limit.unwrap_or(50).clamp(1, 200);
    let items = match q.q.as_deref().filter(|s| !s.trim().is_empty()) {
        Some(text) => { let (a, t) = (app.clone(), text.to_string()); tokio::task::spawn_blocking(move || crate::embed::search(&a, &t, limit)).await.unwrap_or_default() }
        None => crate::memory::all_current(&app.db).into_iter().take(limit as usize).collect(),
    };
    ok(json!({ "items": items }))
}

async fn get_memory(State(app): State<AppState>, Path(id): Path<String>) -> ApiResult {
    let e = crate::memory::get(&app.db, &id).ok_or_else(|| ApiError::not_found("Entry"))?;
    let sources: Vec<Value> = {
        let c = app.db.lock();
        e.sources.iter().filter_map(|s| c.query_row("SELECT id, thread_id, trust, body, created FROM messages WHERE id = ?1", [s], |r| {
            let body: String = r.get(3)?;
            Ok(json!({ "id": r.get::<_, String>(0)?, "thread_id": r.get::<_, String>(1)?, "trust": r.get::<_, String>(2)?,
                "body": serde_json::from_str::<Value>(&body).unwrap_or(Value::Null), "created": r.get::<_, String>(4)? }))
        }).ok()).collect()
    };
    ok(json!({ "entry": e, "sources": sources }))
}

async fn memory_history(State(app): State<AppState>, Path(id): Path<String>) -> ApiResult {
    ok(json!({ "items": crate::memory::history(&app.db, &id) }))
}

/// Entries a client adds or edits by hand are stored with `source_trust: user` (docs/01).
async fn add_memory(State(app): State<AppState>, Json(b): Json<Value>) -> ApiResult {
    let mut e: crate::memory::Entry = serde_json::from_value(b).map_err(|e| ApiError::invalid(e.to_string()))?;
    if e.title.trim().is_empty() || e.summary.trim().is_empty() {
        return Err(ApiError::invalid("title and summary are required"));
    }
    e.sources.clear();
    let made = crate::memory::create(&app.db, e, crate::memory::Trust::User, "user");
    app.soft_dirty_index();
    Ok((StatusCode::CREATED, Json(json!(made))).into_response())
}

async fn edit_memory(State(app): State<AppState>, Path(id): Path<String>, Json(b): Json<Value>) -> ApiResult {
    match crate::memory::update(&app.db, &id, &b, "user", None, Some(crate::memory::Trust::User)) {
        Ok(e) => {
            app.soft_dirty_index();
            ok(json!(e))
        }
        Err(_) => Err(ApiError::not_found("Entry")),
    }
}

async fn erase_memory(State(app): State<AppState>, Path(id): Path<String>) -> ApiResult {
    if crate::memory::erase(&app.db, &id) {
        app.unload_everywhere(&format!("mem:{id}"));
        app.force_index();
        ok(json!({ "erased": id, "note": "Raw sources are kept but will not be used to rebuild this entry. Use forget to remove them too." }))
    } else {
        Err(ApiError::not_found("Entry"))
    }
}

async fn forget_memory(State(app): State<AppState>, Extension(c): Extension<Caller>, Path(id): Path<String>) -> ApiResult {
    need_owner(&c)?;
    crate::dreaming::forget(&app, &id).map(|v| Json(v).into_response()).ok_or_else(|| ApiError::not_found("Entry"))
}

async fn clear_memory(State(app): State<AppState>, Extension(c): Extension<Caller>, Json(b): Json<Value>) -> ApiResult {
    need_owner(&c)?;
    if b["confirm"] != "DELETE ALL MEMORY" {
        return Err(ApiError::invalid("Send {\"confirm\": \"DELETE ALL MEMORY\"} to clear all memory. This cannot be undone."));
    }
    ok(crate::dreaming::clear_all(&app))
}

async fn memory_graph(State(app): State<AppState>) -> ApiResult {
    ok(crate::remember::graph(&app.db))
}

async fn memory_folders(State(app): State<AppState>) -> ApiResult {
    ok(json!({ "items": crate::memory::folder_tree(&app.db) }))
}

async fn make_memory_folder(State(app): State<AppState>, Json(b): Json<Value>) -> ApiResult {
    let path = b["path"].as_str().unwrap_or_default();
    let made = crate::memory::make_folder(&app.db, path, b["description"].as_str().unwrap_or_default()).map_err(ApiError::invalid)?;
    app.force_index();
    Ok((StatusCode::CREATED, Json(json!({ "folder": made }))).into_response())
}

async fn memory_core(State(app): State<AppState>) -> ApiResult {
    ok(json!({ "text": crate::memory::read_core(&app.cfg.data_dir) }))
}

async fn memory_index(State(app): State<AppState>) -> ApiResult {
    let idx = crate::prompt::index(&app.manifests(), |id| app.plugin_usable(id), &app.memory_index());
    ok(json!({ "text": idx }))
}

// ---------------- dreaming, jobs ----------------

#[derive(Deserialize)]
struct DreamRun {
    #[serde(default)]
    dry_run: bool,
}

/// Dry runs are open to clients; real runs need the owner (docs/01 Scopes).
async fn run_dreaming(State(app): State<AppState>, Extension(c): Extension<Caller>, body: Option<Json<DreamRun>>) -> ApiResult {
    let dry = body.map(|b| b.dry_run).unwrap_or(false);
    if !dry {
        need_owner(&c)?;
    }
    crate::dreaming::start_run(&app, dry).map(|v| (StatusCode::ACCEPTED, Json(v)).into_response()).map_err(|e| ApiError::new(StatusCode::CONFLICT, "conflict", e))
}

async fn dreaming_runs(State(app): State<AppState>) -> ApiResult {
    ok(json!({ "items": crate::dreaming::runs(&app) }))
}

async fn dreaming_run(State(app): State<AppState>, Path(id): Path<String>) -> ApiResult {
    crate::dreaming::run_detail(&app, &id).map(|v| Json(v).into_response()).ok_or_else(|| ApiError::not_found("Run"))
}

async fn list_jobs(State(app): State<AppState>) -> ApiResult {
    ok(json!({ "items": crate::scheduler::list_jobs(&app, true) }))
}

/// A front end must be able to cancel jobs without asking the model (docs/01).
async fn cancel_job(State(app): State<AppState>, Path(id): Path<String>) -> ApiResult {
    if crate::scheduler::cancel_job(&app, &id) { ok(json!({ "cancelled": id })) } else { Err(ApiError::not_found("Job")) }
}

async fn get_job(State(app): State<AppState>, Path(id): Path<String>) -> ApiResult {
    crate::scheduler::get_job(&app, &id).map(|j| Json(j).into_response()).ok_or_else(|| ApiError::not_found("Job"))
}

/// Set an alarm or a reminder from a front end, without the model. Body:
/// `{"kind": "alarm", "time": "07:00", "days": "weekdays", "label": "Wake up", "sound": "bell"}` or
/// `{"kind": "reminder", "text": "Call Sam", "in_minutes": 30 | "at": "2026-11-14 15:00", "ring": true}`.
async fn create_job(State(app): State<AppState>, Json(b): Json<Value>) -> ApiResult {
    let r = match b["kind"].as_str().unwrap_or("reminder") {
        "alarm" => crate::scheduler::create_alarm(&app, &b, false, "user", None),
        "reminder" => crate::scheduler::tool(&app, "api", false, "create_reminder", &b),
        "recurring" => crate::scheduler::tool(&app, "api", false, "create_recurring", &b),
        // Automations made on the page: the user's own words, so no approval is needed.
        "task" => {
            let repeats = b["every_minutes"].is_number() || b["every"].is_string() || !b["days"].is_null();
            crate::scheduler::tool(&app, "api", false, if repeats { "schedule_recurring_task" } else { "schedule_task" }, &b)
        }
        "task_on_notification" => crate::scheduler::tool(&app, "api", false, "run_task_on_notification", &b),
        _ => Err("kind is alarm, reminder, recurring, task or task_on_notification".into()),
    };
    r.map(|v| (StatusCode::CREATED, Json(v)).into_response()).map_err(ApiError::invalid)
}

/// Change a job: `text`/`label`, `at`/`time`, `in_minutes`, `every`, `weekday`, `days`, `ring`, `sound`.
async fn update_job(State(app): State<AppState>, Path(id): Path<String>, Json(b): Json<Value>) -> ApiResult {
    if crate::scheduler::get_job(&app, &id).is_none() {
        return Err(ApiError::not_found("Job"));
    }
    crate::scheduler::update_job(&app, &id, &b, false).map(|mut v| { v.as_object_mut().map(|o| o.remove("previous")); Json(v).into_response() }).map_err(ApiError::invalid)
}

async fn pause_job(State(app): State<AppState>, Path(id): Path<String>) -> ApiResult {
    crate::scheduler::set_paused(&app, &id, true).map(|v| Json(v).into_response()).map_err(ApiError::invalid)
}

async fn resume_job(State(app): State<AppState>, Path(id): Path<String>) -> ApiResult {
    crate::scheduler::set_paused(&app, &id, false).map(|v| Json(v).into_response()).map_err(ApiError::invalid)
}

/// Start a task now. Returns the thread its report will be in.
async fn run_job(State(app): State<AppState>, Path(id): Path<String>) -> ApiResult {
    crate::scheduler::run_now(&app, &id).map(|t| (StatusCode::ACCEPTED, Json(json!({ "thread": t }))).into_response()).map_err(ApiError::invalid)
}

/// The phone saw a notification from a watched app: `{app, app_name?, title, text}`. Device or client tokens.
async fn automation_notify(State(app): State<AppState>, Extension(c): Extension<Caller>, Json(b): Json<Value>) -> ApiResult {
    if !app.rate_ok(&format!("notify:{}", c.token_id), 30) {
        return Err(ApiError::new(StatusCode::TOO_MANY_REQUESTS, "rate_limited", "Too many notifications."));
    }
    crate::scheduler::notification(&app, &b).map(|v| Json(v).into_response()).map_err(ApiError::invalid)
}

/// Which apps the phone should forward notifications from (only those that start a task).
async fn automation_watch(State(app): State<AppState>) -> ApiResult {
    ok(json!({ "apps": crate::scheduler::watched_apps(&app) }))
}

async fn alarms_ringing(State(app): State<AppState>) -> ApiResult {
    ok(json!({ "items": crate::scheduler::ringing(&app) }))
}

#[derive(Deserialize, Default)]
struct RingBody {
    id: Option<String>,
    minutes: Option<i64>,
}

async fn snooze_alarm(State(app): State<AppState>, body: Option<Json<RingBody>>) -> ApiResult {
    let b = body.map(|b| b.0).unwrap_or_default();
    crate::scheduler::snooze(&app, b.id.as_deref(), b.minutes).map(|v| Json(v).into_response()).map_err(|e| ApiError::new(StatusCode::CONFLICT, "not_ringing", e))
}

async fn dismiss_alarm(State(app): State<AppState>, body: Option<Json<RingBody>>) -> ApiResult {
    let b = body.map(|b| b.0).unwrap_or_default();
    crate::scheduler::dismiss(&app, b.id.as_deref()).map(|v| Json(v).into_response()).map_err(|e| ApiError::new(StatusCode::CONFLICT, "not_ringing", e))
}

/// Recent notifications, newest last, so a client that was offline can catch up. `cursor` = the last `n` seen.
async fn list_notices(State(app): State<AppState>, Query(p): Query<Page>) -> ApiResult {
    let after: i64 = p.cursor.as_deref().and_then(|c| c.parse().ok()).unwrap_or(0);
    let limit = p.limit.unwrap_or(50).clamp(1, 200);
    let c = app.db.lock();
    let mut st = c.prepare("SELECT n, priority, title, text, silent, delivered, thread_id, ts, job_id FROM notices WHERE n > ?1 ORDER BY n LIMIT ?2").map_err(|e| ApiError::invalid(e.to_string()))?;
    let items: Vec<Value> = st.query_map(params![after, limit], |r| Ok(json!({
        "n": r.get::<_, i64>(0)?, "priority": r.get::<_, String>(1)?, "title": r.get::<_, Option<String>>(2)?, "text": r.get::<_, String>(3)?,
        "silent": r.get::<_, i64>(4)? != 0, "delivered": r.get::<_, i64>(5)? != 0, "thread_id": r.get::<_, Option<String>>(6)?, "created": util::iso(r.get(7)?), "job_id": r.get::<_, Option<String>>(8)? })))
        .map(|x| x.flatten().collect()).unwrap_or_default();
    ok(json!({ "items": items }))
}

/// The owner's verdict on a notice: `{rating: "useful"|"not_useful", note?}`. It goes into the review pack.
async fn notice_feedback(State(app): State<AppState>, Extension(c): Extension<Caller>, Path(n): Path<i64>, Json(b): Json<Value>) -> ApiResult {
    if !matches!(c.scope, Scope::Owner | Scope::Client) {
        return Err(ApiError::scope());
    }
    crate::audit::feedback(&app, n, b["rating"].as_str().unwrap_or_default(), b["note"].as_str().unwrap_or("")).map(|v| Json(v).into_response()).map_err(ApiError::invalid)
}

#[derive(Deserialize)]
struct ReviewQ {
    hours: Option<i64>,
    turns: Option<i64>,
    /// `last`: everything since the review that was last marked.
    since: Option<String>,
    /// `1`: mark this review, so the next `since=last` starts here.
    mark: Option<String>,
}

/// A markdown pack of the last `hours` (default 24): notifications, runs, data pulled, prompts. For an AI agent. Owner only.
async fn audit_review(State(app): State<AppState>, Extension(c): Extension<Caller>, Query(q): Query<ReviewQ>) -> Result<Response, ApiError> {
    need_owner(&c)?;
    let text = tokio::task::spawn_blocking(move || crate::audit::review(&app, q.hours.unwrap_or(24), q.turns.unwrap_or(200), q.since.as_deref() == Some("last"), q.mark.as_deref() == Some("1"))).await.map_err(|e| ApiError::invalid(e.to_string()))?;
    Ok(([(header::CONTENT_TYPE, "text/markdown; charset=utf-8")], text).into_response())
}

/// One run with every model step's exact request. Owner only.
async fn audit_turn(State(app): State<AppState>, Extension(c): Extension<Caller>, Path(id): Path<String>) -> ApiResult {
    need_owner(&c)?;
    crate::audit::turn_trace(&app, &id).map(|v| Json(v).into_response()).ok_or_else(|| ApiError::not_found("Turn"))
}

/// Snooze a reminder's notice: `{minutes?}` (10 by default). The same words come again later.
async fn snooze_notice(State(app): State<AppState>, Path(n): Path<i64>, body: Option<Json<Value>>) -> ApiResult {
    let m = body.and_then(|b| b.0["minutes"].as_i64()).unwrap_or(10);
    crate::scheduler::snooze_notice(&app, n, m).map(|v| Json(v).into_response()).map_err(ApiError::invalid)
}

/// The text-to-speech service is only ever on this computer: a loopback address, plain http.
fn tts_base(app: &App) -> Option<String> {
    tts_base_of(&app.cfg.tts_url)
}

pub fn tts_base_of(url: &str) -> Option<String> {
    let u = url.trim().trim_end_matches('/');
    let rest = u.strip_prefix("http://")?;
    let host = rest.split(['/', ':']).next().unwrap_or("");
    if matches!(host, "127.0.0.1" | "localhost" | "[::1]") { Some(u.to_string()) } else { None }
}

/// `GET /v1/status` (owner): are the local models, the voice and the sandbox working? Runs the checks now.
async fn server_status(State(app): State<AppState>, Extension(c): Extension<Caller>) -> ApiResult {
    need_owner(&c)?;
    let a = app.clone();
    ok(tokio::task::spawn_blocking(move || { let mut v = crate::status::check(&a); v["overview"] = crate::status::overview(&a); v }).await.map_err(|e| ApiError::invalid(e.to_string()))?)
}

/// `GET /v1/power`: is AssistX on? `PUT /v1/power {on}` (owner) switches it (src/power.rs).
async fn get_power(State(app): State<AppState>) -> ApiResult {
    ok(json!({ "on": crate::power::is_on(&app) }))
}

async fn set_power(State(app): State<AppState>, Extension(c): Extension<Caller>, Json(b): Json<Value>) -> ApiResult {
    need_owner(&c)?;
    let on = b["on"].as_bool().ok_or_else(|| ApiError::invalid("on is true or false"))?;
    ok(crate::power::set(&app, on))
}

/// `GET /v1/usage?days=30` (owner): models used, estimated money, power, busiest days.
async fn get_usage(State(app): State<AppState>, Extension(c): Extension<Caller>, axum::extract::Query(q): axum::extract::Query<std::collections::HashMap<String, String>>) -> ApiResult {
    need_owner(&c)?;
    let days = q.get("days").and_then(|d| d.parse().ok()).unwrap_or(30);
    ok(tokio::task::spawn_blocking(move || crate::usage::report(&app, days)).await.map_err(|e| ApiError::invalid(e.to_string()))?)
}

/// `GET /v1/security` (owner): is the data encrypted, and what is not.
async fn get_security(State(app): State<AppState>, Extension(c): Extension<Caller>) -> ApiResult {
    need_owner(&c)?;
    ok(tokio::task::spawn_blocking(move || crate::usage::security(&app)).await.map_err(|e| ApiError::invalid(e.to_string()))?)
}

/// `GET /v1/speech`: can this server read text out loud (Kokoro, on this computer)?
async fn speech_status(State(app): State<AppState>) -> ApiResult {
    ok(json!({ "available": tts_base(&app).is_some(), "engine": "kokoro" }))
}

/// `POST /v1/speech {text}`: the words as a WAV file, spoken by the local Kokoro voice. The text never
/// leaves this computer, and the cloud voice the service may also offer is never asked for.
async fn speak(State(app): State<AppState>, Extension(c): Extension<Caller>, Json(b): Json<Value>) -> ApiResult {
    let base = tts_base(&app).ok_or_else(|| ApiError::new(StatusCode::SERVICE_UNAVAILABLE, "speech_off", "No voice is set up on this server."))?;
    if !app.rate_ok(&format!("speech:{}", c.token_id), 60) {
        return Err(ApiError::new(StatusCode::TOO_MANY_REQUESTS, "rate_limited", "Too much speech. Wait a minute."));
    }
    let text: String = b["text"].as_str().unwrap_or_default().chars().filter(|ch| !matches!(ch, '*' | '_' | '#' | '`' | '>' | '<')).take(1500).collect();
    if text.trim().is_empty() {
        return Err(ApiError::invalid("text is empty"));
    }
    let body = json!({ "text": text, "engine": "kokoro" }).to_string();
    let wav = tokio::task::spawn_blocking(move || -> Result<Vec<u8>, String> {
        let agent: ureq::Agent = ureq::Agent::config_builder().timeout_global(Some(std::time::Duration::from_secs(60))).http_status_as_error(false).build().into();
        let mut r = agent.post(&format!("{base}/speak")).header("Content-Type", "application/json").send(body).map_err(|e| e.to_string())?;
        if r.status().as_u16() != 200 {
            return Err(format!("the voice answered {}", r.status().as_u16()));
        }
        r.body_mut().with_config().limit(20 * 1024 * 1024).read_to_vec().map_err(|e| e.to_string())
    }).await.map_err(|e| ApiError::invalid(e.to_string()))?
        .map_err(|e| ApiError::new(StatusCode::BAD_GATEWAY, "speech_failed", format!("The voice did not work: {e}")))?;
    Ok(([(axum::http::header::CONTENT_TYPE, "audio/wav"), (axum::http::header::CACHE_CONTROL, "no-store")], wav).into_response())
}

async fn list_sounds(State(app): State<AppState>) -> ApiResult {
    ok(json!({ "items": crate::sounds::list(&app), "max_bytes": crate::sounds::MAX_BYTES }))
}

async fn get_sound(State(app): State<AppState>, Path(name): Path<String>) -> ApiResult {
    let (path, mime) = crate::sounds::file(&app, &name).ok_or_else(|| ApiError::not_found("Sound"))?;
    let bytes = std::fs::read(path).map_err(|_| ApiError::not_found("Sound"))?;
    Ok(([(axum::http::header::CONTENT_TYPE, mime), (axum::http::header::CACHE_CONTROL, "private, max-age=86400")], bytes).into_response())
}

/// Upload a custom ringtone (the raw file as the body). Owner or client; never the device scope.
async fn put_sound(State(app): State<AppState>, Path(name): Path<String>, body: axum::body::Bytes) -> ApiResult {
    crate::sounds::save(&app, &name, &body).map(|v| (StatusCode::CREATED, Json(v)).into_response()).map_err(ApiError::invalid)
}

async fn delete_sound(State(app): State<AppState>, Path(name): Path<String>) -> ApiResult {
    if crate::sounds::delete(&app, &name) { ok(json!({ "deleted": name })) } else { Err(ApiError::not_found("Sound")) }
}

// ---------------- body numbers from the phone ----------------

/// `{"days": [{"day": "2025-01-31", "metrics": {"steps": 8123}}]}`. Device or client tokens.
async fn health_upload(State(app): State<AppState>, Extension(c): Extension<Caller>, Json(b): Json<Value>) -> ApiResult {
    let device = if c.label.is_empty() { c.token_id.clone() } else { c.label.clone() };
    let r = crate::health::add_days(&app, &device, &b["days"]);
    if let Ok(v) = &r {
        crate::audit::event(&app, "phone.health", json!({ "device": device, "result": v }));
    }
    r.map(|v| Json(v).into_response()).map_err(ApiError::invalid)
}

async fn health_days(State(app): State<AppState>, axum::extract::Query(r): axum::extract::Query<Range>) -> ApiResult {
    crate::health::days(&app, r.from.as_deref(), r.to.as_deref(), r.limit.unwrap_or(7)).map(|items| Json(json!({ "items": items })).into_response()).map_err(ApiError::invalid)
}

// ---------------- location ----------------

/// The phone's points: `{"points": [{"ts": unix, "lat", "lon", "acc"}]}`. Device or client tokens.
async fn location_upload(State(app): State<AppState>, Extension(c): Extension<Caller>, Json(b): Json<Value>) -> ApiResult {
    let device = if c.label.is_empty() { c.token_id.clone() } else { c.label.clone() };
    crate::location::add_points(&app, &device, &b["points"]).map(|v| Json(v).into_response()).map_err(ApiError::invalid)
}

#[derive(Deserialize)]
struct Range {
    from: Option<String>,
    to: Option<String>,
    limit: Option<i64>,
}

async fn location_points(State(app): State<AppState>, axum::extract::Query(r): axum::extract::Query<Range>) -> ApiResult {
    let now = util::unix_now();
    let num = |s: &Option<String>| s.as_deref().and_then(|v| v.parse::<i64>().ok());
    let to = num(&r.to).unwrap_or(now);
    let from = num(&r.from).unwrap_or(to - 86_400);
    ok(json!({ "from": from, "to": to, "items": crate::location::points(&app, from, to, r.limit.unwrap_or(2000)) }))
}

async fn location_fences(State(app): State<AppState>) -> ApiResult {
    ok(crate::location::fences(&app))
}

/// The phone's geofence: `{place, what: "arrive"|"leave", ts?}`. Device or client tokens.
async fn location_transition(State(app): State<AppState>, Json(b): Json<Value>) -> ApiResult {
    crate::location::transition(&app, b["place"].as_str().unwrap_or_default(), b["what"].as_str().unwrap_or_default(), b["ts"].as_i64()).map(|v| Json(v).into_response()).map_err(ApiError::invalid)
}

async fn location_now(State(app): State<AppState>) -> ApiResult {
    ok(crate::location::now(&app))
}

async fn location_visits(State(app): State<AppState>, axum::extract::Query(r): axum::extract::Query<Range>) -> ApiResult {
    crate::location::history(&app, r.from.as_deref(), r.to.as_deref()).map(|v| Json(v).into_response()).map_err(ApiError::invalid)
}

#[derive(Deserialize)]
struct PlaceQuery {
    q: Option<String>,
}

async fn list_places(State(app): State<AppState>, axum::extract::Query(q): axum::extract::Query<PlaceQuery>) -> ApiResult {
    ok(json!({ "items": crate::location::places(&app, q.q.as_deref()) }))
}

async fn get_place(State(app): State<AppState>, Path(id): Path<String>) -> ApiResult {
    crate::location::get_place(&app, &id).map(|v| Json(v).into_response()).ok_or_else(|| ApiError::not_found("Place"))
}

/// Name a place: `{"name": "Gym"}` names where you are now; add `lat`, `lon` (and `radius`) for somewhere else.
async fn make_place(State(app): State<AppState>, Json(b): Json<Value>) -> ApiResult {
    let at = match (b["lat"].as_f64(), b["lon"].as_f64()) { (Some(la), Some(lo)) => Some((la, lo)), _ => None };
    crate::location::name_place(&app, None, b["name"].as_str().unwrap_or(""), b["radius"].as_f64(), at)
        .map(|mut v| { v.as_object_mut().map(|o| o.remove("previous")); (StatusCode::CREATED, Json(v)).into_response() }).map_err(ApiError::invalid)
}

async fn rename_place(State(app): State<AppState>, Path(id): Path<String>, Json(b): Json<Value>) -> ApiResult {
    if crate::location::get_place(&app, &id).is_none() {
        return Err(ApiError::not_found("Place"));
    }
    crate::location::name_place(&app, Some(&id), b["name"].as_str().unwrap_or(""), b["radius"].as_f64(), None)
        .map(|mut v| { v.as_object_mut().map(|o| o.remove("previous")); Json(v).into_response() }).map_err(ApiError::invalid)
}

async fn forget_place(State(app): State<AppState>, Path(id): Path<String>) -> ApiResult {
    if crate::location::forget_place(&app, &id) { ok(json!({ "deleted": id })) } else { Err(ApiError::not_found("Place")) }
}

// ---------------- computers (nodes) ----------------

async fn list_nodes(State(app): State<AppState>) -> ApiResult {
    ok(json!({ "items": crate::nodes::list(&app) }))
}

#[derive(Deserialize)]
struct NewNode {
    name: String,
}

/// Add a computer: makes a `node` token for it, shown once, plus the command to start it.
async fn add_node(State(app): State<AppState>, Extension(c): Extension<Caller>, Json(b): Json<NewNode>) -> ApiResult {
    need_owner(&c)?;
    let name: String = b.name.trim().chars().filter(|ch| ch.is_ascii_alphanumeric() || *ch == '-' || *ch == '_').take(30).collect();
    if name.is_empty() {
        return Err(ApiError::invalid("name: letters, digits, - and _, like my-mac"));
    }
    if crate::nodes::list(&app).iter().any(|n| n["name"].as_str().is_some_and(|x| x.eq_ignore_ascii_case(&name))) {
        return Err(ApiError::new(StatusCode::CONFLICT, "conflict", "A computer with that name exists. Revoke it under Devices first, or pick another name."));
    }
    let (id, token) = auth::create_token(&app.db, Scope::Node, &name);
    Ok((StatusCode::CREATED, Json(json!({ "id": id, "name": name, "token": token, "note": "Shown once. Give it to agent-node: agent-node setup --server <this server> --token <token>" }))).into_response())
}

/// A computer asks for work. Body: what it says about itself (os, hostname, roots, version).
async fn node_poll(State(app): State<AppState>, Extension(c): Extension<Caller>, body: Option<Json<Value>>) -> ApiResult {
    if c.scope != Scope::Node {
        return Err(ApiError::scope());
    }
    let info = body.map(|b| b.0).unwrap_or(Value::Null);
    match app.nodes.poll(&c.token_id, info).await {
        Some(job) => ok(job),
        None => Ok(StatusCode::NO_CONTENT.into_response()),
    }
}

/// A computer's answer: `{"ok": true, "result": ...}` or `{"ok": false, "error": "..."}`.
async fn node_answer(State(app): State<AppState>, Extension(c): Extension<Caller>, Path(id): Path<String>, Json(b): Json<Value>) -> ApiResult {
    if c.scope != Scope::Node {
        return Err(ApiError::scope());
    }
    let r = if b["ok"] == true { Ok(b["result"].clone()) } else { Err(b["error"].as_str().unwrap_or("failed on the computer").chars().take(2000).collect()) };
    if app.nodes.answer(&c.token_id, &id, r) { ok(json!({ "ok": true })) } else { Err(ApiError::not_found("Job")) }
}

// ---------------- phone actions (src/phone.rs) ----------------

/// The phone takes its waiting actions (each one only once). Device, client or owner tokens.
async fn phone_actions(State(app): State<AppState>, Extension(c): Extension<Caller>) -> ApiResult {
    ok(json!({ "items": crate::phone::take(&app, &c.token_id) }))
}

/// The phone says how an action went: `{ok, error?, shown?: "screen"|"notification", contact?}`.
async fn phone_action_answer(State(app): State<AppState>, Path(id): Path<String>, Json(b): Json<Value>) -> ApiResult {
    match crate::phone::answer(&app, &id, &b) {
        Ok(true) => ok(json!({ "ok": true })),
        Ok(false) => Err(ApiError::new(StatusCode::CONFLICT, "conflict", "This action was already answered or dropped.")),
        Err(_) => Err(ApiError::not_found("Phone action")),
    }
}

// ---------------- calendar ----------------

/// Skip one day of a repeating event: `{date: "2026-11-20", undo?: true}`. The rest of the repeats are untouched.
async fn skip_event_day(State(app): State<AppState>, Path(id): Path<String>, Json(b): Json<Value>) -> ApiResult {
    crate::calendar::skip_day(&app, &id, b["date"].as_str().unwrap_or_default(), b["undo"] == true).map(|v| Json(v).into_response()).map_err(ApiError::invalid)
}

/// The same for a repeating alarm, reminder or task.
async fn skip_job_day(State(app): State<AppState>, Path(id): Path<String>, Json(b): Json<Value>) -> ApiResult {
    crate::scheduler::skip_job_day(&app, &id, b["date"].as_str().unwrap_or_default(), b["undo"] == true).map(|v| Json(v).into_response()).map_err(ApiError::invalid)
}

#[derive(Deserialize)]
struct EventRange {
    from: Option<String>,
    to: Option<String>,
    q: Option<String>,
    /// `1`: keep the days a repeating event was skipped on, marked `skipped: true` (the calendar page dims them).
    skipped: Option<String>,
}

async fn list_events(State(app): State<AppState>, axum::extract::Query(r): axum::extract::Query<EventRange>) -> ApiResult {
    crate::calendar::list_with(&app, r.from.as_deref(), r.to.as_deref(), r.q.as_deref(), r.skipped.as_deref() == Some("1")).map(|v| Json(v).into_response()).map_err(ApiError::invalid)
}

async fn add_event(State(app): State<AppState>, Json(b): Json<Value>) -> ApiResult {
    crate::calendar::add(&app, &b, false).map(|v| (StatusCode::CREATED, Json(v)).into_response()).map_err(ApiError::invalid)
}

async fn get_event(State(app): State<AppState>, Path(id): Path<String>) -> ApiResult {
    crate::calendar::tool(&app, false, "get_event", &json!({ "id": id })).map(|v| Json(v).into_response()).map_err(|_| ApiError::not_found("Event"))
}

async fn update_event(State(app): State<AppState>, Path(id): Path<String>, Json(b): Json<Value>) -> ApiResult {
    match crate::calendar::update(&app, &id, &b, false) {
        Ok(mut v) => { v.as_object_mut().map(|o| o.remove("previous")); Ok(Json(v).into_response()) }
        Err(e) if e == "no such event" => Err(ApiError::not_found("Event")),
        Err(e) => Err(ApiError::invalid(e)),
    }
}

async fn delete_event(State(app): State<AppState>, Path(id): Path<String>) -> ApiResult {
    crate::calendar::delete(&app, &id).map(|v| Json(json!({ "deleted": v["deleted"] })).into_response()).map_err(|_| ApiError::not_found("Event"))
}

// ---------------- secrets, triggers, export ----------------

async fn list_secrets(State(app): State<AppState>, Extension(c): Extension<Caller>) -> ApiResult {
    need_owner(&c)?;
    ok(json!({ "items": crate::store::secret_names(&app) }))
}

#[derive(Deserialize)]
struct SecretBody {
    value: String,
}

async fn set_secret(State(app): State<AppState>, Extension(c): Extension<Caller>, Path(name): Path<String>, Json(b): Json<SecretBody>) -> ApiResult {
    need_owner(&c)?;
    if b.value.is_empty() || b.value.len() > 10_000 {
        return Err(ApiError::invalid("value must be 1 to 10000 bytes"));
    }
    if crate::store::secret_set(&app, &name, &b.value) { ok(json!({ "name": name, "set": true })) } else { Err(ApiError::invalid("name: letters, digits and _ only")) }
}

async fn delete_secret(State(app): State<AppState>, Extension(c): Extension<Caller>, Path(name): Path<String>) -> ApiResult {
    need_owner(&c)?;
    if crate::store::secret_delete(&app, &name) { ok(json!({ "deleted": name })) } else { Err(ApiError::not_found("Secret")) }
}

/// Outside event starts an unattended turn (docs/01 Triggers). Body is untrusted.
async fn trigger(State(app): State<AppState>, Extension(c): Extension<Caller>, Path(source): Path<String>, body: String) -> ApiResult {
    if !(c.scope == Scope::Owner || (c.scope == Scope::Trigger && c.label == source)) {
        return Err(ApiError::scope());
    }
    if source.is_empty() || source.len() > 40 || !source.chars().all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_') {
        return Err(ApiError::invalid("bad source name"));
    }
    if !app.rate_ok(&format!("trigger:{source}"), app.cfg.limits.triggers_per_minute) {
        return Err(ApiError::new(StatusCode::TOO_MANY_REQUESTS, "rate_limited", "Too many triggers from this source."));
    }
    let thread = format!("system:trigger:{source}");
    app.ensure_thread(&thread, "system", &format!("trigger {source}"));
    let text: String = body.chars().take(20_000).collect();
    let turn = app.start_turn(&thread, "trigger", json!({ "trigger": { "source": source, "body": text } }));
    Ok((StatusCode::ACCEPTED, Json(json!({ "turn_id": turn }))).into_response())
}

async fn export(State(app): State<AppState>, Extension(c): Extension<Caller>) -> ApiResult {
    need_owner(&c)?;
    let zip = crate::export::export_zip(&app);
    Ok((
        [(header::CONTENT_TYPE, HeaderValue::from_static("application/zip")),
         (header::CONTENT_DISPOSITION, HeaderValue::from_static("attachment; filename=\"export.zip\""))],
        zip,
    ).into_response())
}

// ---------------- phone capture (docs/08) ----------------

/// The phone uploads text it made itself. A device token names the device; others may pass `device`.
async fn ambient_upload(State(app): State<AppState>, Extension(c): Extension<Caller>, Json(b): Json<Value>) -> ApiResult {
    let named = b["device"].as_str().map(|d| util::clean_line(d, 3)).filter(|d| !d.is_empty());
    let device = if c.scope == Scope::Device || named.is_none() { if c.label.is_empty() { c.token_id.clone() } else { c.label.clone() } } else { named.unwrap_or_default() };
    let known: Vec<String> = app.secret_words();
    match crate::ambient::ingest(&app.db, &device, &b, &known) {
        Ok(mut v) => {
            let s = crate::ambient::settings(&app.db);
            v["listening"] = json!(s.enabled && s.paused_until <= util::unix_now());
            v["paused_until"] = json!(s.paused_until);
            ok(v)
        }
        Err(crate::ambient::IngestError::Invalid(m)) => Err(ApiError::invalid(m)),
        Err(crate::ambient::IngestError::NotListening(m)) => Err(ApiError::new(StatusCode::CONFLICT, "not_listening", m)),
    }
}

async fn ambient_status(State(app): State<AppState>) -> ApiResult {
    ok(crate::ambient::status(&app.db))
}

async fn ambient_get_settings(State(app): State<AppState>) -> ApiResult {
    ok(json!(crate::ambient::settings(&app.db)))
}

async fn ambient_set_settings(State(app): State<AppState>, Json(b): Json<Value>) -> ApiResult {
    crate::ambient::set_settings(&app.db, &b).map(|s| Json(json!(s)).into_response()).map_err(ApiError::invalid)
}

async fn ambient_pause(State(app): State<AppState>, body: Option<Json<Value>>) -> ApiResult {
    let minutes = body.and_then(|b| b["minutes"].as_i64()).unwrap_or(60);
    ok(json!(crate::ambient::pause(&app.db, minutes)))
}

async fn ambient_resume(State(app): State<AppState>) -> ApiResult {
    ok(json!(crate::ambient::resume(&app.db)))
}

async fn ambient_chunks(State(app): State<AppState>, Query(p): Query<Page>) -> ApiResult {
    ok(paged(crate::ambient::list_chunks(&app.db, p.status.as_deref(), p.after(), p.limit()), p.limit()))
}

#[derive(Deserialize)]
struct ForgetQuery {
    id: Option<String>,
    last_minutes: Option<i64>,
    day: Option<String>,
    all: Option<bool>,
}

/// Delete heard text: one chunk, the last N minutes, one day, or everything.
async fn ambient_forget(State(app): State<AppState>, Query(q): Query<ForgetQuery>) -> ApiResult {
    use crate::ambient::Forget;
    let what = match (q.id, q.last_minutes, q.day, q.all) {
        (Some(id), None, None, None) => Forget::One(id),
        (None, Some(m), None, None) => Forget::LastMinutes(m),
        (None, None, Some(d), None) => Forget::Day(d),
        (None, None, None, Some(true)) => Forget::All,
        _ => return Err(ApiError::invalid("give exactly one of: id, last_minutes, day (YYYY-MM-DD), all=true")),
    };
    ok(json!({ "deleted": crate::ambient::forget(&app.db, &what) }))
}

async fn ambient_picks(State(app): State<AppState>, Query(p): Query<Page>) -> ApiResult {
    ok(paged(crate::ambient::list_picks(&app.db, p.status.as_deref(), p.after(), p.limit()), p.limit()))
}

/// Keep, dismiss or edit a pick. Keeping it makes a memory entry with `user` trust.
async fn ambient_pick_act(State(app): State<AppState>, Path(id): Path<String>, Json(b): Json<Value>) -> ApiResult {
    use crate::ambient::PickAction;
    let action = match b["action"].as_str() {
        Some("keep") => PickAction::Keep,
        Some("dismiss") => PickAction::Dismiss,
        Some("edit") => PickAction::Edit,
        _ => return Err(ApiError::invalid("action must be keep, dismiss or edit")),
    };
    let r = crate::ambient::act_on_pick(&app.db, &id, action, &b).map_err(|e| if e == "no such pick" { ApiError::not_found("Pick") } else { ApiError::invalid(e) })?;
    app.force_index();
    ok(r)
}

/// Look through the waiting text now instead of waiting for the timer.
async fn ambient_process(State(app): State<AppState>) -> ApiResult {
    crate::dreaming::start_run(&app, false).map(|v| (StatusCode::ACCEPTED, Json(v)).into_response()).map_err(|e| ApiError::new(StatusCode::CONFLICT, "conflict", e))
}

// ---------------- app updates (docs/09) ----------------

/// The newest phone app the server holds: `data/app/assistant.apk` and `data/app/version.json`
/// (`versionCode`, `versionName`, `notes`). Size and checksum come from the file itself, so the
/// phone can check what it downloaded. 404 when nothing is published.
async fn app_version(State(app): State<AppState>) -> ApiResult {
    let dir = app.cfg.server_dir.join("app");
    let found = tokio::task::spawn_blocking(move || {
        let meta: Value = serde_json::from_str(&std::fs::read_to_string(dir.join("version.json")).ok()?).ok()?;
        let apk = std::fs::read(dir.join("assistant.apk")).ok()?;
        // What changed in recent versions, newest first (the release script appends to changelog.json).
        let history: Value = std::fs::read_to_string(dir.join("changelog.json")).ok().and_then(|s| serde_json::from_str::<Vec<Value>>(&s).ok())
            .map(|mut v| { v.sort_by_key(|x| std::cmp::Reverse(x["versionCode"].as_i64().unwrap_or(0))); v.truncate(15); Value::Array(v) }).unwrap_or(Value::Array(vec![]));
        Some(json!({ "versionCode": meta["versionCode"], "versionName": meta["versionName"], "notes": meta["notes"], "history": history,
                     "size": apk.len(), "sha256": util::sha256_hex(&apk) }))
    })
    .await
    .ok()
    .flatten();
    found.map(|v| Json(v).into_response()).ok_or_else(|| ApiError::not_found("App update"))
}

async fn app_apk(State(app): State<AppState>) -> Result<Response, ApiError> {
    let path = app.cfg.server_dir.join("app").join("assistant.apk");
    let bytes = tokio::task::spawn_blocking(move || std::fs::read(path).ok()).await.ok().flatten().ok_or_else(|| ApiError::not_found("App update"))?;
    Ok((StatusCode::OK, [(header::CONTENT_TYPE, "application/vnd.android.package-archive"), (header::CACHE_CONTROL, "no-store")], bytes).into_response())
}

// ---------------- sign-in for plugins (docs/03) ----------------

async fn oauth_status(State(app): State<AppState>, Extension(c): Extension<Caller>, Path(provider): Path<String>) -> ApiResult {
    need_owner(&c)?;
    crate::oauth::status(&app, &provider).map(|v| Json(v).into_response()).map_err(|_| ApiError::not_found("Provider"))
}

async fn oauth_disconnect(State(app): State<AppState>, Extension(c): Extension<Caller>, Path(provider): Path<String>) -> ApiResult {
    need_owner(&c)?;
    crate::oauth::disconnect(&app, &provider).map(|v| Json(v).into_response()).map_err(|_| ApiError::not_found("Provider"))
}

async fn oauth_start(State(app): State<AppState>, Extension(c): Extension<Caller>, Path(provider): Path<String>, Json(b): Json<Value>) -> ApiResult {
    need_owner(&c)?;
    if crate::oauth::provider(&provider).is_none() {
        return Err(ApiError::not_found("Provider"));
    }
    crate::oauth::start(&app, &provider, b["client_id"].as_str().unwrap_or_default().trim(), b["redirect_uri"].as_str().unwrap_or_default(), b["client_secret"].as_str().unwrap_or_default().trim()).map(|v| Json(v).into_response()).map_err(ApiError::invalid)
}

async fn oauth_finish(State(app): State<AppState>, Extension(c): Extension<Caller>, Path(provider): Path<String>, Json(b): Json<Value>) -> ApiResult {
    need_owner(&c)?;
    if crate::oauth::provider(&provider).is_none() {
        return Err(ApiError::not_found("Provider"));
    }
    let (code, state) = (b["code"].as_str().unwrap_or_default().to_string(), b["state"].as_str().unwrap_or_default().to_string());
    if code.is_empty() || state.is_empty() {
        return Err(ApiError::invalid("code and state are required"));
    }
    let a = app.clone();
    let r = tokio::task::spawn_blocking(move || crate::oauth::finish(&a, &provider, &code, &state)).await.map_err(|e| ApiError::invalid(e.to_string()))?;
    r.map(|v| Json(v).into_response()).map_err(ApiError::invalid)
}

// ---------------- recipes (src/recipes.rs) ----------------

async fn list_recipes(State(app): State<AppState>) -> ApiResult {
    ok(json!({ "items": crate::recipes::list(&app).iter().map(|r| r.json()).collect::<Vec<_>>() }))
}

async fn delete_recipe(State(app): State<AppState>, Path(id): Path<String>) -> ApiResult {
    if crate::recipes::delete(&app, &id) { ok(json!({ "deleted": id })) } else { Err(ApiError::not_found("Recipe")) }
}
