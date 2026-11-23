//! Lists (plugin `lists`): the tools, undo, the index line, a model turn, and the `/v1/lists` routes.
//! Uses the real router, called directly.

use agent_core::auth::{self, Scope};
use agent_core::config::Config;
use agent_core::model::Mock;
use agent_core::{App, api, db::Db, lists};
use axum::Router;
use axum::body::{Body, to_bytes};
use axum::http::Request;
use serde_json::{Value, json};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tower::ServiceExt;

struct T {
    app: Arc<App>,
    mock: Arc<Mock>,
    router: Router,
    owner: String,
}

fn start(name: &str) -> T {
    let dir: PathBuf = std::env::temp_dir().join(format!("lists-{name}-{}", agent_core::util::random_hex(4)));
    std::fs::create_dir_all(dir.join("files")).unwrap();
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut cfg = Config::from_env();
    cfg.port = 9999;
    cfg.bind = "127.0.0.1".into();
    cfg.allowed_hosts = vec![];
    cfg.allowed_origins = vec![];
    cfg.data_dir = dir.clone();
    cfg.server_dir = dir.clone();
    cfg.plugins_dir = root.join("plugins");
    cfg.prompts_dir = root.join("prompts");
    cfg.model.kind = "mock".into();
    cfg.sandbox = "off".into();
    cfg.timezone = "UTC".into();
    cfg.dream_every_mins = 0;
    cfg.files_root = dir.join("files");
    cfg.runner_exe = PathBuf::from(env!("CARGO_BIN_EXE_agent-core"));
    let mock = Arc::new(Mock::new(&[]));
    let app = App::new(cfg, Db::open_memory().unwrap(), mock.clone());
    let (_, owner) = auth::create_token(&app.db, Scope::Owner, "test");
    T { router: api::router(app.clone()), app, mock, owner }
}

impl T {
    async fn raw(&self, method: &str, path: &str, body: Option<Value>) -> (u16, String) {
        let req = Request::builder().method(method).uri(path).header("host", "127.0.0.1:9999")
            .header("authorization", format!("Bearer {}", self.owner)).header("content-type", "application/json")
            .body(Body::from(body.map(|b| b.to_string()).unwrap_or_default())).unwrap();
        let resp = self.router.clone().oneshot(req).await.unwrap();
        let code = resp.status().as_u16();
        let bytes = to_bytes(resp.into_body(), 1 << 20).await.unwrap();
        (code, String::from_utf8_lossy(&bytes).to_string())
    }

    async fn call(&self, method: &str, path: &str, body: Option<Value>) -> (u16, Value) {
        let (c, t) = self.raw(method, path, body).await;
        (c, serde_json::from_str(&t).unwrap_or(Value::Null))
    }

    fn tool(&self, tool: &str, args: Value) -> Result<Value, String> {
        lists::tool(&self.app, false, tool, &args)
    }
}

fn texts(v: &Value) -> Vec<String> {
    v["items"].as_array().unwrap().iter().map(|i| format!("{}{}", if i["done"] == true { "x " } else { "" }, i["text"].as_str().unwrap())).collect()
}

#[tokio::test(flavor = "multi_thread")]
async fn tools_add_tick_remove_clear_rename_delete_and_undo_each() {
    let t = start("tools");
    // add makes the list; the same open words are not added twice
    let a = t.tool("add_items", json!({ "list": "shopping list", "items": ["Milk", { "text": "Eggs", "quantity": 12 }, { "text": "Rice", "quantity": "2 kg", "due": "2026-11-20" }] })).unwrap();
    assert_eq!(a["list"], "Shopping");
    assert_eq!(a["created_list"], true);
    assert_eq!(a["ids"].as_array().unwrap().len(), 3);
    let again = t.tool("add_items", json!({ "list": "groceries", "items": ["milk", "bread"] })).unwrap();
    assert_eq!(again["list"], "Shopping", "groceries is the shopping list");
    assert_eq!(again["already_there"], json!(["milk"]));
    assert_eq!(again["added"], json!(["bread"]));
    assert!(t.tool("add_items", json!({ "list": "shopping", "items": [{ "text": "x", "due": "Friday" }] })).is_err(), "due must be a date");

    let show = t.tool("show_list", json!({ "list": "shopping" })).unwrap();
    assert_eq!(texts(&show), ["Milk", "Eggs", "Rice", "bread"]);
    let eggs = show["items"].as_array().unwrap().iter().find(|i| i["text"] == "Eggs").unwrap().clone();
    assert_eq!(eggs["quantity"], "12");

    // tick by words, partial words, and undo puts them back
    let c = t.tool("check_items", json!({ "list": "shopping", "items": ["milk", "bre", "caviar"] })).unwrap();
    assert_eq!(c["checked"], json!(["Milk", "bread"]));
    assert_eq!(c["not_found"], json!(["caviar"]));
    assert_eq!(texts(&t.tool("show_list", json!({ "list": "shopping" })).unwrap()), ["Eggs", "Rice", "x Milk", "x bread"], "ticked items go last");
    t.tool("uncheck_items", json!({ "args": {}, "result": c })).unwrap();
    assert_eq!(t.tool("show_list", json!({ "list": "shopping" })).unwrap()["list"]["open"], 4);
    assert!(t.tool("check_items", json!({ "list": "shopping", "items": ["caviar"] })).is_err());

    // adding a ticked item opens it again
    t.tool("check_items", json!({ "list": "shopping", "items": ["Milk"] })).unwrap();
    let re = t.tool("add_items", json!({ "list": "shopping", "items": ["milk"] })).unwrap();
    assert_eq!(re["reopened"].as_array().unwrap().len(), 1);
    t.tool("undo_add_items", json!({ "args": {}, "result": re })).unwrap();
    assert_eq!(t.tool("show_list", json!({ "list": "shopping" })).unwrap()["list"]["done"], 1, "undo ticks it again");

    // clear and remove, then restore
    let cl = t.tool("clear_checked", json!({ "list": "shopping" })).unwrap();
    assert_eq!(cl["cleared"], 1);
    let rm = t.tool("remove_items", json!({ "list": "shopping", "items": ["Rice"] })).unwrap();
    assert_eq!(texts(&t.tool("show_list", json!({ "list": "shopping" })).unwrap()), ["Eggs", "bread"]);
    t.tool("restore_items", json!({ "args": {}, "result": rm })).unwrap();
    t.tool("restore_items", json!({ "args": {}, "result": cl })).unwrap();
    let back = t.tool("show_list", json!({ "list": "shopping" })).unwrap();
    assert_eq!(texts(&back), ["Eggs", "Rice", "bread", "x Milk"]);
    assert_eq!(back["items"][1]["due"], "2026-11-20", "restored as it was");

    // rename, delete and their undo
    let to = t.tool("add_items", json!({ "list": "to do", "items": ["Call the bank"] })).unwrap();
    assert_eq!(to["list"], "To-do");
    assert!(t.tool("rename_list", json!({ "list": "todo", "name": "shopping" })).is_err(), "no two lists with one name");
    let rn = t.tool("rename_list", json!({ "list": "todo", "name": "Errands" })).unwrap();
    assert_eq!(rn["list"], "Errands");
    t.tool("restore_list", json!({ "args": {}, "result": rn })).unwrap();
    let del = t.tool("delete_list", json!({ "list": "to-do" })).unwrap();
    assert_eq!(t.tool("list_lists", json!({})).unwrap()["lists"].as_array().unwrap().len(), 1);
    let err = t.tool("show_list", json!({ "list": "to-do" })).unwrap_err();
    assert!(err.contains("Shopping"), "the error names the lists there are: {err}");
    t.tool("restore_list", json!({ "args": {}, "result": del })).unwrap();
    assert_eq!(texts(&t.tool("show_list", json!({ "list": "to-do" })).unwrap()), ["Call the bank"], "items came back with the list");

    // the index shows counts only, as outside content
    let line = lists::index_line(&t.app).unwrap();
    assert!(line.contains("- Shopping: 3 open") && line.contains("- To-do: 1 open") && line.contains("<outside source=\"lists:open\""), "{line}");
    assert!(!line.contains("Eggs"));
}

#[tokio::test(flavor = "multi_thread")]
async fn routes_create_add_tick_share_and_delete() {
    let t = start("routes");
    let (code, made) = t.call("POST", "/v1/lists", Some(json!({ "name": "packing list", "items": ["Socks"] }))).await;
    assert_eq!(code, 201, "{made}");
    assert_eq!(made["list"]["name"], "Packing");
    let id = made["list"]["id"].as_str().unwrap().to_string();
    assert_eq!(t.call("POST", "/v1/lists", Some(json!({ "name": "Packing" }))).await.0, 400, "names are unique");

    let (code, add) = t.call("POST", &format!("/v1/lists/{id}/items"), Some(json!({ "text": "Charger", "note": "the long cable" }))).await;
    assert_eq!(code, 201, "{add}");
    let item = add["ids"][0].as_str().unwrap().to_string();
    assert_eq!(t.call("POST", &format!("/v1/lists/{id}/items"), Some(json!({ "items": ["Hat", "Map"] }))).await.0, 201);
    assert_eq!(t.call("POST", &format!("/v1/lists/{id}/items"), Some(json!({ "text": "" }))).await.0, 400);

    let (code, it) = t.call("PATCH", &format!("/v1/lists/{id}/items/{item}"), Some(json!({ "done": true, "quantity": "2" }))).await;
    assert_eq!(code, 200, "{it}");
    assert_eq!(it["done"], true);
    assert_eq!(t.call("PATCH", &format!("/v1/lists/{id}/items/nope"), Some(json!({ "done": true }))).await.0, 404);

    let (_, l) = t.call("GET", "/v1/lists", None).await;
    assert_eq!(l["lists"][0]["open"], 3);
    assert_eq!(l["lists"][0]["done"], 1);

    let (code, text) = t.raw("GET", &format!("/v1/lists/{id}/text"), None).await;
    assert_eq!(code, 200);
    assert_eq!(text, "Packing\n- [ ] Socks\n- [ ] Hat\n- [ ] Map\n\nDone:\n- [x] Charger (2) - the long cable\n");

    let (code, r) = t.call("PATCH", &format!("/v1/lists/{id}"), Some(json!({ "name": "Trip" }))).await;
    assert_eq!(code, 200);
    assert_eq!(r["list"]["name"], "Trip");
    let hat = r["items"].as_array().unwrap().iter().find(|i| i["text"] == "Hat").unwrap()["id"].as_str().unwrap().to_string();
    assert_eq!(t.call("DELETE", &format!("/v1/lists/{id}/items/{hat}"), None).await.0, 200);
    let (_, cl) = t.call("POST", &format!("/v1/lists/{id}/clear"), None).await;
    assert_eq!(cl["cleared"], 1);
    assert_eq!(texts(&t.call("GET", &format!("/v1/lists/{id}"), None).await.1), ["Socks", "Map"]);

    assert_eq!(t.call("DELETE", &format!("/v1/lists/{id}"), None).await.0, 200);
    assert_eq!(t.call("GET", &format!("/v1/lists/{id}"), None).await.0, 404);
    assert_eq!(t.call("GET", &format!("/v1/lists/{id}/text"), None).await.0, 404);
    // no token, no list
    let req = Request::builder().uri("/v1/lists").header("host", "127.0.0.1:9999").body(Body::empty()).unwrap();
    assert_eq!(t.router.clone().oneshot(req).await.unwrap().status().as_u16(), 401);
}

#[tokio::test(flavor = "multi_thread")]
async fn the_model_adds_items_sees_counts_in_the_index_and_undo_removes_them() {
    let t = start("model");
    let run = |tool: &str, args: Value| json!({ "tool_calls": [{ "name": "run_plugin_tool", "args": { "plugin": "lists", "tool": tool, "args": args, "reason": "test" } }] });
    // "shopping list" is a quick command, so no load step is needed.
    t.mock.push(&[run("add_items", json!({ "list": "shopping", "items": ["Apples", "Pears"] })), json!({ "text": "Added." })]);
    let (_, th) = t.call("POST", "/v1/threads", Some(json!({ "title": "test" }))).await;
    let th = th["id"].as_str().unwrap().to_string();
    let send = |text: &'static str| { let th = th.clone(); let t = &t; async move {
        let (code, v) = t.call("POST", &format!("/v1/threads/{th}/messages"), Some(json!({ "text": text }))).await;
        assert_eq!(code, 202, "{v}");
        let turn = v["turn_id"].as_str().unwrap().to_string();
        let t0 = Instant::now();
        loop {
            let (_, v) = t.call("GET", &format!("/v1/turns/{turn}"), None).await;
            if v["status"] == "done" { return v; }
            assert!(t0.elapsed() < Duration::from_secs(15) && v["status"] != "error", "turn: {v}");
            tokio::time::sleep(Duration::from_millis(30)).await;
        }
    } };
    send("add apples and pears to my shopping list please").await;
    let (_, acts) = t.call("GET", &format!("/v1/actions?thread_id={th}"), None).await;
    let act = acts["items"].as_array().unwrap().iter().find(|a| a["tool"] == "add_items").cloned().unwrap();
    assert_eq!(act["decision"], "auto");
    t.mock.push(&[json!({ "text": "Two things." })]);
    send("what is left?").await;
    let sys = t.mock.requests().last().unwrap().system.clone();
    assert!(sys.contains("## Lists with open items") && sys.contains("- Shopping: 2 open"), "{sys}");
    let (code, u) = t.call("POST", &format!("/v1/actions/{}/undo", act["id"].as_str().unwrap()), None).await;
    assert_eq!(code, 200, "{u}");
    assert!(t.call("GET", "/v1/lists", None).await.1["lists"].as_array().unwrap().is_empty(), "the list made by the call went with it");
}
