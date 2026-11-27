//! Server binary. Binds 127.0.0.1 only (docs/01 Transport).
//!
//! `agent-core`                      run the server
//! `agent-core tool-run <p> <t>`     run one plugin tool (started by the runner inside the sandbox)
//! `agent-core token create <scope> [label]`   mint a token (CLI, docs/01 Auth)
//! `agent-core eval [--cloud] [--profiles a,b] [--server URL --token-file F]`   model tests (docs/12)

use agent_core::{api, auth, build, config::Config};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("tool-run") if args.len() == 4 => std::process::exit(agent_core::plugins::tool_run_main(&args[2], &args[3])),
        Some("token") => std::process::exit(token_cli(&args[2..])),
        Some("eval") => std::process::exit(agent_core::eval::cli(&args[2..])),
        Some("admin-password") => {
            // Two lines on stdin: the admin username, then the password. Not in a command line or the shell history.
            let mut user = String::new();
            let mut pass = String::new();
            let _ = std::io::stdin().read_line(&mut user);
            let _ = std::io::stdin().read_line(&mut pass);
            let dir = std::path::PathBuf::from(std::env::var("AGENT_DATA_DIR").unwrap_or_else(|_| "data".into()));
            match agent_core::hub::Hub::set_admin_password(&dir, user.trim(), pass.trim_end_matches(['\n', '\r'])) {
                Ok(()) => { println!("admin login set (signed-in admin pages end at the next restart)"); std::process::exit(0) }
                Err(e) => { eprintln!("not set: {e}"); std::process::exit(1) }
            }
        }
        Some("server-password") => {
            // The new password comes from stdin, so it is not in a command line or the shell history.
            let mut line = String::new();
            let _ = std::io::stdin().read_line(&mut line);
            let dir = std::path::PathBuf::from(std::env::var("AGENT_DATA_DIR").unwrap_or_else(|_| "data".into()));
            match agent_core::hub::Hub::set_server_password(&dir, line.trim_end_matches(['\n', '\r'])) {
                Ok(()) => { println!("server password changed"); std::process::exit(0) }
                Err(e) => { eprintln!("not changed: {e}"); std::process::exit(1) }
            }
        }
        Some(other) => {
            eprintln!("unknown command '{other}'. Use: agent-core | agent-core token create <client|owner|trigger|device> [label] | agent-core eval [--cloud]");
            std::process::exit(2);
        }
        None => serve(),
    }
}

async fn serve_accounts(cfg: Config) {
    let hub = match agent_core::hub::Hub::open(cfg, Box::new(|c| agent_core::make_model(c))) {
        Ok(h) => h,
        Err(e) => {
            eprintln!("startup failed: {e}");
            std::process::exit(1);
        }
    };
    match hub.ensure_server_token() {
        Ok(Some(p)) => println!("first run: server token written to {} (mode 0600). Enter it in the app to connect.", p.display()),
        Ok(None) => {}
        Err(e) => {
            eprintln!("server token: {e}");
            std::process::exit(1);
        }
    }
    if let Some((code, path)) = hub.ensure_admin_setup() {
        println!("no admin login yet: open /admin and use the setup code {code} (also in {}).", path.display());
    }
    // Old one-owner files left in the data folder (security review #11): the database key sits beside its database.
    // They are not deleted here (they may be the only copy); this says what is left until the owner removes them.
    let left: Vec<&str> = ["core.db", "db.key", "owner.token", "memory", "reports", "core.db.plain-backup"].into_iter().filter(|f| hub.cfg.data_dir.join(f).exists()).collect();
    if !left.is_empty() {
        eprintln!("SECURITY WARNING: old one-owner data in {}: {}. Move it into an account, then delete it.", hub.cfg.data_dir.display(), left.join(", "));
    }
    // Idle lock (security review #1): an account nothing has reached for this long is locked, so its key leaves
    // memory. The phone's background check reaches it every 15 minutes, so this only bites when the phone is gone.
    // `AGENT_IDLE_LOCK_HOURS`, default 24; 0 = never.
    let idle_hours: i64 = std::env::var("AGENT_IDLE_LOCK_HOURS").ok().and_then(|v| v.parse().ok()).unwrap_or(24);
    if idle_hours > 0 {
        let h = hub.clone();
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(std::time::Duration::from_secs(300));
            loop {
                tick.tick().await;
                h.lock_idle(idle_hours * 3600);
            }
        });
    }
    let sandbox = agent_core::sandbox::probe().map(String::from);
    if hub.cfg.sandbox != "macos" && hub.cfg.sandbox != "linux" {
        eprintln!("WARNING: sandbox is '{}'.", hub.cfg.sandbox);
    }
    let _ = sandbox;
    let addr = hub.cfg.bind_addr();
    let listener = match tokio::net::TcpListener::bind(&addr).await {
        Ok(l) => l,
        Err(e) => {
            eprintln!("bind {addr} failed: {e}");
            std::process::exit(1);
        }
    };
    if hub.cfg.bind != "127.0.0.1" {
        eprintln!("WARNING: listening on {}, not only on this computer. Reach this through a private tunnel (for example Tailscale).", hub.cfg.bind);
    }
    println!("listening on http://{addr}  accounts: on  accounts waiting or active: {}", hub.list().len());
    let router = agent_core::hub_http::router(hub);
    let shutdown = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    if let Err(e) = axum::serve(listener, router.into_make_service_with_connect_info::<std::net::SocketAddr>()).with_graceful_shutdown(shutdown).await {
        eprintln!("server error: {e}");
    }
}

fn token_cli(args: &[String]) -> i32 {
    let (app, _) = match build(Config::from_env()) {
        Ok(x) => x,
        Err(e) => {
            eprintln!("{e}");
            return 1;
        }
    };
    match (args.first().map(String::as_str), args.get(1).and_then(|s| auth::Scope::parse(s))) {
        (Some("create"), Some(scope)) => {
            let (id, token) = auth::create_token(&app.db, scope, args.get(2).map(String::as_str).unwrap_or(""));
            println!("{id} {token}");
            0
        }
        (Some("list"), _) => {
            for t in auth::list_tokens(&app.db) {
                println!("{} {} {} {}{}", t.id, t.scope, t.label, t.created, if t.revoked { " revoked" } else { "" });
            }
            0
        }
        (Some("revoke"), _) if args.len() > 1 => {
            if auth::revoke_token(&app.db, &args[1]) { 0 } else { 1 }
        }
        _ => {
            eprintln!("usage: agent-core token create <client|owner|trigger|device> [label] | list | revoke <id>");
            2
        }
    }
}

#[tokio::main]
async fn serve() {
    let cfg = Config::from_env();
    // Accounts are the default (docs/15). `AGENT_SINGLE_USER=1` keeps the old one-owner mode for development.
    if std::env::var("AGENT_SINGLE_USER").map(|v| v != "1").unwrap_or(true) {
        return serve_accounts(cfg).await;
    }
    let (app, new_token) = match build(cfg) {
        Ok(x) => x,
        Err(e) => {
            eprintln!("startup failed: {e}");
            std::process::exit(1);
        }
    };
    for p in app.manifest_problems() {
        eprintln!("plugin note: {p}");
    }
    if app.cfg.sandbox == "linux" {
        eprintln!("note: Linux sandbox (bubblewrap). Network tools reach their allowlist proxy through a socket file in their scratch folder.");
    } else if app.cfg.sandbox != "macos" {
        eprintln!("WARNING: sandbox is '{}'. Tools {}.", app.cfg.sandbox,
            if app.cfg.sandbox == "off" { "run UNSANDBOXED" } else { "are disabled on this system" });
    }
    if let Some(path) = new_token {
        println!("first run: owner token written to {path} (mode 0600)");
    }
    let listener = match tokio::net::TcpListener::bind(app.cfg.bind_addr()).await {
        Ok(l) => l,
        Err(e) => {
            eprintln!("bind {} failed: {e}", app.cfg.bind_addr());
            std::process::exit(1);
        }
    };
    let usable: Vec<String> = app.manifests().keys().filter(|k| app.plugin_usable(k)).cloned().collect();
    if app.cfg.bind != "127.0.0.1" {
        eprintln!("WARNING: listening on {}, not only on this computer. The token is the only lock and plain HTTP shows it to the network. Reach this through a private tunnel (for example Tailscale) and keep AGENT_ALLOWED_HOSTS short.", app.cfg.bind);
    }
    println!("listening on http://{}  model: {}  sandbox: {}  plugins: {}", app.cfg.bind_addr(), app.model.name(), app.cfg.sandbox, usable.join(", "));
    app.spawn_housekeeping();
    let router = api::router(app);
    let shutdown = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    if let Err(e) = axum::serve(listener, router).with_graceful_shutdown(shutdown).await {
        eprintln!("server error: {e}");
    }
}
