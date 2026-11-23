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
        _ => None,
    }
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
            ("page", "#E9E9ED", "#000000"),
            ("card", "#FFFFFF", "#141417"),
            ("raised", "#FFFFFF", "#1C1C21"),
            ("field", "#F2F2F5", "#1C1C21"),
            ("bubble", "#E7E7EB", "#1F1F24"),
            ("code", "#F0F0F4", "#0B0B0E"),
            ("line", "#E3E3E8", "#26262B"),
            ("scrim", "rgba(12,12,16,.32)", "rgba(0,0,0,.56)"),
        ]),
        ("Text & ink", &[
            ("text", "#0C0C10", "#F3F3F6"),
            ("mute", "#5E5E68", "#9B9BA5"),
            ("ink", "#17171C", "#F3F3F6"),
            ("on-ink", "#FFFFFF", "#0C0C10"),
        ]),
        ("Accent", &[
            ("accent", "#4B57D6", "#9AA3FF"),
            ("on-accent", "#FFFFFF", "#0C0C10"),
            ("accent-soft", "#4B57D61F", "#9AA3FF2E"),
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
    let mic = icons.get("mic").map(|s| s.as_str()).unwrap_or("");
    let check = icons.get("check").map(|s| s.as_str()).unwrap_or("");
    let plus = icons.get("plus").map(|s| s.as_str()).unwrap_or("");
    let _ = plus;
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

    // A full mock app screen: chat bubbles, a tool step, an approval card,
    // the composer and the bottom tab bar -- all from the real tokens.
    s.push_str("<h3>Mock app screen</h3>");
    s.push_str(r#"<p class="demo-label">Every part together in one phone frame. Tap a tab to switch; send shows the toast.</p>"#);
    s.push_str(&format!(
        r##"<div class="phone phone-chat"><div class="ph-header"><span class="ph-title">Chat</span></div><div class="ph-body chat-body"><div class="bubbles"><div class="bubble bot">Want me to set a reminder for the meeting?</div><div class="turn"><span class="icon">{check}</span> calendar.read &middot; Team sync 10:00</div><div class="apr"><div class="apr-q">Set a reminder 30 min before Team sync?</div><div class="apr-btns"><button class="btn primary" type="button">Yes</button><button class="btn secondary" type="button">No</button></div></div><div class="bubble me">Yes</div><div class="bubble bot">Done. I'll nudge you at 09:30.</div></div></div><div class="composer"><button class="iconbtn" type="button" aria-label="Voice"><span class="icon">{mic}</span></button><label class="skip" for="demo-msg">Message</label><textarea id="demo-msg" rows="1" placeholder="Message" aria-label="Message"></textarea><button class="btn icon primary" type="button" data-demo="toast" data-target="#demo-toast" aria-label="Send"><span class="icon">{send}</span></button></div>{nav}</div>"##,
        check = check,
        mic = mic,
        send = send,
        nav = nav_html(icons, "message-circle")
    ));

    s.push_str("</section>");
    s
}

// --- design in action: screens, smart-ui, and shared phone-frame helpers ---

const ALERT_SVG: &str = r#"<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"><path d="M10.3 3.9 1.8 18a2 2 0 0 0 1.7 3h17a2 2 0 0 0 1.7-3L13.7 3.9a2 2 0 0 0-3.4 0z"/><path d="M12 9v4"/><path d="M12 17h.01"/></svg>"#;

fn ic<'a>(icons: &'a HashMap<String, String>, name: &str) -> &'a str {
    icons.get(name).map(|s| s.as_str()).unwrap_or("")
}

/// The floating bottom tab bar. `active` is the icon name of the pressed tab.
/// Tapping a tab toggles its `aria-pressed` via `data-demo="demo-tab"`.
fn nav_html(icons: &HashMap<String, String>, active: &str) -> String {
    let tabs: &[(&str, &str)] = &[
        ("home", "Today"),
        ("calendar", "Plan"),
        ("message-circle", "Chat"),
        ("settings", "You"),
    ];
    let mut s = String::from(r#"<nav class="ph-nav">"#);
    for (icon, label) in tabs {
        let pressed = if *icon == active { "true" } else { "false" };
        s.push_str(&format!(
            r#"<button class="demo-tab" type="button" aria-pressed="{pressed}"><span class="icon">{}</span><span>{label}</span></button>"#,
            ic(icons, icon)
        ));
    }
    s.push_str("</nav>");
    s
}

/// A phone-frame mockup of the Today screen, in one forced theme.
fn phone_frame(icons: &HashMap<String, String>, theme: &str) -> String {
    let bell = ic(icons, "bell");
    let cap = if theme == "light" { "Light" } else { "Dark" };
    let cards = r#"<div class="card ph-card"><h3>Team sync</h3><p class="demo-label">10:00 · in 12 min</p></div><div class="card ph-card needs"><span class="ph-dot"></span><div><h3>Needs you</h3><p class="demo-label">1 approval waiting</p></div></div><div class="card ph-card"><h3>Morning brief</h3><p class="demo-label">3 things, 1 notice</p></div>"#;
    let nav = nav_html(icons, "home");
    format!(
        r#"<figure class="demo-frame {theme}"><figcaption class="cap">{cap}</figcaption><div class="phone"><div class="ph-header"><span class="ph-title">Today</span><button class="iconbtn" type="button" aria-label="Notices"><span class="icon">{bell}</span></button></div><div class="ph-body">{cards}</div>{nav}</div></figure>"#,
        theme = theme,
        cap = cap,
        bell = bell,
        cards = cards,
        nav = nav,
    )
}

/// Screens page: the real layout (header, content cards, bottom nav) shown in
/// both light and dark, each frame forced to its theme.
fn screens_showcase(icons: &HashMap<String, String>) -> String {
    let light = phone_frame(icons, "light");
    let dark = phone_frame(icons, "dark");
    format!(
        r#"<section class="showcase"><h2 class="show-title">Live: screens, light and dark</h2><p class="demo-label">A real layout in a phone frame: header, content cards and the bottom tab bar. Tap a tab.</p><div class="phone-row">{light}{dark}</div><p class="demo-label">Each frame is forced to its own theme, so you see both at once on any site theme.</p></section>"#,
        light = light,
        dark = dark,
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
