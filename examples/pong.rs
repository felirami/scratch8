//! PONG — classic two-paddle game for SCRATCH-8
//!
//! Shows:
//! - Full Cart + Console usage with player input (btn held for paddle), btnp for serve
//! - Simple from-scratch physics (f32 ball velocity, wall/paddle bounce with "english")
//! - Multiple drawing primitives in one frame: rect (paddles), circ+circb (ball), line (net + borders), print (scores, labels, win text)
//! - Game state machine (playing / scored / won), scoring, reset, AI opponent for single player
//! - Frame-based timing, clamping, pure integer screen math mixed with f32
//! - "From the very scratch" feel: no external physics, collision is explicit AABB + reflection, everything fits in 128x128 16 colors
//!
//! Controls (in desktop runner once integrated or via your own host):
//!   Arrows or Z/X mapped: UP/DOWN (btn 2/3) move left paddle
//!   Z (btn4) to serve when waiting
//!   1/2/3 to switch carts (when registered in main)
//!
//! This is a self-contained example. The `main` runs a headless simulation loop
//! (prints progress) so `cargo check --examples` and `cargo run --example pong`
//! succeed using only the public lib API (no desktop feature required for the cart itself).

use scratch8::{Cart, Console, HEIGHT, WIDTH};

const PADDLE_H: i32 = 18;
const PADDLE_W: i32 = 4;
const BALL_R: i32 = 2;
const WIN_SCORE: u8 = 7;

pub struct Pong {
    left_y: i32,
    right_y: i32,
    bx: f32,
    by: f32,
    vx: f32,
    vy: f32,
    left_score: u8,
    right_score: u8,
    waiting_for_serve: bool,
    winner: Option<&'static str>,
}

impl Default for Pong {
    fn default() -> Self {
        Self::new()
    }
}

impl Pong {
    pub fn new() -> Self {
        let mut p = Self {
            left_y: 55,
            right_y: 55,
            bx: 64.0,
            by: 64.0,
            vx: 0.0,
            vy: 0.0,
            left_score: 0,
            right_score: 0,
            waiting_for_serve: true,
            winner: None,
        };
        p.reset_ball(true);
        p
    }

    fn reset_ball(&mut self, to_left: bool) {
        self.bx = WIDTH as f32 / 2.0;
        self.by = 30.0 + ((self.left_score as f32 + self.right_score as f32) * 3.7 % 60.0);
        let dir = if to_left { -1.0 } else { 1.0 };
        self.vx = dir * 1.6;
        self.vy = 0.6 + (self.left_score as f32 * 0.07);
        self.waiting_for_serve = true;
    }

    fn ai_move(&mut self) {
        // Very simple AI: follow ball with lag (classic 8-bit difficulty)
        let target = (self.by as i32 - PADDLE_H / 2).clamp(6, HEIGHT as i32 - 6 - PADDLE_H);
        let diff = target - self.right_y;
        if diff > 2 {
            self.right_y += 2;
        } else if diff < -2 {
            self.right_y -= 2;
        }
        self.right_y = self.right_y.clamp(6, HEIGHT as i32 - 6 - PADDLE_H);
    }
}

impl Cart for Pong {
    fn name(&self) -> &'static str {
        "PONG"
    }

    fn init(&mut self, c: &mut Console) {
        c.cls(0);
        self.left_y = 55;
        self.right_y = 55;
        self.left_score = 0;
        self.right_score = 0;
        self.winner = None;
        self.reset_ball(true);
    }

    fn update(&mut self, c: &mut Console) {
        c.cls(0);

        // Decorative border + center net (demonstrates line + rectb)
        c.rectb(0, 0, WIDTH as i32, HEIGHT as i32, 6);
        for y in (8..(HEIGHT as i32 - 8)).step_by(8) {
            c.rect(WIDTH as i32 / 2 - 1, y, 2, 4, 5);
        }

        // Title + instructions (uses print with multiple colors)
        c.print("PONG", 4, 4, 10);
        c.print("SCRATCH-8", WIDTH as i32 - 42, 4, 7);

        if let Some(w) = self.winner {
            c.print("WINNER!", 42, 48, 11);
            c.print(w, 50, 60, 10);
            c.print("Z=RESTART", 36, 80, 6);
            if c.btnp(4) {
                self.init(c);
            }
            // Show final scores
            c.print(&format!("{}  {}", self.left_score, self.right_score), 50, 92, 7);
            return;
        }

        // === INPUT (player left paddle) ===
        if c.btn[2] {
            self.left_y -= 2; // up
        }
        if c.btn[3] {
            self.left_y += 2; // down
        }
        self.left_y = self.left_y.clamp(6, HEIGHT as i32 - 6 - PADDLE_H);

        // AI always moves
        self.ai_move();

        // === SERVE / PHYSICS ===
        if self.waiting_for_serve {
            // Ball sits in center until player serves
            c.print("Z TO SERVE", 36, 70, 11);
            if c.btnp(4) {
                self.waiting_for_serve = false;
                // Give a little random-ish english on serve using frame
                self.vy += ((c.frame % 7) as f32 - 3.0) * 0.1;
            }
        } else {
            self.bx += self.vx;
            self.by += self.vy;

            // Top / bottom walls
            if self.by < 6.0 {
                self.by = 6.0;
                self.vy = -self.vy;
            }
            if self.by > (HEIGHT - 6) as f32 {
                self.by = (HEIGHT - 6) as f32;
                self.vy = -self.vy;
            }

            // Left paddle collision (player)
            let lp_x = 8;
            let lp_y = self.left_y;
            if self.bx < (lp_x + PADDLE_W + BALL_R) as f32
                && self.bx > (lp_x - BALL_R) as f32
                && self.by > lp_y as f32
                && self.by < (lp_y + PADDLE_H) as f32
                && self.vx < 0.0
            {
                self.vx = -self.vx * 1.03; // speed up slightly
                // add english based on hit position
                let hit = (self.by - lp_y as f32) / PADDLE_H as f32 - 0.5;
                self.vy += hit * 1.8;
                self.bx = (lp_x + PADDLE_W + BALL_R) as f32; // push out
            }

            // Right paddle collision (AI)
            let rp_x = WIDTH as i32 - 8 - PADDLE_W;
            let rp_y = self.right_y;
            if self.bx > (rp_x - BALL_R) as f32
                && self.bx < (rp_x + PADDLE_W + BALL_R) as f32
                && self.by > rp_y as f32
                && self.by < (rp_y + PADDLE_H) as f32
                && self.vx > 0.0
            {
                self.vx = -self.vx * 1.03;
                let hit = (self.by - rp_y as f32) / PADDLE_H as f32 - 0.5;
                self.vy += hit * 1.6;
                self.bx = (rp_x - BALL_R) as f32;
            }

            // Scoring
            if self.bx < 0.0 {
                self.right_score += 1;
                if self.right_score >= WIN_SCORE {
                    self.winner = Some("CPU");
                } else {
                    self.reset_ball(false);
                }
            }
            if self.bx > WIDTH as f32 {
                self.left_score += 1;
                if self.left_score >= WIN_SCORE {
                    self.winner = Some("YOU");
                } else {
                    self.reset_ball(true);
                }
            }
        }

        // === DRAW ===
        // Left paddle (player)
        c.rect(8, self.left_y, PADDLE_W, PADDLE_H, 12);
        c.rectb(8, self.left_y, PADDLE_W, PADDLE_H, 7);

        // Right paddle (AI)
        c.rect(WIDTH as i32 - 8 - PADDLE_W, self.right_y, PADDLE_W, PADDLE_H, 8);
        c.rectb(WIDTH as i32 - 8 - PADDLE_W, self.right_y, PADDLE_W, PADDLE_H, 7);

        // Ball
        let bx = self.bx as i32;
        let by = self.by as i32;
        c.circ(bx, by, BALL_R + 1, 10);
        c.circb(bx, by, BALL_R + 2, 7);

        // Scores (big chunky using repeated prints + color)
        c.print(&format!("{}", self.left_score), 28, 18, 7);
        c.print(&format!("{}", self.right_score), WIDTH as i32 - 36, 18, 7);

        // Status line
        c.print("LEFT:UP/DN  Z=SERVE", 4, HEIGHT as i32 - 10, 5);

        // Subtle frame indicator (demo frame usage)
        if c.frame % 30 < 5 {
            c.pset(2, 2, 9);
        }
    }
}

// Headless simulation so the example always compiles and runs with just the core lib.
// `cargo run --example pong` will execute ~180 frames of gameplay and print scores.
fn main() {
    println!("SCRATCH-8 example: PONG");
    println!("This cart demonstrates Console drawing primitives + Cart trait + input simulation.");
    println!("(For interactive play, register Pong in src/main.rs carts list or use future .s8 loader.)");
    println!();

    let mut console = Console::new();
    let mut cart = Pong::new();
    cart.init(&mut console);

    let mut last_left = 0u8;
    let mut last_right = 0u8;

    for frame in 0..180 {
        // Simulate some player input for demo purposes (left paddle moves up/down rhythmically)
        let sim_up = (frame / 12) % 3 == 0;
        let sim_down = (frame / 12) % 3 == 1;
        let sim_serve = frame == 5 || frame % 55 == 0;

        // We can't directly set btn here (update_input is the public host API),
        // but for pure cart simulation we poke state carefully for demo effect.
        // Better: drive through the real update_input + manual button simulation.
        let left = sim_down; // treat as down held for demo movement
        let right = false;
        let up = sim_up;
        let down = sim_down;
        let z = sim_serve;
        let x = false;
        let mx = 0;
        let my = 0;

        console.update_input(left, right, up, down, z, x, mx, my);

        cart.update(&mut console);
        console.tick();

        if cart.left_score != last_left || cart.right_score != last_right {
            println!(
                "frame {}: score {}-{}  (winner? {:?})",
                console.frame, cart.left_score, cart.right_score, cart.winner
            );
            last_left = cart.left_score;
            last_right = cart.right_score;
        }

        // Every so often show we are using the public API surface
        if frame % 45 == 0 {
            let _ = console.pget(10, 10); // exercise pget
            console.pset(10, 10, 0); // and pset (harmless)
        }
    }

    println!();
    println!("Pong cart simulation complete. Final score: {}-{}", cart.left_score, cart.right_score);
    println!("Cart name: {}", cart.name());
    println!("Example ran successfully against the public Console + Cart API.");
}
