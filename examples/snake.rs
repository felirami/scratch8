//! SNAKE — classic growing snake game for SCRATCH-8
//!
//! Shows:
//! - Cart + Console: grid-based game logic using only fixed-size arrays (portable, no_alloc friendly)
//! - Multiple input features: btnp for direction changes (prevents instant reverse), held not required
//! - From-scratch collision: manual head vs body + wall checks using simple loops over tail array
//! - Timing: move only every N frames (using console.frame % speed) — classic feel
//! - Drawing with many primitives: rect (segments + grid feel + food), rectb (border + head highlight), print (score + messages), pset (subtle grid dots)
//! - Game states (playing / dead), grow on eat, wrap or wall death option (here: wall death), restart on btnp
//! - State in struct: position array, length counter, dir enum, food, score, speed
//! - Pure "scratch" implementation: no std collections, integer cell math (cell=4px -> 32x32 playfield), everything deterministic
//!
//! Controls:
//!   Arrow keys (btn 0-3): change direction (Up/Down/Left/Right)
//!   Z (btn4): restart after death
//!
//! Headless sim `main` included so this example compiles and runs cleanly via
//! cargo check --examples / cargo run --example snake using only the lib (Console + Cart).

use scratch8::{Cart, Console, HEIGHT, WIDTH};

const CELL: i32 = 4; // 4px per cell -> nice chunky 32x32 playfield inside 128x128
const GRID_W: i32 = WIDTH as i32 / CELL;
const GRID_H: i32 = HEIGHT as i32 / CELL;
const MAX_LEN: usize = 64; // plenty for tiny screen; fixed for portability

#[derive(Clone, Copy, PartialEq)]
enum Dir {
    Up,
    Down,
    Left,
    Right,
}

pub struct Snake {
    body: [(i32, i32); MAX_LEN], // (gx, gy) grid coords
    len: usize,
    dir: Dir,
    next_dir: Dir,
    food: (i32, i32),
    score: u32,
    dead: bool,
    move_every: u32, // lower = faster. increases slightly on eat for classic ramp
    move_timer: u32,
}

impl Default for Snake {
    fn default() -> Self {
        Self::new()
    }
}

impl Snake {
    pub fn new() -> Self {
        let mut s = Self {
            body: [(0, 0); MAX_LEN],
            len: 3,
            dir: Dir::Right,
            next_dir: Dir::Right,
            food: (20, 12),
            score: 0,
            dead: false,
            move_every: 6,
            move_timer: 0,
        };
        // initial snake in middle-left
        s.body[0] = (8, 12);
        s.body[1] = (7, 12);
        s.body[2] = (6, 12);
        s.place_food();
        s
    }

    fn place_food(&mut self) {
        // naive but tiny grid: just search for empty spot using frame as seed
        let mut fx = 5 + ((self.score as i32 * 7 + self.len as i32) % (GRID_W - 10));
        let mut fy = 5 + ((self.score as i32 * 3 + self.len as i32) % (GRID_H - 10));

        // avoid landing on snake (very small chance but be nice)
        for _ in 0..8 {
            let mut on_snake = false;
            for i in 0..self.len {
                if self.body[i].0 == fx && self.body[i].1 == fy {
                    on_snake = true;
                    break;
                }
            }
            if !on_snake {
                break;
            }
            fx = (fx + 3) % (GRID_W - 4);
            fy = (fy + 5) % (GRID_H - 4);
        }
        self.food = (fx, fy);
    }

    fn move_snake(&mut self) {
        // apply buffered direction (prevents 180 turns in one move)
        self.dir = self.next_dir;

        // compute new head
        let (hx, hy) = self.body[0];
        let (nx, ny) = match self.dir {
            Dir::Up => (hx, hy - 1),
            Dir::Down => (hx, hy + 1),
            Dir::Left => (hx - 1, hy),
            Dir::Right => (hx + 1, hy),
        };

        // Wall death (classic hard mode; change to wrap for easy mode)
        if nx < 1 || nx >= GRID_W - 1 || ny < 1 || ny >= GRID_H - 1 {
            self.dead = true;
            return;
        }

        // Self collision
        for i in 0..self.len {
            if self.body[i].0 == nx && self.body[i].1 == ny {
                self.dead = true;
                return;
            }
        }

        // Eat check BEFORE shifting tail
        let ate = nx == self.food.0 && ny == self.food.1;

        // Shift body (tail follows). We grow by NOT overwriting the last cell when eating.
        if self.len < MAX_LEN {
            if !ate {
                // move tail forward (drop last segment)
                for i in (1..self.len).rev() {
                    self.body[i] = self.body[i - 1];
                }
            } else {
                // grow: shift everything including making room at tail
                for i in (1..=self.len).rev() {
                    if i < MAX_LEN {
                        self.body[i] = self.body[i - 1];
                    }
                }
                self.len += 1;
                self.score += 1;
                // classic ramp difficulty
                if self.move_every > 3 && self.score % 3 == 0 {
                    self.move_every -= 1;
                }
                self.place_food();
            }
            self.body[0] = (nx, ny);
        } else {
            self.dead = true; // full screen snake, you win
        }
    }
}

impl Cart for Snake {
    fn name(&self) -> &'static str {
        "SNAKE"
    }

    fn init(&mut self, c: &mut Console) {
        c.cls(0);
        *self = Self::new();
    }

    fn update(&mut self, c: &mut Console) {
        c.cls(0);

        // === INPUT (direction changes via btnp — buffered) ===
        // Use btnp so a single press reliably queues the turn
        if c.btnp(0) && self.dir != Dir::Right {
            self.next_dir = Dir::Left;
        }
        if c.btnp(1) && self.dir != Dir::Left {
            self.next_dir = Dir::Right;
        }
        if c.btnp(2) && self.dir != Dir::Down {
            self.next_dir = Dir::Up;
        }
        if c.btnp(3) && self.dir != Dir::Up {
            self.next_dir = Dir::Down;
        }

        if self.dead {
            c.print("GAME OVER", 32, 48, 8);
            c.print(&format!("SCORE: {}", self.score), 36, 62, 10);
            c.print("Z TO RESTART", 28, 80, 7);
            c.rectb(0, 0, WIDTH as i32, HEIGHT as i32, 5);

            if c.btnp(4) {
                self.init(c);
            }
            return;
        }

        // === TIMED MOVE (from-scratch, frame based, no timers) ===
        self.move_timer = self.move_timer.wrapping_add(1);
        if self.move_timer >= self.move_every {
            self.move_timer = 0;
            self.move_snake();
        }

        // === DRAW WORLD (chunky 4px cells using rect for segments) ===
        // Outer border
        c.rectb(0, 0, WIDTH as i32, HEIGHT as i32, 6);
        // Subtle grid dots (demo pset + loop)
        for gx in (2..GRID_W - 2).step_by(2) {
            for gy in (2..GRID_H - 2).step_by(2) {
                c.pset(gx * CELL + 1, gy * CELL + 1, 5);
            }
        }

        // Food (nice red square with outline)
        let (fx, fy) = self.food;
        let fx_px = fx * CELL + 1;
        let fy_px = fy * CELL + 1;
        c.rect(fx_px, fy_px, CELL - 2, CELL - 2, 8);
        c.rectb(fx_px - 1, fy_px - 1, CELL, CELL, 9);

        // Snake body (head special)
        for i in (0..self.len).rev() {
            let (gx, gy) = self.body[i];
            let px = gx * CELL;
            let py = gy * CELL;
            let col = if i == 0 { 11 } else { 3 + ((i as u8) % 3) }; // head green, body shades
            c.rect(px + 1, py + 1, CELL - 2, CELL - 2, col);
            if i == 0 {
                // head highlight + eyes using tiny rects / pset
                c.rectb(px + 1, py + 1, CELL - 2, CELL - 2, 7);
                // "eyes" depending on dir
                match self.dir {
                    Dir::Right => {
                        c.pset(px + CELL - 2, py + 2, 0);
                        c.pset(px + CELL - 2, py + CELL - 3, 0);
                    }
                    Dir::Left => {
                        c.pset(px + 2, py + 2, 0);
                        c.pset(px + 2, py + CELL - 3, 0);
                    }
                    Dir::Up => {
                        c.pset(px + 2, py + 2, 0);
                        c.pset(px + CELL - 3, py + 2, 0);
                    }
                    Dir::Down => {
                        c.pset(px + 2, py + CELL - 3, 0);
                        c.pset(px + CELL - 3, py + CELL - 3, 0);
                    }
                }
            }
        }

        // HUD
        c.print("SNAKE", 4, 4, 7);
        c.print(&format!("SCORE:{}", self.score), 4, 12, 10);
        c.print(&format!("LEN:{}", self.len), WIDTH as i32 - 36, 4, 6);
        c.print("ARROWS=TURN", 4, HEIGHT as i32 - 10, 5);

        // Fun: flash food color occasionally using frame
        if c.frame % 20 < 3 {
            let (fx, fy) = self.food;
            c.pset(fx * CELL + 2, fy * CELL + 2, 10);
        }
    }
}

// Headless simulation entry point.
// Demonstrates that the cart is a pure, self-contained user of Console + Cart.
// Runs many frames, occasionally injects direction changes via update_input + btnp simulation.
fn main() {
    println!("SCRATCH-8 example: SNAKE");
    println!("Demonstrates fixed-array game state, frame-timed movement, btnp input, multi-primitive rendering.");
    println!("(Integrate the Snake struct into the desktop runner for real play, or await .s8 carts.)");
    println!();

    let mut console = Console::new();
    let mut cart = Snake::new();
    cart.init(&mut console);

    let mut last_score = 0u32;
    let mut last_len = 3usize;

    // Simulate a play session with occasional "player" direction presses
    for frame in 0..240 {
        // Demo input pattern: mostly go right, occasionally turn up/down/left to show interesting paths
        // We use the real update_input + btnp will be true only on the transition inside the cart's check.
        let mut left = false;
        let mut right = false;
        let mut up = false;
        let mut down = false;
        let z = false;
        let x = false;

        // Inject interesting turns at specific frames (simulates human pressing arrows)
        match frame {
            18 | 70 => up = true,
            42 => left = true,
            95 | 130 => down = true,
            155 => right = true,
            190 => up = true,
            _ => {}
        }

        // The host (desktop, web, embedded) calls this every frame before cart.update
        console.update_input(left, right, up, down, z, x, 0, 0);

        cart.update(&mut console);
        console.tick();

        if cart.score != last_score || cart.len != last_len {
            println!(
                "frame {}: score={} len={} dead={}",
                console.frame, cart.score, cart.len, cart.dead
            );
            last_score = cart.score;
            last_len = cart.len;
        }

        // Exercise other public surface
        if frame % 50 == 0 {
            let _p = console.pget(64, 64);
            // draw a harmless debug pixel that will be cleared next frame anyway
            console.pset(1, 1, 0);
        }

        // If it died in sim, restart so we can continue exercising grow/score code
        if cart.dead && frame % 30 == 0 {
            cart.init(&mut console);
        }
    }

    println!();
    println!("Snake simulation finished. Final: score={} len={} name={}", cart.score, cart.len, cart.name());
    println!("Cart compiled and executed cleanly against the public SCRATCH-8 Console + Cart API.");
}
