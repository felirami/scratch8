# SCRATCH-8 Hardware Reference

**This document is the canonical "machine manual".** Everything here is sacred for the classic mode. The core library (`src/lib.rs`) is the implementation of this spec.

All values and behaviors must be reproducible on desktop, WebAssembly, and bare-metal embedded targets.

## Physical Display

- **Resolution**: 128 pixels wide × 128 pixels high (`WIDTH = 128`, `HEIGHT = 128`)
- **Framebuffer**: 16,384 bytes (`VRAM_SIZE = WIDTH * HEIGHT`). Each byte is a palette index (0–15).
- **Color depth**: 4 bits per pixel (palette lookup). No direct RGB in the machine.
- **Aspect / presentation**: Square pixels in the fantasy. Hosts may scale (current desktop uses 5× integer scaling via minifb).
- **No hardware scrolling, sprites, or layers yet**. All effects are software (your cart code + the provided primitives).

The host reads the `buffer` (via `to_rgb_buffer` or direct indexed access) and presents it. The core never "draws to screen" directly.

## Palette (Locked — 16 colors)

Exact PICO-8 inspired palette. Index 0 is treated as "transparent" for future sprite and map operations.

```rust
pub const PALETTE: [u32; 16] = [
    0x000000, //  0 black
    0x1D2B53, //  1 dark blue
    0x7E2553, //  2 dark purple
    0x008751, //  3 dark green
    0xAB5236, //  4 brown
    0x5F574F, //  5 dark gray
    0xC2C3C7, //  6 light gray
    0xFFF1E8, //  7 white
    0xFF004D, //  8 red
    0xFFA300, //  9 orange
    0xFFEC27, // 10 yellow
    0x00E436, // 11 green
    0x29ADFF, // 12 blue
    0x83769C, // 13 lavender
    0xFF77A8, // 14 pink
    0xFFCCAA, // 15 peach
];
```

RGB values are 0x00RRGGBB (top byte zero). Use `Console::to_rgb_buffer` to expand the index buffer for hosts that want 32-bit pixels.

## Timing & Frame Model

- **Target rate**: 60 frames per second (locked in the fantasy machine).
- **Host responsibility**: Call the cart `update()` + `console.tick()` approximately 60 times per second and present the result.
- **Frame counter**: `pub frame: u32` — strictly monotonic, wraps at `u32::MAX`. Excellent for deterministic animations (`if c.frame % 30 == 0 { ... }`).
- **No built-in real-time clock**. Carts should be deterministic given the same input sequence.

In the desktop runner `minifb` is configured with `set_target_fps(60)`.

## Input Model (Classic 6-button + Mouse)

Buttons (array indices in `console.btn` and argument to `btnp`):

- 0: LEFT
- 1: RIGHT
- 2: UP
- 3: DOWN
- 4: Z / O (primary action / "fire")
- 5: X / K (secondary action / "jump" / "menu")

- `btn[b]` — true while the button is held this frame.
- `btnp(b)` — true only on the frame the button transitioned from released → pressed (edge detect, perfect for menus and single-shot actions).
- Mouse: `mouse_x`, `mouse_y` (i32, clamped 0..127 by the core in `update_input`).
- Hosts push state every frame via `console.update_input(left, right, up, down, z, x, mx, my)`.

Future: gamepad abstraction will map to the same 6 + mouse model.

## Drawing Primitives (All Pure Software, From Scratch)

Implemented pixel-by-pixel or with classic algorithms inside the core. No external 2D libraries.

- `cls(col)` — fill entire screen to palette color (masked to 0-15).
- `pset(x, y, col)` / `pget(x, y) -> u8` — single pixel (clipped; pget returns 0 outside).
- `rect(x, y, w, h, col)` — filled rectangle.
- `rectb(x, y, w, h, col)` — rectangle outline (1 px).
- `circ(cx, cy, r, col)` — filled circle (distance-squared test).
- `circb(cx, cy, r, col)` — circle outline (8-way symmetry + decision variable, Bresenham-like).
- `line(x0, y0, x1, y1, col)` — Bresenham line.
- `print(text, x, y, col)` — 4×6 chunky monospaced font (hand-authored bit patterns for 0-9 A-Z a-z + punctuation). Advances 5 px per char.

All coordinates are `i32`. Out-of-bounds pixels are silently clipped. Colors are always masked `& 0x0F`.

See `src/lib.rs` for the exact implementations (Bresenham, circle, font table, etc.). These are part of the machine spec.

## Limits (Current — Classic Mode)

- VRAM: exactly 16 KB palette indices.
- Screen: exactly 128×128.
- Palette: exactly 16 colors (no "higher color" mode in classic).
- Buttons: exactly 6 + mouse.
- Cart state: whatever your Rust struct holds (on capable hosts). On tiny embedded you are encouraged to use fixed-size arrays only.
- No dynamic allocation required in the drawing or input hot paths.
- Max reasonable cart complexity: small games that fit in a human's head (the whole point).

**Future limits (when implemented)** will be documented here:
- Sprite RAM size
- Map size (probably 32×32 or 128×32 tiles)
- Number of audio channels (target 4–6)
- Max cartridge size for .s8

## Memory Map (Emulated / Logical)

There is no real address bus or MMIO in the current core — access is only through the typed `Console` API. This map describes the **logical layout** we are aiming for and what already exists.

Current (in `Console` struct):

- `0x0000 – 0x3FFF` (16384 bytes): Framebuffer — palette indices, row-major (y * 128 + x)
- Frame counter, previous button state, mouse (API only, not byte-addressable today)
- Cart-private Rust state (your `struct` fields) — lives in host RAM, not inside the 16 KB "machine" RAM

Planned / future (comments already exist in the source):

- Sprite RAM (e.g. starting ~0x4000): 8×8 or 16×16 stamps, palette indices or packed. Methods: `spr(...)`, editor support.
- Tile map RAM: indices into sprite sheet. Methods: `map(...)`.
- Audio registers: per-channel frequency, volume, waveform type, envelope, vibrato. (Pure synth produces PCM; registers are the "PSG" interface.)
- User / cart save RAM (persistence section of .s8).
- Possibly a small "zero page" for fast cart variables in future VM/bytecode carts.

Carts never poke bytes directly today. All access is high-level method calls. This keeps the machine model clean and portable.

Direct peek/poke (if added later) will be behind an explicit `mem_peek` / `mem_poke` for advanced use and will still respect the map.

## Audio (Planned — Not Yet in Public API)

- Pure software PSG (no external sound libs in the core).
- Channels: pulse/square (variable duty), triangle, noise.
- Features: volume envelopes, simple vibrato / pitch slides, tracker-style patterns.
- Output: host receives `i16` mono PCM buffers via `Host::push_audio`.
- Sample rate chosen by host (22050 / 44100 / ...); synth will match or resample internally.
- See `src/audio/` (module skeleton) and the "audio" Cargo feature.

Until the audio track delivers, hosts may implement `push_audio` as a no-op.

## Portability & "Any Chip" Constraints

- `#![no_std]` (with `extern crate alloc` for `Box<dyn Cart>` convenience). Hot paths aim for no-alloc.
- All types are fixed-size or explicitly bounded.
- No `std::` items in the machine logic.
- `debug_assert!` (not `assert!`) used in hot conversion paths so embedded targets with minimal panic handlers do not blow up on host misuse.
- `NoopHost` is provided for tests, simulators, and bring-up.
- The same `Cart` you write against `Console` must be runnable on a Cortex-M with a 128×128 SPI OLED + timer interrupt + DAC (or even bit-banged audio) once a host adapter exists.

Cross-compilation is verified by `scripts/verify-portability.sh`.

## Boot & Reset Behavior

- On Console creation / cart `init()`: screen is cleared to color 0 (the core does `c.cls(0)` in `new()`).
- The desktop binary shows a short boot splash using the portable Console before entering the cart loop.

## What Is *Not* Hardware (Intentionally)

- No file system, networking, or OS services in the machine.
- No floating-point trig tables or complex math in the core (use simple approximations or precomputed small tables in your cart if needed).
- Editors, cartridge persistence, and the "in-console dev environment" are **software** running on top of this hardware (Phase 3).

## Versioning

This spec corresponds to the state of `src/lib.rs` at the time of writing. Any deviation between this document and the code is a bug — file an issue or send a patch.

When we add sprites, maps, or audio registers we will extend (never contradict) this document.

---

**Related**:
- `src/lib.rs` — the implementation + rustdoc
- `docs/TUTORIAL.md` — how to program this machine
- `docs/CARTRIDGE_FORMAT.md` — how carts (the software) will be packaged and loaded
- `GOALS.md` — why these exact constraints exist

Enjoy the 128×128 box. It is more than enough.
