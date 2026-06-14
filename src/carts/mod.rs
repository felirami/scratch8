//! SCRATCH-8 Cartridge Format (.s8) — v1 implementation.
//!
//! This module defines the on-disk / shareable cartridge format for the fantasy console.
//! Goals:
//! - Extremely simple binary format (no external deps, no_std + alloc friendly).
//! - Portable: same bytes load on desktop, web (once web runner + WASM carts), and embedded (with FS or flash).
//! - v1: metadata + code section STUB + gfx data STUB + sfx data STUB.
//! - Future: WASM carts (code section holds a .wasm blob that exports `update`/`draw` and imports host fns like `pset`, `cls` etc from the runtime).
//! - Optional later: RON/JSON flavor for human-editable v1 carts, or PNG-steg embedding (PICO-8 style).
//!
//! The "code" for v1 is a stub (e.g. "builtin:0" string) because our carts are currently
//! Rust `Cart` trait impls compiled into the host. This lets us export "carts" today for
//! the persistence / share story and as a placeholder for real loadable code sections.
//!
//! Save/load here are pure (de)serialization. std-gated file I/O lives behind cfg.
//!
//! COORDINATION:
//! - This fulfills the "cartridge-system" track (see GOALS.md Phase 2, success criterion 7).
//! - Pairs with wasm-port track: once we have web runner + WASM cart loading, a .s8 from desktop
//!   can be loaded in browser (initially by name lookup into builtins, later by extracting WASM payload).
//! - See also wasm-carts-future notes in lib.rs and this file.
//!
//! Binary layout (little-endian where multi-byte):
//!   [0..4]   magic: b"S8\x00\x01"  (S8 v1)
//!   [4]      name_len: u8 (0..=63)
//!   [5..5+name_len] name: utf8 bytes
//!   [5+name_len] author_len: u8
//!   [...]    author bytes
//!   [...]    u32 reserved_flags (0 for v1)
//!   [...]    u16 code_len
//!   [...]    code bytes (stub: b"builtin:0" or b"builtin:BOUNCER" etc; later wasm: followed by bytes)
//!   [...]    u16 gfx_len
//!   [...]    gfx bytes (v1 stub: empty or small sprite/tile data; future full sprite sheet / map)
//!   [...]    u16 sfx_len
//!   [...]    sfx bytes (v1 stub: empty; future tracker patterns / instrument defs)
//!   [...]    u16 meta_len
//!   [...]    meta bytes (v1: empty or simple "key=value" or future RON/JSON fragment)
//!
//! Limits chosen for tiny targets: strings capped, sections small in v1.

#![allow(unused)] // stubs; will be used more as we flesh out examples + editors

extern crate alloc;

use alloc::{string::{String, ToString}, vec::Vec};

/// Magic bytes for .s8 v1 files.
pub const S8_MAGIC: &[u8; 4] = b"S8\x00\x01";

/// Max length for name/author in v1 (keeps header tiny and embedded-friendly).
const MAX_NAME_LEN: usize = 63;

/// A loaded or constructible .s8 cartridge (v1).
///
/// For v1 the `code`, `gfx`, `sfx`, `meta` are intentionally stubby:
/// - code: identifier for which builtin (or later the WASM module bytes)
/// - gfx/sfx: asset containers (currently empty; will hold real data when sprite/map/audio systems land)
#[derive(Clone, Debug, PartialEq)]
pub struct S8Cartridge {
    pub version: u8,
    pub name: String,
    pub author: String,
    /// Code section (stub for v1). Examples: b"builtin:0", b"builtin:PAINTER", or future raw .wasm bytes prefixed.
    pub code: Vec<u8>,
    /// Graphics data section (stub). Future: 8x8/16x16 sprite stamps, tilemaps, etc. packed here.
    pub gfx: Vec<u8>,
    /// Sound/sfx data section (stub). Future: chiptune patterns, envelopes, instruments.
    pub sfx: Vec<u8>,
    /// Freeform metadata (v1 empty; later can hold RON/JSON for description, tags, version of cart source, etc.)
    pub meta: Vec<u8>,
}

impl S8Cartridge {
    /// Create a minimal v1 cart for one of our builtin demos.
    /// Used by the desktop runner when exporting.
    pub fn from_builtin(cart_name: &str, cart_index: usize) -> Self {
        let code_str = alloc::format!("builtin:{}", cart_index);
        Self {
            version: 1,
            name: cart_name.to_string(),
            author: "scratch8-user".to_string(),
            code: code_str.into_bytes(),
            gfx: Vec::new(), // v1 stub — real gfx assets will live here
            sfx: Vec::new(), // v1 stub — chiptune data will live here
            meta: b"v1-stub;exported-from-desktop".to_vec(),
        }
    }

    /// Create an empty template cart (for future editors / new cart creation).
    pub fn new_empty(name: &str) -> Self {
        Self {
            version: 1,
            name: name.to_string(),
            author: "scratch8".to_string(),
            code: b"builtin:custom".to_vec(),
            gfx: Vec::new(),
            sfx: Vec::new(),
            meta: Vec::new(),
        }
    }

    /// Serialize to the canonical .s8 v1 binary format.
    /// Always produces a well-formed file; no_std safe (just alloc).
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(64 + self.name.len() + self.code.len() + self.gfx.len() + self.sfx.len() + self.meta.len());

        out.extend_from_slice(S8_MAGIC);

        // name (capped)
        let name_bytes = self.name.as_bytes();
        let name_len = name_bytes.len().min(MAX_NAME_LEN) as u8;
        out.push(name_len);
        out.extend_from_slice(&name_bytes[..name_len as usize]);

        // author (capped)
        let author_bytes = self.author.as_bytes();
        let author_len = author_bytes.len().min(MAX_NAME_LEN) as u8;
        out.push(author_len);
        out.extend_from_slice(&author_bytes[..author_len as usize]);

        // reserved flags
        out.extend_from_slice(&0u32.to_le_bytes());

        // code section
        let code_len = (self.code.len() as u16).min(u16::MAX);
        out.extend_from_slice(&code_len.to_le_bytes());
        out.extend_from_slice(&self.code[..code_len as usize]);

        // gfx
        let gfx_len = (self.gfx.len() as u16).min(u16::MAX);
        out.extend_from_slice(&gfx_len.to_le_bytes());
        out.extend_from_slice(&self.gfx[..gfx_len as usize]);

        // sfx
        let sfx_len = (self.sfx.len() as u16).min(u16::MAX);
        out.extend_from_slice(&sfx_len.to_le_bytes());
        out.extend_from_slice(&self.sfx[..sfx_len as usize]);

        // meta
        let meta_len = (self.meta.len() as u16).min(u16::MAX);
        out.extend_from_slice(&meta_len.to_le_bytes());
        out.extend_from_slice(&self.meta[..meta_len as usize]);

        out
    }

    /// Parse from .s8 v1 bytes. Strict on magic/version; tolerant on lengths (clamped).
    pub fn from_bytes(data: &[u8]) -> Result<Self, &'static str> {
        if data.len() < 4 + 1 + 1 + 4 + 2 {
            return Err("too short for .s8 v1 header");
        }
        if &data[0..4] != S8_MAGIC {
            return Err("bad magic (not a .s8 v1 cart)");
        }

        let mut pos = 4usize;

        // name
        let name_len = data[pos] as usize;
        pos += 1;
        if pos + name_len > data.len() {
            return Err("truncated name");
        }
        let name = core::str::from_utf8(&data[pos..pos + name_len])
            .map_err(|_| "invalid utf8 in name")?
            .to_string();
        pos += name_len;

        // author
        if pos >= data.len() {
            return Err("truncated author len");
        }
        let author_len = data[pos] as usize;
        pos += 1;
        if pos + author_len > data.len() {
            return Err("truncated author");
        }
        let author = core::str::from_utf8(&data[pos..pos + author_len])
            .map_err(|_| "invalid utf8 in author")?
            .to_string();
        pos += author_len;

        // flags
        if pos + 4 > data.len() {
            return Err("truncated flags");
        }
        // let _flags = u32::from_le_bytes([data[pos], data[pos+1], data[pos+2], data[pos+3]]);
        pos += 4;

        // code
        if pos + 2 > data.len() {
            return Err("truncated code_len");
        }
        let code_len = u16::from_le_bytes([data[pos], data[pos + 1]]) as usize;
        pos += 2;
        if pos + code_len > data.len() {
            return Err("truncated code section");
        }
        let code = data[pos..pos + code_len].to_vec();
        pos += code_len;

        // gfx
        if pos + 2 > data.len() {
            return Err("truncated gfx_len");
        }
        let gfx_len = u16::from_le_bytes([data[pos], data[pos + 1]]) as usize;
        pos += 2;
        if pos + gfx_len > data.len() {
            return Err("truncated gfx section");
        }
        let gfx = data[pos..pos + gfx_len].to_vec();
        pos += gfx_len;

        // sfx
        if pos + 2 > data.len() {
            return Err("truncated sfx_len");
        }
        let sfx_len = u16::from_le_bytes([data[pos], data[pos + 1]]) as usize;
        pos += 2;
        if pos + sfx_len > data.len() {
            return Err("truncated sfx section");
        }
        let sfx = data[pos..pos + sfx_len].to_vec();
        pos += sfx_len;

        // meta
        if pos + 2 > data.len() {
            return Err("truncated meta_len");
        }
        let meta_len = u16::from_le_bytes([data[pos], data[pos + 1]]) as usize;
        pos += 2;
        if pos + meta_len > data.len() {
            return Err("truncated meta section");
        }
        let meta = data[pos..pos + meta_len].to_vec();
        // pos += meta_len; // not needed

        Ok(Self {
            version: 1,
            name,
            author,
            code,
            gfx,
            sfx,
            meta,
        })
    }
}

/// Helper: attempt to interpret the code stub as a "builtin:<idx>" reference.
/// Returns the index if it matches the pattern used by from_builtin.
pub fn code_as_builtin_index(code: &[u8]) -> Option<usize> {
    let s = core::str::from_utf8(code).ok()?;
    if let Some(rest) = s.strip_prefix("builtin:") {
        rest.parse::<usize>().ok()
    } else {
        None
    }
}

// === std-gated persistence (for desktop runner export + future load) ===

#[cfg(feature = "std")]
use std::io::{self, Write};

#[cfg(feature = "std")]
/// Write a cartridge to a .s8 file on disk (desktop / std hosts only).
pub fn save_s8(cart: &S8Cartridge, path: &str) -> io::Result<()> {
    let bytes = cart.to_bytes();
    std::fs::write(path, &bytes)?;
    Ok(())
}

#[cfg(feature = "std")]
/// Load a .s8 cartridge from disk.
pub fn load_s8(path: &str) -> io::Result<S8Cartridge> {
    let bytes = std::fs::read(path)?;
    S8Cartridge::from_bytes(&bytes).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

// Future expansion notes (for swarm coordination):
// - RON/JSON v1 alternative: behind another feature or a `to_ron()` fn using minimal writer (no dep).
// - When sprite system lands (editors track), gfx section will get a defined layout (e.g. 16x 8x8 tiles = 1024 bytes + header).
// - For WASM carts: code section can be > a few KB; loader will validate wasm magic, provide import object with
//   host fns (the Console drawing/audio/input surface exposed to guest), call guest's exported update/draw.
// - "compile once run anywhere": a cart author compiles their Rust (or C/Zig) to wasm32-unknown-unknown targeting
//   our import ABI, gets a .wasm, wraps it in a .s8 with metadata/gfx/sfx, and it runs in browser (native wasm),
//   desktop (wasmtime or our thin host), and on embedded with a tiny wasm3/m3 or our future micro-interpreter.
// - See lib.rs for more on the Cart trait evolution and host fn exposure.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_builtin() {
        let c = S8Cartridge::from_builtin("BOUNCER", 0);
        let b = c.to_bytes();
        assert!(b.starts_with(S8_MAGIC));
        let c2 = S8Cartridge::from_bytes(&b).expect("parse ok");
        assert_eq!(c, c2);
        assert_eq!(code_as_builtin_index(&c2.code), Some(0));
    }

    #[test]
    fn rejects_bad_magic() {
        let mut b = S8Cartridge::from_builtin("X", 1).to_bytes();
        b[0] = b'X';
        assert!(S8Cartridge::from_bytes(&b).is_err());
    }
}
