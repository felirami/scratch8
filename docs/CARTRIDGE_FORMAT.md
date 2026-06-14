# SCRATCH-8 Cartridge Format (.s8)

**Status**: Initial seed definition (Phase 2 work). **Not yet implemented**.

Current carts are Rust source structs that implement the `Cart` trait and are compiled into the binary (see `src/lib.rs` built-ins and `examples/*.rs`). The `.s8` format and loader will allow **data-driven** carts (code + assets in a single portable file) that can be created in the console editors, saved, shared, and loaded on desktop / web / embedded without recompiling the host.

This document defines the on-disk / in-memory layout we will target. It will evolve as the Cartridge Format & Loader Engineer track and the Editor track make progress. Feedback and patches welcome.

## Design Goals

- Small and simple enough for embedded targets and slow storage.
- Self-describing (contains title, author, sections present).
- Supports multiple "code" representations:
  - Rust source (for the in-console editor + future "compile inside the machine" dream)
  - WASM module bytes (powerful, portable guest code)
  - Future tiny custom bytecode / VM image (maximum purity on the tiniest chips)
- Contains first-class graphics, sound, and map data so a cart is a complete artifact.
- Extensible via section flags / future section types.
- Optional "PNG steganography" mode (like PICO-8) for fun shareable images that are also valid .s8 when decoded.
- CRC / checksum for integrity.
- Versioned so old hosts can at least recognize (and possibly ignore) newer carts.

## Binary .s8 Layout (Draft v0)

All multi-byte integers are **little-endian**.

```
Offset  Size    Field
0       4       Magic: b"S8\x1F\x00"   (or "SCR8" for human readability in hex dumps)
4       2       Format version (u16, currently 0)
6       2       Header size in bytes (u16) — allows future header growth
8       4       Flags (u32 bitfield)
                bit 0: has_code
                bit 1: has_gfx
                bit 2: has_sfx
                bit 3: has_map
                bit 4: compressed (future zlib or custom)
                bits 5-31: reserved
12      16      Title (null-terminated UTF-8, padded)
28      16      Author (null-terminated UTF-8, padded)
44      2       Reserved / minor version or icon index
46      2       CRC16 of header (optional quick check)
48      ...     Section directory or direct sections (see below)

Then sequential sections until EOF.
Each section:
  u32  section_type   (magic tag)
  u32  size_bytes
  [u8; size] payload

Section types (proposed):
  b"CODE"  — the program (see "Code payload" below)
  b"GFX "  — sprite / stamp / tile graphics (palette-indexed bitmap data)
  b"SFX "  — sound patterns, instruments, sequences
  b"MAP "  — tile map data
  b"USER"  — cart-specific persistent save data (high scores, unlocked levels, etc.)
  b"META"  — extra JSON or key-value metadata (future)

After last section: optional 4-byte CRC32 of the entire payload (everything after magic).
```

A minimal empty cart might be just the header + no sections (or a CODE section with only a name).

## Code Payload Details

The CODE section is the most interesting and will have an inner header:

```
u16  code_format
     0 = Rust source text (UTF-8). The in-console editor writes this.
     1 = WASM binary (wasm32-unknown-unknown module)
     2 = SCRATCH-8 bytecode (future tiny VM)
     3 = reserved / Lua subset etc.
u32  code_size
[u8] the code bytes (source or compiled bytes)
```

For Rust source carts today (before a full VM):
- The "code" is the literal source of the `struct Foo { ... } impl Cart for Foo { ... }`.
- At load time the host (desktop) could theoretically offer a "compile this cart" button that turns the source + the current lib into a loadable module, or we simply treat it as documentation + data for the editors.
- For maximum compatibility on tiny chips we expect most shared carts to eventually be WASM or our bytecode.

## Graphics (GFX) Payload

Simple for v0:

- u16 sprite_width, sprite_height (in pixels, usually 8 or 16)
- u16 count or total_bytes
- Raw palette-index bytes (row-major). Color 0 = transparent by convention.

Later versions may add:
- Multiple sheets / pages
- RLE or simple compression
- 1-bit + palette modes for even smaller size

The editors will produce and consume this section directly.

## Sound (SFX) & Map Payloads

- SFX: tracker rows, per-channel notes, instrument definitions (wave type, attack/decay, vibrato depth), pattern order list. Keep it tiny (target a few hundred bytes for a whole song).
- MAP: 2D array of tile indices (u8 or u16). Dimensions stored in header of the section. 32×32 or 128×32 are good starting points for 128 px screen with 4 px or 8 px tiles.

Exact binary layout for SFX/MAP will be defined by the audio and cartridge engineers when they implement the corresponding Console APIs (`tone`, `sfx`, `map`, `spr`).

## USER Data

Arbitrary cart-chosen bytes (high scores, progress). The cart itself decides the schema. The host just persists the blob when the user chooses "save cart" or on exit.

## PNG Steganography Mode (Optional, Fun)

A valid 128×128 or larger PNG whose pixel data (least-significant bits of certain channels) contains the .s8 binary after a small header.

- The PNG itself is a valid, viewable screenshot or title image of the game.
- Special decoder in the host (or a standalone tool) extracts the hidden .s8 payload.
- This gives the delightful "share a pretty picture that is secretly a full game" experience of PICO-8.

Not required for v1 of the format; the pure binary .s8 is the primary artifact.

## Loading & Saving API (Planned)

In the core (behind feature or always):

```rust
// Future
impl Console {
    pub fn load_cart(&mut self, data: &[u8]) -> Result<Box<dyn Cart>, CartError> { ... }
    pub fn save_cart(&self, cart: &dyn Cart) -> Vec<u8> { ... } // or write to host via callback
}
```

Or a separate `Cartridge` module.

Desktop host will add file-open / export .s8 (and .png) menu items.

The in-console editors will be the primary authoring tool for .s8 files.

## Current Reality vs Future

**Today**:
- Carts = Rust `impl Cart` compiled into the binary or copied by the user.
- No .s8 loader exists.
- `builtin_carts()` returns the three demos.
- Examples in `examples/` are source you study and paste.

**Soon (this phase)**:
- .s8 binary definition stabilized in this document.
- Basic loader that can at least read metadata + GFX/MAP for the editors.
- Rust source carts may be stored inside .s8 for the code editor even if execution still requires recompilation on desktop.

**Later**:
- WASM cart execution (guest calls into a JS/Rust shim that forwards to Console methods).
- Tiny custom bytecode interpreter (for the smallest embedded devices that can't run a WASM runtime).
- Full round-trip: edit in console → save .s8 → load on another device or in browser → play.

## Example (Conceptual) Minimal .s8 (hex sketch)

```
53 38 1F 00  00 00  30 00   ...flags... "MY GAME\0..." ...
43 4F 44 45  00 00 00 2A  "struct Tiny{...} impl Cart..."
... GFX section etc. ...
```

(Real files will be generated by code; this is just to illustrate the tag + length layout.)

## Open Questions (for the swarm)

- Exact compression (if any) — keep it optional.
- Maximum total .s8 size for "embedded friendly" carts (64 KB? 256 KB?).
- How to securely/ergonomically compile Rust source carts at runtime on desktop (or do we treat source carts as editor-only + require WASM export for distribution?).
- Versioning strategy when we add new section types or change audio format.
- Whether the format should also support a tiny "ROM header" for pure data carts that use a built-in engine (like early 8-bit games).

## How to Help

- The Cartridge Format & Loader track owns the implementation.
- Documentation & Examples track (this author) will keep this doc in sync and will produce example .s8 files (once the writer exists) plus carts that exercise the new sections.
- When the format is real, the Pong and Snake examples above will be the first candidates to be "exported" as .s8 + source inside the same file.

Until then: keep writing beautiful `impl Cart` games in `examples/` and `carts/`. They are the soul of the machine.

---

See also:
- `GOALS.md` — Phase 2 (Cartridges + Examples)
- `docs/HARDWARE.md` — the machine the carts run on
- `docs/TUTORIAL.md` — how to write a cart today
- `src/carts/` (future location for cart-related code)
