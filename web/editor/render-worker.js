// One isolated WASM instance per render. Terminating the worker releases its
// scene, textures and buffers, and cancellation never waits for a frame.
self.onmessage = async ({ data }) => {
  try {
    const wasm = await import(data.module);
    const binary = new URL(data.module);
    binary.pathname = binary.pathname.replace(/\.js$/, "_bg.wasm");
    await wasm.default({ module_or_path: binary.href });
    const result = wasm.render_worker(data.request, data.assets);
    self.postMessage({ type: "done", bytes: result }, [result.buffer]);
  } catch (error) {
    self.postMessage({ type: "error", error: String(error) });
  }
};
