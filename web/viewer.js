// Viewer glue: loads newera_web.wasm and talks to its tiny C ABI.
const status = document.getElementById("status");
const planBox = document.getElementById("plan");
const viewBox = document.getElementById("view");
const image = document.getElementById("image");

const { instance } = await WebAssembly.instantiateStreaming(fetch("newera_web.wasm"), {});
const wasm = instance.exports;
status.textContent = "Pronto. Abra um projeto .newera.";

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
    status.textContent = `Não foi possível abrir ${name}: ${text()}`;
    return;
  }
  const info = JSON.parse(text());
  status.textContent = `${info.name}: ${info.walls} paredes, ${info.rooms} cômodos, ${info.furniture} móveis`;
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
