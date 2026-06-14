# SCRATCH-8

**A complete fantasy console built from the very scratch** — with the explicit goal of being extremely portable ("compiles and runs on any chip").

See [GOALS.md](GOALS.md) for the full ambitious vision, success criteria, "any chip" portability requirements, non-negotiables, and how the **agent swarm** is building it in parallel tracks.

Current status: The portable `#![no_std]` core (graphics primitives, Console, Cart trait, 3 demos) is in `src/lib.rs` and already cross-checks for desktop + wasm + embedded targets. The desktop runner is now a thin layer on top. Chiptune audio, editors, real cartridge format (.s8), web runtime, and many game examples are being built right now by the swarm.

**New in this pass (Documentation & Examples track)**:
- `docs/TUTORIAL.md` — write your first interactive cart in ~15 minutes
- `docs/HARDWARE.md`, `docs/CARTRIDGE_FORMAT.md` (in progress)
- `examples/pong.rs` and `examples/snake.rs` — two polished, self-contained, fully documented game carts using only the public API (plus more planned)
- `TODO.md` — master task tracker for the swarm

All new material prioritizes clarity, the "from the very scratch" philosophy, and concrete copy-paste-ready code.

**Everything that matters is implemented by us**:
- 128×128 pixel display, locked to a 16-color palette (PICO-8 inspired).
- A pure-Rust software framebuffer. All drawing primitives (`pset`, `rect`, `circ`, `line` (Bresenham), `print` with a hand-authored 4×6 font, etc.) are written pixel-by-pixel with no external 2D drawing libraries.
- 60 FPS locked loop.
- Classic 6-button + mouse input.
- Cartridge system via a tiny `Cart` trait. Making a new "game" means writing a small Rust struct that calls the `Console` API.

This is the real core of a fantasy console — the *machine* and the *constraints* — not a wrapper around a game engine.

## Current status (v0.1 foundation)

- ✅ 128×128 × 16 colors, enforced by design (16 KB "VRAM")
- ✅ All drawing primitives from scratch (Bresenham line, midpoint-ish circle, filled shapes, custom font)
- ✅ 3 playable demo carts (built into the core for immediate use):
  - **BOUNCER** — classic bouncing ball that changes color + uses lines
  - **PAINTER** — mouse painting with trails + pget demo
  - **PARTICLES** — gravity fountain you can influence with Z/X
- ✅ Cart switching with 1/2/3 keys (exactly the fantasy console feel)
- ✅ Frame counter, btn/btnp style input
- ✅ Boot screen
- ✅ High-quality standalone examples in `examples/`: Pong and Snake (see below)
- ✅ World-class starting documentation: TUTORIAL.md + HARDWARE.md seed + CARTRIDGE_FORMAT.md seed + this README + rustdoc
- ✅ **Chiptune Audio (audio-chiptune track complete)**: Full from-scratch PSG in `src/audio.rs` (feature "audio").
  - 4 channels: pulse (variable duty 12.5/25/50/75%), triangle, LFSR noise.
  - Envelopes (simple decay+release), vibrato (triangle LFO), frequency slide/portamento.
  - PICO-8 style API: `tone(freq, dur, vol, wave)`, `sfx(id, vol)`, `play_music(pattern, bpm)` + low-level.
  - Pattern player: multi-voice tracker-style sequencer (fixed arrays, sample-accurate).
  - Mixer: `fill_buffer_i16` / `fill_buffer_f32` — pure math, any sample rate, no_std friendly.
  - Desktop: cpal real-time output wired; press **4** for the **CHIPTUNE** demo cart (plays a bouncy pattern + live sfx on Z/X, visualizer).
  - All synthesis (phase accum, osc math, 16-bit LFSR, state machines) written by hand — zero synth crates.

**Not yet implemented (but planned)**:
- Sprites + tilemaps + `spr()` / `map()` API + sprite editor
- Sprites + tilemaps + `spr()` / `map()` API + sprite editor
- In-console code + sprite + map + music editors
- Real cartridge file format (`.s8` or PNG-encoded like PICO-8)
- Custom scripting language or first-class WASM cart support (so non-Rust people can write carts easily)
- Export to web / standalone player

See `TODO.md` for the detailed swarm task list (game-examples, documentation, etc.).

## Running it

```bash
cd ~/projects/scratch8
cargo run --release
```

- **Escape** to quit
- **1 / 2 / 3** to switch demo carts
- **Arrows + Z/X** (or O/K) for input inside carts
- Mouse works in PAINTER

See `examples/` for two additional high-quality standalone games (Pong, Snake) you can study or integrate. The `docs/TUTORIAL.md` gives a complete guided path.

## How to make your own cart (the "from scratch" way)

**Best starting point**: Read [`docs/TUTORIAL.md`](docs/TUTORIAL.md) ("Your first cart in 15 minutes"). It walks you from a blank struct all the way to a complete playable catcher game with physics, scoring, multiple primitives, and input.

### Quick version

1. Study a complete working example: `examples/pong.rs` or `examples/snake.rs`. They are **self-contained**, heavily commented, and demonstrate real gameplay using only the public `Console` + `Cart` API.

2. Copy the struct + `impl Cart` pattern (or the whole file) into your project.

3. For now (until the `.s8` loader + editors land), register it in the desktop runner:

Open `src/main.rs` and add your cart (copy one of the existing ones as a template):

```rust
struct MyGame {
    // your state here
}

impl Cart for MyGame {
    fn name(&self) -> &'static str { "MY GAME" }

    fn init(&mut self, c: &mut Console) {
        c.cls(0);
    }

    fn update(&mut self, c: &mut Console) {
        c.cls(1);
        // draw whatever you want using the pure primitives
        c.print("HELLO FROM SCRATCH", 10, 50, 7);

        if c.btn[4] {
            c.circ(64, 64, 20, 8);
        }
    }
}
```

Add it to the list in `main()`:

```rust
let mut carts: Vec<Box<dyn Cart>> = vec![
    Box::new(Bouncer::new()),
    Box::new(MyGame::new()),
    ...
];
```

That's it. `cargo run --release` — your cart is now a first-class citizen switchable with 1/2/3.

### Standalone verification of your cart
The carts in `examples/` include a small `main()` that runs a headless simulation loop. This lets you do:

```bash
cargo check --examples
cargo run --example pong
cargo run --example snake
```

These always succeed using only the library (no windowing required) and prove your code is a valid, portable consumer of the `Console` + `Cart` public surface.

All the "hardware" (the `Console` methods) is deliberately tiny and constrained so that the experience feels like a real 8-bit fantasy machine. Full API reference is in the rustdoc (`cargo doc --open`) and the source comments in `src/lib.rs`.

## Examples & Gallery (start here for inspiration)

- **Built-in** (always available, switch with 1/2/3):
  - BOUNCER, PAINTER, PARTICLES (see `src/lib.rs`)
- **Standalone polished games** (in `examples/` — copy the `struct + impl Cart`):
  - `pong.rs` — classic Pong with AI opponent, serve, scoring to 7, english on bounces, full use of rect/circ/line/print
  - `snake.rs` — full Snake on a 32×32 grid using fixed arrays (portable), timed steps, btnp turns, growth, death/restart
- Guided path: `docs/TUTORIAL.md` builds a complete "Catcher" paddle game from zero.

More examples (Breakout stub, platformer, shooter, paint toy, music visualizer) are coming as part of the 6–8 target in GOALS.md. Each will have a comment header and exercise multiple machine features.

## Next phases (we can keep building this together)

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────┐
│                        Host Platform                        │
│  (minifb+cpal desktop | Canvas+WebAudio | bare-metal LCD)   │
│                                                             │
│  implements Host trait  +  calls console.update_input()     │
└───────────────────────────────┬─────────────────────────────┘
                                │
                                ▼
┌─────────────────────────────────────────────────────────────┐
│                  scratch8 (core lib)  #![no_std]            │
│                                                             │
│  Console { buffer:[u8;16384], frame, btn, mouse, ... }      │
│    ├── pset / pget / rect / rectb / circ / circb / line     │
│    ├── print (hand-rolled 4x6 font)                         │
│    ├── cls, tick, to_rgb_buffer, update_input, btnp         │
│    └── (future: tone/sfx, spr, map under features)          │
│                                                             │
│  trait Cart { name, init, update(&mut self, &mut Console) } │
│  trait Host { present, push_audio }                         │
│                                                             │
│  Built-in demos (Bouncer, Painter, Particles) +             │
│  your carts (Pong, Snake, ...)                              │
└─────────────────────────────────────────────────────────────┘
```

- **Core is sacred and portable**. Everything that makes the *machine* is in `src/lib.rs` (and soon `src/audio/` etc.).
- **Carts are the software**. Implement `Cart` (Rust today; WASM/bytecode tomorrow).
- **Host is a thin adapter**. See `src/main.rs` for the desktop example. Embedded and web will have their own ~50-200 line hosts.
- Full hardware details: `docs/HARDWARE.md`
- First cart walkthrough: `docs/TUTORIAL.md`
- Cartridge file format (future): `docs/CARTRIDGE_FORMAT.md`

The public API surface you program against as a game author is tiny by design (`Console` methods + `Cart` trait).

## How to Run on Different Chips ("Any Chip" Portability)

The entire machine logic is `#![no_std]` + `alloc` and contains **zero** platform-specific code. Platform details live only in hosts.

### Desktop (current)
```bash
cargo run --release --features full
# or
cargo run --release
```
See "Running it" above. Uses minifb (window + input) + cpal (audio when ready).

### Verify the whole matrix (recommended after any core change)
```bash
./scripts/verify-portability.sh
```
It runs:
- `cargo check --lib --no-default-features` (pure core)
- Desktop full
- `cargo check --lib --target wasm32-unknown-unknown --features wasm`
- Embedded: `thumbv7em-none-eabihf` (Cortex-M) and `riscv32imac-unknown-none-elf`

### Web / WASM (future thin host)
```bash
rustup target add wasm32-unknown-unknown
cargo check --lib --target wasm32-unknown-unknown --features wasm
```
A real web runtime (Canvas + requestAnimationFrame + Web Audio) is being built by the WASM specialist. The exact same `Cart` you write here will run in the browser.

### Embedded / Bare Metal ("any chip")
```bash
# Cortex-M4F etc.
rustup target add thumbv7em-none-eabihf
cargo check --lib --target thumbv7em-none-eabihf --no-default-features --features embedded

# RISC-V
rustup target add riscv32imac-unknown-none-elf
...
```

For a real device you will:
1. Implement `Host` (or call the present/push_audio hooks).
2. Feed input via `console.update_input(...)` every frame from your buttons/ADC.
3. Drive the 60 Hz loop yourself (timer interrupt or busy loop).
4. Use `NoopHost` (exported) for headless tests or early bring-up.
5. For the absolute smallest chips without alloc you can avoid `Box<dyn Cart>` and use a fixed enum or concrete type.

See `src/lib.rs` comments around `Host`, `NoopHost`, `builtin_carts`, and the portability notes. The verify script + target checks are the proof.

All carts (including the ones in `examples/`) are written against the public API and therefore are automatically portable once a host exists for the target.

## Tech choices (why this is "from scratch")

- **No game engine**. No Bevy, ggez, macroquad, etc.
- **No graphics crate** for drawing. We literally write to a `[u8; 16384]` palette buffer.
- **minifb** is only used for "give me a window and some input events + present this buffer". It is one of the thinnest possible layers. We can later replace it with winit + softbuffer if we want to go even deeper.
- The font, the circle algorithm, the line algorithm, the input mapping, the cart abstraction — all authored here.

## Next phases (we can keep building this together)

1. ✅ Audio (full 4-ch PSG + mixer + tone/sfx/pattern player + cpal desktop output — done by Chiptune Audio Wizard)
2. Sprite RAM + `spr(x, y, sx, sy, w, h, flip)` + a built-in sprite editor (press TAB or E)
3. Map / tile layer
4. Cartridge persistence (save/load .s8 files containing code + gfx + sfx)
5. Either:
   - A tiny custom bytecode VM + text language for carts (maximum "scratch" purity), **or**
   - WASM cart support (like WASM-4 — users write in Rust/Zig/C and compile to .wasm)
6. Polish, better font, more demo carts, web target (wasm + canvas)

If you want any of these next, or changes to the current spec (different resolution? different palette? different input buttons? pure terminal version first?), just say the word.

Start with the docs:
- `docs/TUTORIAL.md` — your first cart
- `docs/HARDWARE.md` — the exact machine spec
- `GOALS.md` — the full vision
- `TODO.md` — what the swarm is working on right now

This already proves the answer to your question:

**Yes. We can (and just did) build a real fantasy console from the very scratch.**

Enjoy playing with it — and let's keep extending the machine.
