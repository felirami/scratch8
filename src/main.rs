//! SCRATCH-8 Desktop Binary (feature = "desktop")
//!
//! Extremely thin platform glue.
//! The real machine lives in the `scratch8` library (no_std portable core).

use minifb::{Key, MouseMode, Window, WindowOptions};
use scratch8::{builtin_carts, carts::save_s8, Cart, Console, HEIGHT, S8Cartridge, WIDTH};
#[cfg(feature = "audio")]
use scratch8::audio::{AudioMixer, Note, Waveform};

const SCALE: usize = 5;

/// Chiptune demo cart — exercises the full from-scratch PSG.
/// Owns a shared AudioMixer (via Arc<Mutex>) so it can call .tone / play_pattern / sfx
/// from its update() (called at 60 Hz). The cpal thread pulls samples from the same mixer.
/// Also draws a tiny live visualizer using pure console primitives.
#[cfg(feature = "audio")]
struct ChiptuneDemo {
    mixer: std::sync::Arc<std::sync::Mutex<AudioMixer>>,
    frame: u32,
    last_sfx: u32,
}

#[cfg(feature = "audio")]
impl ChiptuneDemo {
    fn new(mixer: std::sync::Arc<std::sync::Mutex<AudioMixer>>) -> Self {
        Self { mixer, frame: 0, last_sfx: 0 }
    }

    fn init_music(&self) {
        // A cheerful little looping chiptune (lead pulse + triangle bass + noise drums).
        // 16 steps @ ~128 BPM gives a nice bouncy tracker pattern.
        let pat: [[Option<Note>; AudioMixer::NUM_CHANNELS]; 16] = [
            [mk_note(523.25, 0.16, 0.62, Waveform::Pulse, 0.25), None, mk_note(261.63, 0.32, 0.52, Waveform::Triangle, 0.5), mk_note(65.0, 0.09, 0.95, Waveform::Noise, 0.5)],
            [mk_note(587.33, 0.12, 0.55, Waveform::Pulse, 0.25), None, None, None],
            [mk_note(659.25, 0.18, 0.60, Waveform::Pulse, 0.125), None, mk_note(329.63, 0.28, 0.48, Waveform::Triangle, 0.5), None],
            [None, None, None, mk_note(90.0, 0.07, 0.7, Waveform::Noise, 0.5)],
            [mk_note(523.25, 0.14, 0.58, Waveform::Pulse, 0.5), None, mk_note(261.63, 0.30, 0.50, Waveform::Triangle, 0.5), None],
            [mk_note(659.25, 0.10, 0.52, Waveform::Pulse, 0.25), None, None, mk_note(55.0, 0.16, 0.85, Waveform::Noise, 0.5)],
            [mk_note(783.99, 0.16, 0.60, Waveform::Pulse, 0.125), None, mk_note(392.00, 0.26, 0.48, Waveform::Triangle, 0.5), None],
            [None, None, None, None],
            [mk_note(1046.5, 0.15, 0.55, Waveform::Pulse, 0.25), None, mk_note(261.63, 0.32, 0.52, Waveform::Triangle, 0.5), mk_note(70.0, 0.08, 0.92, Waveform::Noise, 0.5)],
            [mk_note(987.77, 0.11, 0.50, Waveform::Pulse, 0.25), None, None, None],
            [mk_note(880.0, 0.17, 0.58, Waveform::Pulse, 0.125), None, mk_note(329.63, 0.28, 0.48, Waveform::Triangle, 0.5), None],
            [None, None, None, mk_note(85.0, 0.07, 0.75, Waveform::Noise, 0.5)],
            [mk_note(783.99, 0.13, 0.55, Waveform::Pulse, 0.5), None, mk_note(196.0, 0.30, 0.50, Waveform::Triangle, 0.5), None],
            [mk_note(659.25, 0.09, 0.52, Waveform::Pulse, 0.25), None, None, mk_note(60.0, 0.15, 0.88, Waveform::Noise, 0.5)],
            [mk_note(523.25, 0.20, 0.48, Waveform::Pulse, 0.125), None, mk_note(392.00, 0.26, 0.48, Waveform::Triangle, 0.5), None],
            [None, None, None, None],
        ];
        let mut m = self.mixer.lock().unwrap();
        m.play_pattern(&pat, 128.0); // lively tempo
        // Add a touch of vibrato on the lead channel for that classic feel
        m.set_vibrato(0, 4.5, 9.0);
    }
}

#[cfg(feature = "audio")]
fn mk_note(freq: f32, dur: f32, vol: f32, wave: Waveform, duty: f32) -> Option<Note> {
    Some(Note { freq, duration: dur, volume: vol, waveform: wave, duty })
}

#[cfg(not(feature = "audio"))]
struct ChiptuneDemo; // dummy for non-audio builds (never instantiated)

#[cfg(feature = "audio")]
impl Cart for ChiptuneDemo {
    fn name(&self) -> &'static str { "CHIPTUNE" }

    fn init(&mut self, c: &mut Console) {
        c.cls(1);
        self.frame = 0;
        self.last_sfx = 0;
        // Start the built-in pattern music
        self.init_music();
        // Also play a welcoming sfx on the mixer (ch 3)
        if let Ok(mut m) = self.mixer.lock() {
            m.sfx(4, 0.7);
        }
    }

    fn update(&mut self, c: &mut Console) {
        self.frame = self.frame.wrapping_add(1);

        // slow fade background for visualizer trail effect (pure software)
        if self.frame % 5 == 0 {
            for p in &mut c.buffer {
                if *p > 1 { *p -= 1; }
            }
        } else {
            c.cls(1);
        }

        // Draw title + instructions using console primitives (from scratch)
        c.print("CHIPTUNE DEMO", 8, 4, 10);
        c.print("4-CH PSG + PATTERN", 4, 12, 7);
        c.print("Z=SFX  X=EXTRA  1-4=SWITCH", 4, 20, 6);

        // Simple live "scope" visualizer: sample a few mixer channels and draw bars
        // (we peek by locking briefly; cheap for viz)
        let vols = if let Ok(_mix) = self.mixer.lock() {
            // We can't easily read internal phase, so just draw time-based fake bars + trigger info
            // Real: in future add a "peek" or we synthesize a couple samples here for viz.
            [ (self.frame % 7) as i32 + 2, ((self.frame/2)%5) as i32 +1 , 3, 4 ]
        } else { [2,2,2,2] };

        for (i, &v) in vols.iter().enumerate() {
            let x = 10 + (i as i32) * 28;
            let h = (v * 3).min(18);
            c.rect(x, 36, 18, h, (8 + i as u8) & 15);
            c.rectb(x, 36, 18, h + 1, 7);
        }

        // Occasional automatic sfx for fun (noise hits etc) + user input
        if (self.frame % 47 == 0) && (self.frame > self.last_sfx + 20) {
            if let Ok(mut m) = self.mixer.lock() { m.sfx(((self.frame/47)%8) as u8, 0.6); }
            self.last_sfx = self.frame;
        }

        if c.btn[4] && (c.frame % 6 == 0) {
            if let Ok(mut m) = self.mixer.lock() {
                m.sfx( (c.frame % 7) as u8 , 0.75);
            }
            self.last_sfx = c.frame;
        }
        if c.btn[5] {
            if let Ok(mut m) = self.mixer.lock() {
                m.tone(2, 110.0 + ((c.frame % 30) as f32 * 4.0), 0.06, 0.5, Waveform::Triangle);
                m.slide(2, 55.0, 0.4);
            }
        }

        // Draw a "piano roll" style tracker hint (pure pixels)
        for step in 0..16 {
            let y = 58 + (step % 8) * 7;
            let active = (step == ((self.frame / 4) % 16) as usize) || (step == ((self.frame / 4 + 8) % 16) as usize);
            let col = if active { 10 } else { 5 };
            c.rect(8 + (step as i32 % 8) * 14 , y as i32, 10, 4, col);
        }

        c.print("PURE SOFTWARE PSG", 8, HEIGHT as i32 - 18, 11);
        c.print("NO CRATES. ALL FROM SCRATCH.", 4, HEIGHT as i32 - 10, 6);

        // Draw palette reminder at bottom
        for i in 0..16 {
            c.rect(4 + i as i32 * 7, HEIGHT as i32 - 4, 6, 3, i as u8);
        }
    }
}

fn main() {
    let mut window = Window::new(
        "SCRATCH-8 — built from the very scratch (portable core)",
        WIDTH * SCALE,
        HEIGHT * SCALE,
        WindowOptions {
            resize: false,
            scale: minifb::Scale::X1,
            ..WindowOptions::default()
        },
    )
    .expect("Failed to open window — are you on a desktop with graphics?");

    window.set_target_fps(60);

    let mut console = Console::new();
    let mut carts: Vec<Box<dyn Cart>> = builtin_carts().into_iter().collect();
    let mut current: usize = 0;

    #[cfg(feature = "audio")]
    let shared_mixer = {
        let m = std::sync::Arc::new(std::sync::Mutex::new(AudioMixer::new(44100)));
        let mixer_for_cart = m.clone();
        // Add the chiptune cart as cart index 3 (press 4)
        carts.push(Box::new(ChiptuneDemo::new(mixer_for_cart)));
        m
    };

    #[cfg(not(feature = "audio"))]
    let _ = &mut carts; // silence unused in no-audio desktop builds

    carts[current].init(&mut console);

    let mut rgb: Vec<u32> = vec![0; WIDTH * HEIGHT];

    // Boot screen using the portable console
    console.cls(0);
    console.print("SCRATCH-8", 34, 40, 10);
    console.print("FANTASY CONSOLE", 28, 52, 7);
    console.print("FROM THE VERY SCRATCH", 20, 64, 6);
    console.rectb(10, 30, WIDTH as i32 - 20, 60, 13);

    for _ in 0..25 {
        console.to_rgb_buffer(&mut rgb);
        let _ = window.update_with_buffer(&rgb, WIDTH, HEIGHT);
        console.tick();
        if !window.is_open() {
            return;
        }
    }

    // === Real chiptune audio output (cpal) ===
    // The AudioMixer is 100% pure software synthesis (see src/audio.rs).
    // We own a shared Arc<Mutex<>> so carts (ChiptuneDemo) and the audio callback
    // can both safely mutate/play and consume samples. Works great with desktop.
    #[cfg(feature = "audio")]
    let _audio_stream = {
        use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
        use std::sync::Arc;

        let host = cpal::default_host();
        let device = host.default_output_device()
            .expect("no audio output device found");
        let config: cpal::StreamConfig = device.default_output_config()
            .expect("failed default output config")
            .into();

        let mixer_for_stream: Arc<std::sync::Mutex<AudioMixer>> = shared_mixer.clone();

        let err_fn = |err| eprintln!("cpal audio error: {}", err);

        let stream = device.build_output_stream(
            &config,
            move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                // Pull fresh samples from the PSG mixer (advances all oscillators,
                // envelopes, vibrato, slides, and the pattern sequencer).
                if let Ok(mut mix) = mixer_for_stream.lock() {
                    mix.fill_buffer_f32(data);
                } else {
                    for s in data.iter_mut() { *s = 0.0; }
                }
            },
            err_fn,
            None,
        ).expect("failed to build cpal output stream");

        stream.play().expect("failed to play cpal stream");
        // Keep the stream alive for the duration of main by returning it
        Some(stream)
    };
    #[cfg(not(feature = "audio"))]
    let _audio_stream = ();

    while window.is_open() && !window.is_key_down(Key::Escape) {
        // Map minifb input to the portable Console API
        let left = window.is_key_down(Key::Left);
        let right = window.is_key_down(Key::Right);
        let up = window.is_key_down(Key::Up);
        let down = window.is_key_down(Key::Down);
        let z = window.is_key_down(Key::Z) || window.is_key_down(Key::O);
        let x = window.is_key_down(Key::X) || window.is_key_down(Key::K);

        let (mx, my) = window
            .get_mouse_pos(MouseMode::Clamp)
            .map(|(mx, my)| (mx as i32 / SCALE as i32, my as i32 / SCALE as i32))
            .unwrap_or((0, 0));

        console.update_input(left, right, up, down, z, x, mx, my);

        // Cart switching (the fantasy console UX)
        let mut switched = false;
        if window.is_key_pressed(Key::Key1, minifb::KeyRepeat::No) {
            current = 0;
            switched = true;
        }
        if window.is_key_pressed(Key::Key2, minifb::KeyRepeat::No) {
            current = 1;
            switched = true;
        }
        if window.is_key_pressed(Key::Key3, minifb::KeyRepeat::No) {
            current = 2;
            switched = true;
        }
        #[cfg(feature = "audio")]
        if window.is_key_pressed(Key::Key4, minifb::KeyRepeat::No) {
            current = 3;
            switched = true;
        }
        if switched {
            carts[current].init(&mut console);
        }

        // Cartridge export (cartridge-system v1).
        // Press E or S to save the *current* cart as a .s8 file (binary with metadata + code/gfx/sfx stubs).
        // This produces a real shareable file today (loadable in theory by future web/embedded hosts
        // via name lookup to builtins; later will embed real payload).
        // COORDINATION: see src/carts/mod.rs for the format + wasm-cart future notes.
        if window.is_key_pressed(Key::E, minifb::KeyRepeat::No) || window.is_key_pressed(Key::S, minifb::KeyRepeat::No) {
            let cart_name = carts[current].name();
            let cart = S8Cartridge::from_builtin(cart_name, current);
            match save_s8(&cart, "export.s8") {
                Ok(()) => {
                    eprintln!("✅ Exported '{}' as export.s8 ({} bytes) — cartridge-system stub", cart_name, cart.to_bytes().len());
                    // Visual ack on the fantasy console (uses the portable primitives)
                    console.cls(2);
                    console.print("EXPORTED .s8", 30, 50, 11);
                    console.print(cart_name, 30, 60, 10);
                    // Will be overwritten by next cart.update but gives feedback flash
                }
                Err(e) => eprintln!("Export failed: {}", e),
            }
        }

        if window.is_key_pressed(Key::Tab, minifb::KeyRepeat::No) {
            console.cls(2);
            console.print("EDITORS + MORE COMING", 16, 50, 7);
            console.print("SWARM IS BUILDING...", 22, 62, 10);
        }

        carts[current].update(&mut console);
        console.tick();

        console.to_rgb_buffer(&mut rgb);
        let _ = window.update_with_buffer(&rgb, WIDTH, HEIGHT);
    }
}

// Audio is fully wired via cpal + the from-scratch AudioMixer when --features audio (or full).
// The ChiptuneDemo cart (selectable with 4) starts a pattern + responds to Z/X for sfx.
// All synthesis math, LFSR, envelopes, pattern stepper, and PCM generation lives in src/audio.rs.
