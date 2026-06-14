# SCRATCH-8 Master TODO / Progress Tracker

This file tracks the agent swarm tracks and concrete deliverables. It is the "master todo" referenced in GOALS.md and individual agent charters.

Updated by: Documentation & Game Examples Author (this agent track)

See also: GOALS.md (full vision + success criteria), README.md (user-facing status)

---

## Documentation Track (this agent)

### Completed in this session (2026-06-14 Documentation & Examples pass)
- [x] Created `docs/TUTORIAL.md` — "Your first cart in 15 minutes" with step-by-step, full working Catcher toy code, API walkthrough, from-scratch philosophy.
- [x] Created `examples/pong.rs` and `examples/snake.rs` (see Game Examples section below).
- [x] Created `docs/HARDWARE.md` (full machine spec).
- [x] Created `docs/CARTRIDGE_FORMAT.md` (initial .s8 seed).
- [x] Major README expansion (architecture, chips section, cross-refs, examples guidance) + full verification of `cargo check --examples`.
- [x] Master TODO.md created and maintained.

- [ ] (Optional) `docs/API.md` placeholder — currently relying on rustdoc + TUTORIAL + HARDWARE + CARTRIDGE_FORMAT for v1.

### Outstanding (Documentation)
- Gallery / screenshots section once more carts + web demo exist.
- When audio lands: add chiptune tutorial section or "write a chiptune in 10 min".
- Possibly `docs/CONTRIBUTING.md` or minor GOALS polish.
- Add more rustdoc examples in lib.rs on future passes.
- Keep docs in sync as audio, editors, and cartridge loader tracks deliver.

---

## Game Examples / Carts Track (this agent + coordination)

### Completed (this session — first 2 of target 4-6+ , with full docs + verification)
- [x] Pong (`examples/pong.rs`) — see header comment in the file
- [x] Snake (`examples/snake.rs`) — see header comment in the file
- [x] Also delivered TUTORIAL.md (with Catcher toy), HARDWARE.md, CARTRIDGE_FORMAT.md, README overhaul, and master TODO.md as part of the same coherent documentation + examples effort.

### Planned / Outstanding (need 4-6 total high-quality)
- [ ] Breakout / Arkanoid stub (bricks, paddle, ball, multiple ball speeds, particle on break — uses rect + line + collision math)
- [ ] Simple platformer (side view, gravity jump, platforms via rects or future map, collectibles)
- [ ] Music / audio visualizer demo (once PSG + Console audio methods available in lib; bars + particles synced to synth)
- [ ] Paint / art toy (enhance or replace builtin PAINTER with stamps, undo, palette picker, save to "cart" data)
- [ ] Top-down shooter stub (player ship + bullets + enemies using pset or small rects, spawn waves, score)
- [ ] At least one cart that exercises future sprite/map/audio heavily when those land.

Each must:
- Live in `examples/` or `carts/`
- Have short comment header describing what it shows + features exercised
- Use **only** the public `Console` + `Cart` (no internal crate details)
- Be "from scratch" (hand-written collision, timing, drawing loops where appropriate)
- Compile via `cargo check --examples`

Current builtin demos (in lib for boot): BOUNCER, PAINTER, PARTICLES — keep for now as educational.

---

## Other Tracks (for context / swarm coordination — not owned by this agent)

**Portability Engineer**
- no_std purity, Host trait, cross targets (thumbv7em, riscv, wasm)
- Verify script already exists (`scripts/verify-portability.sh`)

**Chiptune Audio Wizard**
- PSG (square/triangle/noise + envelopes), pure software synthesis to i16 PCM
- `src/audio/` (currently empty dir)
- Expose `tone`, `sfx`, mixer on Console behind "audio" feature

**Cartridge Format & Loader Engineer**
- .s8 (and PNG) loader/saver
- See CARTRIDGE_FORMAT.md once seeded

**Editor Implementer**
- In-console code/sprite/map/music editors (TAB or E key today is placeholder)
- `src/editors/`

**WASM + Web Runtime**
- wasm32 target + Canvas/WebAudio thin host

**Build & Verification Guardian**
- CI matrix, clippy, tests for primitives/synth/parser, more verify scripts

**Phase status** (from GOALS):
- Phase 0 (Foundation): largely complete (core Console/Cart, primitives, 3 demos, desktop thin runner)
- Phase 1 (Portability + Audio): in progress (verify script, audio module started in structure)
- Phase 2 (Cartridges + Examples): this agent's focus — 2/6+ examples + format docs started
- Phase 3+: editors, web, embedded demos

---

## Quick Commands for This Track

```bash
# After changes to docs or examples
cargo check --lib --no-default-features   # core portable
cargo check --features full
cargo check --examples                    # critical for game examples track
./scripts/verify-portability.sh

cargo doc --open                          # see API surface + our docs comments
cargo run --release --features full       # play (add your new carts to the vec in main.rs for now)
cargo run --example pong                  # headless sim of the example
cargo run --example snake
```

## Notes / Decisions Captured Here

- Examples are currently **standalone compilable demos** (with `main` that runs simulation + prints) so they always `cargo check --examples` succeed against the lib without pulling desktop windowing. Users copy the `struct + impl Cart` into the desktop runner (or future cart system) for interactive play. This matches "from scratch" + portable spirit.
- We rely primarily on rustdoc (excellent comments already in lib.rs) + TUTORIAL.md + HARDWARE.md rather than a redundant API.md for v1.
- .s8 format doc is a **seed** — format will evolve once loader work begins. Current carts are still source-level Rust structs (maximum power, zero-cost on desktop).
- All new carts must stay tiny and educational while being "complete" and fun within the 128x128 box.

Keep this file updated after each significant doc or example PR / agent pass.
Last major update: 2026-06-14 — first tutorial + Pong + Snake delivered.
