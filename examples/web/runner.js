// SCRATCH-8 Thin Web Runner (JS side of wasm-port track)
// Uses Canvas 2D for video. Pure no-frills WebAssembly + manual memory + palette mapping.
// Web Audio stub is present for the chiptune future (when s8_ffi_fill_audio starts returning samples).
//
// This file + index.html are intentionally minimal so they can be understood in <5 minutes.
// No bundlers, no npm, no wasm-bindgen — matches the "from the very scratch" + portability philosophy.
//
// COORDINATION:
// - Mirrors the FFI added in src/lib.rs (s8_ffi_init / update / copy_buffer / palette / width etc).
// - The same .s8 carts exported from desktop (cartridge-system) will be loadable here later
//   (first by builtin name, later by extracting a real WASM payload from the code section).
// - For "WASM carts" future (compile once run anywhere): a guest .wasm would import host
//   symbols we can provide from here (or the runtime), export its own update fn, and we
//   would call it instead of the builtin FFI dispatch. See design notes in lib.rs and carts/mod.rs.
//
// Build the wasm as described in index.html (see pre block), then serve this folder (or configure
// your static host / Vercel rootDirectory: examples/web + ensure .wasm served as application/wasm).

const WIDTH = 128;
const HEIGHT = 128;
const SCALE = 4; // CSS pixels per console pixel (for visibility)

const PALETTE = [ // must match src/lib.rs PALETTE (0x00RRGGBB -> r,g,b). Synced via s8_ffi_palette_ptr() when possible.
  [0,0,0], [0x1D,0x2B,0x53], [0x7E,0x25,0x53], [0x00,0x87,0x51],
  [0xAB,0x52,0x36], [0x5F,0x57,0x4F], [0xC2,0xC3,0xC7], [0xFF,0xF1,0xE8],
  [0xFF,0x00,0x4D], [0xFF,0xA3,0x00], [0xFF,0xEC,0x27], [0x00,0xE4,0x36],
  [0x29,0xAD,0xFF], [0x83,0x76,0x9C], [0xFF,0x77,0xA8], [0xFF,0xCC,0xAA]
];

let wasmInstance = null;
let wasmMemory = null;
let ctx = null;
let canvas = null;
let imageData = null;
let inputState = { left:0, right:0, up:0, down:0, z:0, x:0, mx:0, my:0 };
let currentDemo = 0;
let frameCounter = 0;

function setStatus(msg) {
  const el = document.getElementById('status');
  if (el) el.textContent = msg;
}

function updateInputFromKeys(e, isDown) {
  const k = e.key.toLowerCase();
  if (k === 'arrowleft' || k === 'a') inputState.left = isDown ? 1 : 0;
  if (k === 'arrowright' || k === 'd') inputState.right = isDown ? 1 : 0;
  if (k === 'arrowup' || k === 'w') inputState.up = isDown ? 1 : 0;
  if (k === 'arrowdown' || k === 's') inputState.down = isDown ? 1 : 0;
  if (k === 'z' || k === 'o') inputState.z = isDown ? 1 : 0;
  if (k === 'x' || k === 'k') inputState.x = isDown ? 1 : 0;
  // Keyboard demo switch (matches desktop 1/2/3)
  if (isDown) {
    if (k === '1') switchDemo(0);
    if (k === '2') switchDemo(1);
    if (k === '3') switchDemo(2);
  }
}

function setupInput(canvasEl) {
  // Keyboard
  window.addEventListener('keydown', e => updateInputFromKeys(e, true));
  window.addEventListener('keyup', e => updateInputFromKeys(e, false));

  // Mouse on canvas (for painter demo etc)
  canvasEl.addEventListener('mousemove', (e) => {
    const rect = canvasEl.getBoundingClientRect();
    // Map from displayed pixels back to 0..127 console space
    inputState.mx = Math.floor(((e.clientX - rect.left) / rect.width) * WIDTH);
    inputState.my = Math.floor(((e.clientY - rect.top) / rect.height) * HEIGHT);
  });
  canvasEl.addEventListener('mousedown', (e) => {
    if (e.button === 0) inputState.z = 1;
    if (e.button === 2) inputState.x = 1;
  });
  canvasEl.addEventListener('mouseup', (e) => {
    if (e.button === 0) inputState.z = 0;
    if (e.button === 2) inputState.x = 0;
  });
  canvasEl.addEventListener('contextmenu', e => e.preventDefault()); // no right-click menu

  // Touch (very basic mobile support)
  canvasEl.addEventListener('touchmove', (e) => {
    const rect = canvasEl.getBoundingClientRect();
    const t = e.touches[0];
    inputState.mx = Math.floor(((t.clientX - rect.left) / rect.width) * WIDTH);
    inputState.my = Math.floor(((t.clientY - rect.top) / rect.height) * HEIGHT);
  });
  canvasEl.addEventListener('touchstart', (e) => { inputState.z = 1; e.preventDefault(); });
  canvasEl.addEventListener('touchend', () => { inputState.z = 0; });
}

async function loadWasm() {
  setStatus('Fetching scratch8.wasm ...');
  try {
    const response = await fetch('scratch8.wasm', { cache: 'no-cache' });
    if (!response.ok) {
      const ct = response.headers.get('content-type') || '';
      throw new Error('Failed to fetch scratch8.wasm (status ' + response.status + ', type ' + ct + '). ' +
        'Check deployment rootDirectory (examples/web), file committed, and public access (no auth wall).');
    }
    // Optional: warn on bad MIME (Vercel/static hosts should return application/wasm for *.wasm)
    const ct = response.headers.get('content-type') || '';
    if (ct && !ct.includes('wasm') && !ct.includes('octet-stream') && !ct.includes('application')) {
      console.warn('Unexpected Content-Type for wasm:', ct, '(may still work but some browsers enforce application/wasm)');
    }
    const bytes = await response.arrayBuffer();

    setStatus('Instantiating wasm...');
    const result = await WebAssembly.instantiate(bytes, {
      // Future: import object for WASM carts would go here.
      // e.g. env: { s8_host_pset: (x,y,c) => { ... mutate a console ... } }
    });
    wasmInstance = result.instance;
    wasmMemory = wasmInstance.exports.memory;

    // Sanity: the FFI symbols we expect from lib.rs
    const required = ['s8_ffi_init', 's8_ffi_update', 's8_ffi_width', 's8_ffi_height', 's8_ffi_copy_buffer', 's8_ffi_palette_ptr', 's8_ffi_frame'];
    for (const name of required) {
      if (typeof wasmInstance.exports[name] !== 'function') {
        throw new Error('Missing export: ' + name + ' (wasm may be stale or built without the FFI section)');
      }
    }

    // Pre-grow memory generously so the copy_buffer destOffset hack is reliable (avoid per-frame growth races).
    // The FFI copy_buffer writes to a caller-chosen wasm addr; we pick near the high end after growth.
    try { wasmMemory.grow(16); } catch (_) {} // ~1 MiB extra pages for scratch + future audio + safety

    // (Optional) sync PALETTE from FFI to avoid drift (palette_ptr returns *const u32 to the 16-entry table).
    // For v1 we keep the JS const in sync manually; future: read via DataView + update PALETTE.
    // const palPtr = wasmInstance.exports.s8_ffi_palette_ptr();

    // Initial console + first demo
    wasmInstance.exports.s8_ffi_init(currentDemo);

    setStatus('Running (wasm OK). Use keyboard or buttons.');
    return true;
  } catch (err) {
    console.error('SCRATCH-8 wasm load error:', err);
    setStatus('ERROR: ' + (err && err.message ? err.message : err) + ' — see browser console. Ensure scratch8.wasm is present + publicly fetchable.');
    return false;
  }
}

function switchDemo(id) {
  currentDemo = id % 3;
  if (wasmInstance && wasmInstance.exports.s8_ffi_init) {
    wasmInstance.exports.s8_ffi_init(currentDemo);
    setStatus('Switched to demo ' + currentDemo);
  }
}

// Make available to inline scripts in index.html (btn3 handler etc)
window.switchDemo = switchDemo;

function renderFrame() {
  if (!wasmInstance || !ctx || !imageData) return;

  try {
    const w = wasmInstance.exports.s8_ffi_width();
    const h = wasmInstance.exports.s8_ffi_height();

    // Feed current input state + step the machine (this runs the real Cart logic inside wasm)
    wasmInstance.exports.s8_ffi_update(
      inputState.left, inputState.right, inputState.up, inputState.down,
      inputState.z, inputState.x, inputState.mx, inputState.my
    );

    // Pull the palette-index buffer out of wasm linear memory via the copy FFI.
    // Robust version of the dest hack:
    // - pre-grow at load (see loadWasm)
    // - always re-read .buffer (grows detach previous)
    // - use conservative high offset with margin
    const pageSize = 65536;
    const needed = (w * h) + 4096;
    let bufLen = wasmMemory.buffer.byteLength;
    if (bufLen < needed) {
      const pagesNeeded = Math.ceil((needed - bufLen) / pageSize) + 3;
      try { wasmMemory.grow(pagesNeeded); bufLen = wasmMemory.buffer.byteLength; } catch (_) {}
    }

    // Destination near (but safely before) the absolute end of linear memory.
    let destOffset = bufLen - (w * h) - 256;
    if (destOffset < 0) destOffset = 0; // fallback (unlikely)

    // NOTE: s8_ffi_copy_buffer(out_ptr, len) — out_ptr is *wasm address* (i32). Must be writable in linear mem.
    wasmInstance.exports.s8_ffi_copy_buffer(destOffset, w * h);

    // Map indices through palette into the ImageData. Re-create view each frame (post-grow safety).
    const idxView = new Uint8Array(wasmMemory.buffer, destOffset, w * h);
    const data = imageData.data; // RGBA
    for (let i = 0; i < w * h; i++) {
      const col = idxView[i] & 15;
      const rgb = PALETTE[col];
      const o = i * 4;
      data[o + 0] = rgb[0];
      data[o + 1] = rgb[1];
      data[o + 2] = rgb[2];
      data[o + 3] = 255;
    }
    ctx.putImageData(imageData, 0, 0);

    // (Optional) read frame for debug
    // const f = wasmInstance.exports.s8_ffi_frame();
    frameCounter++;
  } catch (e) {
    // Do not kill the rAF loop on transient errors (e.g. during a grow edge case).
    console.warn('renderFrame error (continuing):', e);
  }
}

function mainLoop() {
  renderFrame();
  requestAnimationFrame(mainLoop);
}

async function start() {
  canvas = document.getElementById('canvas');
  if (!canvas) {
    console.error('No canvas element'); return;
  }
  ctx = canvas.getContext('2d', { alpha: false });

  // Scale the internal 128x128 up for the display
  canvas.style.width = (WIDTH * SCALE) + 'px';
  canvas.style.height = (HEIGHT * SCALE) + 'px';
  canvas.width = WIDTH;
  canvas.height = HEIGHT;

  imageData = ctx.createImageData(WIDTH, HEIGHT);

  setupInput(canvas);

  // Wire demo buttons (guard missing elements so we never throw before loadWasm)
  const b0 = document.getElementById('btn0'); if (b0) b0.onclick = () => switchDemo(0);
  const b1 = document.getElementById('btn1'); if (b1) b1.onclick = () => switchDemo(1);
  const b2 = document.getElementById('btn2'); if (b2) b2.onclick = () => switchDemo(2);
  const bAudio = document.getElementById('btn-audio');
  if (bAudio) {
    bAudio.onclick = () => {
      setStatus('Web Audio stub — see s8_ffi_fill_audio in lib.rs (chiptune track + wasm-port)');
      // Future: create AudioContext + AudioWorklet that pulls from s8_ffi_fill_audio into a sourceBuffer.
    };
  }

  const ok = await loadWasm();
  if (ok) {
    // Kick the main loop (video only for the first web step)
    mainLoop();
  }
}

// Auto-start
if (document.readyState === 'loading') {
  document.addEventListener('DOMContentLoaded', start);
} else {
  start();
}

// Bonus: allow dropping a future .s8 here (will be parsed in JS or sent to wasm loader later)
document.addEventListener('dragover', e => e.preventDefault());
document.addEventListener('drop', e => {
  e.preventDefault();
  setStatus('Cart drop received (future .s8 / WASM cart loader will handle this)');
  // TODO (cartridge-system + wasm-port): read the file, parse header with JS, extract code section,
  // if it looks like wasm instantiate it with our importObject and drive its exports instead of builtins.
});
