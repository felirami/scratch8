//! SCRATCH-8 Chiptune / PSG (Programmable Sound Generator) Audio System
//!
//! Complete from-scratch implementation. Pure software sample generation.
//! NO external synth crates, NO host audio code (cpal, etc) in this module.
//!
//! ## Portability
//! - Works in `#![no_std]` + `alloc` contexts (this module uses *zero* alloc or std).
//! - Feature-gated behind "audio" (see Cargo.toml; "audio" does not force "std").
//! - Cross-compiles to thumbv7em-none-eabihf, riscv32, wasm32, desktop, etc.
//! - All math is simple f32 phase accumulators + integer LFSR. Deterministic.
//!
//! ## Design (PSG on paper, implemented here)
//! - **4 channels** (classic fantasy console count; PICO-8-ish, NES/GB inspired).
//!   - Channels 0,1: Pulse / Square waves with *variable duty cycle* (12.5%, 25%, 50%, 75%).
//!   - Channel 2: Triangle wave (smooth for basslines / leads).
//!   - Channel 3: Noise (16-bit LFSR from scratch — authentic chiptune percussion/hiss).
//! - **Oscillator core**: Phase accumulator (0.0..1.0). phase_inc = freq / sample_rate.
//!   Waveforms computed mathematically on the fly each sample (no ROM tables).
//!     Pulse: sign(phase < duty)
//!     Triangle: linear up/down fold
//!     Noise: LFSR clocked at a rate derived from freq (step LFSR when frac overflows).
//! - **Envelopes**: Simple but effective per-channel decay envelope.
//!   - tone() sets env = volume, decay_rate so it reaches ~0 at end of duration.
//!   - On duration expiry: quick release fade.
//!   - Hold notes (duration=0): sustain until stop() or overwritten (decay_rate=0).
//! - **Effects** (per-channel state machines):
//!   - Vibrato: independent triangle LFO phase. Modulates *effective freq* (cheap, chiptune-y).
//!   - Slide / portamento: linear freq ramp toward target (sample-accurate).
//!   - Both advance every sample inside the sample() call.
//! - **Mixer**:
//!   - Sums channel outputs (scaled by env), applies master_vol, clamps.
//!   - `fill_buffer_i16` / `fill_buffer_f32`: advance N samples, write PCM.
//!     This *is* the entire synthesis engine and all state machines.
//!   - Sample rate is parameter to `new(sr)` (22050 or 44100 recommended; any u32 works).
//! - **High-level API** (PICO-8 fantasy console spirit):
//!   - `tone(ch, freq_hz, duration_secs, vol_0_1, waveform)` — immediate play.
//!   - `tone_duty(...)` for precise pulse width.
//!   - `sfx(id, vol)` — 8 built-in one-shot effects (blip, hit, laser, kick, powerup, etc.).
//!   - `play_pattern(steps, bpm)` — multi-voice music pattern player (tracker primitive).
//!     Steps are `[[Option<Note>; 4]; N]` (N<=32). Each row triggers simultaneous notes on channels.
//!     Timing derived from bpm (16th-note-ish steps). Notes carry their own envelope duration.
//!   - Low-level: `stop(ch)`, `set_vibrato(ch, rate_hz, depth_hz)`, `slide(ch, target, time_secs)`.
//! - **Music pattern player**:
//!   - Internal fixed-size storage (no Vec).
//!   - Advances on sample boundaries inside fill (keeps music locked to audio rate, no drift).
//!   - Simple but rich enough for chiptune loops, arps, bass+melody+drums.
//! - **Note struct**: reusable for patterns or future sfx defs.
//!
//! ## Usage (in carts / host)
//! In a Cart::update (60 Hz):
//! ```ignore
//! mixer.tone(0, 440.0, 0.2, 0.6, Waveform::Pulse);
//! mixer.sfx(3, 0.8); // noise kick on ch
//! mixer.set_vibrato(0, 6.0, 8.0);
//! ```
//! In host audio callback (any rate, any thread):
//! ```ignore
//! mixer.fill_buffer_f32(data); // or i16 for push_audio
//! ```
//!
//! The Console (when "audio" feature) also exposes convenience wrappers `c.tone(...)` etc
//! that delegate to its internal `AudioMixer` (great for single-threaded or embedded hosts).
//!
//! ## Authenticity & Limits
//! - No floating point "overkill" beyond phase (perfectly acceptable on Cortex-M4F).
//! - Headroom: master_vol defaults 0.7 to avoid clipping with 4 voices.
//! - LFSR poly: 16-bit with taps 0,2,3,5 (good period and timbre for chiptunes).
//! - Future extension points: full ADSR, duty per note, more waveforms (saw), filters, sfx RAM.
//!
//! Everything here authored from scratch for SCRATCH-8 purity goals.
//! See GOALS.md for the chiptune requirements.

#![allow(clippy::excessive_precision)] // we like readable phase math

use core::f32;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Waveform {
    /// Variable duty square/pulse (use tone_duty or default 50%)
    Pulse,
    /// Classic triangle (great for basses and clean leads)
    Triangle,
    /// LFSR noise — the heartbeat of chiptune percussion
    Noise,
}

impl Default for Waveform {
    fn default() -> Self {
        Waveform::Pulse
    }
}

/// A note descriptor for patterns / music playback.
/// duration is envelope duration in seconds (0 = hold / use pattern step timing only).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Note {
    pub freq: f32,
    pub duration: f32,
    pub volume: f32,
    pub waveform: Waveform,
    /// Pulse duty (0.125 = 12.5%, 0.25, 0.5, 0.75). Ignored for other waves. 0 or >1 uses default 0.5.
    pub duty: f32,
}

impl Default for Note {
    fn default() -> Self {
        Note {
            freq: 440.0,
            duration: 0.1,
            volume: 0.6,
            waveform: Waveform::Pulse,
            duty: 0.5,
        }
    }
}

impl Note {
    pub fn new(freq: f32, duration: f32, volume: f32, waveform: Waveform) -> Self {
        Note { freq, duration, volume, waveform, duty: 0.5 }
    }

    pub fn with_duty(mut self, duty: f32) -> Self {
        self.duty = duty;
        self
    }
}

const NUM_CHANNELS: usize = 4;
const MAX_PATTERN_STEPS: usize = 32;

/// Internal per-channel state machine. All synthesis lives here.
struct Channel {
    // Core oscillator
    freq: f32,
    phase: f32,
    waveform: Waveform,
    duty: f32,

    // Amplitude
    volume: f32, // requested peak
    env: f32,    // current envelope scalar (decays from volume)
    decay_rate: f32,
    remaining: u32, // samples remaining in this note (0 = infinite/hold)

    // Effects state
    vib_phase: f32,
    vib_rate: f32,   // LFO Hz
    vib_depth: f32,  // Hz deviation

    slide_target: f32,
    slide_rate: f32, // Hz per sample

    // Noise LFSR (from scratch, 16-bit)
    lfsr: u16,
    noise_out: f32,
    noise_clock: f32, // fractional accumulator for sub-sample / rate control
}

impl Channel {
    fn new(seed: u16) -> Self {
        Self {
            freq: 0.0,
            phase: 0.0,
            waveform: Waveform::Pulse,
            duty: 0.5,
            volume: 0.0,
            env: 0.0,
            decay_rate: 0.0,
            remaining: 0,
            vib_phase: 0.0,
            vib_rate: 0.0,
            vib_depth: 0.0,
            slide_target: 0.0,
            slide_rate: 0.0,
            lfsr: seed | 1, // never zero
            noise_out: 0.0,
            noise_clock: 0.0,
        }
    }

    /// Trigger a new sound (the heart of tone() / pattern playback).
    fn trigger(&mut self, freq: f32, duration: f32, volume: f32, wave: Waveform, duty: f32, sr: u32) {
        self.freq = freq.max(0.1);
        self.waveform = wave;
        self.duty = if duty > 0.04 && duty < 0.96 { duty } else { 0.5 };
        self.volume = volume.clamp(0.0, 1.0);
        self.phase = 0.0;
        self.vib_phase = 0.0;
        self.slide_target = self.freq;
        self.slide_rate = 0.0;

        let dur_samps = if duration > 0.0 {
            (duration * sr as f32 + 0.5) as u32
        } else {
            0
        };
        self.remaining = dur_samps;

        self.env = self.volume;

        if self.remaining > 1 {
            // Linear decay so we hit near zero at end of note (classic chiptune envelope)
            self.decay_rate = self.env / (self.remaining as f32);
        } else if self.remaining == 0 {
            // Hold / sustained note: no automatic decay (caller manages stop or overwrite)
            self.decay_rate = 0.0;
        } else {
            self.decay_rate = 0.01;
        }

        // Give noise a fresh LFSR seed perturbation for variety
        self.lfsr = self.lfsr.wrapping_add(0x1234).wrapping_mul(3) | 1;
        self.noise_clock = 0.0;
        self.noise_out = 0.0;
    }

    fn stop(&mut self) {
        self.env = 0.0;
        self.volume = 0.0;
        self.freq = 0.0;
        self.remaining = 0;
        self.decay_rate = 0.0;
        self.slide_rate = 0.0;
    }

    fn set_vibrato(&mut self, rate_hz: f32, depth_hz: f32) {
        self.vib_rate = rate_hz.max(0.0);
        self.vib_depth = depth_hz.max(0.0);
    }

    fn start_slide(&mut self, target: f32, slide_secs: f32, sr: u32) {
        self.slide_target = target.max(0.1);
        if slide_secs > 0.001 {
            let steps = slide_secs * sr as f32;
            self.slide_rate = (self.slide_target - self.freq) / steps;
        } else {
            self.freq = self.slide_target;
            self.slide_rate = 0.0;
        }
    }

    /// Clock the LFSR once. Pure integer, from scratch.
    /// Taps give a nice bright noise timbre suitable for drums and wind.
    #[inline]
    fn clock_lfsr(&mut self) {
        // 16-bit LFSR feedback network (taps on bits 0,2,3,5)
        let bit = ((self.lfsr >> 0) & 1)
            ^ ((self.lfsr >> 2) & 1)
            ^ ((self.lfsr >> 3) & 1)
            ^ ((self.lfsr >> 5) & 1);
        self.lfsr = (self.lfsr >> 1) | ((bit & 1) << 15);
        self.noise_out = if (self.lfsr & 1) != 0 { 0.85 } else { -0.85 };
    }

    /// Generate ONE sample for this channel and advance its entire state machine.
    /// Returns value in -1.0 .. 1.0 (before master volume).
    #[inline]
    fn sample(&mut self, sr: u32) -> f32 {
        if self.freq < 1.0 || self.env <= 0.0001 {
            // Advance phase a tiny bit anyway so restart is clean; mostly silent
            self.phase = (self.phase + (20.0 / sr as f32)) % 1.0;
            return 0.0;
        }

        // --- Slide (portamento) ---
        if self.slide_rate != 0.0 {
            self.freq += self.slide_rate;
            if (self.slide_rate > 0.0 && self.freq >= self.slide_target) ||
               (self.slide_rate < 0.0 && self.freq <= self.slide_target) {
                self.freq = self.slide_target;
                self.slide_rate = 0.0;
            }
        }

        // --- Vibrato LFO (triangle wave — pure, cheap, authentic) ---
        let mut eff_freq = self.freq;
        if self.vib_rate > 0.1 && self.vib_depth > 0.01 {
            self.vib_phase = (self.vib_phase + self.vib_rate / sr as f32) % 1.0;
            // Triangle LFO: -1 .. +1
            let lfo = if self.vib_phase < 0.5 {
                self.vib_phase * 4.0 - 1.0
            } else {
                3.0 - self.vib_phase * 4.0
            };
            eff_freq += lfo * self.vib_depth;
            if eff_freq < 1.0 {
                eff_freq = 1.0;
            }
        }

        let phase_inc = eff_freq / sr as f32;

        // --- Waveform generation (the actual PSG voices) ---
        let osc = match self.waveform {
            Waveform::Pulse => {
                if self.phase < self.duty { 1.0 } else { -1.0 }
            }
            Waveform::Triangle => {
                let p = self.phase;
                if p < 0.5 {
                    p * 4.0 - 1.0
                } else {
                    3.0 - p * 4.0
                }
            }
            Waveform::Noise => {
                // Noise clock rate follows frequency (higher freq = faster LFSR stepping)
                self.noise_clock += phase_inc * 1.6; // 1.6 gives pleasing noise pitch range
                while self.noise_clock >= 1.0 {
                    self.clock_lfsr();
                    self.noise_clock -= 1.0;
                }
                self.noise_out
            }
        };

        // Advance phase (mod 1)
        self.phase = (self.phase + phase_inc) % 1.0;

        // --- Envelope decay ---
        if self.decay_rate > 0.0 {
            self.env -= self.decay_rate;
            if self.env < 0.0 {
                self.env = 0.0;
            }
        }

        if self.remaining > 0 {
            self.remaining -= 1;
            if self.remaining == 0 {
                // Auto-release at end of duration: start a quick fade
                self.decay_rate = (self.env * 0.035).max(0.0008);
            }
        }

        osc * self.env
    }
}

/// The complete chiptune audio mixer + sequencer.
///
/// This is the public type carts and hosts interact with.
/// All synthesis, envelopes, effects, and the pattern player are implemented here.
pub struct AudioMixer {
    sample_rate: u32,
    channels: [Channel; NUM_CHANNELS],
    master_vol: f32,

    // === Music pattern player (simple tracker) ===
    pattern_steps: [[Option<Note>; NUM_CHANNELS]; MAX_PATTERN_STEPS],
    pattern_len: usize,
    pattern_step: usize,
    pattern_samples_per_step: u32,
    pattern_samples_left: u32,
    pattern_playing: bool,
}

impl AudioMixer {
    pub const NUM_CHANNELS: usize = NUM_CHANNELS;

    /// Create a new mixer. `sample_rate` can be 22050, 44100, or any reasonable value.
    /// The phase accumulators adapt automatically.
    pub fn new(sample_rate: u32) -> Self {
        let sr = if sample_rate < 4000 { 44100 } else { sample_rate };
        let m = Self {
            sample_rate: sr,
            channels: [
                Channel::new(0xACE1),
                Channel::new(0xBEEF),
                Channel::new(0xCAFE),
                Channel::new(0xF00D),
            ],
            master_vol: 0.68, // conservative headroom — 4 voices can add up
            pattern_steps: [[None; NUM_CHANNELS]; MAX_PATTERN_STEPS],
            pattern_len: 0,
            pattern_step: 0,
            pattern_samples_per_step: sr / 16,
            pattern_samples_left: 0,
            pattern_playing: false,
        };
        m
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// 0.0 = silent, 1.0 = full (values >1.0 allowed for loud carts but risk clipping).
    pub fn set_master_volume(&mut self, v: f32) {
        self.master_vol = v.clamp(0.0, 2.0);
    }

    pub fn master_volume(&self) -> f32 {
        self.master_vol
    }

    /// Hard reset: silence everything, stop music, clear state.
    pub fn reset(&mut self) {
        for (i, ch) in self.channels.iter_mut().enumerate() {
            *ch = Channel::new(0xACE1u16.wrapping_add(i as u16 * 177));
        }
        self.stop_music();
    }

    // ---------------------------------------------------------------------
    // Simple PICO-8-style API
    // ---------------------------------------------------------------------

    /// Play a tone on a channel.
    ///
    /// `freq`: Hz (e.g. 261.6 = C4, 440 = A4). Values <1 treated as silence.
    /// `duration`: seconds. 0.0 means "hold until next tone or stop".
    /// `volume`: 0.0..1.0
    /// `waveform`: Pulse / Triangle / Noise
    pub fn tone(&mut self, ch: usize, freq: f32, duration: f32, volume: f32, waveform: Waveform) {
        if ch >= NUM_CHANNELS {
            return;
        }
        self.channels[ch].trigger(freq, duration, volume, waveform, 0.5, self.sample_rate);
    }

    /// Same as tone but allows explicit duty cycle for Pulse waves (0.125, 0.25, 0.5, 0.75 classic values).
    pub fn tone_duty(&mut self, ch: usize, freq: f32, duration: f32, volume: f32, waveform: Waveform, duty: f32) {
        if ch >= NUM_CHANNELS {
            return;
        }
        self.channels[ch].trigger(freq, duration, volume, waveform, duty, self.sample_rate);
    }

    /// Immediately silence a channel.
    pub fn stop(&mut self, ch: usize) {
        if ch < NUM_CHANNELS {
            self.channels[ch].stop();
        }
    }

    /// Add vibrato (warble) to a channel. rate_hz is LFO speed, depth_hz is max deviation in Hz.
    /// Example: set_vibrato(0, 5.0, 12.0) for a gentle vibrato.
    pub fn set_vibrato(&mut self, ch: usize, rate_hz: f32, depth_hz: f32) {
        if ch < NUM_CHANNELS {
            self.channels[ch].set_vibrato(rate_hz, depth_hz);
        }
    }

    /// Portamento / frequency slide on a channel.
    /// Reaches target over `slide_secs` (linear in Hz).
    pub fn slide(&mut self, ch: usize, target_freq: f32, slide_secs: f32) {
        if ch < NUM_CHANNELS {
            self.channels[ch].start_slide(target_freq, slide_secs, self.sample_rate);
        }
    }

    /// Built-in sound effects. id & 7 gives variety. Perfect for game actions without defining patterns.
    /// Effects often use multiple channels or slide/vib internally for richness.
    pub fn sfx(&mut self, id: u8, volume: f32) {
        let vol = volume.clamp(0.0, 1.0);
        // Prefer sfx on channels 1-3 so ch 0 melody is less likely interrupted
        let ch = ((id as usize) % 3) + 1;

        match id & 7 {
            0 => self.tone(ch, 1318.5, 0.06, vol * 0.85, Waveform::Pulse), // high blip
            1 => {
                // percussive hit: noise + short pulse
                self.tone(ch, 90.0, 0.18, vol, Waveform::Noise);
                self.tone_duty(ch, 180.0, 0.05, vol * 0.7, Waveform::Pulse, 0.25);
            }
            2 => self.tone(ch, 2400.0, 0.09, vol, Waveform::Pulse), // zap / laser
            3 => self.tone(ch, 55.0, 0.22, vol * 1.05, Waveform::Noise), // kick / boom
            4 => {
                // power-up arpeggio feel
                self.tone_duty(ch, 523.25, 0.04, vol, Waveform::Pulse, 0.125);
                self.tone(ch + 1, 659.25, 0.09, vol * 0.65, Waveform::Pulse);
                self.tone(ch, 784.0, 0.14, vol * 0.5, Waveform::Triangle);
            }
            5 => self.tone(ch, 98.0, 0.28, vol, Waveform::Triangle), // subby triangle thump
            6 => {
                // rising whoosh with slide + vibrato
                self.tone(ch, 220.0, 0.22, vol, Waveform::Pulse);
                self.slide(ch, 1760.0, 0.22);
                self.set_vibrato(ch, 9.0, 25.0);
            }
            _ => {
                // metallic noise scrape
                self.tone(ch, 420.0, 0.13, vol * 0.9, Waveform::Noise);
                self.slide(ch, 180.0, 0.11);
            }
        }
    }

    // ---------------------------------------------------------------------
    // Music pattern player (tracker primitive)
    // ---------------------------------------------------------------------

    /// Start playing a multi-channel pattern (the "music" part of the requirement).
    ///
    /// `steps` is a slice of rows. Each row is an array of 4 optional notes (one per channel).
    /// The pattern loops.
    /// `bpm` controls speed (internally we treat steps as 16th notes for groovy chiptune feel).
    ///
    /// Notes in the pattern supply their own `duration` for envelopes; the step timing
    /// controls when the next row fires.
    pub fn play_pattern(&mut self, steps: &[[Option<Note>; NUM_CHANNELS]], bpm: f32) {
        let n = steps.len().min(MAX_PATTERN_STEPS);
        self.pattern_len = n;
        for i in 0..n {
            self.pattern_steps[i] = steps[i];
        }
        for i in n..MAX_PATTERN_STEPS {
            self.pattern_steps[i] = [None; NUM_CHANNELS];
        }
        self.pattern_step = 0;
        self.pattern_playing = n > 0;
        if !self.pattern_playing {
            return;
        }

        let beats_per_sec = (bpm / 60.0).max(0.25);
        // 4 steps per beat = 16th notes
        let steps_per_sec = beats_per_sec * 4.0;
        self.pattern_samples_per_step =
            ((self.sample_rate as f32 / steps_per_sec).max(32.0)) as u32;
        self.pattern_samples_left = 0; // fire first row on next sample
    }

    pub fn stop_music(&mut self) {
        self.pattern_playing = false;
        self.pattern_len = 0;
        self.pattern_step = 0;
        self.pattern_samples_left = 0;
    }

    /// Internal: fire current row (if any) and advance the sequencer.
    /// Called from the sample generation hot path — keeps everything locked to audio clock.
    fn advance_pattern(&mut self) {
        if !self.pattern_playing || self.pattern_len == 0 {
            return;
        }
        if self.pattern_samples_left > 0 {
            self.pattern_samples_left -= 1;
            return;
        }

        let row = &self.pattern_steps[self.pattern_step];
        for (ci, note_opt) in row.iter().enumerate() {
            if let Some(n) = *note_opt {
                let dur = if n.duration > 0.0 { n.duration } else { 0.18 };
                let d = if n.duty > 0.0 { n.duty } else { 0.5 };
                self.channels[ci].trigger(n.freq, dur, n.volume, n.waveform, d, self.sample_rate);
            }
        }

        self.pattern_step = (self.pattern_step + 1) % self.pattern_len;
        self.pattern_samples_left = self.pattern_samples_per_step;
    }

    // ---------------------------------------------------------------------
    // Sample generation (the actual mixer)
    // ---------------------------------------------------------------------

    /// Generate the next mono sample. Also advances pattern + all channel state machines.
    #[inline]
    fn next_sample(&mut self) -> f32 {
        self.advance_pattern();

        let mut mix = 0.0f32;
        for ch in &mut self.channels {
            mix += ch.sample(self.sample_rate);
        }
        mix *= self.master_vol;
        // final safety clamp (prevents any wild accumulation)
        mix.clamp(-1.0, 1.0)
    }

    /// Fill a buffer with signed 16-bit mono PCM samples.
    /// This is the primary "pull" API for hosts (see Host::push_audio).
    /// The buffer length determines how many samples of synthesis are advanced.
    pub fn fill_buffer_i16(&mut self, buf: &mut [i16]) {
        for sample in buf.iter_mut() {
            let v = self.next_sample();
            *sample = (v * 32767.0) as i16;
        }
    }

    /// Fill a buffer with f32 mono samples in -1.0..+1.0 range.
    /// Preferred by many modern audio APIs (cpal, WebAudio, etc).
    pub fn fill_buffer_f32(&mut self, buf: &mut [f32]) {
        for sample in buf.iter_mut() {
            *sample = self.next_sample();
        }
    }
}

// Convenience re-export for users who want a default 44100 mixer.
impl Default for AudioMixer {
    fn default() -> Self {
        AudioMixer::new(44100)
    }
}
