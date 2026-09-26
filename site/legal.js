// The language of the legal pages.
//
// Each page carries both texts, one section per language, so that a visitor
// without JavaScript — and a search engine — still reads everything. With
// JavaScript, only the section the visitor reads is shown, chosen the way the
// rest of the site chooses it: `?lang=`, then what they last picked, then what
// the browser asks for. The choice is stored under the same key the home page
// uses, so picking English here keeps English there.
//
// Only Portuguese and English exist in the legal pages. A visitor whose
// browser asks for anything else gets English, which is the one the rest of
// the world reads; the home page still speaks Spanish and French.
(() => {
  "use strict";

  const LANGS = ["pt", "en"];
  const TAG = { pt: "pt-BR", en: "en" };
  const KEY = "newera-lang";

  const sections = new Map();
  for (const code of LANGS) {
    const found = document.getElementById(code);
    if (found) sections.set(code, found);
  }
  // A page that was not split into sections: leave it exactly as it is.
  if (sections.size < 2) return;

  // What the stylesheet waits for before showing the switch: until here, both
  // sections are on the page and there is nothing to switch between.
  document.documentElement.classList.add("js");

  function read(key) {
    try {
      return localStorage.getItem(key);
    } catch (_) {
      return null; // private mode
    }
  }

  function store(key, value) {
    try {
      localStorage.setItem(key, value);
    } catch (_) {
      /* private mode */
    }
  }

  /// The two letters of a tag like `pt-BR` or `en-GB`, when a section speaks
  /// it. Spanish and French fall through to English below.
  function normalize(tag) {
    const base = String(tag || "").toLowerCase().slice(0, 2);
    return LANGS.includes(base) ? base : null;
  }

  function fromBrowser() {
    const asked =
      navigator.languages && navigator.languages.length
        ? navigator.languages
        : [navigator.language];
    for (const tag of asked) {
      const known = normalize(tag);
      if (known) return known;
    }
    return "en";
  }

  const description = document.querySelector('meta[name="description"]');

  function show(next, remember) {
    const lang = normalize(next) || "en";
    for (const [code, section] of sections) section.hidden = code !== lang;
    document.documentElement.lang = TAG[lang];
    // The tab and the shared link should say what the page says.
    const section = sections.get(lang);
    if (section?.dataset.title) document.title = section.dataset.title;
    if (description && section?.dataset.description) {
      description.setAttribute("content", section.dataset.description);
    }
    for (const button of document.querySelectorAll(".legal__lang button")) {
      button.setAttribute("aria-pressed", String(button.dataset.lang === lang));
    }
    if (remember) store(KEY, lang);
    return lang;
  }

  const params = new URLSearchParams(location.search);
  // `#en` is how the pages linked to each other before there was a switch;
  // those links still land on English.
  const hash = location.hash === "#en" ? "en" : null;
  const initial =
    normalize(params.get("lang")) || hash || normalize(read(KEY)) || fromBrowser();
  show(initial, false);

  document.addEventListener("click", (event) => {
    const button = event.target.closest(".legal__lang button");
    if (!button) return;
    event.preventDefault();
    const lang = show(button.dataset.lang, true);
    // The address should say what is on screen, so a reload or a shared link
    // keeps it — and the old `#en` does not fight the new choice.
    const url = new URL(location.href);
    url.searchParams.set("lang", lang);
    url.hash = "";
    history.replaceState(null, "", url);
  });
})();
