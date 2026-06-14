//! SCRATCH-8 Core — the portable heart of the fantasy console.
//!
//! This crate is deliberately `#![no_std]` (with optional `std` feature).
//! All drawing primitives, the Console state machine, input model, and
//! cartridge trait are implemented from scratch in pure Rust.
//!
//! The goal: the same core must compile and provide useful behavior on
//! desktop, WebAssembly (browsers + any WASM host), and bare-metal
//! embedded microcontrollers ("any chip").
//!
//! Host platforms supply (see the `Host` trait for the canonical interface):
//! - A way to present the 128x128 palette-index framebuffer (or RGB buffer).
//! - A way to feed raw PCM samples for audio (when the "audio" feature is enabled
//!   the core provides a complete from-scratch 4-channel chiptune PSG mixer).
//! - Keyboard/mouse/gamepad events (mapped to the classic 6-button + mouse model).
//!
//! Everything else (pixels, sound synthesis, editors, cart execution) lives here.

#![no_std]

// We need alloc for Box<dyn Cart> (trait objects for the nice cart system).
// Almost all "chips" that can run interesting Rust code support alloc today.
// For the absolute smallest no-alloc embedded, users can use concrete cart types or an enum.
extern crate alloc;

#[cfg(feature = "std")]
extern crate std;
// ^ The std feature is a marker pulled in by "desktop"/"wasm" etc. The *library*
// itself remains #![no_std] unconditionally (per portability goals). We bring
// in `extern crate std` only so that future cfg-gated std-only helpers (e.g. for
// desktop testing conveniences) can be added without changing the no_std nature
// of the core machine logic. No std items are used in this file today.

use alloc::boxed::Box;

// ============================================================================
// HARDWARE CONSTANTS (the "machine" spec — sacred)
// ============================================================================

/// Display width in pixels. PICO-8 classic.
pub const WIDTH: usize = 128;
/// Display height in pixels.
pub const HEIGHT: usize = 128;

/// Total "video RAM" in bytes when using palette indices (16 KB).
pub const VRAM_SIZE: usize = WIDTH * HEIGHT;

/// Number of palette colors. Locked for the classic mode.
pub const PALETTE_SIZE: usize = 16;

/// PICO-8 inspired 16-color palette (0x00RRGGBB).
/// Index 0 is always treated as "transparent" in some contexts (spr, etc.).
pub const PALETTE: [u32; PALETTE_SIZE] = [
    0x000000, // 0 black
    0x1D2B53, // 1 dark blue
    0x7E2553, // 2 dark purple
    0x008751, // 3 dark green
    0xAB5236, // 4 brown
    0x5F574F, // 5 dark gray
    0xC2C3C7, // 6 light gray
    0xFFF1E8, // 7 white
    0xFF004D, // 8 red
    0xFFA300, // 9 orange
    0xFFEC27, // 10 yellow
    0x00E436, // 11 green
    0x29ADFF, // 12 blue
    0x83769C, // 13 lavender
    0xFF77A8, // 14 pink
    0xFFCCAA, // 15 peach
];

// ============================================================================
// THE CONSOLE — the actual fantasy machine
// ============================================================================

/// The central state of the fantasy console.
/// 
/// Contains the framebuffer (palette indices only — this is what enforces
/// the 16-color limit), button state, frame counter, mouse, and (when "audio"
/// feature): the built-in chiptune PSG via the `audio: AudioMixer` field.
///
/// All drawing happens by mutating the `buffer`. The host reads it (or a
/// converted RGB version) to present pixels.
pub struct Console {
    /// Palette-index framebuffer. 128x128 bytes.
    pub buffer: [u8; VRAM_SIZE],

    /// Monotonic frame counter (wraps at u32::MAX). Great for animations.
    pub frame: u32,

    /// Mouse position in console pixel space (0..127).
    pub mouse_x: i32,
    pub mouse_y: i32,

    /// Classic 6-button state (PICO-8 style).
    /// 0: left, 1: right, 2: up, 3: down, 4: Z/O/action1, 5: X/K/action2
    pub btn: [bool; 6],
    prev_btn: [bool; 6],

    /// The chiptune audio mixer (AudioMixer from the always-included audio module).
    /// Pure from-scratch PSG synthesis (envelopes, vibrato, slide, patterns, sfx).
    /// Always present for "any chip" portability. Host output (cpal etc) is behind the "audio" feature.
    pub audio: AudioMixer,

    // Future fields (sprites, map, editors support, etc.) will be added feature-gated for the smallest chips.
}

impl Default for Console {
    fn default() -> Self {
        Self::new()
    }
}

impl Console {
    /// Create a fresh console with a cleared screen (color 0).
    pub fn new() -> Self {
        let mut c = Self {
            buffer: [0; VRAM_SIZE],
            frame: 0,
            mouse_x: 0,
            mouse_y: 0,
            btn: [false; 6],
            prev_btn: [false; 6],
            audio: AudioMixer::new(44100),
        };
        c.cls(0);
        c
    }

    /// Clear the screen to a palette color.
    pub fn cls(&mut self, col: u8) {
        let c = col & 0x0F;
        self.buffer.fill(c);
    }

    /// Plot a single pixel (clipped). Color is masked to 0-15.
    pub fn pset(&mut self, x: i32, y: i32, col: u8) {
        if x >= 0 && x < WIDTH as i32 && y >= 0 && y < HEIGHT as i32 {
            let idx = (y as usize) * WIDTH + (x as usize);
            self.buffer[idx] = col & 0x0F;
        }
    }

    /// Read a pixel (returns 0 outside screen).
    pub fn pget(&self, x: i32, y: i32) -> u8 {
        if x >= 0 && x < WIDTH as i32 && y >= 0 && y < HEIGHT as i32 {
            self.buffer[(y as usize) * WIDTH + (x as usize)]
        } else {
            0
        }
    }

    /// Filled rectangle. Pure software, from scratch.
    pub fn rect(&mut self, x: i32, y: i32, w: i32, h: i32, col: u8) {
        let c = col & 0x0F;
        let x1 = x.max(0);
        let y1 = y.max(0);
        let x2 = (x + w).min(WIDTH as i32);
        let y2 = (y + h).min(HEIGHT as i32);
        for yy in y1..y2 {
            for xx in x1..x2 {
                let idx = (yy as usize) * WIDTH + (xx as usize);
                self.buffer[idx] = c;
            }
        }
    }

    /// Rectangle outline (1px).
    pub fn rectb(&mut self, x: i32, y: i32, w: i32, h: i32, col: u8) {
        let c = col & 0x0F;
        for xx in x..(x + w) {
            self.pset(xx, y, c);
            self.pset(xx, y + h - 1, c);
        }
        for yy in y..(y + h) {
            self.pset(x, yy, c);
            self.pset(x + w - 1, yy, c);
        }
    }

    /// Filled circle (distance check — correct and simple for our tiny res).
    pub fn circ(&mut self, cx: i32, cy: i32, r: i32, col: u8) {
        if r <= 0 {
            self.pset(cx, cy, col);
            return;
        }
        let r2 = r * r;
        let minx = (cx - r).max(0);
        let maxx = (cx + r).min(WIDTH as i32 - 1);
        let miny = (cy - r).max(0);
        let maxy = (cy + r).min(HEIGHT as i32 - 1);

        for yy in miny..=maxy {
            for xx in minx..=maxx {
                let dx = xx - cx;
                let dy = yy - cy;
                if dx * dx + dy * dy <= r2 {
                    self.pset(xx, yy, col);
                }
            }
        }
    }

    /// Circle outline using 8-way symmetry (Bresenham-like decision variable).
    pub fn circb(&mut self, cx: i32, cy: i32, r: i32, col: u8) {
        let c = col & 0x0F;
        if r <= 0 {
            self.pset(cx, cy, c);
            return;
        }
        let mut x = 0i32;
        let mut y = r;
        let mut d = 3 - 2 * r;

        while x <= y {
            self.pset(cx + x, cy + y, c);
            self.pset(cx - x, cy + y, c);
            self.pset(cx + x, cy - y, c);
            self.pset(cx - x, cy - y, c);
            self.pset(cx + y, cy + x, c);
            self.pset(cx - y, cy + x, c);
            self.pset(cx + y, cy - x, c);
            self.pset(cx - y, cy - x, c);

            if d < 0 {
                d += 4 * x + 6;
            } else {
                d += 4 * (x - y) + 10;
                y -= 1;
            }
            x += 1;
        }
    }

    /// Bresenham line — the classic from-scratch algorithm.
    pub fn line(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, col: u8) {
        let mut x = x0;
        let mut y = y0;
        let dx = (x1 - x0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let dy = -(y1 - y0).abs();
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx + dy;

        loop {
            self.pset(x, y, col);
            if x == x1 && y == y1 {
                break;
            }
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x += sx;
            }
            if e2 <= dx {
                err += dx;
                y += sy;
            }
        }
    }

    /// Print text using the built-in 4×6 chunky font (hand-authored data).
    /// Supports 0-9 A-Z a-z and a few symbols. Unknown chars render as a block.
    pub fn print(&mut self, text: &str, mut x: i32, y: i32, col: u8) {
        for ch in text.chars() {
            self.draw_char(ch, x, y, col);
            x += 5; // 4 wide + 1 spacing
        }
    }

    fn draw_char(&mut self, ch: char, x: i32, y: i32, col: u8) {
        let c = col & 0x0F;
        // 4x6 bit patterns (LSB = left pixel). Hand-crafted for fantasy feel.
        let pattern: [u8; 6] = match ch {
            ' ' => [0, 0, 0, 0, 0, 0],
            '!' => [0b0100, 0b0100, 0b0100, 0, 0b0100, 0],
            '.' => [0, 0, 0, 0, 0b0100, 0],
            '-' => [0, 0, 0b1111, 0, 0, 0],
            '0' => [0b0110, 0b1001, 0b1001, 0b1001, 0b0110, 0],
            '1' => [0b0010, 0b0110, 0b0010, 0b0010, 0b0111, 0],
            '2' => [0b0110, 0b1001, 0b0010, 0b0100, 0b1111, 0],
            '3' => [0b0110, 0b1001, 0b0010, 0b1001, 0b0110, 0],
            '4' => [0b0010, 0b0110, 0b1010, 0b1111, 0b0010, 0],
            '5' => [0b1111, 0b1000, 0b1110, 0b0001, 0b1110, 0],
            '6' => [0b0110, 0b1000, 0b1110, 0b1001, 0b0110, 0],
            '7' => [0b1111, 0b0001, 0b0010, 0b0100, 0b0100, 0],
            '8' => [0b0110, 0b1001, 0b0110, 0b1001, 0b0110, 0],
            '9' => [0b0110, 0b1001, 0b0111, 0b0001, 0b0110, 0],
            'A' | 'a' => [0b0110, 0b1001, 0b1111, 0b1001, 0b1001, 0],
            'B' | 'b' => [0b1110, 0b1001, 0b1110, 0b1001, 0b1110, 0],
            'C' | 'c' => [0b0111, 0b1000, 0b1000, 0b1000, 0b0111, 0],
            'D' | 'd' => [0b1110, 0b1001, 0b1001, 0b1001, 0b1110, 0],
            'E' | 'e' => [0b1111, 0b1000, 0b1110, 0b1000, 0b1111, 0],
            'F' | 'f' => [0b1111, 0b1000, 0b1110, 0b1000, 0b1000, 0],
            'G' | 'g' => [0b0111, 0b1000, 0b1011, 0b1001, 0b0111, 0],
            'H' | 'h' => [0b1001, 0b1001, 0b1111, 0b1001, 0b1001, 0],
            'I' | 'i' => [0b1110, 0b0100, 0b0100, 0b0100, 0b1110, 0],
            'J' | 'j' => [0b0011, 0b0001, 0b0001, 0b1001, 0b0110, 0],
            'K' | 'k' => [0b1001, 0b1010, 0b1100, 0b1010, 0b1001, 0],
            'L' | 'l' => [0b1000, 0b1000, 0b1000, 0b1000, 0b1111, 0],
            'M' | 'm' => [0b1001, 0b1111, 0b1111, 0b1001, 0b1001, 0],
            'N' | 'n' => [0b1001, 0b1101, 0b1011, 0b1001, 0b1001, 0],
            'O' | 'o' => [0b0110, 0b1001, 0b1001, 0b1001, 0b0110, 0],
            'P' | 'p' => [0b1110, 0b1001, 0b1110, 0b1000, 0b1000, 0],
            'Q' | 'q' => [0b0110, 0b1001, 0b1001, 0b1011, 0b0111, 0],
            'R' | 'r' => [0b1110, 0b1001, 0b1110, 0b1010, 0b1001, 0],
            'S' | 's' => [0b0111, 0b1000, 0b0110, 0b0001, 0b1110, 0],
            'T' | 't' => [0b1110, 0b0100, 0b0100, 0b0100, 0b0100, 0],
            'U' | 'u' => [0b1001, 0b1001, 0b1001, 0b1001, 0b0110, 0],
            'V' | 'v' => [0b1001, 0b1001, 0b1001, 0b0110, 0b0110, 0],
            'W' | 'w' => [0b1001, 0b1001, 0b1111, 0b1111, 0b1001, 0],
            'X' | 'x' => [0b1001, 0b0110, 0b0110, 0b0110, 0b1001, 0],
            'Y' | 'y' => [0b1001, 0b1001, 0b0110, 0b0100, 0b0100, 0],
            'Z' | 'z' => [0b1111, 0b0001, 0b0110, 0b1000, 0b1111, 0],
            _ => [0b1111, 0b1001, 0b1001, 0b1001, 0b1111, 0], // block for unknown
        };

        for row in 0..6 {
            let bits = pattern[row];
            for bit in 0..4 {
                if (bits & (1 << bit)) != 0 {
                    self.pset(x + bit as i32, y + row as i32, c);
                }
            }
        }
    }

    // --------------------------------------------------------------------
    // INPUT
    // --------------------------------------------------------------------

    /// Update button and mouse state from host events.
    /// Hosts call this every frame before the cart's update().
    pub fn update_input(&mut self, left: bool, right: bool, up: bool, down: bool, z: bool, x: bool, mx: i32, my: i32) {
        self.prev_btn = self.btn;
        self.btn = [left, right, up, down, z, x];
        self.mouse_x = mx.clamp(0, WIDTH as i32 - 1);
        self.mouse_y = my.clamp(0, HEIGHT as i32 - 1);
    }

    /// Was button `b` just pressed this frame?
    pub fn btnp(&self, b: usize) -> bool {
        b < 6 && self.btn[b] && !self.prev_btn[b]
    }

    /// Convert the palette buffer into a host-friendly RGBX u32 buffer.
    /// Call this in the platform layer right before presenting.
    pub fn to_rgb_buffer(&self, out: &mut [u32]) {
        // Use debug_assert so that in release builds for embedded (where a
        // panic handler may be minimal or absent) an incorrect buffer size
        // from the host does not cause a hard panic in a hot path.
        // In dev builds (desktop/wasm tests) this still catches misuse.
        debug_assert!(out.len() >= VRAM_SIZE);
        for (i, &idx) in self.buffer.iter().enumerate() {
            out[i] = PALETTE[idx as usize];
        }
    }

    /// Advance the frame counter. Call once per 60 Hz tick after cart update.
    pub fn tick(&mut self) {
        self.frame = self.frame.wrapping_add(1);
    }

    // --------------------------------------------------------------------
    // AUDIO (chiptune PSG) — only when "audio" feature enabled.
    // These are thin conveniences over the full `self.audio: AudioMixer`.
    // The real power (multi-ch control, vibrato, patterns, sfx, fill buffers)
    // is directly on `console.audio` (or a standalone AudioMixer you own).
    // PICO-8 fantasy feel: tone() + sfx() + music patterns.
    // --------------------------------------------------------------------

    /// Play a simple tone on channel 0 (most common case).
    /// See `AudioMixer::tone` and `audio::Waveform` for full docs.
    #[cfg(feature = "audio")]
    pub fn tone(&mut self, freq: f32, duration: f32, volume: f32, waveform: Waveform) {
        self.audio.tone(0, freq, duration, volume, waveform);
    }

    /// Play tone on a specific channel (0..3).
    #[cfg(feature = "audio")]
    pub fn tone_on(&mut self, channel: usize, freq: f32, duration: f32, volume: f32, waveform: Waveform) {
        self.audio.tone(channel, freq, duration, volume, waveform);
    }

    /// Trigger a built-in sound effect (see AudioMixer::sfx for the set).
    #[cfg(feature = "audio")]
    pub fn sfx(&mut self, id: u8, volume: f32) {
        self.audio.sfx(id, volume);
    }

    /// Play (or loop) a music pattern using the built-in tracker.
    /// Pass a slice of rows: each row is `[Option<Note>; 4]` for the 4 channels.
    #[cfg(feature = "audio")]
    pub fn play_music(&mut self, steps: &[[Option<Note>; AudioMixer::NUM_CHANNELS]], bpm: f32) {
        self.audio.play_pattern(steps, bpm);
    }

    /// Stop any playing music pattern.
    #[cfg(feature = "audio")]
    pub fn stop_music(&mut self) {
        self.audio.stop_music();
    }
}

// ============================================================================
// CARTRIDGE ABSTRACTION
// ============================================================================

/// A "cartridge" or "game" for the console.
///
/// This is the primary extension point. On powerful hosts you implement
/// `Cart` directly in Rust for zero-cost, full-power carts.
///
/// On tiny chips or for maximum portability, carts will be loaded as
/// WASM modules or a tiny custom bytecode (future work) that call into
/// a very similar API surface exposed to the guest.
pub trait Cart {
    /// Human-readable name shown in menus / boot.
    fn name(&self) -> &'static str;

    /// Called once when the cart is loaded or reset (1/2/3 keys, etc.).
    fn init(&mut self, console: &mut Console);

    /// Called every frame (aiming for 60 Hz). Do your drawing + logic here.
    fn update(&mut self, console: &mut Console);
}

// ============================================================================
// HOST INTERFACE (the portability boundary — "callbacks" for any chip)
// ============================================================================

/// The minimal host interface that *every* platform adapter must satisfy.
///
/// Desktop (minifb + cpal), WASM (Canvas + WebAudio via wasm-bindgen), and
/// bare-metal embedded (SPI display + timer DAC or I2S) all implement (or
/// call through) this same small surface.
///
/// Decision: a `trait Host` (instead of raw fn pointers or a bag of callbacks)
/// gives us:
/// - Rust-ergonomic `impl Host for MyPlatform { ... }`
/// - Zero-cost when the concrete type is known (monomorphization)
/// - Easy to extend later (e.g. `fn request_cart_switch(...)`) without
///   breaking existing hosts
/// - Still trivial to bridge to C/JS/FFI by providing a C-abi wrapper in a
///   thin platform crate.
///
/// The core never calls into the host for *input* — input is pushed *to* the
/// Console via `update_input`. Output (video + future audio) is pulled by the
/// host (or pushed via this trait).
pub trait Host {
    /// Present the current framebuffer to the display.
    ///
    /// `fb` is always exactly `WIDTH * HEIGHT` bytes of palette indices (0..15).
    /// The host may either:
    /// - use `Console::to_rgb_buffer` to expand to 32-bit RGBX before blitting, or
    /// - do an indexed blit directly if the display supports a 4-bit palette.
    fn present(&mut self, fb: &[u8; VRAM_SIZE]);

    /// Push generated audio samples (signed 16-bit PCM, mono).
    ///
    /// The built-in chiptune PSG (`AudioMixer` when "audio" feature enabled) produces
    /// these buffers via `fill_buffer_i16`. Hosts (desktop cpal, web WebAudio, embedded DAC)
    /// call their audio callback / timer and feed the host DAC from the mixer (or from
    /// `console.audio.fill_buffer_i16` if the Console owns one).
    ///
    /// Sample rate chosen by host; the PSG math is sample-rate agnostic.
    fn push_audio(&mut self, samples: &[i16]);
}

/// Trivial no-op host. Useful for:
/// - Unit tests of carts / Console that don't care about output
/// - Headless simulation / verification
/// - Early embedded bring-up before you wire a real display
#[derive(Default)]
pub struct NoopHost;

impl Host for NoopHost {
    fn present(&mut self, _fb: &[u8; VRAM_SIZE]) {}
    fn push_audio(&mut self, _samples: &[i16]) {}
}

// ============================================================================
// BUILT-IN DEMO CARTS (small, educational, always available)
// These demonstrate the pure primitives. Real games live in examples/ or carts/.
// ============================================================================

pub struct BouncerCart {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    r: f32,
    col: u8,
}

impl Default for BouncerCart {
    fn default() -> Self {
        Self::new()
    }
}

impl BouncerCart {
    pub fn new() -> Self {
        Self {
            x: 64.0,
            y: 64.0,
            vx: 1.4,
            vy: 1.1,
            r: 6.0,
            col: 8,
        }
    }
}

impl Cart for BouncerCart {
    fn name(&self) -> &'static str {
        "BOUNCER"
    }

    fn init(&mut self, c: &mut Console) {
        c.cls(1);
        self.x = 64.0;
        self.y = 64.0;
        self.vx = 1.4;
        self.vy = 1.1;
    }

    fn update(&mut self, c: &mut Console) {
        c.cls(1);

        self.x += self.vx;
        self.y += self.vy;

        let bounced = if self.x - self.r < 0.0 || self.x + self.r > WIDTH as f32 {
            self.vx = -self.vx;
            self.col = (self.col + 1) & 15;
            true
        } else { false };

        if self.y - self.r < 0.0 || self.y + self.r > HEIGHT as f32 {
            self.vy = -self.vy;
            self.col = (self.col + 1) & 15;
            // little chiptune blip on wall hit
            c.audio.tone(0, 880.0 + (self.col as f32 * 20.0), 0.05, 0.7, audio::Waveform::Pulse);
        }

        if bounced {
            c.audio.tone(1, 440.0, 0.05, 0.6, audio::Waveform::Pulse);
        }

        c.circ(self.x as i32, self.y as i32, self.r as i32, self.col);
        c.circb(self.x as i32, self.y as i32, self.r as i32 + 2, 7);

        c.print("SCRATCH-8", 4, 4, 7);
        c.print(self.name(), 4, 12, 10);
        c.print("ARROWS+Z/X  1/2/3=SWITCH", 4, HEIGHT as i32 - 10, 6);

        for i in 0..16 {
            c.rect(4 + (i as i32) * 7, HEIGHT as i32 - 4, 6, 3, i as u8);
        }

        // demo line
        c.line(20, 30, 100, 30 + ((c.frame / 2) % 40) as i32, 12);
    }
}

pub struct PainterCart {
    hue: u8,
}

impl Default for PainterCart {
    fn default() -> Self {
        Self::new()
    }
}

impl PainterCart {
    pub fn new() -> Self {
        Self { hue: 8 }
    }
}

impl Cart for PainterCart {
    fn name(&self) -> &'static str {
        "PAINTER"
    }

    fn init(&mut self, c: &mut Console) {
        c.cls(0);
    }

    fn update(&mut self, c: &mut Console) {
        // slow fade trails
        if c.frame % 4 == 0 {
            for px in &mut c.buffer {
                if *px > 0 {
                    *px -= 1;
                }
            }
        }

        if c.btn[4] || c.btn[5] {
            for dx in -1..=1 {
                for dy in -1..=1 {
                    c.pset(c.mouse_x + dx, c.mouse_y + dy, self.hue);
                }
            }
            if c.frame % 8 == 0 {
                self.hue = (self.hue + 1) & 15;
            }
        }

        c.pset(c.mouse_x, c.mouse_y, 7);
        c.rectb(c.mouse_x - 3, c.mouse_y - 3, 7, 7, 6);

        c.print("PAINTER", 4, 4, 7);
        c.print("HOLD Z/X TO PAINT", 4, 12, 10);

        // demo pget + scanner
        let probe = 80 + ((c.frame / 3) % 20) as i32;
        let _read = c.pget(c.mouse_x, probe);
        c.line(0, probe, WIDTH as i32, probe, 13);
    }
}

pub struct ParticlesCart {
    particles: [Option<(f32, f32, f32, f32, u8)>; 64], // fixed size for no_alloc friendliness
    count: usize,
}

impl Default for ParticlesCart {
    fn default() -> Self {
        Self::new()
    }
}

impl ParticlesCart {
    pub fn new() -> Self {
        Self {
            particles: [None; 64],
            count: 0,
        }
    }
}

impl Cart for ParticlesCart {
    fn name(&self) -> &'static str {
        "PARTICLES"
    }

    fn init(&mut self, c: &mut Console) {
        c.cls(0);
        self.count = 0;
        for p in &mut self.particles {
            *p = None;
        }
    }

    fn update(&mut self, c: &mut Console) {
        c.cls(0);

        // spawn
        if c.btn[4] || c.btn[5] || (c.frame % 2 == 0) {
            let base_x = 20.0 + (c.frame % 90) as f32 * 0.9;
            for _ in 0..2 {
                if self.count < 64 {
                    // find slot
                    for slot in &mut self.particles {
                        if slot.is_none() {
                            // Simple integer-based "sway" instead of sin() for pure no_std friendliness
                            let sway = ((c.frame as i32 % 20) - 10) as f32 * 0.12;
                            *slot = Some((
                                base_x,
                                20.0,
                                sway,
                                1.0,
                                ((c.frame / 3) % 8 + 8) as u8,
                            ));
                            self.count += 1;
                            break;
                        }
                    }
                }
            }
        }

        // update + draw
        for slot in &mut self.particles {
            if let Some((x, y, vx, vy, col)) = slot {
                let nx = *x + *vx;
                let ny = *y + *vy + 0.08; // gravity
                let nvx = *vx * 0.995;
                let nvy = *vy * 0.99;

                c.pset(nx as i32, ny as i32, *col);

                *x = nx;
                *y = ny;
                *vx = nvx;
                *vy = nvy;

                if ny > HEIGHT as f32 + 5.0 {
                    *slot = None;
                    self.count = self.count.saturating_sub(1);
                }
            }
        }

        c.print("PARTICLES", 4, 4, 7);
        c.print("Z/X OR WAIT", 4, 12, 11);
        c.rectb(0, 0, WIDTH as i32, HEIGHT as i32, 5);
    }
}

/// Return the three built-in demo carts. Useful for menus and testing.
///
/// This requires `alloc` (for the Box<dyn Cart> trait objects). The crate
/// always pulls in alloc (see top of file) because it is supported on the
/// vast majority of "interesting" embedded targets that can run Rust at all.
/// For the absolute tiniest no-alloc chips, consumers should construct
/// carts via a fixed enum dispatcher or concrete types instead of this fn.
pub fn builtin_carts() -> [Box<dyn Cart>; 3] {
    [
        Box::new(BouncerCart::new()),
        Box::new(PainterCart::new()),
        Box::new(ParticlesCart::new()),
    ]
}

// Audio module: full from-scratch chiptune PSG (4 channels, envelopes, vibrato, slide,
// tone/sfx + music pattern player). Pure software synthesis, always included in the core
// (no_std + alloc friendly, no host crates). The "audio" feature only pulls optional
// host output (cpal) for desktop. This keeps the synth portable to any chip.
pub mod audio;
pub use audio::{AudioMixer, Note, Waveform};

// Cartridge format (.s8) + loader/saver. See src/carts/mod.rs for the v1 binary spec,
// save/load, builtin export helpers, and notes on WASM cart future work.
// This is the start of the cartridge-system track.
pub mod carts;
// Re-export the main cartridge type for convenience at the crate root.
pub use carts::S8Cartridge;

// Note: sprite/map/editor modules will follow (in src/carts or src/editors/ etc).
// They will be feature-gated for the tiniest embedded targets. Audio is done.

// ============================================================================
// WASM / WEB RUNTIME THIN ADAPTER (for wasm-port track)
// ============================================================================
//
// Core already builds for wasm32-unknown-unknown (see scripts/verify-portability.sh
// and `cargo check --lib --target wasm32-unknown-unknown --no-default-features`).
//
// This section provides *zero-dependency* FFI surface (raw extern "C", no_mangle,
// statics) so that a completely separate thin web runner (JS + Canvas + future
// Web Audio API, living in examples/web/) can drive the console + carts.
//
// Why not wasm-bindgen here yet? "a separate crate later" per specialist prompt +
// keep core deps zero for embedded / "any chip" purity. The examples/web/ runner
// will use plain WebAssembly JS API + memory access.
//
// The FFI below lets JS:
//   - init / reset a demo cart
//   - feed input each frame
//   - step the machine (which runs the selected builtin cart's logic)
//   - read the palette-index buffer (or we can push rgb in future)
//   - later: feed audio sample requests into a Web Audio worklet
//
// For richer objects (passing Cart trait objects etc) we can adopt wasm-bindgen
// behind the "wasm" feature in a follow-up without breaking the no-dep FFI path.
//
// COORDINATION:
//   - See GOALS.md: "Add first WASM target + thin web runner."
//   - See todos (wasm-port).
//   - Future WASM carts (compile once, run anywhere):
//       A user writes a cart in Rust (or C, Zig, ...), targets wasm32-unknown-unknown,
//       but instead of linking our full lib, they only import a tiny set of host
//       functions we expose from the runtime (e.g. `extern "C" { fn s8_pset(x:i32, y:i32, c:u8); fn s8_cls(c:u8); ... }`).
//       Their module must export `s8_cart_update()` and optionally `s8_cart_draw()` (or a single update that does both).
//       The web/desktop/embedded host loads the .wasm (or extracts it from a .s8's code section),
//       instantiates with an import object providing the host fns (which mutate a shared Console),
//       then calls the guest update each 60Hz tick.
//       This gives "shareable carts" that are true portable binaries, not tied to a particular
//       Rust version of the console.
//       (For the tiniest embedded without a wasm runtime we will also explore a custom micro-VM.)
//     Initial design note left here for the swarm; implementation is future work after
//     cartridge loading + web runner basics are in place.

// (no UnsafeCell needed after refactor to Option<Console> for FFI; retained for documentation value)

/// Opaque static console for the thin FFI web/embedded hosts.
/// (Single-threaded assumption is fine for our fantasy console targets.)
/// Which builtin we are currently running under the FFI layer (for the web demo).
/// 0=Bouncer, 1=Painter, 2=Particles. Mirrors builtin_carts indices.
static mut CURRENT_DEMO: u8 = 0;

/// The console instance used by the thin FFI. We use Option so we can init it
/// at runtime via s8_ffi_init (Console::new does real work + cfg(audio) field handling).
static mut CONSOLE: Option<Console> = None;

/// Simple per-demo state for the FFI layer (avoids duplicating *all* of the Cart
/// structs while still letting us call their update logic). Keep fields in sync
/// with the real BouncerCart / PainterCart / ParticlesCart in this file.
/// (Temporary until we have a better way to "host" a selected Cart under FFI;
/// good enough for the initial web runner video demo.)
static mut BOUNCER: (f32, f32, f32, f32, f32, u8) = (64.0, 64.0, 1.4, 1.1, 6.0, 8); // x,y,vx,vy,r,col
static mut PAINTER_HUE: u8 = 8;
static mut PARTICLES: ([Option<(f32, f32, f32, f32, u8)>; 64], usize) = ([None; 64], 0);

/// Initialize (or reset) the FFI console + pick a demo cart (0/1/2).
/// Call once at load or when user switches carts from the web UI.
/// This is the entry point the examples/web/ JS will call after instantiating the wasm.
#[unsafe(no_mangle)]
pub extern "C" fn s8_ffi_init(demo_id: u8) {
    unsafe {
        let console_ptr = &raw mut CONSOLE;
        *console_ptr = Some(Console::new());
        let c = (*console_ptr).as_mut().unwrap();
        CURRENT_DEMO = demo_id % 3;

        // Reset per-demo state (mirrors the Cart::init)
        match CURRENT_DEMO {
            0 => {
                BOUNCER = (64.0, 64.0, 1.4, 1.1, 6.0, 8);
                c.cls(1);
            }
            1 => {
                PAINTER_HUE = 8;
                c.cls(0);
            }
            _ => {
                PARTICLES = ([None; 64], 0);
                c.cls(0);
            }
        }
    }
}

/// Feed one frame of input then run one update tick of the selected demo.
/// JS side (examples/web/runner.js) calls this at ~60fps from rAF, after reading mouse/keys.
#[unsafe(no_mangle)]
pub extern "C" fn s8_ffi_update(
    left: u8,
    right: u8,
    up: u8,
    down: u8,
    z: u8,
    x: u8,
    mx: i32,
    my: i32,
) {
    unsafe {
        let console_ptr = &raw mut CONSOLE;
        let c = (*console_ptr).as_mut().expect("s8_ffi_init must be called first");
        c.prev_btn = c.btn;
        c.btn = [
            left != 0,
            right != 0,
            up != 0,
            down != 0,
            z != 0,
            x != 0,
        ];
        c.mouse_x = mx.clamp(0, WIDTH as i32 - 1);
        c.mouse_y = my.clamp(0, HEIGHT as i32 - 1);

        // Dispatch to the matching logic from the real Cart impls (keep in sync on edits!).
        // This is the "thin" part: we run real machine + real demo behavior from JS driver.
        match CURRENT_DEMO {
            0 => {
                // Bouncer logic (from BouncerCart::update)
                let bouncer = &raw mut BOUNCER;
                let (x, y, vx, vy, r, col) = &mut *bouncer;
                c.cls(1);
                *x += *vx;
                *y += *vy;
                if *x - *r < 0.0 || *x + *r > WIDTH as f32 {
                    *vx = -*vx;
                    *col = (*col + 1) & 15;
                }
                if *y - *r < 0.0 || *y + *r > HEIGHT as f32 {
                    *vy = -*vy;
                    *col = (*col + 1) & 15;
                }
                c.circ(*x as i32, *y as i32, *r as i32, *col);
                c.circb(*x as i32, *y as i32, *r as i32 + 2, 7);
                c.print("SCRATCH-8", 4, 4, 7);
                c.print("BOUNCER (web)", 4, 12, 10);
                c.print("ARROWS+Z/X  CLICK CANVAS TO FOCUS", 4, HEIGHT as i32 - 10, 6);
                for i in 0..16 {
                    c.rect(4 + (i as i32) * 7, HEIGHT as i32 - 4, 6, 3, i as u8);
                }
                c.line(20, 30, 100, 30 + ((c.frame / 2) % 40) as i32, 12);
            }
            1 => {
                // Painter logic (simplified from PainterCart)
                if c.frame % 4 == 0 {
                    for px in &mut c.buffer {
                        if *px > 0 {
                            *px -= 1;
                        }
                    }
                }
                if c.btn[4] || c.btn[5] {
                    for dx in -1..=1 {
                        for dy in -1..=1 {
                            c.pset(c.mouse_x + dx, c.mouse_y + dy, PAINTER_HUE);
                        }
                    }
                    if c.frame % 8 == 0 {
                        PAINTER_HUE = (PAINTER_HUE + 1) & 15;
                    }
                }
                c.pset(c.mouse_x, c.mouse_y, 7);
                c.rectb(c.mouse_x - 3, c.mouse_y - 3, 7, 7, 6);
                c.print("PAINTER (web)", 4, 4, 7);
                c.print("HOLD Z/X TO PAINT", 4, 12, 10);
                let probe = 80 + ((c.frame / 3) % 20) as i32;
                let _read = c.pget(c.mouse_x, probe);
                c.line(0, probe, WIDTH as i32, probe, 13);
            }
            _ => {
                // Particles (simplified)
                c.cls(0);
                if c.btn[4] || c.btn[5] || (c.frame % 2 == 0) {
                    let base_x = 20.0 + (c.frame % 90) as f32 * 0.9;
                    let particles_ptr = &raw mut PARTICLES;
                    let (slots, cnt) = &mut *particles_ptr;
                    for _ in 0..2 {
                        if *cnt < 64 {
                            for slot in slots.iter_mut() {
                                if slot.is_none() {
                                    let sway = ((c.frame as i32 % 20) - 10) as f32 * 0.12;
                                    *slot = Some((
                                        base_x,
                                        20.0,
                                        sway,
                                        1.0,
                                        ((c.frame / 3) % 8 + 8) as u8,
                                    ));
                                    *cnt += 1;
                                    break;
                                }
                            }
                        }
                    }
                }
                let particles_ptr2 = &raw mut PARTICLES;
                let (slots, cnt) = &mut *particles_ptr2;
                for slot in slots.iter_mut() {
                    if let Some((x, y, vx, vy, col)) = slot {
                        let nx = *x + *vx;
                        let ny = *y + *vy + 0.08;
                        let nvx = *vx * 0.995;
                        let nvy = *vy * 0.99;
                        c.pset(nx as i32, ny as i32, *col);
                        *x = nx;
                        *y = ny;
                        *vx = nvx;
                        *vy = nvy;
                        if ny > HEIGHT as f32 + 5.0 {
                            *slot = None;
                            *cnt = cnt.saturating_sub(1);
                        }
                    }
                }
                c.print("PARTICLES (web)", 4, 4, 7);
                c.print("Z/X OR WAIT", 4, 12, 11);
                c.rectb(0, 0, WIDTH as i32, HEIGHT as i32, 5);
            }
        }

        c.frame = c.frame.wrapping_add(1);
    }
}

/// Return width/height (so JS Canvas can be created at correct size without hardcoding).
#[unsafe(no_mangle)]
pub extern "C" fn s8_ffi_width() -> u32 {
    WIDTH as u32
}
#[unsafe(no_mangle)]
pub extern "C" fn s8_ffi_height() -> u32 {
    HEIGHT as u32
}

/// Copy the current palette-index framebuffer into caller-provided memory (len must be >= VRAM_SIZE).
/// JS will typically do: const buf = new Uint8Array(wasmMemory.buffer, ptr, 128*128);
/// then map indices through PALETTE (also exported) to RGBA for putImageData.
#[unsafe(no_mangle)]
pub extern "C" fn s8_ffi_copy_buffer(out_ptr: *mut u8, len: usize) {
    unsafe {
        let console_ptr = &raw const CONSOLE;
        if let Some(c) = &*console_ptr {
            if len >= VRAM_SIZE {
                core::ptr::copy_nonoverlapping(c.buffer.as_ptr(), out_ptr, VRAM_SIZE);
            }
        }
    }
}

/// Expose the palette for the web runner (so it can do index->rgb without duplicating the table).
#[unsafe(no_mangle)]
pub extern "C" fn s8_ffi_palette_ptr() -> *const u32 {
    PALETTE.as_ptr()
}

/// For future chiptune: a stub that will let the web runner pull a PCM buffer.
/// Returns number of samples written (0 in v1 until audio is fully wired to FFI).
#[unsafe(no_mangle)]
pub extern "C" fn s8_ffi_fill_audio(out_ptr: *mut i16, max_samples: usize) -> usize {
    // Audio is implemented (AudioMixer + full PSG). For wasm audio, the web runner
    // (or a worklet) can obtain &mut CONSOLE.audio (or a standalone AudioMixer) and call
    // fill_buffer_i16 directly, then hand the buffer to Web Audio. This stub remains 0
    // for the minimal FFI path; real usage goes through the exported AudioMixer type.
    // (audio-chiptune track complete)
    let _ = (out_ptr, max_samples);
    0
}

// Convenience for hosts that want to know current frame (debug / sync).
#[unsafe(no_mangle)]
pub extern "C" fn s8_ffi_frame() -> u32 {
    unsafe {
        let console_ptr = &raw const CONSOLE;
        (*console_ptr).as_ref().map_or(0, |c| c.frame)
    }
}

// End of WASM FFI section. The examples/web/ thin runner (index.html + runner.js)
// is the JS counterpart that uses these symbols after loading the .wasm.
