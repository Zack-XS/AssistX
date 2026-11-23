//! CSS and JS shipped as static assets. No outside scripts, fonts or CSS:
//! Inter is bundled (woff2), Lucide icons are bundled (inline SVG).

pub const TOKENS_CSS: &str = r#"@font-face{font-family:Inter;font-style:normal;font-weight:400;font-display:swap;src:url("fonts/inter-latin-400.woff2") format("woff2")}
@font-face{font-family:Inter;font-style:normal;font-weight:500;font-display:swap;src:url("fonts/inter-latin-500.woff2") format("woff2")}
@font-face{font-family:Inter;font-style:normal;font-weight:600;font-display:swap;src:url("fonts/inter-latin-600.woff2") format("woff2")}
@font-face{font-family:Inter;font-style:normal;font-weight:700;font-display:swap;src:url("fonts/inter-latin-700.woff2") format("woff2")}

:root{
  --font-sans:Inter,ui-sans-serif,system-ui,-apple-system,sans-serif;
  --font-mono:ui-monospace,Menlo,monospace;
  --page:#E9E9ED;--card:#FFFFFF;--raised:#FFFFFF;--field:#F2F2F5;--bubble:#E7E7EB;
  --code:#F0F0F4;--line:#E3E3E8;--scrim:rgba(12,12,16,.32);
  --text:#0C0C10;--mute:#5E5E68;--ink:#17171C;--on-ink:#FFFFFF;
  --accent:#4B57D6;--on-accent:#FFFFFF;--accent-soft:#4B57D61F;
  --ok:#157A42;--warn:#8A5D00;--bad:#B8291F;
  --shadow:0 1px 2px rgba(0,0,0,.05),0 10px 28px rgba(0,0,0,.06);
  --shadow-raised:0 2px 6px rgba(0,0,0,.06),0 18px 48px rgba(0,0,0,.14);
  --space-1:2px;--space-2:4px;--space-3:6px;--space-4:8px;--space-5:12px;--space-6:16px;--space-7:20px;--space-8:24px;--space-9:32px;--space-10:48px;
  --radius-xs:6px;--radius-sm:10px;--radius-md:14px;--radius-lg:20px;--radius-xl:22px;--radius-2xl:30px;--radius-full:999px;
  --touch-min:44px;--control-h:40px;--row-h:52px;--read-max:680px;
  --icon-xs:16px;--icon-sm:20px;--icon-md:22px;--icon-lg:28px;--icon-stroke:1.9;--icon-stroke-active:2.4;
  --fs-display:34px;--lh-display:37px;--fs-title:22px;--lh-title:28px;--fs-headline:17px;--lh-headline:24px;
  --fs-body:16px;--lh-body:23px;--fs-small:14px;--lh-small:20px;--fs-caption:12px;--lh-caption:16px;
  --fs-mono:13px;--lh-mono:19px;--fs-number:44px;--lh-number:48px;
  --fw-400:400;--fw-500:500;--fw-600:600;--fw-700:700;
  --dur-instant:80ms;--dur-quick:150ms;--dur-base:240ms;--dur-slow:320ms;--dur-page:420ms;
  --ease-out:cubic-bezier(.2,.8,.2,1);--ease-in:cubic-bezier(.4,0,1,1);--ease-in-out:cubic-bezier(.7,0,.2,1);--ease-spring:cubic-bezier(.3,1.6,.5,1);
  --bp-medium:600px;--bp-expanded:840px;--bp-large:1200px;
  color-scheme:light;
}
:root[data-theme="dark"]{
  --page:#000000;--card:#141417;--raised:#1C1C21;--field:#1C1C21;--bubble:#1F1F24;--code:#0B0B0E;--line:#26262B;--scrim:rgba(0,0,0,.56);
  --text:#F3F3F6;--mute:#9B9BA5;--ink:#F3F3F6;--on-ink:#0C0C10;
  --accent:#9AA3FF;--on-accent:#0C0C10;--accent-soft:#9AA3FF2E;
  --ok:#43C076;--warn:#E2A826;--bad:#FF6B63;
  --shadow:none;--shadow-raised:0 0 0 1px #26262B;color-scheme:dark;
}
@media (prefers-color-scheme: dark){
  :root:not([data-theme="light"]):not([data-theme="dark"]){
    --page:#000000;--card:#141417;--raised:#1C1C21;--field:#1C1C21;--bubble:#1F1F24;--code:#0B0B0E;--line:#26262B;--scrim:rgba(0,0,0,.56);
    --text:#F3F3F6;--mute:#9B9BA5;--ink:#F3F3F6;--on-ink:#0C0C10;
    --accent:#9AA3FF;--on-accent:#0C0C10;--accent-soft:#9AA3FF2E;
    --ok:#43C076;--warn:#E2A826;--bad:#FF6B63;--shadow:none;--shadow-raised:0 0 0 1px #26262B;color-scheme:dark;
  }
}
"#;

pub const MAIN_CSS: &str = r#"*{box-sizing:border-box}
html{scroll-behavior:smooth}
body{margin:0;background:var(--page);color:var(--text);font-family:var(--font-sans);font-size:var(--fs-body);line-height:var(--lh-body);-webkit-text-size-adjust:100%}
a{color:var(--accent);text-decoration:none}
a:hover{text-decoration:underline}
h1,h2,h3,h4,h5,h6{font-weight:var(--fw-600);line-height:1.25;color:var(--text)}
h1{font-size:var(--fs-display);line-height:var(--lh-display);font-weight:var(--fw-700);letter-spacing:-0.02em;margin:0 0 var(--space-6)}
h2{font-size:var(--fs-title);line-height:var(--lh-title);letter-spacing:-0.02em;margin:var(--space-9) 0 var(--space-5);padding-bottom:var(--space-3);border-bottom:1px solid var(--line)}
h3{font-size:var(--fs-headline);line-height:var(--lh-headline);margin:var(--space-7) 0 var(--space-4)}
h4{font-size:var(--fs-body);margin:var(--space-6) 0 var(--space-3)}
p{margin:0 0 var(--space-5)}
ul,ol{margin:0 0 var(--space-5);padding-left:1.4em}
li{margin:0 0 var(--space-2)}
code{font-family:var(--font-mono);font-size:var(--fs-mono);background:var(--code);padding:1px 5px;border-radius:var(--radius-xs)}
pre{background:var(--code);border-radius:var(--radius-md);padding:var(--space-6);overflow:auto}
pre code{background:none;padding:0;font-size:var(--fs-mono);line-height:var(--lh-mono)}
hr{border:0;border-top:1px solid var(--line);margin:var(--space-8) 0}
blockquote{margin:0 0 var(--space-5);padding:var(--space-3) var(--space-6);border-left:3px solid var(--line);color:var(--mute)}
table{border-collapse:collapse;width:100%;margin:0 0 var(--space-6);font-size:var(--fs-small);background:var(--card);border-radius:var(--radius-lg);overflow:hidden;box-shadow:var(--shadow)}
th,td{text-align:left;padding:var(--space-4) var(--space-5);border-bottom:1px solid var(--line);vertical-align:top}
th{font-weight:var(--fw-600);background:var(--field)}
tr:last-child td{border-bottom:0}
img{max-width:100%;height:auto}
.skip{position:absolute;left:-999px}
.skip:focus{left:var(--space-6);top:var(--space-6);background:var(--card);padding:var(--space-3) var(--space-5);border-radius:var(--radius-sm);z-index:50}

.layout{min-height:100vh}
.icon{display:inline-flex;align-items:center;justify-content:center;color:inherit}
.icon svg{width:var(--icon-md);height:var(--icon-md);stroke-width:var(--icon-stroke);flex:none}

/* header */
.topbar{position:sticky;top:0;z-index:20;display:none;align-items:center;gap:var(--space-4);padding:var(--space-4) var(--space-6);background:var(--card);border-bottom:1px solid var(--line)}
.topbar .menu-btn{display:inline-flex;align-items:center;gap:var(--space-2);min-height:var(--touch-min);padding:var(--space-2) var(--space-4);background:none;border:0;color:var(--text);border-radius:var(--radius-sm);font:inherit}
.topbar .menu-btn:hover{background:var(--field)}
.topbar .brand{font-weight:var(--fw-600);color:var(--text)}

/* sidebar */
.sidebar{position:fixed;top:0;left:0;bottom:0;width:288px;display:flex;flex-direction:column;background:var(--card);border-right:1px solid var(--line);overflow-y:auto;z-index:30}
.sidebar-head{display:flex;align-items:center;justify-content:space-between;padding:var(--space-6) var(--space-6) var(--space-4)}
.sidebar-head .brand{font-weight:var(--fw-700);font-size:var(--fs-headline);letter-spacing:-0.02em;color:var(--text)}
.iconbtn{display:inline-flex;align-items:center;justify-content:center;width:var(--touch-min);height:var(--touch-min);min-width:var(--touch-min);background:none;border:0;color:var(--text);border-radius:var(--radius-sm);cursor:pointer}
.iconbtn:hover{background:var(--field)}
.iconbtn:active{transform:scale(.98)}
.theme-btn .moon{display:none}
:root[data-theme="dark"] .theme-btn .sun{display:none}
:root[data-theme="dark"] .theme-btn .moon{display:inline-flex}
@media (prefers-color-scheme: dark){:root:not([data-theme="light"]):not([data-theme="dark"]) .theme-btn .sun{display:none}:root:not([data-theme="light"]):not([data-theme="dark"]) .theme-btn .moon{display:inline-flex}}
.demo-label{font-family:var(--font-mono);font-size:var(--fs-caption);color:var(--mute);display:block;margin:var(--space-2) 0}
.search{padding:0 var(--space-6) var(--space-4)}
.search input{width:100%;height:var(--control-h);padding:0 var(--space-5);background:var(--field);border:1px solid var(--line);border-radius:var(--radius-md);font:inherit;color:var(--text)}
.search input:focus{outline:2px solid var(--ink);outline-offset:2px}
#results{list-style:none;margin:var(--space-2) 0 0;padding:0}
#results li a{display:block;padding:var(--space-3) var(--space-5);color:var(--text);border-radius:var(--radius-sm)}
#results li a:hover{background:var(--field);text-decoration:none}
#results li a mark{background:var(--accent-soft);color:inherit;border-radius:2px}
#results:empty{display:none}
.nav{padding:0 var(--space-3) var(--space-8);flex:1}
.nav .section{margin:var(--space-5) 0 0;padding:0 var(--space-3)}
.nav .section > .stitle{display:block;font-size:var(--fs-caption);font-weight:var(--fw-600);letter-spacing:.01em;color:var(--mute);text-transform:uppercase;padding:0 var(--space-3) var(--space-2)}
.nav .section > .stitle a{color:var(--mute)}
.nav .section > .stitle a:hover{color:var(--text);text-decoration:none}
.nav .pl{display:block;padding:var(--space-2) var(--space-3);border-radius:var(--radius-sm);color:var(--text);font-size:var(--fs-small)}
.nav .pl:hover{background:var(--field);text-decoration:none}
.nav .pl.active{background:var(--accent-soft);color:var(--accent);font-weight:var(--fw-600)}

/* main */
.main{margin-left:288px}
.main .prose{max-width:var(--read-max);margin:0 auto;padding:var(--space-9) var(--space-8) var(--space-10)}
.scrim{display:none}
.home-list{list-style:none;padding:0}
.home-list>li{margin:0 0 var(--space-7)}
.home-list h2{margin:0 0 var(--space-3);border:0;padding:0}
.home-list .sub{list-style:none;padding:0;margin:0}
.home-list .sub li{margin:0 0 var(--space-2)}

/* responsive */
@media (max-width:839px){
  .topbar{display:flex}
  .sidebar{transform:translateX(-100%);transition:transform var(--dur-slow) var(--ease-out);box-shadow:var(--shadow-raised)}
  .sidebar.open{transform:translateX(0)}
  .main{margin-left:0}
  .main .prose{padding:var(--space-6) var(--space-6) var(--space-10)}
  .scrim.show{display:block;position:fixed;inset:0;background:var(--scrim);z-index:25}
  table{display:block;overflow-x:auto}
}
@media (max-width:599px){
  h1{font-size:28px;line-height:32px}
}

/* showcase */
.showcase{margin-top:var(--space-9);border-top:1px solid var(--line);padding-top:var(--space-8)}
.showcase h2.show-title{margin-top:0}
.swatch-row{display:grid;grid-template-columns:repeat(auto-fill,minmax(150px,1fr));gap:var(--space-4);margin:0 0 var(--space-6)}
.swatch{background:var(--card);border:1px solid var(--line);border-radius:var(--radius-lg);overflow:hidden;box-shadow:var(--shadow)}
.swatch .chip{height:64px}
.swatch .meta{padding:var(--space-3) var(--space-4)}
.swatch .meta .name{font-weight:var(--fw-600);font-size:var(--fs-small)}
.swatch .meta .val{font-family:var(--font-mono);font-size:var(--fs-caption);color:var(--mute)}
.dark-swatches{background:var(--page)}
.type-scale .row{display:flex;align-items:baseline;gap:var(--space-6);padding:var(--space-3) 0;border-bottom:1px solid var(--line)}
.type-scale .row .lbl{width:120px;flex:none;font-family:var(--font-mono);font-size:var(--fs-caption);color:var(--mute)}
.space-bar{height:var(--space-6);background:var(--accent-soft);border-radius:var(--radius-xs);margin:var(--space-2) 0}
.demo-row{display:flex;flex-wrap:wrap;gap:var(--space-4);align-items:center;margin:0 0 var(--space-6)}
.btn{display:inline-flex;align-items:center;gap:var(--space-2);min-height:var(--control-h);padding:0 var(--space-6);border-radius:var(--radius-full);font:inherit;font-weight:var(--fw-500);border:1px solid transparent;cursor:pointer;transition:transform var(--dur-instant) var(--ease-out),background var(--dur-quick) var(--ease-out)}
.btn:active{transform:scale(.98)}
.btn:disabled{opacity:.4;cursor:default}
.btn.primary{background:var(--ink);color:var(--on-ink)}
.btn.secondary{background:var(--field);color:var(--text);border-color:var(--line)}
.btn.ghost{background:none;color:var(--text)}
.btn.danger{color:var(--bad)}
.btn.danger.solid{background:var(--bad);color:#fff}
.btn.icon{width:var(--touch-min);height:var(--touch-min);padding:0;justify-content:center;border-radius:var(--radius-full)}
.bubbles{display:flex;flex-direction:column;gap:var(--space-7);max-width:420px;margin:0 0 var(--space-6)}
.bubble{max-width:82%;padding:var(--space-4) var(--space-5);border-radius:var(--radius-lg)}
.bubble.me{align-self:flex-end;background:var(--ink);color:var(--on-ink);border-bottom-right-radius:var(--radius-xs)}
.bubble.bot{align-self:flex-start;background:var(--bubble);color:var(--text);border-bottom-left-radius:var(--radius-xs);max-width:88%}
.card{background:var(--card);border-radius:var(--radius-xl);padding:var(--space-6);box-shadow:var(--shadow);border:1px solid var(--line);max-width:420px;margin:0 0 var(--space-6)}
.card h3{margin:0 0 var(--space-2)}
.toast{position:fixed;left:50%;transform:translateX(-50%);bottom:var(--space-8);background:var(--ink);color:var(--on-ink);padding:var(--space-4) var(--space-6);border-radius:var(--radius-full);box-shadow:var(--shadow-raised);opacity:0;pointer-events:none;transition:opacity var(--dur-quick) var(--ease-out),transform var(--dur-base) var(--ease-spring)}
.toast.show{opacity:1;transform:translateX(-50%) translateY(-4px)}
.sheet-demo{position:relative;height:0;overflow:hidden}
.sheet-panel{position:fixed;left:50%;bottom:0;transform:translateX(-50%) translateY(100%);width:min(420px,90vw);background:var(--raised);border-radius:var(--radius-2xl) var(--radius-2xl) 0 0;box-shadow:var(--shadow-raised);padding:var(--space-8) var(--space-6) var(--space-10);transition:transform var(--dur-slow) var(--ease-out);z-index:40}
.sheet-panel.open{transform:translateX(-50%) translateY(0)}
.sheet-scrim{position:fixed;inset:0;background:var(--scrim);opacity:0;pointer-events:none;transition:opacity var(--dur-base) var(--ease-out);z-index:35}
.sheet-scrim.open{opacity:1;pointer-events:auto}
.icon-grid{display:grid;grid-template-columns:repeat(auto-fill,minmax(96px,1fr));gap:var(--space-4);margin:0 0 var(--space-6)}
.icon-cell{display:flex;flex-direction:column;align-items:center;gap:var(--space-2);padding:var(--space-5) var(--space-3);background:var(--card);border:1px solid var(--line);border-radius:var(--radius-lg)}
.icon-cell .icon svg{width:var(--icon-lg);height:var(--icon-lg)}
.icon-cell .name{font-size:var(--fs-caption);color:var(--mute)}
.radius-sample{width:72px;height:72px;background:var(--accent-soft);border:1px solid var(--line);margin:0 0 var(--space-2)}
.radius-row{display:flex;flex-wrap:wrap;gap:var(--space-6);align-items:flex-end;margin:0 0 var(--space-6)}
.radius-row figure{margin:0;text-align:center}
.radius-row figcaption{font-family:var(--font-mono);font-size:var(--fs-caption);color:var(--mute)}
.motion-card{background:var(--card);border:1px solid var(--line);border-radius:var(--radius-lg);padding:var(--space-6);margin:0 0 var(--space-6)}
.motion-stage{height:80px;position:relative;background:var(--field);border-radius:var(--radius-md);overflow:hidden;margin-top:var(--space-4)}
.motion-stage .mover{position:absolute;top:50%;left:0;width:24px;height:24px;border-radius:var(--radius-full);background:var(--accent);transform:translateY(-50%)}
/* roadmap visual */
.roadmap .rm-timeline{position:relative;display:flex;overflow-x:auto;padding:var(--space-6) 0 var(--space-4);gap:var(--space-1)}
.roadmap .rm-line{position:absolute;top:calc(var(--space-6) + 18px);left:60px;right:60px;height:2px;background:var(--ok)}
.roadmap .rm-node{flex:0 0 auto;min-width:76px;display:flex;flex-direction:column;align-items:center;text-align:center;position:relative;z-index:1;padding:0 var(--space-2)}
.roadmap .rm-dot{width:36px;height:36px;border-radius:var(--radius-full);display:flex;align-items:center;justify-content:center;margin-bottom:var(--space-3);background:var(--card);border:1px solid var(--line)}
.roadmap .rm-dot svg{width:18px;height:18px}
.roadmap .rm-done .rm-dot{background:var(--ok);color:#fff;border-color:var(--ok)}
.roadmap .rm-step{font-family:var(--font-mono);font-size:var(--fs-caption);color:var(--mute)}
.roadmap .rm-label{font-size:var(--fs-small);line-height:var(--lh-small);max-width:72px}
.roadmap .rm-pills{display:flex;flex-wrap:wrap;gap:var(--space-2);margin:0 0 var(--space-5)}
.roadmap .rm-pill{background:var(--accent-soft);color:var(--accent);border-radius:var(--radius-full);padding:var(--space-2) var(--space-4);font-size:var(--fs-caption);font-weight:var(--fw-500)}
.roadmap .rm-next{display:grid;grid-template-columns:repeat(auto-fill,minmax(180px,1fr));gap:var(--space-4);margin:0 0 var(--space-5)}
.roadmap .rm-milestone{border:1px dashed var(--line);border-radius:var(--radius-lg);padding:var(--space-5)}
.roadmap .rm-milestone .rm-dot{width:32px;height:32px;background:transparent;border:1px dashed var(--mute);color:var(--mute);margin-bottom:var(--space-3)}
.roadmap .rm-pending{color:var(--mute)}
.roadmap .rm-mtitle{font-weight:var(--fw-600)}
.roadmap .rm-msub{font-size:var(--fs-caption);color:var(--mute);margin-top:var(--space-1)}
.roadmap .rm-legend{display:flex;gap:var(--space-5);flex-wrap:wrap;margin:0 0 var(--space-5)}
.roadmap .rm-li{display:inline-flex;align-items:center;gap:var(--space-2);font-size:var(--fs-caption);color:var(--mute)}
.roadmap .rm-li svg{width:16px;height:16px}
.roadmap .rm-decisions{list-style:none;padding:0;margin:0 0 var(--space-5);display:grid;gap:var(--space-2)}
.roadmap .rm-dec{display:flex;align-items:center;gap:var(--space-3);padding:var(--space-3) var(--space-4);background:var(--card);border:1px solid var(--line);border-radius:var(--radius-md)}
.roadmap .rm-ico{flex:none;display:inline-flex}
.roadmap .rm-ico svg{width:18px;height:18px}
.roadmap .rm-dec.rm-decided .rm-ico{color:var(--ok)}
.roadmap .rm-dec.rm-proposed .rm-ico{color:var(--accent)}
.roadmap .rm-dec.rm-open .rm-ico{color:var(--warn)}
.roadmap .rm-q{flex:1;font-size:var(--fs-small)}
.roadmap .rm-tag{font-size:var(--fs-caption);font-weight:var(--fw-500);color:var(--mute)}
.roadmap .rm-overview{display:grid;grid-template-columns:repeat(auto-fill,minmax(180px,1fr));gap:var(--space-4);margin:0 0 var(--space-5)}
.roadmap .rm-phase{display:flex;flex-direction:column;gap:var(--space-2);padding:var(--space-6);border-radius:var(--radius-lg);border:1px solid var(--line);background:var(--card);color:var(--text);text-decoration:none}
.roadmap .rm-phase:hover{text-decoration:none;border-color:var(--accent)}
.roadmap .rm-pico{width:36px;height:36px;border-radius:var(--radius-full);display:inline-flex;align-items:center;justify-content:center;background:var(--field)}
.roadmap .rm-pico svg{width:18px;height:18px}
.roadmap .rm-phase.rm-done .rm-pico{background:var(--ok);color:#fff}
.roadmap .rm-phase.rm-next .rm-pico{color:var(--accent)}
.roadmap .rm-phase.rm-open .rm-pico{color:var(--warn)}
.roadmap .rm-ptitle{font-weight:var(--fw-600);font-size:var(--fs-body)}
.roadmap .rm-psub{font-size:var(--fs-caption);color:var(--mute)}

/* design in action: phone frames, mock screen, adaptive states */
.demo-frame.light{--page:#E9E9ED;--card:#FFFFFF;--raised:#FFFFFF;--field:#F2F2F5;--bubble:#E7E7EB;--code:#F0F0F4;--line:#E3E3E8;--scrim:rgba(12,12,16,.32);--text:#0C0C10;--mute:#5E5E68;--ink:#17171C;--on-ink:#FFFFFF;--accent:#4B57D6;--on-accent:#FFFFFF;--accent-soft:#4B57D61F;--ok:#157A42;--warn:#8A5D00;--bad:#B8291F;--shadow:0 1px 2px rgba(0,0,0,.05),0 10px 28px rgba(0,0,0,.06);--shadow-raised:0 2px 6px rgba(0,0,0,.06),0 18px 48px rgba(0,0,0,.14)}
.demo-frame.dark{--page:#000000;--card:#141417;--raised:#1C1C21;--field:#1C1C21;--bubble:#1F1F24;--code:#0B0B0E;--line:#26262B;--scrim:rgba(0,0,0,.56);--text:#F3F3F6;--mute:#9B9BA5;--ink:#F3F3F6;--on-ink:#0C0C10;--accent:#9AA3FF;--on-accent:#0C0C10;--accent-soft:#9AA3FF2E;--ok:#43C076;--warn:#E2A826;--bad:#FF6B63;--shadow:none;--shadow-raised:0 0 0 1px #26262B}
.phone-row{display:flex;gap:var(--space-8);flex-wrap:wrap;margin:0 0 var(--space-6)}
.demo-frame{margin:0}
.demo-frame .cap{font-family:var(--font-mono);font-size:var(--fs-caption);color:var(--mute);margin:0 0 var(--space-3)}
.phone{width:300px;background:var(--page);border:1px solid var(--line);border-radius:var(--radius-2xl);box-shadow:var(--shadow-raised);overflow:hidden;display:flex;flex-direction:column;height:560px}
.phone-chat{height:540px}
.ph-header{display:flex;align-items:center;justify-content:space-between;padding:var(--space-5);background:var(--card);border-bottom:1px solid var(--line)}
.ph-title{font-size:var(--fs-headline);font-weight:var(--fw-700);letter-spacing:-0.02em}
.ph-body{flex:1;overflow:auto;padding:var(--space-5);display:flex;flex-direction:column;gap:var(--space-4)}
.ph-body.chat-body{padding:var(--space-4)}
.ph-card{max-width:none;margin:0;box-shadow:none}
.ph-card.needs{display:flex;align-items:center;gap:var(--space-3)}
.ph-dot{width:9px;height:9px;border-radius:var(--radius-full);background:var(--bad);flex:none}
.ph-nav{display:flex;background:var(--card);border-top:1px solid var(--line)}
.demo-tab{flex:1;display:flex;flex-direction:column;align-items:center;gap:2px;padding:var(--space-3) 0;background:none;border:0;color:var(--mute);font:inherit;font-size:var(--fs-caption);cursor:pointer}
.demo-tab .icon svg{width:var(--icon-sm);height:var(--icon-sm)}
.demo-tab[aria-pressed="true"]{color:var(--accent)}
.demo-tab:hover{color:var(--text)}
.composer{display:flex;align-items:flex-end;gap:var(--space-3);padding:var(--space-3) var(--space-4);background:var(--card);border-top:1px solid var(--line)}
.composer textarea{flex:1;resize:none;border:1px solid var(--line);background:var(--field);border-radius:var(--radius-md);padding:var(--space-3) var(--space-4);font:inherit;color:var(--text);min-height:40px;max-height:120px}
.composer textarea:focus{outline:2px solid var(--ink);outline-offset:1px}
.turn{align-self:flex-start;background:var(--code);color:var(--mute);font-family:var(--font-mono);font-size:var(--fs-caption);padding:var(--space-2) var(--space-4);border-radius:var(--radius-md);max-width:88%;display:inline-flex;align-items:center;gap:var(--space-2)}
.turn .icon svg{width:14px;height:14px}
.turn .icon{color:var(--ok)}
.apr{background:var(--card);border:1px solid var(--line);border-radius:var(--radius-lg);padding:var(--space-4) var(--space-5);max-width:88%;align-self:flex-start;box-shadow:var(--shadow)}
.apr-q{font-weight:var(--fw-600);margin:0 0 var(--space-3)}
.apr-btns{display:flex;gap:var(--space-3)}
.chat-body .bubbles{margin:0;gap:var(--space-4)}
.swatch .meta .val{background:none;border:0;padding:0;text-align:left;cursor:pointer}
.swatch .meta .val:hover{color:var(--accent);text-decoration:underline}
.ui-state-btns{display:flex;flex-wrap:wrap;gap:var(--space-3);margin:0 0 var(--space-5)}
.smart-card{max-width:380px;margin:0 0 var(--space-5)}
.smart-card .ui-state{display:none}
.smart-card[data-state="loading"] .ui-loading,
.smart-card[data-state="empty"] .ui-empty,
.smart-card[data-state="error"] .ui-error,
.smart-card[data-state="success"] .ui-success{display:block}
.skel{height:14px;border-radius:var(--radius-xs);background:var(--field);margin:0 0 var(--space-4);position:relative;overflow:hidden}
.skel.w80{width:80%}
.skel.w60{width:60%}
.skel::after{content:"";position:absolute;inset:0;transform:translateX(-100%);background:linear-gradient(90deg,transparent,var(--line),transparent);animation:skelShimmer 1.5s var(--ease-in-out) infinite}
@keyframes skelShimmer{to{transform:translateX(100%)}}
.ui-err{display:flex;align-items:center;gap:var(--space-2);color:var(--bad);font-weight:var(--fw-500);margin:0 0 var(--space-4)}
.ui-ico svg{width:18px;height:18px}

@media (prefers-reduced-motion:reduce){
  *{animation:none!important;transition:none!important}
  .sidebar{transition:none}
}
"#;

pub const APP_JS: &str = r#"(function(){
  var root=document.documentElement;
  var KEY='assistx-docs-theme';
  function apply(mode){
    if(mode==='light'||mode==='dark'){root.setAttribute('data-theme',mode);}
    else{root.removeAttribute('data-theme');}
    var btn=document.querySelector('.theme-btn');
    if(btn){btn.dataset.mode=mode||'auto';}
  }
  var saved=localStorage.getItem(KEY)||'auto';
  apply(saved);
  document.addEventListener('click',function(e){
    var t=e.target.closest('.theme-btn');
    if(t){e.preventDefault();var m=t.dataset.mode||'auto';var next=m==='auto'?'light':m==='light'?'dark':'auto';localStorage.setItem(KEY,next);apply(next);}
  });
  // sidebar
  var sb=document.getElementById('sidebar'),scrim=document.getElementById('scrim');
  function openSB(v){sb.classList.toggle('open',v);scrim.classList.toggle('show',v);}
  document.addEventListener('click',function(e){
    if(e.target.closest('.menu-btn')){openSB(!sb.classList.contains('open'));}
    else if(e.target===scrim||e.target.closest('[data-close-nav]')){openSB(false);}
  });
  // search
  var input=document.getElementById('q'),results=document.getElementById('results');
  var data=null,base=root.getAttribute('data-root')||'';
  var q='';
  async function load(){if(data)return;try{var r=await fetch(base+'assets/search.json');data=await r.json();}catch(e){data=[];}}
  function score(item,query){
    query=query.toLowerCase();var t=0;
    if(item.title.toLowerCase().indexOf(query)>=0)t+=5;
    if(item.description&&item.description.toLowerCase().indexOf(query)>=0)t+=2;
    if(item.headings&&item.headings.toLowerCase().indexOf(query)>=0)t+=1;
    return t;
  }
  function render(list){results.innerHTML='';list.slice(0,12).forEach(function(it){
    var li=document.createElement('li');var a=document.createElement('a');
    a.href=base+it.path;a.textContent=it.title;
    if(it.description){var s=document.createElement('span');s.textContent=' — '+it.description;s.style.color='var(--mute)';s.style.fontSize='var(--fs-caption)';a.appendChild(s);}
    li.appendChild(a);results.appendChild(li);
  });results.hidden=list.length===0;}
  input.addEventListener('input',function(){q=input.value.trim();if(!q){results.innerHTML='';results.hidden=true;return;}load().then(function(){
    var scored=[];for(var i=0;i<data.length;i++){var s=score(data[i],q);if(s>0)scored.push([s,data[i]]);}
    scored.sort(function(a,b){return b[0]-a[0];});render(scored.map(function(x){return x[1];}));
  });});
  // showcase interactions
  document.addEventListener('click',function(e){
    var b=e.target.closest('[data-demo]');
    if(!b)return;
    var kind=b.dataset.demo;
    if(kind==='play-motion'){document.querySelectorAll(b.dataset.target).forEach(function(m){m.classList.remove('run');void m.offsetWidth;m.classList.add('run');});}
    if(kind==='toast'){var t=document.querySelector(b.dataset.target);if(t){t.classList.add('show');setTimeout(function(){t.classList.remove('show');},3200);}}
    if(kind==='sheet-open'){document.querySelector(b.dataset.target).classList.add('open');document.querySelector(b.dataset.scrim).classList.add('open');}
    if(kind==='sheet-close'){document.querySelector(b.dataset.target).classList.remove('open');document.querySelector(b.dataset.scrim).classList.remove('open');}
    if(kind==='copy'){var val=b.dataset.copy||'';var t=document.querySelector(b.dataset.target);if(t){t.textContent='Copied '+val;t.classList.add('show');setTimeout(function(){t.classList.remove('show');},1600);}try{navigator.clipboard&&navigator.clipboard.writeText(val);}catch(err){}}
    if(kind==='ui-state'){var t=document.querySelector(b.dataset.target);if(t){t.dataset.state=b.dataset.state;}document.querySelectorAll('[data-demo="ui-state"][data-target="'+b.dataset.target+'"]').forEach(function(x){x.setAttribute('aria-pressed',x===b?'true':'false');});}
    if(kind==='demo-tab'){var nav=b.closest('.ph-nav');if(nav){nav.querySelectorAll('.demo-tab').forEach(function(x){x.setAttribute('aria-pressed',x===b?'true':'false');});}}
  });
})();
"#;
