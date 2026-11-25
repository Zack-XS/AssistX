//! CSS and JS shipped as static assets. No outside scripts, fonts or CSS:
//! Inter is bundled (woff2), Lucide icons are bundled (inline SVG).

pub const TOKENS_CSS: &str = r#"@font-face{font-family:Inter;font-style:normal;font-weight:400;font-display:swap;src:url("fonts/inter-latin-400.woff2") format("woff2")}
@font-face{font-family:Inter;font-style:normal;font-weight:500;font-display:swap;src:url("fonts/inter-latin-500.woff2") format("woff2")}
@font-face{font-family:Inter;font-style:normal;font-weight:600;font-display:swap;src:url("fonts/inter-latin-600.woff2") format("woff2")}
@font-face{font-family:Inter;font-style:normal;font-weight:700;font-display:swap;src:url("fonts/inter-latin-700.woff2") format("woff2")}

:root{
  --font-sans:Inter,ui-sans-serif,system-ui,-apple-system,sans-serif;
  --font-mono:ui-monospace,Menlo,monospace;
  --page:#FAFAFA;--card:#FFFFFF;--raised:#FFFFFF;--field:#F1F1F1;--bubble:#EDEDED;
  --code:#F4F4F4;--line:#E4E4E4;--scrim:rgba(10,10,10,.32);
  --text:#0A0A0A;--mute:#666666;--ink:#0A0A0A;--on-ink:#FFFFFF;
  --accent:#0A0A0A;--on-accent:#FFFFFF;--accent-soft:#0A0A0A0F;
  --ok:#157A42;--warn:#8A5D00;--bad:#B8291F;
  --shadow:0 0 0 1px #E4E4E4;
  --shadow-raised:0 0 0 1px #E4E4E4,0 12px 32px rgba(0,0,0,.08);
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
  --page:#000000;--card:#111111;--raised:#1A1A1A;--field:#1A1A1A;--bubble:#1C1C1C;--code:#0A0A0A;--line:#262626;--scrim:rgba(0,0,0,.56);
  --text:#EDEDED;--mute:#8F8F8F;--ink:#EDEDED;--on-ink:#0A0A0A;
  --accent:#EDEDED;--on-accent:#0A0A0A;--accent-soft:#EDEDED1A;
  --ok:#43C076;--warn:#E2A826;--bad:#FF6B63;
  --shadow:none;--shadow-raised:0 0 0 1px #262626;color-scheme:dark;
}
@media (prefers-color-scheme: dark){
  :root:not([data-theme="light"]):not([data-theme="dark"]){
    --page:#000000;--card:#111111;--raised:#1A1A1A;--field:#1A1A1A;--bubble:#1C1C1C;--code:#0A0A0A;--line:#262626;--scrim:rgba(0,0,0,.56);
    --text:#EDEDED;--mute:#8F8F8F;--ink:#EDEDED;--on-ink:#0A0A0A;
    --accent:#EDEDED;--on-accent:#0A0A0A;--accent-soft:#EDEDED1A;
    --ok:#43C076;--warn:#E2A826;--bad:#FF6B63;--shadow:none;--shadow-raised:0 0 0 1px #262626;color-scheme:dark;
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

/* roadmap board: the Roadmap home page */
.prose:has(.rm-board){max-width:1180px}
.rm-board{display:flex;flex-direction:column;gap:var(--space-8)}
.rm-hero{background:linear-gradient(135deg,var(--accent-soft),var(--card));border:1px solid var(--line);border-radius:var(--radius-2xl);padding:var(--space-8);display:flex;flex-direction:column;gap:var(--space-5)}
.rm-kicker{margin:0;font-family:var(--font-mono);font-size:var(--fs-caption);letter-spacing:.08em;text-transform:uppercase;color:var(--accent)}
.rm-h1{margin:0;font-size:var(--fs-display);line-height:var(--lh-display);font-weight:var(--fw-700);max-width:22ch}
.rm-stats{display:grid;grid-template-columns:repeat(auto-fit,minmax(150px,1fr));gap:var(--space-3)}
.rm-stat{display:flex;flex-direction:column;gap:var(--space-1);padding:var(--space-4) var(--space-5);background:var(--card);color:var(--text);border:1px solid var(--line);border-radius:var(--radius-lg);text-decoration:none;transition:transform var(--dur-quick) var(--ease-out),border-color var(--dur-quick) var(--ease-out)}
.rm-stat:hover{text-decoration:none;transform:translateY(-2px);border-color:var(--accent)}
.rm-num{font-size:var(--fs-headline);line-height:1.1;font-weight:var(--fw-700);font-variant-numeric:tabular-nums}
.rm-cap{font-size:var(--fs-caption);color:var(--mute)}
.rm-s-done .rm-num{color:var(--ok)}.rm-s-next .rm-num{color:var(--accent)}.rm-s-open .rm-num{color:var(--warn)}
.rm-progress{height:8px;border-radius:var(--radius-full);background:var(--field);overflow:hidden}
.rm-progress span{display:block;height:100%;border-radius:inherit;background:var(--ok);transform-origin:left;animation:rmGrow var(--dur-page) var(--ease-out) both}
@keyframes rmGrow{from{transform:scaleX(0)}to{transform:scaleX(1)}}

/* the winding road */
.rm-journey{position:relative;margin:var(--space-4) 0}
.rm-svg{position:absolute;inset:0;width:100%;height:100%;overflow:visible}
.rm-narrow{display:none}
.rm-surface{fill:none;stroke:var(--field);stroke-width:30px;stroke-linecap:round}
.rm-ahead{fill:none;stroke:var(--mute);stroke-width:3px;stroke-dasharray:2 9;stroke-linecap:round;opacity:.65}
.rm-travelled{fill:none;stroke:var(--ok);stroke-width:5px;stroke-linecap:round;stroke-dasharray:1;stroke-dashoffset:1;animation:rmDraw 1.6s var(--ease-out) .2s forwards}
@keyframes rmDraw{to{stroke-dashoffset:0}}
.rm-stops{list-style:none;margin:0;padding:0}
.rm-at{position:absolute;left:0;right:0;height:0;margin:0;padding:0;opacity:0;animation:rmIn var(--dur-slow) var(--ease-out) forwards;animation-delay:calc(var(--d) * 45ms)}
@keyframes rmIn{from{opacity:0;transform:translateY(8px)}to{opacity:1;transform:none}}
.rm-pin{position:absolute;left:var(--xw);top:0;transform:translate(-50%,-50%);z-index:2;width:40px;height:40px;border-radius:var(--radius-full);display:flex;align-items:center;justify-content:center;background:var(--card);border:2px dashed var(--accent);color:var(--accent);font-weight:var(--fw-700);font-size:var(--fs-small);box-shadow:var(--shadow)}
.rm-pin svg{width:18px;height:18px}
.rm-k-done .rm-pin{background:var(--ok);border:2px solid var(--ok);color:#fff}
.rm-k-later .rm-pin{border-color:var(--warn);color:var(--warn)}
.rm-k-now .rm-pin{width:54px;height:54px;border:3px solid var(--accent);background:var(--accent-soft)}
.rm-pulse{width:16px;height:16px;border-radius:var(--radius-full);background:var(--accent);animation:rmPulse 2s var(--ease-out) infinite}
@keyframes rmPulse{0%{box-shadow:0 0 0 0 var(--accent)}70%{box-shadow:0 0 0 16px transparent}100%{box-shadow:0 0 0 0 transparent}}
.rm-card{position:absolute;top:0;transform:translateY(-50%);display:flex;flex-direction:column;gap:2px;width:min(320px,calc(var(--xw) - 52px));padding:var(--space-3) var(--space-4);background:var(--card);color:var(--text);border:1px solid var(--line);border-radius:var(--radius-lg);box-shadow:var(--shadow);transition:transform var(--dur-quick) var(--ease-out),border-color var(--dur-quick) var(--ease-out)}
.rm-left .rm-card{right:calc(100% - var(--xw) + 34px);text-align:right}
.rm-right .rm-card{left:calc(var(--xw) + 34px);width:min(320px,calc(100% - var(--xw) - 52px))}
.rm-at:hover .rm-card{border-color:var(--accent);transform:translateY(calc(-50% - 2px))}
.rm-k-done .rm-card{border-color:color-mix(in srgb,var(--ok) 35%,var(--line))}
.rm-k-now .rm-card{border:2px solid var(--accent);background:var(--accent-soft)}
.rm-k-later .rm-card{border-style:dashed}
.rm-tag{font-family:var(--font-mono);font-size:var(--fs-caption);color:var(--mute);text-transform:uppercase;letter-spacing:.04em}
.rm-k-done .rm-tag{color:var(--ok)}.rm-k-now .rm-tag{color:var(--accent);font-weight:var(--fw-600)}.rm-k-next .rm-tag{color:var(--accent)}.rm-k-later .rm-tag{color:var(--warn)}
.rm-ct{font-weight:var(--fw-600);font-size:var(--fs-small);line-height:var(--lh-small)}
.rm-cd{font-size:var(--fs-caption);line-height:var(--lh-caption);color:var(--mute)}
@media (max-width:720px){
  .rm-wide{display:none}.rm-narrow{display:block}
  .rm-pin{left:var(--xn)}
  .rm-left .rm-card,.rm-right .rm-card{left:calc(23% + 30px);right:0;width:auto;text-align:left}
  .rm-surface{stroke-width:22px}
  .rm-cd{display:-webkit-box;-webkit-line-clamp:2;-webkit-box-orient:vertical;overflow:hidden}
}

/* choices to make */
.rm-decide h2,.rm-notes h2{display:flex;align-items:center;gap:var(--space-3)}
.rm-lc{font-family:var(--font-mono);font-size:var(--fs-caption);color:var(--mute);background:var(--field);border-radius:var(--radius-full);padding:0 var(--space-3)}
.rm-grid{display:grid;grid-template-columns:repeat(auto-fill,minmax(220px,1fr));gap:var(--space-3)}
.rm-q{display:flex;flex-direction:column;gap:var(--space-2);padding:var(--space-4);background:var(--card);color:var(--text);border:1px solid var(--line);border-left:3px solid var(--warn);border-radius:var(--radius-md)}
.rm-q .rm-tagc{align-self:flex-start}
.rm-cm{font-family:var(--font-mono);font-size:var(--fs-caption);color:var(--mute)}
.rm-more .rm-ct{color:var(--mute)}
.rm-tagc{display:inline-block;padding:0 var(--space-2);border-radius:var(--radius-full);font-size:var(--fs-caption);font-weight:var(--fw-500)}
.rm-t-open{background:var(--warn);color:#fff}
.rm-t-proposed{background:var(--accent-soft);color:var(--accent)}
.rm-notes .rm-details{margin-top:var(--space-3)}

/* roadmap sub-pages: visual first, notes folded */
.rm-details{margin-top:var(--space-8);border:1px solid var(--line);border-radius:var(--radius-lg);padding:var(--space-4) var(--space-5);background:var(--card)}
.rm-details>summary{cursor:pointer;font-weight:var(--fw-600);color:var(--accent);list-style:none}
.rm-details>summary::-webkit-details-marker{display:none}
.rm-details>summary::before{content:"+ "}
.rm-details[open]>summary::before{content:"− "}
.rm-details[open]>summary{margin-bottom:var(--space-4)}
.showcase.roadmap:first-of-type{margin-top:var(--space-5);border-top:0;padding-top:0}

/* haptics showcase */
.hap-grid{display:grid;grid-template-columns:repeat(auto-fill,minmax(150px,1fr));gap:var(--space-3);margin:var(--space-4) 0}
.hap-pad{display:flex;flex-direction:column;align-items:flex-start;gap:var(--space-1);padding:var(--space-5) var(--space-4);min-height:var(--touch-min);background:var(--card);color:var(--text);border:1px solid var(--line);border-radius:var(--radius-lg);font:inherit;text-align:left;cursor:pointer;transition:border-color var(--dur-quick) var(--ease-out)}
.hap-pad:hover{border-color:var(--accent)}
.hap-pad:focus-visible{outline:2px solid var(--accent);outline-offset:2px}
.hap-dot{width:18px;height:18px;border-radius:var(--radius-full);background:var(--accent);margin-bottom:var(--space-2)}
.hap-reject .hap-dot{background:var(--bad)}.hap-confirm .hap-dot{background:var(--ok)}.hap-heavy .hap-dot{background:var(--ink)}.hap-attention .hap-dot{background:var(--warn)}
.hap-name{font-family:var(--font-mono);font-size:var(--fs-caption);font-weight:var(--fw-600)}
.hap-use{font-size:var(--fs-caption);color:var(--mute)}
.hap-pad.buzz{animation:hapShake 180ms var(--ease-out)}
.hap-tick.buzz{animation-duration:90ms}
.hap-confirm.buzz,.hap-reject.buzz{animation:hapShake 120ms var(--ease-out) 2}
.hap-heavy.buzz{animation:hapThud 220ms var(--ease-out)}
.hap-attention.buzz .hap-dot{animation:hapRise 360ms var(--ease-out)}
@keyframes hapShake{0%,100%{transform:none}25%{transform:translateX(-2px)}75%{transform:translateX(2px)}}
@keyframes hapThud{0%,100%{transform:none}40%{transform:scale(.96)}}
@keyframes hapRise{from{transform:scale(.4);opacity:.4}to{transform:none;opacity:1}}

/* ---------- home page ---------- */
body.hm{background:var(--page);color:var(--text)}
.hm main{max-width:1120px;margin:0 auto;padding:0 var(--space-8) var(--space-10);counter-reset:sec}
.hm a{color:var(--text)}
.hm-nav{position:sticky;top:0;z-index:20;display:flex;align-items:center;gap:var(--space-9);padding:0 var(--space-7);height:56px;background:color-mix(in srgb,var(--page) 90%,transparent);backdrop-filter:blur(10px);-webkit-backdrop-filter:blur(10px);border-bottom:1px solid var(--line)}
.hm-logo{display:inline-flex;align-items:center;gap:10px;font-weight:var(--fw-600);font-size:15px;color:var(--text);text-decoration:none;letter-spacing:-.01em}
.hm-logo:hover{text-decoration:none}
.hm-mark{width:14px;height:14px;border-radius:50%;background:var(--text)}
.hm-nav nav{display:flex;align-items:center;gap:var(--space-7);flex:1}
.hm-nav nav a{display:inline-flex;align-items:center;gap:6px;min-height:var(--touch-min);color:var(--mute);text-decoration:none;font-weight:var(--fw-500);font-size:14px;transition:color var(--dur-quick) var(--ease-out)}
.hm-nav nav a svg{width:18px;height:18px}
.hm-nav nav a.gh{margin-left:auto;width:36px;justify-content:center;color:var(--mute)}
.hm-nav nav a.gh span{position:absolute;width:1px;height:1px;overflow:hidden;clip:rect(0 0 0 0);white-space:nowrap}
.hm-nav nav a:hover{color:var(--text);text-decoration:none}
.hm-nav nav a[aria-current="page"]{color:var(--text)}
.hm-nav .theme-btn{flex:none;width:36px;height:36px;min-width:36px;color:var(--mute);margin-left:calc(var(--space-9) * -1 + var(--space-2))}
.hm-nav .theme-btn:hover{color:var(--text);background:none}
@media (max-width:560px){.hm-nav{gap:var(--space-6);padding:0 var(--space-6)}.hm-nav nav{gap:var(--space-5)}}

/* Hero */
.hm-hero{display:grid;grid-template-columns:minmax(0,1fr) 250px;gap:var(--space-10);align-items:center;padding:120px 0 96px}
@media (max-width:860px){.hm-hero{grid-template-columns:1fr;padding:56px 0 64px;gap:var(--space-9)}}
.hm-kicker{margin:var(--space-6) 0 0;font-size:13px;color:var(--mute)}
.hm-hero h1{margin:0;font-size:clamp(42px,6.4vw,80px);line-height:.98;letter-spacing:-.045em;font-weight:var(--fw-600)}
.hm-hero h1 span{color:var(--mute)}
.hm-lede{color:var(--mute);font-size:var(--fs-body);line-height:1.6;max-width:50ch}
.hm-hero .hm-lede{font-size:18px;margin:var(--space-8) 0 var(--space-9)}
.hm-ctas{display:flex;flex-wrap:wrap;gap:var(--space-4)}
.hm-btn{display:inline-flex;align-items:center;gap:var(--space-3);min-height:var(--touch-min);padding:0 var(--space-7);border-radius:var(--radius-sm);font-weight:var(--fw-500);font-size:15px;text-decoration:none;border:1px solid var(--text);transition:background var(--dur-quick) var(--ease-out),color var(--dur-quick) var(--ease-out)}
.hm-btn:hover{text-decoration:none}
.hm-btn svg{width:18px;height:18px;flex:none}
.hm-btn span{display:flex;flex-direction:column;line-height:1.25}
.hm-btn b{font-weight:var(--fw-600);font-size:15px}
.hm-btn small{font-size:12px;opacity:.6}
.hm-primary{background:var(--text);color:var(--page) !important}
.hm-primary:hover{background:transparent;color:var(--text) !important}
.hm-secondary{background:transparent;color:var(--text);border-color:var(--line)}
.hm-secondary:hover{border-color:var(--text)}
.hm-hero-art{display:flex;justify-content:flex-end}
@media (max-width:860px){.hm-hero-art{justify-content:flex-start}}

/* Sections: a rule, a small number and a title, then the content. */
.hm-sec{padding:var(--space-9) 0 112px;border-top:1px solid var(--line)}
.hm-sec h2{display:block;margin:0 0 var(--space-10);padding:0;border:0;font-size:clamp(26px,3.2vw,36px);letter-spacing:-.03em;font-weight:var(--fw-600)}

/* Why: a numbered list on hairlines, no boxes, no icons. */
.hm-why{display:grid;grid-template-columns:1fr;border-top:1px solid var(--text)}
.hm-card{display:grid;grid-template-columns:minmax(0,1fr) minmax(0,2fr);gap:var(--space-8);align-items:baseline;padding:var(--space-7) 0;border-bottom:1px solid var(--line);transition:padding var(--dur-base) var(--ease-out)}
@media (hover:hover){.hm-card:hover{padding-left:var(--space-4)}}
@media (max-width:560px){.hm-card{grid-template-columns:1fr;gap:var(--space-2)}}
.hm-card h3{margin:0;font-size:20px;font-weight:var(--fw-600);letter-spacing:-.015em}
.hm-card p{margin:0;color:var(--mute);font-size:17px;line-height:1.5}
.hm-ico{display:none}

/* Screens */
.hm-screens{display:grid;grid-template-columns:repeat(3,1fr);gap:var(--space-8);justify-items:center}
@media (max-width:860px){.hm-screens{grid-template-columns:none;grid-auto-flow:column;grid-auto-columns:250px;overflow-x:auto;scroll-snap-type:x mandatory;justify-items:start;padding-bottom:var(--space-4)}.hm-screens figure{scroll-snap-align:center}}
.hm-screens figure{margin:0;display:flex;flex-direction:column;align-items:flex-start;gap:var(--space-5)}
.hm-screens figcaption{font-family:var(--font-mono);font-size:12px;color:var(--mute)}

/* Phone: flat, drawn with lines, no tilt and no shadow. */
.hm-phone{position:relative;width:250px;height:510px;padding:7px;border-radius:38px;background:var(--page);border:1.5px solid var(--text)}
.hm-notch{position:absolute;top:16px;left:50%;transform:translateX(-50%);width:62px;height:16px;border-radius:var(--radius-full);background:var(--text);z-index:2}
.hm-screen{height:100%;border-radius:31px;background:var(--page);color:var(--text);overflow:hidden;display:flex;flex-direction:column;gap:8px;padding:44px 12px 12px;font-size:12px;line-height:1.4;border:1px solid var(--line)}
.hm-shot{padding:7px}
.hm-shot img{width:100%;height:100%;object-fit:cover;border-radius:31px;display:block}
.hm-sbar{display:flex;align-items:center;justify-content:space-between;font-size:14px;margin-bottom:4px}
.hm-sbar span{color:var(--mute);font-size:11px}
.hm-live{width:7px;height:7px;border-radius:50%;background:var(--text);animation:hmBreath 1.4s var(--ease-in-out) infinite}
@keyframes hmBreath{50%{opacity:.25}}
.hm-msg{max-width:86%;padding:8px 11px;border-radius:14px}
.hm-me{align-self:flex-end;background:var(--text);color:var(--page);border-bottom-right-radius:4px}
.hm-ai{align-self:flex-start;background:var(--field);border-bottom-left-radius:4px}
.hm-approve{border:1px solid var(--text);border-radius:12px;padding:9px 10px;display:flex;flex-direction:column;gap:7px}
.hm-row{display:flex;gap:6px}
.hm-b{flex:1;text-align:center;padding:5px;border-radius:7px;border:1px solid var(--line);font-weight:600;font-size:11px}
.hm-b.hm-ok{background:var(--text);color:var(--page);border-color:var(--text)}
.hm-compose{margin-top:auto;display:flex;align-items:center;justify-content:space-between;border:1px solid var(--line);border-radius:999px;padding:7px 7px 7px 14px;color:var(--mute)}
.hm-compose i{width:22px;height:22px;border-radius:50%;background:var(--text)}
.hm-tcard{border-top:1px solid var(--line);padding:9px 2px 4px;display:flex;flex-direction:column;gap:3px}
.hm-tcard small{color:var(--mute);font-size:10px;font-family:var(--font-mono)}
.hm-pbar{height:2px;background:var(--line);margin-top:6px}
.hm-pbar span{display:block;width:62%;height:100%;background:var(--text)}
.hm-tabs{margin-top:auto;display:flex;justify-content:space-around;padding:10px 0 2px;border-top:1px solid var(--line)}
.hm-tabs i{width:18px;height:18px;border-radius:50%;border:1.5px solid var(--line)}
.hm-tabs i.on{background:var(--text);border-color:var(--text)}
.hm-orb{margin:44px auto 28px;width:112px;height:112px;border-radius:50%;border:1.5px solid var(--text);display:flex;align-items:center;justify-content:center;animation:hmOrb 2.4s var(--ease-in-out) infinite}
.hm-orb span{width:40px;height:40px;border-radius:50%;background:var(--text)}
@keyframes hmOrb{50%{transform:scale(1.06)}}
.hm-heard{text-align:center;color:var(--mute);margin:0}
.hm-said{text-align:center;font-size:14px;font-weight:600;margin:6px 8px 0}

/* Run it yourself */
.hm-split{display:grid;grid-template-columns:.8fr 1.2fr;gap:var(--space-10);align-items:start}
@media (max-width:860px){.hm-split{grid-template-columns:1fr}}
.hm-split p a{text-decoration:underline;text-underline-offset:4px;text-decoration-color:var(--line)}
.hm-split p a:hover{text-decoration-color:var(--text)}
.hm-term{background:#0A0A0A;color:#EDEDED;border-radius:var(--radius-md);overflow:hidden;border:1px solid #262626}
.hm-term-bar{display:flex;align-items:center;padding:10px 16px;border-bottom:1px solid #262626}
.hm-term-bar span{width:9px;height:9px;border-radius:50%;background:#333333}
.hm-term-bar b{font-weight:400;font-size:12px;color:#8F8F8F;font-family:var(--font-mono)}
.hm-term pre{margin:0;padding:var(--space-6) var(--space-7);background:none;font-family:var(--font-mono);font-size:13px;line-height:1.8;overflow-x:auto;white-space:pre;border-radius:0}
.hm-term code{background:none;color:inherit;padding:0}
.hm-term .l{display:block;opacity:0;animation:hmLine .3s var(--ease-out) forwards}
.hm-term .l:nth-child(1){animation-delay:.2s}.hm-term .l:nth-child(2){animation-delay:.7s}.hm-term .l:nth-child(3){animation-delay:1.2s}.hm-term .l:nth-child(4){animation-delay:1.7s}.hm-term .l:nth-child(5){animation-delay:1.9s}.hm-term .l:nth-child(6){animation-delay:2.4s}.hm-term .l:nth-child(7){animation-delay:2.9s}
@keyframes hmLine{from{opacity:0;transform:translateX(-4px)}to{opacity:1;transform:none}}
.hm-term i{font-style:normal;color:#6B6B6B;margin-right:8px}
.hm-term em{font-style:normal;color:#6B6B6B}
.hm-cursor::after{content:"";display:inline-block;width:8px;height:15px;background:#EDEDED;vertical-align:-2px;animation:hmBlink 1s steps(1) infinite}
@keyframes hmBlink{50%{opacity:0}}

/* Community: open columns on hairlines instead of boxes. */
.hm-community{display:grid;grid-template-columns:repeat(2,minmax(0,1fr));gap:var(--space-10) var(--space-10)}
@media (max-width:860px){.hm-community{grid-template-columns:1fr}}
.hm-panel{display:flex;flex-direction:column;gap:var(--space-5)}
.hm-panel h3{margin:0;padding-bottom:var(--space-4);border-bottom:1px solid var(--text);font-size:14px;font-weight:var(--fw-600)}
.hm-empty{margin:0;color:var(--mute);font-size:var(--fs-small)}
.hm-more{margin-top:auto;font-size:14px;color:var(--mute) !important}
.hm-more:hover{color:var(--text) !important;text-decoration:none}
.hm-leader{list-style:none;margin:0;padding:0;display:flex;flex-direction:column}
.hm-lrow{display:grid;grid-template-columns:24px 28px minmax(0,1fr) 1fr auto;align-items:center;gap:var(--space-4);padding:var(--space-3) 0;border-bottom:1px solid var(--line)}
.hm-lrow img,.hm-av{width:28px;height:28px;border-radius:50%;background:var(--field);filter:grayscale(1)}
.hm-av{display:flex;align-items:center;justify-content:center;font-weight:600;font-size:12px}
.hm-lrow a{font-weight:var(--fw-500);font-size:14px;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}
.hm-rank{font-family:var(--font-mono);font-size:12px;color:var(--mute)}
.hm-gold .hm-rank,.hm-silver .hm-rank,.hm-bronze .hm-rank{color:var(--text)}
.hm-gold .hm-rank{font-weight:var(--fw-700)}
.hm-bar{height:2px;background:var(--line);overflow:hidden}
.hm-bar span{display:block;height:100%;background:var(--text);transform-origin:left;animation:rmGrow var(--dur-page) var(--ease-out) both}
.hm-cnt{font-family:var(--font-mono);font-size:12px;color:var(--mute);white-space:nowrap}
.hm-log{list-style:none;margin:0;padding:0;display:flex;flex-direction:column}
.hm-log>li{display:flex;flex-direction:column;gap:3px;padding:var(--space-4) 0;border-bottom:1px solid var(--line)}
.hm-lt{display:flex;align-items:baseline;gap:var(--space-4);color:var(--text);font-weight:var(--fw-500);font-size:14px}
.hm-lt:hover{text-decoration:underline;text-underline-offset:3px}
.hm-date{font-family:var(--font-mono);font-size:12px;color:var(--mute)}
.hm-tag{font-family:var(--font-mono);font-size:12px;color:var(--mute);flex:none}
.hm-notes{margin:var(--space-1) 0 0;padding-left:var(--space-6);color:var(--mute);font-size:12px}
.hm-state{font-family:var(--font-mono);font-size:11px;padding:0 6px;border-radius:4px;flex:none;border:1px solid var(--line);color:var(--mute)}
.hm-merged{background:var(--text);color:var(--page);border-color:var(--text)}
.hm-open{color:var(--text);border-color:var(--text)}
.hm-closed{text-decoration:line-through}
.hm-sha{font-size:12px;color:var(--mute);background:none;padding:0;flex:none}
.hm-fetched{color:var(--mute);font-size:12px;font-family:var(--font-mono);margin:var(--space-8) 0 0}
.hm-foot{display:flex;flex-wrap:wrap;align-items:center;gap:var(--space-7);max-width:1120px;margin:0 auto;padding:var(--space-8);border-top:1px solid var(--line);color:var(--mute);font-size:14px}
.hm-foot span{font-weight:var(--fw-600);color:var(--text);margin-right:auto}
.hm-foot a{display:inline-flex;align-items:center;gap:var(--space-2);color:var(--mute)}
.hm-foot a:hover{color:var(--text);text-decoration:none}
.hm-foot svg{width:15px;height:15px}
@media (prefers-reduced-motion:reduce){.hm-term .l{opacity:1}}

/* design in action: phone frames, mock screen, adaptive states */
.demo-frame.light{--page:#FAFAFA;--card:#FFFFFF;--raised:#FFFFFF;--field:#F1F1F1;--bubble:#EDEDED;--code:#F4F4F4;--line:#E4E4E4;--scrim:rgba(10,10,10,.32);--text:#0A0A0A;--mute:#666666;--ink:#0A0A0A;--on-ink:#FFFFFF;--accent:#0A0A0A;--on-accent:#FFFFFF;--accent-soft:#0A0A0A0F;--ok:#157A42;--warn:#8A5D00;--bad:#B8291F;--shadow:0 0 0 1px #E4E4E4;--shadow-raised:0 0 0 1px #E4E4E4,0 12px 32px rgba(0,0,0,.08)}
.demo-frame.dark{--page:#000000;--card:#111111;--raised:#1A1A1A;--field:#1A1A1A;--bubble:#1C1C1C;--code:#0A0A0A;--line:#262626;--scrim:rgba(0,0,0,.56);--text:#EDEDED;--mute:#8F8F8F;--ink:#EDEDED;--on-ink:#0A0A0A;--accent:#EDEDED;--on-accent:#0A0A0A;--accent-soft:#EDEDED1A;--ok:#43C076;--warn:#E2A826;--bad:#FF6B63;--shadow:none;--shadow-raised:0 0 0 1px #262626}
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
  .rm-at{opacity:1}
  .rm-travelled{stroke-dashoffset:0}
  .sidebar{transition:none}
}

/* ===== One site: shared header, docs layout, theme and motion ===== */
.hm-nav .menu-btn{display:none;margin-left:calc(var(--space-3) * -1)}
.topbar,.sidebar-head{display:none !important}
.sidebar{top:56px;background:var(--page);border-right:1px solid var(--line);z-index:15}
.search{padding:var(--space-7) var(--space-6) var(--space-4)}
.search input{background:transparent;border:1px solid var(--line);border-radius:var(--radius-sm);transition:border-color var(--dur-quick) var(--ease-out)}
.search input:focus{outline:0;border-color:var(--text)}
.nav .section > .stitle{font-size:12px;font-weight:var(--fw-500);letter-spacing:0;text-transform:none;color:var(--mute);padding:0 var(--space-3) var(--space-3)}
.nav .pl{color:var(--mute);border-radius:0;padding:5px var(--space-3);box-shadow:inset 1px 0 0 var(--line);transition:color var(--dur-quick) var(--ease-out),box-shadow var(--dur-quick) var(--ease-out)}
.nav .pl:hover{background:none;color:var(--text)}
.nav .pl.active{background:none;color:var(--text);font-weight:var(--fw-500);box-shadow:inset 2px 0 0 var(--text)}
.main .prose{padding-top:var(--space-10)}
.prose h1{font-size:clamp(32px,4.2vw,44px);line-height:1.05;letter-spacing:-.035em;font-weight:var(--fw-600);margin-bottom:var(--space-8)}
.prose h2{border-bottom:0;border-top:1px solid var(--line);padding:var(--space-7) 0 0;margin-top:var(--space-10);letter-spacing:-.02em}
.prose a{color:var(--text);text-decoration:underline;text-underline-offset:3px;text-decoration-color:var(--line);transition:text-decoration-color var(--dur-quick) var(--ease-out)}
.prose a:hover{text-decoration-color:var(--text)}
.prose table{background:none;border-radius:0;box-shadow:none;border-top:1px solid var(--text)}
.prose th{background:none;font-size:12px;font-weight:var(--fw-500);color:var(--mute)}
.prose pre{border:1px solid var(--line)}
.prose blockquote{border-left:2px solid var(--text)}
@media (max-width:839px){
  .hm-nav .menu-btn{display:inline-flex}
  .sidebar{top:56px;box-shadow:none;border-right:1px solid var(--line)}
  .main .prose{padding-top:var(--space-8)}
}

/* Theme switch: everything eases between light and dark. */
html.theme-anim,html.theme-anim *,html.theme-anim *::before,html.theme-anim *::after{transition:background-color var(--dur-slow) var(--ease-in-out),color var(--dur-slow) var(--ease-in-out),border-color var(--dur-slow) var(--ease-in-out),fill var(--dur-slow) var(--ease-in-out) !important}
::view-transition-old(root),::view-transition-new(root){animation-duration:var(--dur-slow);animation-timing-function:var(--ease-in-out)}
.theme-btn .icon{transition:transform var(--dur-base) var(--ease-spring)}
.theme-btn:active .icon{transform:rotate(-30deg) scale(.9)}

/* Moving between pages: the header and sidebar stay still, only the page content fades up. */
html.motion .docs .main .prose{animation:axIn var(--dur-base) var(--ease-out) both}
@keyframes axIn{from{opacity:0;transform:translateY(6px)}to{opacity:1;transform:none}}

/* Things rise in as they scroll into view (see Motion: 16 px, dur-page, 40 ms apart). */
html.motion .rv{opacity:0;transform:translateY(16px);transition:opacity var(--dur-page) var(--ease-out),transform var(--dur-page) var(--ease-out);transition-delay:var(--d,0ms)}
html.motion .rv.in{opacity:1;transform:none}

/* Small touches. */
.hm-btn{transition:background var(--dur-quick) var(--ease-out),color var(--dur-quick) var(--ease-out),border-color var(--dur-quick) var(--ease-out),transform var(--dur-instant) var(--ease-out)}
.hm-btn:active{transform:scale(.98)}
.hm-phone{transition:transform var(--dur-slow) var(--ease-out)}
@media (hover:hover){.hm-screens figure:hover .hm-phone{transform:translateY(-6px)}}

@media (prefers-reduced-motion:reduce){
  html.motion .docs .main .prose{animation:none}
  html.motion .rv{opacity:1;transform:none;transition:none}
  html.theme-anim *{transition:none !important}
  .hm-live,.hm-orb,.hm-cursor::after{animation:none !important}
}
"#;

pub const APP_JS: &str = r#"(function(){
  var root=document.documentElement;
  // Theme: one choice shared by the home page and every docs page.
  // Over http it lives in localStorage. Opened as files, it rides in ?theme= on each link.
  var KEY='assistx-theme';
  var media=window.matchMedia?matchMedia('(prefers-color-scheme: dark)'):null;
  var reduce=window.matchMedia&&matchMedia('(prefers-reduced-motion: reduce)').matches;
  function saved(){
    var m=/[?&]theme=(light|dark)/.exec(location.search)||/ax-theme=(light|dark)/.exec(window.name||'');
    var tab=m&&m[1],v=null;try{v=localStorage.getItem(KEY)||localStorage.getItem('assistx-docs-theme');}catch(e){}
    // On file:// the tab's value is newest; on http localStorage is shared, so it wins.
    if(location.protocol==='file:'&&tab)return tab;
    if(v==='light'||v==='dark')return v;
    return tab||null;}
  function system(){return media&&media.matches?'dark':'light';}
  function current(){return root.getAttribute('data-theme')||system();}
  function apply(mode){if(mode==='light'||mode==='dark'){root.setAttribute('data-theme',mode);}else{root.removeAttribute('data-theme');}}
  function save(mode){
    try{localStorage.setItem(KEY,mode);localStorage.removeItem('assistx-docs-theme');}catch(e){}
    window.name='ax-theme='+mode;
    // file:// pages (and Firefox/Zen, which clears window.name between them) only
    // keep what is in the address, so write the choice into this page's own URL too.
    if(location.protocol==='file:'){try{var u=new URL(location.href);u.searchParams.set('theme',mode);history.replaceState(history.state,'',u.href);}catch(e){}}
  }
  function switchTo(mode){
    if(reduce){apply(mode);return;}
    if(document.startViewTransition){document.startViewTransition(function(){apply(mode);});return;}
    root.classList.add('theme-anim');apply(mode);setTimeout(function(){root.classList.remove('theme-anim');},400);
  }
  var first=saved();apply(first);if(first){save(first);}
  document.addEventListener('click',function(e){
    var t=e.target.closest('.theme-btn');
    if(t){e.preventDefault();var next=current()==='dark'?'light':'dark';save(next);switchTo(next);}
  });
  // Another tab changed it: follow along.
  window.addEventListener('storage',function(e){if(e.key===KEY&&(e.newValue==='light'||e.newValue==='dark'))switchTo(e.newValue);});
  // Motion: page fade, and things rising in as they scroll into view.
  window.addEventListener('pageshow',function(){var s=saved();if(s)apply(s);});
  // Keep the sidebar where it was, so changing page does not jump the list.
  var side=document.getElementById('sidebar');
  if(side){
    try{var top=sessionStorage.getItem('ax-side');if(top!==null)side.scrollTop=+top;}catch(e){}
    var act=side.querySelector('.pl.active');
    if(act){var r=act.getBoundingClientRect(),sr=side.getBoundingClientRect();if(r.top<sr.top||r.bottom>sr.bottom)act.scrollIntoView({block:'center'});}
    window.addEventListener('pagehide',function(){try{sessionStorage.setItem('ax-side',side.scrollTop);}catch(e){}});
  }
  document.addEventListener('click',function(e){
    if(e.defaultPrevented||e.button!==0||e.metaKey||e.ctrlKey||e.shiftKey||e.altKey)return;
    var a=e.target.closest('a[href]');if(!a||a.target||a.hasAttribute('download'))return;
    var href=a.getAttribute('href');if(!href||href.charAt(0)==='#'||/^(mailto:|tel:|javascript:)/i.test(href))return;
    var url=new URL(a.href,location.href);
    if(url.protocol!==location.protocol||url.host!==location.host)return;
    if(url.pathname===location.pathname&&url.hash)return;
    // file:// pages do not share storage, so carry the choice in the link.
    var mode=saved();if(location.protocol==='file:'&&mode){url.searchParams.set('theme',mode);}
    if(url.href!==a.href){e.preventDefault();location.href=url.href;}
  });
  if(!reduce&&'IntersectionObserver' in window){
    var groups=[
      '.hm-hero-text > *','.hm-hero-art',
      '.hm-sec > h2','.hm-card','.hm-screens figure','.hm-split > div','.hm-panel'
    ];
    var seen=new IntersectionObserver(function(entries){
      var batch=0;
      entries.forEach(function(en){if(!en.isIntersecting)return;var el=en.target;
        el.style.setProperty('--d',Math.min(batch*40,200)+'ms');batch++;
        el.classList.add('in');seen.unobserve(el);});
    },{rootMargin:'0px 0px -8% 0px',threshold:0.05});
    groups.forEach(function(sel){document.querySelectorAll(sel).forEach(function(el){el.classList.add('rv');seen.observe(el);});});
  }
  // sidebar
  var sb=document.getElementById('sidebar'),scrim=document.getElementById('scrim');
  function openSB(v){if(!sb)return;sb.classList.toggle('open',v);scrim.classList.toggle('show',v);}
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
  if(input)input.addEventListener('input',function(){q=input.value.trim();if(!q){results.innerHTML='';results.hidden=true;return;}load().then(function(){
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
    if(kind==='buzz'){var pat=(b.dataset.pattern||'10').split(',').map(Number);try{navigator.vibrate&&navigator.vibrate(pat.length>1?pat:pat[0]);}catch(err){}b.classList.remove('buzz');void b.offsetWidth;b.classList.add('buzz');}
    if(kind==='demo-tab'){var nav=b.closest('.ph-nav');if(nav){nav.querySelectorAll('.demo-tab').forEach(function(x){x.setAttribute('aria-pressed',x===b?'true':'false');});}}
  });
})();
"#;
