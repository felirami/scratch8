# Your First Cart in 15 Minutes — SCRATCH-8

**Goal**: In about 15 minutes of reading + typing you will have a complete, playable, from-scratch interactive "cart" (game) running inside SCRATCH-8 using only the public `Console` + `Cart` API. No engines, no external drawing crates — pure Rust operating on a raw 128×128×16-color framebuffer.

This is the "from the very scratch" spirit of the console: every pixel comes from your code or the tiny primitives we hand-wrote (Bresenham line, midpoint circle, custom 4×6 font, etc.).

## What You Need

- The SCRATCH-8 repo cloned.
- Rust toolchain (`cargo`).
- (For playing) a desktop that can `cargo run --release --features full`.
- Text editor.

Everything else lives in the portable core.

## The Mental Model (2 minutes)

SCRATCH-8 is a tiny 8-bit fantasy machine:

- **Display**: Exactly 128×128 pixels. You draw by writing palette indices (0–15) into `console.buffer` or (better) by calling the provided methods.
- **Palette**: 16 fixed colors (PICO-8 inspired). See `PALETTE` or `docs/HARDWARE.md`. Color 0 is "background" in many cases.
- **Input**: 6 classic buttons + mouse position. `btn[b]` (held) and `btnp(b)` (pressed this frame).
- **Time**: `console.frame` (u32, increments every 60 Hz tick). Use it for animations.
- **Your code**: A struct holding your game state + `impl Cart for YourStruct`.

The `Cart` trait (defined in the library):

```rust
pub trait Cart {
    fn name(&self) -> &'static str;
    fn init(&mut self, console: &mut Console);
    fn update(&mut self, console: &mut Console);
}
```

- `init`: Called when cart loads or resets (press 1/2/3 in the desktop runner, or when you switch carts).
- `update`: Called every frame (~60×/second). Do **all** your logic + drawing here. Keep it fast — this is an 8-bit machine.

`Console` gives you the hardware:

- `cls(col)` — clear screen
- `pset(x, y, col)`, `pget(x, y)` — single pixels (clipped)
- `rect`, `rectb`, `circ`, `circb`, `line` — all pure software, written by hand
- `print(text, x, y, col)` — 4×6 font, supports A-Z 0-9 and basics
- `btn[0..5]`, `btnp(b)`, `mouse_x`, `mouse_y`, `frame`

See the full surface in the source (`src/lib.rs`) or generate rustdoc with `cargo doc --open`.

**Limits are the fun**: 128×128 forces creativity. No floating point sin tables in the core for portability (we use simple integer or careful f32 where needed).

## Step 1: The Absolute Minimum Skeleton (3 minutes)

Create a new file or (temporarily for testing) add inside `src/main.rs`. For permanent examples see `examples/`.

```rust
use scratch8::{Cart, Console};

struct MyFirstCart {
    // Your game state lives here. Fixed-size arrays preferred for embedded friendliness.
    ball_x: f32,
    ball_y: f32,
}

impl MyFirstCart {
    pub fn new() -> Self {
        Self {
            ball_x: 64.0,
            ball_y: 64.0,
        }
    }
}

impl Cart for MyFirstCart {
    fn name(&self) -> &'static str {
        "MY FIRST"
    }

    fn init(&mut self, c: &mut Console) {
        c.cls(0); // clear to black
        // Reset any state you want on reload
        self.ball_x = 64.0;
        self.ball_y = 64.0;
    }

    fn update(&mut self, c: &mut Console) {
        c.cls(1); // dark blue background every frame (classic style)

        // Draw something — the ball
        c.circ(self.ball_x as i32, self.ball_y as i32, 4, 10); // yellow

        // Label
        c.print(self.name(), 4, 4, 7);
        c.print("ARROWS + Z/X  1/2/3 SWITCH", 4, 118, 6);
    }
}
```

To try it now (quick & dirty):

1. Add `MyFirstCart` next to the other carts in `src/main.rs`.
2. Add `Box::new(MyFirstCart::new()),` to the `carts` vec.
3. `cargo run --release`
4. Press 1/2/3 to cycle (you may need to renumber).

You will see a static yellow circle on dark blue with text. Zero external dependencies used for drawing.

## Step 2: Add Life — Animation & Input (4 minutes)

Use `c.frame` for time-based movement and `c.btn[]` / `c.btnp()` for player control.

Extend the update:

```rust
fn update(&mut self, c: &mut Console) {
    c.cls(1);

    // Gentle drift using frame (pure, no_std friendly)
    self.ball_x = 30.0 + ((c.frame as f32 * 0.8) % 70.0);
    self.ball_y = 40.0 + (((c.frame as f32 * 0.6).sin() * 20.0) + 20.0);  // careful sin ok here

    // Player can push the ball with buttons (Z/X change "gravity")
    if c.btn[4] { self.ball_y -= 1.5; } // Z/O = up thrust
    if c.btn[5] { self.ball_y += 1.5; } // X/K = down thrust
    if c.btn[0] { self.ball_x -= 1.2; } // left
    if c.btn[3] { self.ball_x += 1.2; } // down (example)

    // Wrap around edges (classic 8-bit feel)
    if self.ball_x < 0.0 { self.ball_x += WIDTH as f32; }
    if self.ball_x >= WIDTH as f32 { self.ball_x -= WIDTH as f32; }
    if self.ball_y < 0.0 { self.ball_y += HEIGHT as f32; }
    if self.ball_y >= HEIGHT as f32 { self.ball_y -= HEIGHT as f32; }

    c.circ(self.ball_x as i32, self.ball_y as i32, 5, 8); // red ball
    c.circb(self.ball_x as i32, self.ball_y as i32, 7, 7); // white outline

    // HUD
    c.print("MY FIRST CART", 4, 4, 7);
    c.print(&format!("FRAME:{}", c.frame), 4, 12, 10);
    if c.btnp(4) {
        c.print("Z PRESSED!", 50, 50, 11);
    }

    c.print("HOLD Z/X + ARROWS", 4, 110, 6);
}
```

`WIDTH` and `HEIGHT` are `pub const` in the crate root — `use scratch8::{Cart, Console, WIDTH, HEIGHT};`

`btnp` only triggers the frame the button goes down — perfect for "fire once".

Run it. You now have a living, controllable bouncing thing. This is already the heart of dozens of 8-bit games.

## Step 3: Real Game Feel — State, Scoring, Reset, Multiple Primitives (5 minutes)

Let's turn it into a tiny "Paddle Catcher" toy that demonstrates more of the API and real gameplay loop.

New state:

```rust
struct Catcher {
    paddle_x: i32,
    ball_x: f32,
    ball_y: f32,
    ball_vx: f32,
    ball_vy: f32,
    score: u32,
    lives: u8,
}
```

In `update`:

- Clear
- Move paddle with left/right (clamped)
- Move ball with velocity + gravity feel
- On paddle hit (simple AABB test using ball pos vs paddle rect) bounce + score++
- On bottom miss: lives--, reset ball
- On lives==0: show GAME OVER + btnp(4) to restart
- Draw:
  - `rect` for paddle
  - `circ` + `circb` for ball
  - `line` for "ground" or decorative elements
  - `print` for score + lives + title
  - Use `rectb` for border
- On every 30 frames do something cute with color or a particle (pset trail)

Full working version of this "Catcher" toy lives in the spirit of `examples/pong.rs` and `examples/snake.rs` (study them — they are self-contained and heavily commented).

Key patterns you will reuse in every cart:

- Always `c.cls(bg)` near top of update (or manually fade trails like the built-in PAINTER).
- Draw order: background → playfield → UI.
- Store velocities/positions in your struct (f32 is fine and portable).
- Use integer math + fixed arrays when you can for tiniest targets.
- `if c.btnp(N) { ... }` for menus / actions.
- `c.frame % N == 0` for timed events without timers.

## Step 4: Running & Iterating (1 minute)

- **Desktop**: `cargo run --release --features full` (or just `cargo run --release`)
- Switch carts with 1/2/3 (once you register yours).
- Escape quits.
- For quick iteration on a single cart you can temporarily make it the only one in the vec in `main.rs`.

Later (Phase 2+):
- `cargo run --example pong` (headless sim + prints) or integrate.
- Web (wasm) and embedded will use the exact same `Cart` impl.
- Real `.s8` cartridges will let you save/load/share without recompiling.

## Step 5: Polish & "From Scratch" Tips

- Palette discipline: stick to 0–15. Mask with `& 0x0F` if paranoid (the primitives do it).
- No allocations in the hot path if targeting bare metal.
- Collisions: for now use your own math (`if ball_x > paddle_x && ...`). When sprites + `spr()` arrive you can also read back with `pget` for pixel-perfect (expensive but fun for tiny res).
- Sound: once the PSG lands you will call tone / sfx methods on Console (pure generated samples, host feeds to cpal/WebAudio).
- Performance: 128×128 is tiny — even naive per-pixel loops in `rect` are fine at 60 Hz on modern chips and many embedded.

## What's Next?

- Read `docs/HARDWARE.md` for the full sacred spec (resolution, palette table, timing, planned memory map).
- Read `docs/CARTRIDGE_FORMAT.md` for how `.s8` files will work.
- Study the two full examples we provide: Pong (paddle physics + scoring + AI) and Snake (array-based body, timed movement, self-collision).
- Look at built-in demos in `src/lib.rs` (BouncerCart, PainterCart, ParticlesCart) — they show trails, particles with fixed arrays, pget, etc.
- Generate API reference: `cargo doc --open` (the `Console` and `Cart` docs are the canonical API).
- When you're ready for the full console experience: in-console sprite editor, map editor, music tracker (coming in Phase 3).

## Complete Tiny Example You Can Paste (Catcher Toy)

Here is a compact, self-contained version of the catcher game that demonstrates almost every current primitive. Drop the struct + impl into your carts list and play immediately.

```rust
use scratch8::{Cart, Console, HEIGHT, WIDTH};

struct Catcher {
    paddle: i32,
    bx: f32, by: f32, vx: f32, vy: f32,
    score: u32,
    lives: u8,
    game_over: bool,
}

impl Catcher {
    pub fn new() -> Self {
        Self {
            paddle: 54,
            bx: 20.0, by: 20.0, vx: 1.1, vy: 1.3,
            score: 0, lives: 3, game_over: false,
        }
    }
}

impl Cart for Catcher {
    fn name(&self) -> &'static str { "CATCHER" }

    fn init(&mut self, c: &mut Console) {
        c.cls(0);
        self.paddle = 54;
        self.bx = 20.0; self.by = 20.0;
        self.vx = 1.1; self.vy = 1.3;
        self.score = 0; self.lives = 3; self.game_over = false;
    }

    fn update(&mut self, c: &mut Console) {
        c.cls(0);

        if self.game_over {
            c.print("GAME OVER", 36, 50, 8);
            c.print(&format!("SCORE:{}", self.score), 40, 62, 10);
            c.print("Z TO RESTART", 32, 80, 7);
            if c.btnp(4) {
                self.init(c);
            }
            c.rectb(0, 0, WIDTH as i32, HEIGHT as i32, 5);
            return;
        }

        // Input
        if c.btn[0] { self.paddle -= 3; }
        if c.btn[1] { self.paddle += 3; }
        self.paddle = self.paddle.clamp(8, (WIDTH as i32) - 8 - 24);

        // Ball physics (from-scratch style)
        self.bx += self.vx;
        self.by += self.vy;

        // Wall bounce
        if self.bx < 4.0 || self.bx > (WIDTH - 4) as f32 { self.vx = -self.vx; }
        if self.by < 4.0 { self.vy = -self.vy; }

        // Paddle hit (simple rect test)
        let px = self.paddle;
        let py = HEIGHT as i32 - 12;
        if self.by > (py - 4) as f32
            && self.bx > px as f32
            && self.bx < (px + 24) as f32
            && self.vy > 0.0
        {
            self.vy = -self.vy * 1.05; // speed up a little
            self.vx += (self.bx - (px + 12) as f32) * 0.06; // english
            self.score += 1;
            // flash effect using pset trail (demo multiple features)
            for i in 0..3 {
                c.pset(self.bx as i32 + i, py - 6, 11);
            }
        }

        // Missed
        if self.by > HEIGHT as f32 + 6.0 {
            self.lives -= 1;
            if self.lives == 0 {
                self.game_over = true;
            } else {
                self.bx = 30.0 + (c.frame % 70) as f32;
                self.by = 10.0;
                self.vy = 1.4;
            }
        }

        // Draw everything using the primitives
        c.rectb(0, 0, WIDTH as i32, HEIGHT as i32, 5); // border
        c.line(0, HEIGHT as i32 - 4, WIDTH as i32, HEIGHT as i32 - 4, 6); // ground line

        c.rect(self.paddle, py, 24, 4, 12); // paddle (blue)
        c.rectb(self.paddle, py, 24, 4, 7);

        c.circ(self.bx as i32, self.by as i32, 3, 10); // ball
        c.circb(self.bx as i32, self.by as i32, 4, 7);

        // HUD
        c.print("CATCHER", 4, 4, 7);
        c.print(&format!("SCORE:{}", self.score), 4, 12, 11);
        c.print(&format!("LIVES:{}", self.lives), WIDTH as i32 - 40, 4, 8);

        if c.btnp(5) {
            // X resets ball position (demo btnp)
            self.bx = 64.0; self.by = 30.0;
        }
    }
}
```

Copy, register, play. You now understand the entire current machine.

Congratulations — you have written a real cart **from the very scratch**.

## Resources

- `docs/HARDWARE.md` — exact machine spec + memory map
- `docs/CARTRIDGE_FORMAT.md` — .s8 definition (work in progress)
- `examples/pong.rs` and `examples/snake.rs` — full polished games with headers
- `src/lib.rs` — the authoritative implementation + rustdoc comments
- `GOALS.md` — the big vision and roadmap
- `scripts/verify-portability.sh` — prove it runs on any chip

Now go make something weird and wonderful within the 128×128 box. The constraints are the joy.

— Documentation & Game Examples Author (agent track)
