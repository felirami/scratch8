# SCRATCH-8 Project Goal

## The Vision

**SCRATCH-8** is a complete fantasy console built **entirely from scratch** in the spirit (and constraints) of PICO-8, TIC-80, and WASM-4 — but with a ruthless focus on purity, portability, and self-containment.

It must feel like a real 8-bit-era machine you fell in love with in the 80s/90s, but implemented with modern Rust and zero reliance on game engines, heavy graphics libraries, or vendor audio stacks for the *core logic*.

### Non-Negotiables ("From the Very Scratch")

- **Graphics**: 128×128 pixels @ 60 FPS, locked 16-color palette. Every pixel, every line, every circle, every character of the font is drawn by hand-written pure Rust code operating on a raw palette-index framebuffer. No `draw2d`, no shaders for core, no external rasterizers.
- **Audio / Chiptune**: Full PSG (programmable sound generator) with multiple channels (square/pulse with variable duty, triangle, noise), envelopes, vibrato, simple tracker support. All synthesis is pure software sample generation — no external sound libraries in the core. Output is always raw PCM buffers that a host can feed to any DAC or audio API.
- **Input**: Classic 6-button + mouse (or gamepad) model. `btn()`, `btnp()`, mouse state — all emulated inside the console.
- **Cartridges**: First-class `.s8` format (and optionally PNG-steg like PICO-8) containing code + graphics + sound + map data. Carts must be loadable, editable, and shareable.
- **Development Environment**: Usable entirely inside the console — code editor, sprite editor (with 8×8 / 16×16 stamps), tilemap editor, chiptune music tracker. No external tools required to make a full game.
- **Examples**: At least 8 polished, complete game/demos that exercise every feature (Pong, Snake, Breakout/Arkanoid, simple platformer, top-down shooter, music visualizer + player, paint/art toy, full chiptune composer demo).
- **Documentation**: World-class. Hardware reference (exact memory map, timing, limits), complete API docs (rustdoc + book), step-by-step tutorials ("make your first cart in 10 minutes", "write a chiptune"), cartridge format spec, "how the machine works" deep dives, contribution guide.
- **Portability — "Compiles and Runs on Any Chip"**:
  - The **core library** (`scratch8-core`) is `#![no_std]` (and preferably `no_alloc` for the hot paths).
  - It must cross-compile and provide a working runtime (with host-supplied "present framebuffer" and "fill audio buffer" callbacks) on:
    - Desktop (x86_64, aarch64 — macOS, Linux, Windows)
    - Web browsers (`wasm32-unknown-unknown` + Canvas + Web Audio API)
    - Embedded microcontrollers ("any chip"): ARM Cortex-M (thumbv7m / thumbv7em), RISC-V, ESP32, etc.
  - Carts themselves should be portable. Two paths:
    1. Native Rust carts (for maximum power on capable hosts).
    2. WASM carts (or a tiny custom bytecode) so that a game written once can run on a browser, a desktop, *and* a small embedded device with a WASM runtime (or our tiny interpreter).
  - No hard platform dependencies in the machine logic. Platform adapters (minifb, winit+softbuffer, web-sys, embassy HALs, etc.) live in thin separate crates or examples.

Everything that gives the *feeling* of the console — the drawing, the sound synthesis, the editors, the cartridge execution model — must be authored by us or extremely minimal, auditable dependencies.

## Success Criteria (v1.0)

1. `cargo build` (desktop) produces a beautiful, fully playable console app with editors and audio.
2. `cargo build --target wasm32-unknown-unknown` succeeds for the core + web runtime; a playable web demo exists (GitHub Pages or single HTML + wasm).
3. `cargo check --target thumbv7em-none-eabihf` (or equivalent bare-metal target) succeeds for the core library. A minimal embedded example or simulator exists that "runs" a cart (even if output is over serial or a small LCD simulator).
4. All drawing primitives, the full chiptune synth, input, and cartridge loader are 100% from-scratch implementations inside the crate.
5. At least 8 complete, fun, well-documented game examples live in `examples/` or `carts/`.
6. In-console editors for code, sprites, maps, and music are functional and delightful.
7. `.s8` (and optionally PNG) cartridge save/load/export works. A user can create a cart on desktop, load it in the web version or on an embedded device.
8. Documentation is complete enough that a motivated stranger can:
   - Understand the entire machine in < 1 hour of reading.
   - Write a non-trivial game using only the exposed API.
   - Contribute a new primitive or editor feature.
9. The project builds cleanly with `cargo clippy`, has good tests for the core (primitives, synth, cartridge parser), and has cross-compilation verification scripts.
10. The agent swarm that built it left clear traces (design decisions, alternative approaches considered, etc.).

## Constraints & Philosophy

- **Purity over convenience**. We will implement Bresenham, circle rasterization, a software PSG mixer, a simple text editor widget, a sprite stamp system, etc., by hand even when a crate would be faster.
- **Constraints are the fun**. 128×128, 16 colors, limited "RAM" (we can emulate the limits), 4-6 audio channels. These limits make creativity explode.
- **"Any chip" is not marketing**. The architecture must actually support tiny MCUs. That means fixed-size arrays, no panicking on allocation in hot paths, pure functions for synth and drawing where possible, and clear host boundaries.
- **Multiple cart languages by design**. Start with Rust `Cart` trait (powerful, zero-cost). Add WASM cart support (or a tiny custom VM) so non-Rust programmers (and embedded targets without full Rust) can participate.
- **Swarm development**. This project is being built by a coordinated swarm of specialized AI agents (portability, audio, editors, docs, examples, cartridge format, verification). All work must be reviewable, incremental, and documented.

## Phased Roadmap (High Level)

**Phase 0 (Foundation - current)**: Portable core Console + drawing + Cart trait + a few demos. (Already done.)

**Phase 1 (Portability + Audio)**: Extract clean `no_std` lib. Implement full from-scratch chiptune PSG + mixer. Add first WASM target + thin web runner. Cross-compile verification for embedded targets.

**Phase 2 (Cartridges + Examples)**: `.s8` format + loader/saver. 6-8 high-quality game examples (must include at least one that heavily uses audio). Basic persistence.

**Phase 3 (Editors)**: Full in-console development suite (code editor with live reload, sprite editor with copy/paste/stamps, tilemap, chiptune tracker that can play and save patterns).

**Phase 4 (Polish + "Any Chip" Demo)**: Web playable demo. Embedded proof-of-concept (simulator or real hardware if possible). Excellent docs, gallery, packaging, release.

**Phase 5 (Stretch)**: Tiny custom bytecode VM for carts (maximum scratch purity on the tiniest chips), more languages (e.g., a subset of Lua or a Forth-like), better font, more colors option (but keep 16-color "classic" mode), networking? (no — keep the fantasy), export to standalone player, etc.

## How the Agent Swarm Works Here

We are using parallel specialized sub-agents (via the Grok Build subagent system) assigned to tracks. They work in git worktrees or focused scopes, commit clean incremental changes, and surface decisions. The main orchestrator (me) integrates, reviews, resolves conflicts, and keeps the global vision.

Current active tracks (see todo list and individual agent prompts):
- Portability Engineer (no_std lib, multiple targets, host traits)
- Chiptune Audio Wizard (full PSG from scratch + tracker primitives)
- WASM + Web Runtime Specialist
- Documentation & Examples Author (tutorials, rustdoc, game gallery, spec)
- Editor Implementer (the four editors)
- Cartridge Format & Loader Engineer
- Build & Verification Guardian (cross-compile matrix, tests, CI scripts)

## Definition of Done for the Whole Project

A stranger can:
1. `git clone` this repo.
2. `cargo run --release` and immediately make a fun game using only the built-in tools.
3. Build the web version and play the same cart in a browser.
4. Cross-compile the core for their weird embedded board and have the drawing + synth logic "just work" once they plug in a display and audio output.
5. Read the docs and understand *why* every design decision was made.

This is the goal. Let's swarm it into existence.

— Built with a coordinated agent swarm, from the very scratch.