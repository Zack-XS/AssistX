//! Design showcase: live HTML parts the builder appends under each
//! `docs/design/*.md` page. The markdown stays plain; the builder injects this.

use std::collections::HashMap;

/// Append-only HTML for a design page, based on its file stem. None for pages
/// that get no showcase.
pub fn showcase_for(stem: &str, icons: &HashMap<String, String>) -> Option<String> {
    match stem {
        "color" | "tokens" => Some(color_showcase()),
        "typography" => Some(type_showcase()),
        "spacing-and-layout" => Some(spacing_showcase()),
        "motion" => Some(motion_showcase()),
        "icons" => Some(icons_showcase(icons)),
        "components" => Some(components_showcase(icons)),
        "screens" => Some(screens_showcase(icons)),
        "smart-ui" => Some(smart_ui_showcase()),
        "haptics" => Some(haptics_showcase()),
        _ => None,
    }
}

/// Haptics: one button per token. On an Android phone each button buzzes
/// (navigator.vibrate, an approximation of the native feel); the pad also
/// pulses so the meaning shows without a motor.
fn haptics_showcase() -> String {
    let mut s = open("Try the haptic set");
    s.push_str(r#"<p class="demo-label">Open this page on an Android phone to feel each one. Everywhere else the pad shows it.</p><div class="hap-grid">"#);
    for (name, pattern, what) in [
        ("tick", "5", "slider steps, snaps"),
        ("click", "10", "toggle, tab, send"),
        ("confirm", "10,45,14", "saved, answer done"),
        ("reject", "25,55,25", "error, refused"),
        ("heavy", "22", "long press, drag"),
        ("attention", "6,30,12", "approval needed"),
    ] {
        s.push_str(&format!(
            r#"<button type="button" class="hap-pad hap-{name}" data-demo="buzz" data-pattern="{pattern}" aria-label="Feel haptic-{name}"><span class="hap-dot"></span><span class="hap-name">haptic-{name}</span><span class="hap-use">{what}</span></button>"#
        ));
    }
    s.push_str(r#"</div><p class="demo-label">Browser buzzes are rough copies. The app uses Android's tuned effects (CONFIRM, REJECT…).</p></section>"#);
    s
}

fn open(title: &str) -> String {
    format!(
        r#"<section class="showcase"><h2 class="show-title">{title}</h2>"#
    )
}

fn color_showcase() -> String {
    // (name, light, dark)
    let groups: &[(&str, &[(&str, &str, &str)])] = &[
        ("Surfaces", &[
            ("page", "#FAFAFA", "#000000"),
            ("card", "#FFFFFF", "#111111"),
            ("raised", "#FFFFFF", "#1A1A1A"),
            ("field", "#F1F1F1", "#1A1A1A"),
            ("bubble", "#EDEDED", "#1C1C1C"),
            ("code", "#F4F4F4", "#0A0A0A"),
            ("line", "#E4E4E4", "#262626"),
            ("scrim", "rgba(10,10,10,.32)", "rgba(0,0,0,.56)"),
        ]),
        ("Text & ink", &[
            ("text", "#0A0A0A", "#EDEDED"),
            ("mute", "#666666", "#8F8F8F"),
            ("ink", "#0A0A0A", "#EDEDED"),
            ("on-ink", "#FFFFFF", "#0A0A0A"),
        ]),
        ("Accent", &[
            ("accent", "#0A0A0A", "#EDEDED"),
            ("on-accent", "#FFFFFF", "#0A0A0A"),
            ("accent-soft", "#0A0A0A0F", "#EDEDED1A"),
        ]),
        ("State", &[
            ("ok", "#157A42", "#43C076"),
            ("warn", "#8A5D00", "#E2A826"),
            ("bad", "#B8291F", "#FF6B63"),
        ]),
    ];
    let mut s = open("Live: colour swatches");
    for (gname, items) in groups {
        s.push_str(&format!("<h3>{gname}</h3><p class=\"demo-label\">Light</p><div class=\"swatch-row\">"));
        for (n, l, _d) in *items {
            s.push_str(&swatch(n, l));
        }
        s.push_str("</div><p class=\"demo-label\">Dark</p><div class=\"swatch-row dark-swatches\">");
        for (n, _l, d) in *items {
            s.push_str(&swatch(n, d));
        }
        s.push_str("</div>");
    }
    s.push_str(r#"<p class="demo-label">Tap a hex value to copy it.</p><div class="toast" id="color-toast">Copied</div>"#);
    s.push_str("</section>");
    s
}

fn swatch(name: &str, val: &str) -> String {
    format!(
        r##"<div class="swatch"><div class="chip" style="background:{val}"></div><div class="meta"><div class="name">{name}</div><button class="val" type="button" data-demo="copy" data-copy="{val}" data-target="#color-toast" title="Copy {val}">{val}</button></div></div>"##
    )
}

fn type_showcase() -> String {
    let rows: &[(&str, &str, &str)] = &[
        ("display", "34px / 700", "The screen title"),
        ("title", "22px / 600", "A card title"),
        ("headline", "17px / 600", "A row title"),
        ("body", "16px / 400", "Normal body text for reading."),
        ("small", "14px / 400", "A second line, or a chip"),
        ("caption", "12px / 500", "A tab label, a time"),
        ("mono", "13px / 400", "code and tool steps"),
        ("number", "44px / 500", "07:30"),
    ];
    let mut s = open("Live: type scale");
    s.push_str("<div class=\"type-scale\">");
    for (name, spec, sample) in rows {
        let style = match *name {
            "display" => "font-size:34px;line-height:37px;font-weight:700;letter-spacing:-0.02em",
            "title" => "font-size:22px;line-height:28px;font-weight:600;letter-spacing:-0.02em",
            "headline" => "font-size:17px;line-height:24px;font-weight:600",
            "body" => "font-size:16px;line-height:23px;font-weight:400",
            "small" => "font-size:14px;line-height:20px;font-weight:400",
            "caption" => "font-size:12px;line-height:16px;font-weight:500;letter-spacing:.01em",
            "mono" => "font-family:var(--font-mono);font-size:13px;line-height:19px",
            "number" => "font-size:44px;line-height:48px;font-weight:500;font-variant-numeric:tabular-nums",
            _ => "",
        };
        s.push_str(&format!(
            r#"<div class="row"><div class="lbl">{name}<br>{spec}</div><div style="{style}">{sample}</div></div>"#
        ));
    }
    s.push_str("</div></section>");
    s
}

fn spacing_showcase() -> String {
    let spaces: &[(u8, u32)] = &[
        (1, 2), (2, 4), (3, 6), (4, 8), (5, 12), (6, 16), (7, 20), (8, 24), (9, 32), (10, 48),
    ];
    let radii: &[(u8, u32)] = &[
        (6, 6), (10, 10), (14, 14), (20, 20), (22, 22), (30, 30),
    ];
    let mut s = open("Live: spacing and corners");
    s.push_str("<h3>Spacing scale</h3>");
    for (n, v) in spaces {
        s.push_str(&format!(
            r#"<div><span class="demo-label">space-{n} · {v}px</span><div class="space-bar" style="width:{v}px;height:{v}px;min-width:{v}px"></div></div>"#
        ));
    }
    s.push_str("<h3>Corner radii</h3><div class=\"radius-row\">");
    for (n, v) in radii {
        s.push_str(&format!(
            r#"<figure><div class="radius-sample" style="border-radius:{v}px"></div><figcaption>radius-{n} {v}</figcaption></figure>"#
        ));
    }
    s.push_str("</div></section>");
    s
}

fn motion_showcase() -> String {
    let mut s = open("Live: motion");
    s.push_str(
        r#"<style>
@keyframes fadeRise{from{opacity:0;transform:translateY(16px)}to{opacity:1;transform:translateY(0)}}
@keyframes slideAcross{from{left:0}to{left:calc(100% - 24px)}}
@keyframes checkDraw{from{stroke-dashoffset:48}to{stroke-dashoffset:0}}
.run .mover{animation:slideAcross var(--dur-slow) var(--ease-spring)}
.run .riser{animation:fadeRise var(--dur-page) var(--ease-out)}
.run .check-path{stroke-dasharray:48;stroke-dashoffset:48;animation:checkDraw var(--dur-slow) var(--ease-out) forwards}
</style>"#,
    );
    s.push_str(r#"<div class="motion-card"><h3>Content rises in</h3><p class="demo-label">dur-page · ease-out · 16px rise</p><div class="motion-stage"><div class="mover riser" style="left:8px;width:80%;height:24px;background:var(--accent);border-radius:var(--radius-md)"></div></div></div>"#);
    s.push_str(r#"<div class="motion-card"><h3>Spring slide</h3><p class="demo-label">ease-spring overshoot</p><div class="motion-stage"><div class="mover" style="width:24px"></div></div></div>"#);
    s.push_str(r#"<div class="motion-card"><h3>Check draws</h3><p class="demo-label">stroke-dash · dur-slow</p><div class="motion-stage" style="display:flex;align-items:center;justify-content:center"><svg width="56" height="56" viewBox="0 0 48 48" fill="none" stroke="var(--ok)" stroke-width="4" stroke-linecap="round" stroke-linejoin="round"><path class="check-path" d="M10 26l10 10 18-22"/></svg></div></div>"#);
    s.push_str(
        r#"<button class="btn primary" data-demo="play-motion" data-target=".motion-card .motion-stage">Replay demos</button>"#,
    );
    s.push_str("<p class=\"demo-label\">Reduced-motion is respected: nothing moves when your system asks for less motion.</p>");
    s.push_str("</section>");
    s
}

fn icons_showcase(icons: &HashMap<String, String>) -> String {
    // (svg file name, label shown) — matching the meanings table in icons.md.
    let grid: &[(&str, &str)] = &[
        ("home", "Home · Today"),
        ("calendar", "Calendar"),
        ("clock", "Alarms, time"),
        ("list", "Lists"),
        ("zap", "Tasks (bolt)"),
        ("message-circle", "Chat"),
        ("settings", "You (gear)"),
        ("bell", "Notices"),
        ("lightbulb", "Noticed"),
        ("check", "Done"),
        ("sliders-horizontal", "Appearance"),
        ("mic", "Voice"),
        ("puzzle", "Skills"),
        ("brain", "Models"),
        ("folder", "Memory"),
        ("shield", "Privacy"),
        ("heart", "Health"),
        ("phone", "Access"),
        ("box", "Computers"),
        ("map-pin", "Location (pin)"),
        ("activity", "Status (pulse)"),
        ("map", "Memory map"),
    ];
    let mut s = open("Live: icon grid");
    s.push_str("<div class=\"icon-grid\">");
    for (name, label) in grid {
        if let Some(svg) = icons.get(*name) {
            s.push_str(&format!(
                r#"<div class="icon-cell"><span class="icon">{svg}</span><span class="name">{label}</span></div>"#
            ));
        }
    }
    s.push_str("</div></section>");
    s
}

fn components_showcase(icons: &HashMap<String, String>) -> String {
    let send = icons.get("send").map(|s| s.as_str()).unwrap_or("");
    let mut s = open("Live: components");
    s.push_str("<h3>Buttons</h3><div class=\"demo-row\">");
    s.push_str(r#"<button class="btn primary">Primary</button>"#);
    s.push_str(r#"<button class="btn secondary">Secondary</button>"#);
    s.push_str(r#"<button class="btn ghost">Ghost</button>"#);
    s.push_str(r#"<button class="btn danger">Danger</button>"#);
    s.push_str(r#"<button class="btn primary" disabled>Loading…</button>"#);
    s.push_str(&format!(
        r#"<button class="btn icon primary" aria-label="Send">{send}</button>"#
    ));
    s.push_str("</div>");

    s.push_str("<h3>Chat bubbles</h3><div class=\"bubbles\">");
    s.push_str(r#"<div class="bubble me">Hello, what can you do today?</div>"#);
    s.push_str(r#"<div class="bubble bot">I can read your calendar, set reminders and start work for you. Want me to plan your day?</div>"#);
    s.push_str("</div>");

    s.push_str("<h3>Card</h3><div class=\"card\"><h3>Next event</h3><p class=\"demo-label\">Team sync · 10:00 · in 12 min</p></div>");

    s.push_str("<h3>Sheet</h3>");
    s.push_str(r##"<button class="btn secondary" data-demo="sheet-open" data-target="#demo-sheet" data-scrim="#demo-scrim">Open sheet</button>"##);
    s.push_str(r##"<div class="sheet-scrim" id="demo-scrim" data-demo="sheet-close" data-target="#demo-sheet" data-scrim="#demo-scrim"></div>"##);
    s.push_str(r##"<div class="sheet-panel" id="demo-sheet"><div class="demo-label">Pick a model</div><div style="margin:var(--space-4) 0"><button class="btn secondary" style="width:100%;justify-content:flex-start">Qwen 9B — local</button></div><button class="btn ghost" data-demo="sheet-close" data-target="#demo-sheet" data-scrim="#demo-scrim">Cancel</button></div>"##);

    s.push_str("<h3>Toast</h3>");
    s.push_str(r##"<button class="btn secondary" data-demo="toast" data-target="#demo-toast">Show toast</button>"##);
    s.push_str(r#"<div class="toast" id="demo-toast">Saved to memory</div>"#);

    // The real chat screen, light and dark, so the parts above can be seen in place.
    s.push_str("<h3>In the app</h3>");
    s.push_str(r#"<div class="phone-row"><figure class="real-shot"><img src="../assets/screens/light/chat.png" alt="Assistant screen, light" width="390" height="844" loading="lazy"><figcaption class="cap">Light</figcaption></figure><figure class="real-shot"><img src="../assets/screens/dark/chat.png" alt="Assistant screen, dark" width="390" height="844" loading="lazy"><figcaption class="cap">Dark</figcaption></figure></div>"#);

    s.push_str("</section>");
    s
}

// --- design in action: screens, smart-ui, and shared phone-frame helpers ---

const ALERT_SVG: &str = r#"<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"><path d="M10.3 3.9 1.8 18a2 2 0 0 0 1.7 3h17a2 2 0 0 0 1.7-3L13.7 3.9a2 2 0 0 0-3.4 0z"/><path d="M12 9v4"/><path d="M12 17h.01"/></svg>"#;

/// Screens page: real screenshots of the app, light and dark side by side.
/// Design pages sit one folder deep, so the path goes up one level.
fn screens_showcase(_icons: &HashMap<String, String>) -> String {
    let shot = |theme: &str, cap: &str| {
        format!(r#"<figure class="real-shot"><img src="../assets/screens/{theme}/home.png" alt="Home screen, {cap}" width="390" height="844" loading="lazy"><figcaption class="cap">{cap}</figcaption></figure>"#)
    };
    format!(
        r#"<section class="showcase"><h2 class="show-title">The Home screen, light and dark</h2><div class="phone-row">{}{}</div><p class="demo-label">Screenshots of the app page, taken with sample data.</p></section>"#,
        shot("light", "Light"),
        shot("dark", "Dark"),
    )
}

/// Smart UI page: a Today card that switches between loading, empty, error and
/// success -- the four states every screen shows.
fn smart_ui_showcase() -> String {
    let btns = |state: &str, label: &str, pressed: bool| -> String {
        let p = if pressed { "true" } else { "false" };
        format!(
            r##"<button class="btn secondary" type="button" data-demo="ui-state" data-target="#smart-card" data-state="{state}" aria-pressed="{p}">{label}</button>"##
        )
    };
    format!(
        r##"<section class="showcase"><h2 class="show-title">Live: adaptive states</h2><p class="demo-label">A Today card in every state. Pick one.</p><div class="ui-state-btns">{load}{empty}{err}{ok}</div><div class="card smart-card" id="smart-card" data-state="loading"><div class="ui-state ui-loading"><div class="skel"></div><div class="skel w80"></div><div class="skel w60"></div></div><div class="ui-state ui-empty"><p class="demo-label">Nothing coming up.</p><button class="btn primary" type="button">Add event</button></div><div class="ui-state ui-error"><p class="ui-err"><span class="ui-ico">{alert}</span> Couldn't load.</p><button class="btn secondary" type="button">Try again</button></div><div class="ui-state ui-success"><h3>Team sync</h3><p class="demo-label">10:00 · in 12 min</p></div></div><p class="demo-label">Loading shows skeleton lines, never a blank screen. Empty says what will appear. Error says what to do. Success shows the real thing.</p></section>"##,
        load = btns("loading", "Loading", true),
        empty = btns("empty", "Empty", false),
        err = btns("error", "Error", false),
        ok = btns("success", "Success", false),
        alert = ALERT_SVG,
    )
}
