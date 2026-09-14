(() => {
  "use strict";
  document.documentElement.classList.add("js");

  const REPO = "leandrodaf/3d-new-era-ai";
  const MCP = "http://127.0.0.1:7878/mcp";
  const RAW = `https://raw.githubusercontent.com/${REPO}/main/scripts`;

  const EN = {
    "skip": "Skip to content",
    "nav.how": "How it works", "nav.gallery": "Gallery", "nav.ai": "Connect AI", "nav.faq": "FAQ", "nav.download": "Download",
    "hero.eyebrow": "Open source · free · Windows, macOS and Linux",
    "hero.title1": "Design homes by talking to your AI.",
    "hero.title2": "It draws, furnishes and takes the photos. You approve.",
    "hero.download": "Download free", "hero.see": "See how it works",
    "hero.fig": "FIG. 01 — Dining room at dusk, rendered by the app",
    "works": "Works with", "works.more": "and others via MCP",
    "st.strong": "An architecture editor with an MCP server inside.",
    "st.rest": "You describe what you want; the agent uses the same tools you do — walls, doors, joinery, lighting, camera — and every change shows up on screen, one Ctrl+Z away.",
    "product.fig": "FIG. 02 — The editor: catalog, rendered floor plan and live 3D, with the AI working over MCP",
    "t1.h": "The AI edits the real file", "t1.p": "It's not an image generator. The agent edits the actual project, in centimeters, and you watch every wall appear.",
    "t2.h": "Everything is true to size", "t2.p": "A 158 × 208 cm bed is exactly that in the plan, in 3D and in collision checks.",
    "t3.h": "Runs on your computer", "t3.p": "Your project stays with you. No account, no subscription, no cloud. Renders photos even without a graphics card.",
    "cv.h1": "One conversation, one apartment.", "cv.h2": "A real 105 m² floor plan, built entirely by the AI.",
    "cv.you": "you", "cv.prompt": "Build this FOCSA floor plan at real scale, furnish it like a modern apartment and light it to code.",
    "cv.c1": "plan at scale", "cv.c2": "walls and rooms", "cv.c3": "swinging the right way", "cv.c4": "finishes", "cv.c5": "slatted wall",
    "cv.c6": "kitchen and cabinets", "cv.c8": "nothing blocked ✓", "cv.c9": "dining photo",
    "cv.fig": "FIG. 03 — Rendered floor plan exported by the app",
    "s1": "of real floor plan, traced from the image", "s2": "furniture pieces in the right place", "s3": "rooms with lighting checked against code", "s4": "MCP tools at the AI's disposal",
    "dn.h1": "3 pm sun or lamps on.", "dn.h2": "Same camera, the lights you placed.",
    "dn.dining": "Dining", "dn.kitchen": "Kitchen", "dn.suite": "Bedroom",
    "dn.fig": "FIG. 04 — Drag to compare · path tracing with the sun from the compass and fixtures in lumens",
    "dn.aria": "Compare day and night",
    "ft.h1": "Made for people who really design.", "ft.h2": "And for the AI working with them.",
    "f1.h": "A floor plan from a picture", "f1.p": "Drop in a plan image at scale and the AI finds the walls. Opens Sweet Home 3D projects too.",
    "f2.h": "Joinery a workshop can build", "f2.p": "Cabinets, slatted panels, countertops with sink and cooktop cutouts. Cut lists in CSV and DXF.",
    "f3.h": "Lighting checked against code", "f3.p": "Lux per room by photometry, against NBR ISO/CIE 8995-1. The AI adds the fixtures a room is missing.",
    "f4.h": "Ergonomics for who lives there", "f4.p": "Circulation, wheelchair turning space, the kitchen triangle — NBR 9050 and 15575, with the fix ready.",
    "f5.h": "Photos, sections and videos", "f5.p": "Rendered plans, elevations, sections, aerial views, realistic photos and walkthrough videos.",
    "f6.h": "Desktop, browser and teams", "f6.p": "The editor on your computer or in the browser, plugins in any language and several people on one project.",
    "gl.h1": "Everything here came out of the app.", "gl.h2": "No Photoshop, no render farm.", "gl.how": "How it was made →",
    "g1": "Aerial cutaway", "g2": "Slatted wall with built-in TV", "g3": "L kitchen, terrazzo", "g4": "Home office", "g5": "Bedroom at night", "g6": "Living and dining, 33 m²", "g7": "Kitchen at night", "g8": "Bathroom, Nero marble", "g9": "Bedroom with wardrobes", "g10": "Dining at 3 pm",
    "ai.h1": "Connect the AI you already use.", "ai.h2": "One command, and it can see your project.",
    "ai.s1h": "Open 3D New Era AI", "ai.s1p": "The MCP server starts with it, at <code>http://127.0.0.1:7878/mcp</code>.",
    "ai.s2h": "Run your AI's command", "ai.s2p": "Just once. The installers already do it for Claude Code and Codex.",
    "ai.s3h": "Ask", "ai.s3p": "“Draw a 4 × 5 m bedroom with a door and a window, furnish it and render a photo.”",
    "ai.other": "DeepSeek and others", "copy": "Copy", "copied": "Copied",
    "dl.h1": "Download and start now.", "dl.h2": "Free, no account, no administrator password.",
    "dl.files": "Or download the file",
    "dl.first": "First launch of a downloaded file",
    "dl.firstp": "The apps aren't signed with a paid Apple or Microsoft certificate, so the system asks once. <b>Windows:</b> open <code>newera-gui.exe</code> and, if SmartScreen shows up, click <i>More info → Run anyway</i>. <b>macOS:</b> move the app to Applications, right-click → <i>Open</i> (macOS 15+: <i>Settings → Privacy &amp; Security → Open Anyway</i>). The command above skips these prompts.",
    "dl.notes": "release notes", "dl.license": "MIT or Apache 2.0",
    "faq.h": "FAQ",
    "q1": "Is it really free?", "a1": "Yes. It's open source (MIT or Apache 2.0), with no account and no subscription. You only pay for the AI you choose, if it's a paid one.",
    "q2": "Do I need to know how to code?", "a2": "No. Install with one command, open the app and draw with the mouse like any editor. The AI is optional — once connected, you just talk to it.",
    "q3": "Which AI works?", "a3": "Any app that speaks MCP: Claude Code, Claude Desktop, Codex, Gemini CLI, Cursor, VS Code, Windsurf, and apps like Cline and Cherry Studio that run DeepSeek, Qwen, Llama and other models.",
    "q4": "Does my project go to the cloud?", "a4": "No. The app and its MCP server run on your computer and only accept connections from the same machine. The project is your own <code>.newera</code> file.",
    "q5": "Do I need a graphics card?", "a5": "No. The 3D view uses the GPU when there is one, and photos render on the CPU — even on a headless server.",
    "q6": "Does it open my Sweet Home 3D projects?", "a6": "Yes, <code>.sh3d</code> files open with walls, rooms, furniture, lights, cameras and levels.",
    "foot.by": "Built in Rust by Leandro Ferreira.", "foot.releases": "Releases", "foot.showcase": "Showcase", "foot.issues": "Report an issue",
    "foot.credits": "Reference plan: <i>Typical apartment floor plan FOCSA Building</i>, Osvaldo Valdes, CC BY-SA 4.0. Textures: ambientCG, CC0."
  };

  const T = {
    pt: {
      winLead: "Abra o <b>PowerShell</b> (menu Iniciar → digite “PowerShell”) e cole:",
      winAfter: "Instala em segundos, cria atalho no Menu Iniciar e na Área de Trabalho e aparece em Configurações → Aplicativos para desinstalar.",
      macLead: "Abra o <b>Terminal</b> (⌘ + espaço → “Terminal”) e cole:",
      macAfter: "Baixa a versão certa para Apple Silicon ou Intel, coloca o app no Launchpad e não pede senha. Rode de novo para atualizar.",
      linLead: "Baixe, descompacte e abra:",
      linAfter: "Precisa das bibliotecas de janela do sistema (já presentes na maioria das distribuições com desktop).",
      dlWin: "Baixar para Windows", dlMac: "Baixar para Mac", dlLinux: "Baixar para Linux", dlAny: "Baixar grátis",
      where: { claude: "No terminal:", codex: "No terminal:", gemini: "No terminal:", vscode: "No terminal:", cursor: "Arquivo ~/.cursor/mcp.json", desktop: "Configurações → Desenvolvedor → Editar configuração", other: "Nas configurações de MCP do app (Cline, Roo Code, Cherry Studio, LM Studio…)" },
      note: {
        claude: "Os instaladores já registram sozinhos se o Claude Code estiver instalado.",
        codex: "Os instaladores já registram sozinhos se o Codex estiver instalado.",
        gemini: "Depois abra o Gemini CLI e peça o projeto.",
        vscode: "Use no modo agente do Copilot.",
        cursor: "Reinicie o Cursor depois de salvar.",
        desktop: "Precisa do Node.js instalado para a ponte mcp-remote. Reinicie o Claude Desktop.",
        other: "O modelo pode ser DeepSeek, Qwen, Llama ou outro: o que importa é o app ter suporte a MCP. No opencode, use o bloco acima em opencode.json."
      }
    },
    en: {
      winLead: "Open <b>PowerShell</b> (Start menu → type “PowerShell”) and paste:",
      winAfter: "Installs in seconds, adds Start menu and desktop shortcuts, and shows up in Settings → Apps to uninstall.",
      macLead: "Open <b>Terminal</b> (⌘ + space → “Terminal”) and paste:",
      macAfter: "Picks the right build for Apple Silicon or Intel, puts the app in Launchpad and never asks for a password. Run it again to update.",
      linLead: "Download, extract and run:",
      linAfter: "Needs the system's windowing libraries (already there on most desktop distributions).",
      dlWin: "Download for Windows", dlMac: "Download for Mac", dlLinux: "Download for Linux", dlAny: "Download free",
      where: { claude: "In a terminal:", codex: "In a terminal:", gemini: "In a terminal:", vscode: "In a terminal:", cursor: "File ~/.cursor/mcp.json", desktop: "Settings → Developer → Edit Config", other: "In the app's MCP settings (Cline, Roo Code, Cherry Studio, LM Studio…)" },
      note: {
        claude: "The installers register it for you when Claude Code is installed.",
        codex: "The installers register it for you when Codex is installed.",
        gemini: "Then open Gemini CLI and ask for your project.",
        vscode: "Use it in Copilot's agent mode.",
        cursor: "Restart Cursor after saving.",
        desktop: "Needs Node.js for the mcp-remote bridge. Restart Claude Desktop.",
        other: "The model can be DeepSeek, Qwen, Llama or anything else: what matters is that the app speaks MCP. In opencode, put the block above in opencode.json."
      }
    }
  };

  const CODE = {
    claude: `claude mcp add --transport http newera ${MCP}`,
    codex: `codex mcp add newera --url ${MCP}`,
    gemini: `gemini mcp add --transport http newera ${MCP}`,
    vscode: `code --add-mcp '{"name":"newera","type":"http","url":"${MCP}"}'`,
    cursor: `{\n  "mcpServers": {\n    "newera": { "url": "${MCP}" }\n  }\n}`,
    desktop: `{\n  "mcpServers": {\n    "newera": {\n      "command": "npx",\n      "args": ["-y", "mcp-remote", "${MCP}"]\n    }\n  }\n}`,
    other: `{\n  "mcp": {\n    "newera": { "type": "remote", "url": "${MCP}" }\n  }\n}`
  };

  const OS_CODE = {
    windows: `irm ${RAW}/install-windows.ps1 | iex`,
    mac: `curl -fsSL ${RAW}/install-macos.sh | bash`,
    linux: `curl -LO https://github.com/${REPO}/releases/latest/download/newera-linux-x64.tar.gz\ntar xzf newera-linux-x64.tar.gz && ./newera/newera`
  };

  const $ = (s, root = document) => root.querySelector(s);
  const $$ = (s, root = document) => Array.from(root.querySelectorAll(s));

  // ---- Language ----
  const PT = {};
  $$("[data-i18n]").forEach((el) => { PT[el.dataset.i18n] = el.innerHTML; });
  let lang = "pt";
  let tab = "claude";
  let os = detectOS();

  function store(key, value) { try { localStorage.setItem(key, value); } catch (_) { /* private mode */ } }
  function read(key) { try { return localStorage.getItem(key); } catch (_) { return null; } }

  function setLang(next) {
    lang = next === "en" ? "en" : "pt";
    const dict = lang === "en" ? EN : PT;
    $$("[data-i18n]").forEach((el) => {
      const value = dict[el.dataset.i18n];
      if (value !== undefined) el.innerHTML = value;
    });
    document.documentElement.lang = lang === "en" ? "en" : "pt-BR";
    $$(".lang button").forEach((b) => b.setAttribute("aria-pressed", String(b.dataset.lang === lang)));
    const range = $(".compare__range");
    if (range) range.setAttribute("aria-label", lang === "en" ? EN["dn.aria"] : "Comparar dia e noite");
    renderTab();
    renderOS();
    store("newera-lang", lang);
  }

  $$(".lang button").forEach((b) => b.addEventListener("click", () => setLang(b.dataset.lang)));

  // ---- OS detection ----
  function detectOS() {
    const platform = (navigator.userAgentData && navigator.userAgentData.platform) || navigator.platform || "";
    const ua = navigator.userAgent || "";
    if (/win/i.test(platform) || /Windows/i.test(ua)) return "windows";
    if (/mac/i.test(platform) || /Mac OS X/i.test(ua)) return /iPhone|iPad/i.test(ua) ? "mac" : "mac";
    if (/linux/i.test(platform) && !/Android/i.test(ua)) return "linux";
    return "windows";
  }
  const detected = (() => {
    const ua = navigator.userAgent || "";
    if (/Android|iPhone|iPad/i.test(ua)) return null;
    return detectOS();
  })();

  function renderOS() {
    const t = T[lang];
    $$(".os button").forEach((b) => b.setAttribute("aria-selected", String(b.dataset.os === os)));
    const lead = { windows: t.winLead, mac: t.macLead, linux: t.linLead }[os];
    const after = { windows: t.winAfter, mac: t.macAfter, linux: t.linAfter }[os];
    $("[data-os-lead]").innerHTML = lead;
    $("[data-os-after]").textContent = after;
    $("[data-os-code]").textContent = OS_CODE[os];
    const label = $("[data-download-label]");
    if (label) label.textContent = detected ? { windows: t.dlWin, mac: t.dlMac, linux: t.dlLinux }[detected] : t.dlAny;
  }
  $$(".os button").forEach((b) => b.addEventListener("click", () => { os = b.dataset.os; renderOS(); }));
  $$(".os button").forEach((b, i, all) => b.addEventListener("keydown", (e) => arrowNav(e, all, i)));

  // ---- AI client tabs ----
  function renderTab() {
    const t = T[lang];
    $$(".tabs__list button").forEach((b) => b.setAttribute("aria-selected", String(b.dataset.tab === tab)));
    $("[data-where]").textContent = t.where[tab];
    $("[data-code]").textContent = CODE[tab];
    $("[data-note]").textContent = t.note[tab];
  }
  $$(".tabs__list button").forEach((b) => b.addEventListener("click", () => { tab = b.dataset.tab; renderTab(); }));
  $$(".tabs__list button").forEach((b, i, all) => b.addEventListener("keydown", (e) => arrowNav(e, all, i)));

  function arrowNav(e, all, i) {
    if (e.key !== "ArrowRight" && e.key !== "ArrowLeft") return;
    const next = all[(i + (e.key === "ArrowRight" ? 1 : all.length - 1)) % all.length];
    next.focus();
    next.click();
  }

  // ---- Copy ----
  function copy(button, text) {
    const done = () => {
      button.textContent = lang === "en" ? EN.copied : "Copiado";
      button.classList.add("is-done");
      setTimeout(() => { button.textContent = lang === "en" ? EN.copy : PT.copy; button.classList.remove("is-done"); }, 1800);
    };
    if (navigator.clipboard && window.isSecureContext) {
      navigator.clipboard.writeText(text).then(done, () => fallback(text, done));
    } else fallback(text, done);
  }
  function fallback(text, done) {
    const area = document.createElement("textarea");
    area.value = text; area.setAttribute("readonly", ""); area.style.position = "fixed"; area.style.opacity = "0";
    document.body.appendChild(area); area.select();
    try { document.execCommand("copy"); done(); } catch (_) { /* nothing to do */ }
    area.remove();
  }
  $("[data-copy]").addEventListener("click", (e) => copy(e.currentTarget, CODE[tab]));
  $("[data-copy-os]").addEventListener("click", (e) => copy(e.currentTarget, OS_CODE[os]));

  // ---- Day / night ----
  const SCENES = {
    jantar: ["02-jantar-estar", "09-jantar-noite", "19:30"],
    cozinha: ["04-cozinha", "10-cozinha-noite", "19:30"],
    suite: ["07-suite", "11-suite-noite", "20:00"]
  };
  const compare = $(".compare");
  const range = $(".compare__range");
  const setPos = (v) => compare.style.setProperty("--pos", `${v}%`);
  range.addEventListener("input", () => setPos(range.value));
  $$(".chips button").forEach((b) => b.addEventListener("click", () => {
    const [day, night, hour] = SCENES[b.dataset.scene];
    $$(".chips button").forEach((c) => c.setAttribute("aria-selected", String(c === b)));
    $("[data-day]").src = `images/showcase/${day}.jpg`;
    $("[data-night]").src = `images/showcase/${night}.jpg`;
    $(".compare__tag--r").textContent = hour;
  }));
  // A gentle hint of what the slider does, once, when it first comes into view.
  const hint = () => {
    if (window.matchMedia("(prefers-reduced-motion: reduce)").matches) return;
    let t0 = null;
    const step = (t) => {
      if (t0 === null) t0 = t;
      const p = Math.min((t - t0) / 1600, 1);
      const v = 50 + Math.sin(p * Math.PI * 2) * 18 * (1 - p);
      setPos(v.toFixed(2)); range.value = v;
      if (p < 1) requestAnimationFrame(step);
    };
    requestAnimationFrame(step);
  };

  // ---- Reveal on scroll ----
  const reveal = $$("[data-reveal]");
  if ("IntersectionObserver" in window) {
    const io = new IntersectionObserver((entries) => {
      entries.forEach((entry) => {
        if (!entry.isIntersecting) return;
        const el = entry.target;
        el.classList.add("is-in");
        if (el.classList.contains("term")) {
          $$(".term__calls li", el).forEach((li, i) => { li.style.transitionDelay = `${250 + i * 170}ms`; });
        }
        if (el === compare) hint();
        io.unobserve(el);
      });
    }, { threshold: 0.25 });
    reveal.forEach((el) => io.observe(el));
    io.observe(compare);
    // Never leave content hidden if the observer doesn't fire (old browsers, print, crawlers).
    setTimeout(() => reveal.forEach((el) => el.classList.add("is-in")), 6000);
  } else {
    reveal.forEach((el) => el.classList.add("is-in"));
  }

  // ---- Nav border on scroll ----
  const nav = $(".nav");
  const onScroll = () => nav.classList.toggle("is-scrolled", window.scrollY > 8);
  window.addEventListener("scroll", onScroll, { passive: true });
  onScroll();

  // ---- Latest version ----
  fetch(`https://api.github.com/repos/${REPO}/releases/latest`, { headers: { Accept: "application/vnd.github+json" } })
    .then((r) => (r.ok ? r.json() : null))
    .then((release) => { if (release && release.tag_name) $$("[data-version]").forEach((el) => { el.textContent = release.tag_name; }); })
    .catch(() => {});

  // ---- Start ----
  const params = new URLSearchParams(location.search);
  if (params.get("theme") === "light" || params.get("theme") === "dark") document.documentElement.dataset.theme = params.get("theme");
  const initial = params.get("lang") || read("newera-lang") || ((navigator.language || "").toLowerCase().startsWith("pt") ? "pt" : "en");
  if (detected) os = detected;
  setLang(initial);
})();
