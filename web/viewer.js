// Viewer glue: loads newera_web.wasm and talks to its tiny C ABI.
const status = document.getElementById("status");
const planBox = document.getElementById("plan");
const viewBox = document.getElementById("view");
const image = document.getElementById("image");

// The page is written in Portuguese; these are the other languages the app
// speaks, picked by what the browser asks for.
// A number and what it counts in Ukrainian, which has three forms: after 1,
// after 2–4 and after the rest (1 стіна, 2 стіни, 5 стін).
const uk = (n, [one, few, many]) => {
  const unit = n % 10, tens = n % 100;
  return `${n} ${unit === 1 && tens !== 11 ? one : unit >= 2 && unit <= 4 && (tens < 12 || tens > 14) ? few : many}`;
};
const WORDS = {
  en: { open: "Open a project (.newera)", loading: "Loading the engine…", plan: "Floor plan", drag: "Drag to orbit",
        ready: "Ready. Open a .newera project.", failed: (n, e) => `Could not open ${n}: ${e}`,
        about: (i) => `${i.name}: ${i.walls} walls, ${i.rooms} rooms, ${i.furniture} pieces` },
  es: { open: "Abrir proyecto (.newera)", loading: "Cargando el motor…", plan: "Plano", drag: "Arrastre para girar",
        ready: "Listo. Abra un proyecto .newera.", failed: (n, e) => `No se pudo abrir ${n}: ${e}`,
        about: (i) => `${i.name}: ${i.walls} paredes, ${i.rooms} habitaciones, ${i.furniture} muebles` },
  fr: { open: "Ouvrir un projet (.newera)", loading: "Chargement du moteur…", plan: "Plan", drag: "Glissez pour pivoter",
        ready: "Prêt. Ouvrez un projet .newera.", failed: (n, e) => `Impossible d'ouvrir ${n} : ${e}`,
        about: (i) => `${i.name} : ${i.walls} murs, ${i.rooms} pièces, ${i.furniture} meubles` },
  uk: { open: "Відкрити проєкт (.newera)", loading: "Завантаження рушія…", plan: "План", drag: "Перетягніть, щоб обертати",
        ready: "Готово. Відкрийте проєкт .newera.", failed: (n, e) => `Не вдалося відкрити ${n}: ${e}`,
        about: (i) => `${i.name}: ${uk(i.walls, ["стіна", "стіни", "стін"])}, ${uk(i.rooms, ["кімната", "кімнати", "кімнат"])}, ${uk(i.furniture, ["предмет меблів", "предмети меблів", "предметів меблів"])}` },
  pt: { ready: "Pronto. Abra um projeto .newera.", failed: (n, e) => `Não foi possível abrir ${n}: ${e}`,
        about: (i) => `${i.name}: ${i.walls} paredes, ${i.rooms} cômodos, ${i.furniture} móveis` },
};
const tag = (navigator.languages ?? [navigator.language ?? ""])
  .map((name) => String(name).slice(0, 2).toLowerCase())
  .find((name) => name in WORDS) ?? "en";
const words = WORDS[tag];
if (tag !== "pt") {
  document.documentElement.lang = tag;
  for (const element of document.querySelectorAll("[data-t]")) {
    const word = words[element.dataset.t];
    if (word) element.textContent = word;
  }
}

// The stamp this file was loaded with, handed on to the wasm beside it: one
// build, one pair, whatever any cache in between is holding.
const build = new URL(import.meta.url).searchParams.get("v");
const wasmUrl = build ? `newera_web.wasm?v=${build}` : "newera_web.wasm";
const { instance } = await WebAssembly.instantiateStreaming(fetch(wasmUrl), {});
const wasm = instance.exports;
status.textContent = words.ready;

const output = () => new Uint8Array(wasm.memory.buffer, wasm.output_ptr(), wasm.output_len()).slice();
const text = () => new TextDecoder().decode(output());

let yaw = -60, pitch = 45, rendering = false, pending = false;

function renderView() {
  if (rendering) { pending = true; return; }
  rendering = true;
  requestAnimationFrame(() => {
    const rect = viewBox.getBoundingClientRect();
    const scale = Math.min(1, 480 / Math.max(rect.width, 1));
    const w = Math.max(64, Math.round(rect.width * scale));
    const h = Math.max(48, Math.round(rect.height * scale));
    if (wasm.render_view_png(w, h, yaw, pitch) > 0) {
      const url = URL.createObjectURL(new Blob([output()], { type: "image/png" }));
      image.onload = () => URL.revokeObjectURL(url);
      image.src = url;
    }
    rendering = false;
    if (pending) { pending = false; renderView(); }
  });
}

async function open(bytes, name) {
  const ptr = wasm.alloc(bytes.length);
  new Uint8Array(wasm.memory.buffer, ptr, bytes.length).set(bytes);
  const result = wasm.load_project(ptr, bytes.length);
  wasm.dealloc(ptr, bytes.length);
  if (result !== 0) {
    status.textContent = words.failed(name, text());
    return;
  }
  const info = JSON.parse(text());
  status.textContent = words.about(info);
  if (wasm.render_plan_svg() > 0) {
    planBox.querySelector("svg")?.remove();
    planBox.insertAdjacentHTML("beforeend", text());
  }
  renderView();
}

document.getElementById("file").addEventListener("change", async (event) => {
  const file = event.target.files[0];
  if (file) open(new Uint8Array(await file.arrayBuffer()), file.name);
});

// A project can also come from the address: viewer?project=casa.newera
const fromUrl = new URLSearchParams(location.search).get("project");
if (fromUrl) {
  const response = await fetch(fromUrl);
  if (response.ok) open(new Uint8Array(await response.arrayBuffer()), fromUrl);
}

let drag = null;
viewBox.addEventListener("pointerdown", (e) => { drag = { x: e.clientX, y: e.clientY }; viewBox.setPointerCapture(e.pointerId); });
viewBox.addEventListener("pointerup", () => { drag = null; });
viewBox.addEventListener("pointermove", (e) => {
  if (!drag) return;
  yaw += (e.clientX - drag.x) * 0.5;
  pitch = Math.min(85, Math.max(5, pitch + (e.clientY - drag.y) * 0.3));
  drag = { x: e.clientX, y: e.clientY };
  renderView();
});
window.addEventListener("resize", renderView);
