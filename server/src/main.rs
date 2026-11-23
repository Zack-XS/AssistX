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
