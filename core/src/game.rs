//! The game state machine: active piece, gravity, lock delay, hold, line clears,
//! and guideline scoring (including T-spins, back-to-back, and combos).
//!
//! Time is fed in via [`Game::tick`] (milliseconds), keeping the engine
//! independent of frame rate and fully reproducible from `seed + inputs + dt`.

use crate::bag::Bag;
use crate::board::{Board, BUFFER_HEIGHT, WIDTH};
use crate::piece::{cells, kicks, Kind, Rotation};

const SPAWN_X: i32 = 3;
const SPAWN_Y: i32 = (BUFFER_HEIGHT as i32) - 2; // box top-left, so piece straddles the buffer/visible line
const LOCK_DELAY_MS: f64 = 500.0;
const MAX_LOCK_RESETS: u32 = 15;
const SOFT_DROP_FACTOR: f64 = 20.0; // soft drop is 20x gravity

/// Discrete player inputs. Applied immediately by [`Game::input`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Action {
    MoveLeft = 0,
    MoveRight = 1,
    SoftDrop = 2,
    HardDrop = 3,
    RotateCW = 4,
    RotateCCW = 5,
    Hold = 6,
}

/// What happened during a `tick` or `input` call.
#[derive(Debug, Clone, Copy, Default)]
pub struct StepResult {
    pub lines_cleared: u32,
    pub score_delta: u32,
    pub locked: bool,
    pub done: bool,
}

struct Active {
    kind: Kind,
    rot: Rotation,
    x: i32,
    y: i32,
}

pub struct Game {
    board: Board,
    bag: Bag,
    active: Active,
    hold: Option<Kind>,
    can_hold: bool,

    score: u32,
    lines: u32,
    level: u32,
    combo: i32,
    back_to_back: bool,
    done: bool,

    // Timing / lock state.
    gravity_accum_ms: f64,
    soft_dropping: bool,
    on_ground: bool,
    lock_timer_ms: f64,
    lock_resets: u32,
    last_move_was_rotation: bool,
}

impl Game {
    pub fn new(seed: u64) -> Self {
        let mut bag = Bag::new(seed);
        let first = bag.next();
        let mut g = Game {
            board: Board::default(),
            bag,
            active: Active {
                kind: first,
                rot: Rotation::SPAWN,
                x: SPAWN_X,
                y: SPAWN_Y,
            },
            hold: None,
            can_hold: true,
            score: 0,
            lines: 0,
            level: 1,
            combo: -1,
            back_to_back: false,
            done: false,
            gravity_accum_ms: 0.0,
            soft_dropping: false,
            on_ground: false,
            lock_timer_ms: 0.0,
            lock_resets: 0,
            last_move_was_rotation: false,
        };
        // If the very first piece overlaps, the game is over immediately.
        if g.collides(g.active.kind, g.active.rot, g.active.x, g.active.y) {
            g.done = true;
        }
        g
    }

    pub fn reset(&mut self, seed: u64) {
        *self = Game::new(seed);
    }

    // --- queries -----------------------------------------------------------

    pub fn is_done(&self) -> bool {
        self.done
    }
    pub fn score(&self) -> u32 {
        self.score
    }
    pub fn lines(&self) -> u32 {
        self.lines
    }
    pub fn level(&self) -> u32 {
        self.level
    }
    pub fn hold_piece(&self) -> Option<Kind> {
        self.hold
    }
    pub fn active_kind(&self) -> Kind {
        self.active.kind
    }
    pub fn board(&self) -> &Board {
        &self.board
    }

    /// Next pieces (front = soonest).
    pub fn preview(&mut self, n: usize) -> Vec<Kind> {
        self.bag.preview(n)
    }

    /// Absolute board cells `(x, y)` of the active piece.
    pub fn active_cells(&self) -> [(i32, i32); 4] {
        Self::abs_cells(
            self.active.kind,
            self.active.rot,
            self.active.x,
            self.active.y,
        )
    }

    /// Where the active piece would land if hard-dropped.
    pub fn ghost_cells(&self) -> [(i32, i32); 4] {
        let mut gy = self.active.y;
        while !self.collides(self.active.kind, self.active.rot, self.active.x, gy + 1) {
            gy += 1;
        }
        Self::abs_cells(self.active.kind, self.active.rot, self.active.x, gy)
    }

    // --- input -------------------------------------------------------------

    pub fn input(&mut self, action: Action) -> StepResult {
        if self.done {
            return StepResult {
                done: true,
                ..Default::default()
            };
        }
        match action {
            Action::MoveLeft => {
                if self.try_move(-1, 0) {
                    self.last_move_was_rotation = false;
                    self.touch_lock_reset();
                }
                StepResult::default()
            }
            Action::MoveRight => {
                if self.try_move(1, 0) {
                    self.last_move_was_rotation = false;
                    self.touch_lock_reset();
                }
                StepResult::default()
            }
            Action::SoftDrop => {
                if self.try_move(0, 1) {
                    self.last_move_was_rotation = false;
                    self.score += 1;
                    self.gravity_accum_ms = 0.0;
                }
                StepResult::default()
            }
            Action::HardDrop => {
                let mut dropped = 0u32;
                while self.try_move(0, 1) {
                    dropped += 1;
                }
                self.score += dropped * 2;
                self.lock_piece()
            }
            Action::RotateCW => {
                if self.try_rotate(true) {
                    self.last_move_was_rotation = true;
                    self.touch_lock_reset();
                }
                StepResult::default()
            }
            Action::RotateCCW => {
                if self.try_rotate(false) {
                    self.last_move_was_rotation = true;
                    self.touch_lock_reset();
                }
                StepResult::default()
            }
            Action::Hold => {
                self.do_hold();
                StepResult::default()
            }
        }
    }

    // --- time --------------------------------------------------------------

    /// Advance the simulation by `dt_ms` milliseconds.
    pub fn tick(&mut self, dt_ms: f64) -> StepResult {
        if self.done {
            return StepResult {
                done: true,
                ..Default::default()
            };
        }

        let interval = self.drop_interval_ms()
            / if self.soft_dropping {
                SOFT_DROP_FACTOR
            } else {
                1.0
            };
        self.gravity_accum_ms += dt_ms;

        let mut result = StepResult::default();
        while self.gravity_accum_ms >= interval {
            self.gravity_accum_ms -= interval;
            if self.try_move(0, 1) {
                self.last_move_was_rotation = false;
            } else {
                break;
            }
        }

        // Lock delay handling.
        let grounded = self.collides(
            self.active.kind,
            self.active.rot,
            self.active.x,
            self.active.y + 1,
        );
        if grounded {
            self.on_ground = true;
            self.lock_timer_ms += dt_ms;
            if self.lock_timer_ms >= LOCK_DELAY_MS {
                result = self.lock_piece();
            }
        } else {
            self.on_ground = false;
            self.lock_timer_ms = 0.0;
            self.lock_resets = 0;
        }
        result
    }

    pub fn set_soft_drop(&mut self, on: bool) {
        self.soft_dropping = on;
    }

    // --- internals ---------------------------------------------------------

    fn abs_cells(kind: Kind, rot: Rotation, x: i32, y: i32) -> [(i32, i32); 4] {
        let local = cells(kind, rot);
        let mut out = [(0, 0); 4];
        for (i, (cx, cy)) in local.iter().enumerate() {
            out[i] = (x + *cx as i32, y + *cy as i32);
        }
        out
    }

    fn collides(&self, kind: Kind, rot: Rotation, x: i32, y: i32) -> bool {
        Self::abs_cells(kind, rot, x, y)
            .iter()
            .any(|(cx, cy)| !self.board.is_empty_cell(*cx, *cy))
    }

    fn try_move(&mut self, dx: i32, dy: i32) -> bool {
        let (nx, ny) = (self.active.x + dx, self.active.y + dy);
        if self.collides(self.active.kind, self.active.rot, nx, ny) {
            return false;
        }
        self.active.x = nx;
        self.active.y = ny;
        true
    }

    fn try_rotate(&mut self, cw: bool) -> bool {
        if self.active.kind == Kind::O {
            return false; // O does not rotate
        }
        let from = self.active.rot;
        let to = if cw { from.cw() } else { from.ccw() };
        for (kx, ky) in kicks(self.active.kind, from, cw) {
            let nx = self.active.x + kx as i32;
            let ny = self.active.y + ky as i32;
            if !self.collides(self.active.kind, to, nx, ny) {
                self.active.rot = to;
                self.active.x = nx;
                self.active.y = ny;
                return true;
            }
        }
        false
    }

    fn touch_lock_reset(&mut self) {
        if self.on_ground && self.lock_resets < MAX_LOCK_RESETS {
            self.lock_timer_ms = 0.0;
            self.lock_resets += 1;
        }
    }

    fn do_hold(&mut self) {
        if !self.can_hold {
            return;
        }
        let current = self.active.kind;
        let next = match self.hold {
            Some(k) => k,
            None => self.bag.next(),
        };
        self.hold = Some(current);
        self.can_hold = false;
        self.spawn(next);
    }

    fn spawn(&mut self, kind: Kind) {
        self.active = Active {
            kind,
            rot: Rotation::SPAWN,
            x: SPAWN_X,
            y: SPAWN_Y,
        };
        self.gravity_accum_ms = 0.0;
        self.on_ground = false;
        self.lock_timer_ms = 0.0;
        self.lock_resets = 0;
        self.last_move_was_rotation = false;
        if self.collides(kind, Rotation::SPAWN, SPAWN_X, SPAWN_Y) {
            self.done = true;
        }
    }

    fn lock_piece(&mut self) -> StepResult {
        let t_spin = self.detect_t_spin();
        let id = self.active.kind.cell_id();
        for (x, y) in self.active_cells() {
            self.board.set(x, y, id);
        }

        let cleared = self.board.clear_lines();
        let score_before = self.score;

        // Scoring.
        let difficult = cleared == 4 || (t_spin && cleared > 0);
        let base = match (t_spin, cleared) {
            (true, 0) => 400,
            (true, 1) => 800,
            (true, 2) => 1200,
            (true, 3) => 1600,
            (false, 1) => 100,
            (false, 2) => 300,
            (false, 3) => 500,
            (false, 4) => 800,
            _ => 0,
        };
        let mut gain = base * self.level;
        if difficult && self.back_to_back && base > 0 {
            gain += gain / 2; // back-to-back: +50%
        }
        if cleared > 0 {
            self.combo += 1;
            if self.combo > 0 {
                gain += 50 * self.combo as u32 * self.level;
            }
            self.back_to_back = difficult;
            self.lines += cleared;
            self.level = self.lines / 10 + 1;
        } else {
            self.combo = -1;
        }
        self.score += gain;

        // Next piece.
        self.can_hold = true;
        let next = self.bag.next();
        self.spawn(next);

        StepResult {
            lines_cleared: cleared,
            score_delta: self.score - score_before,
            locked: true,
            done: self.done,
        }
    }

    /// Guideline 3-corner T-spin test: the piece is a T, the last successful
    /// movement was a rotation, and at least 3 of the 4 corners of its 3x3 box
    /// are occupied.
    fn detect_t_spin(&self) -> bool {
        if self.active.kind != Kind::T || !self.last_move_was_rotation {
            return false;
        }
        let (x, y) = (self.active.x, self.active.y);
        let corners = [(x, y), (x + 2, y), (x, y + 2), (x + 2, y + 2)];
        let filled = corners
            .iter()
            .filter(|(cx, cy)| !self.board.is_empty_cell(*cx, *cy))
            .count();
        filled >= 3
    }

    /// Milliseconds per row drop at the current level (guideline curve).
    fn drop_interval_ms(&self) -> f64 {
        let l = (self.level.min(20) - 1) as f64;
        let secs = (0.8 - l * 0.007).powf(l);
        (secs * 1000.0).max(1.0)
    }
}

impl Game {
    /// Convenience for rendering: visible board cells (bottom 20 rows), row-major.
    pub fn visible_board(&self) -> Vec<u8> {
        self.board.visible()
    }
    pub const WIDTH: usize = WIDTH;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_game_not_done() {
        let g = Game::new(1);
        assert!(!g.is_done());
        assert_eq!(g.score(), 0);
        assert_eq!(g.level(), 1);
    }

    #[test]
    fn hard_drop_locks_and_spawns() {
        let mut g = Game::new(1);
        let first = g.active_kind();
        let r = g.input(Action::HardDrop);
        assert!(r.locked);
        // A new piece should be active (board has the locked one now).
        let _ = first;
        assert!(!g.is_done());
    }

    #[test]
    fn deterministic_replay() {
        let actions = [
            Action::MoveLeft,
            Action::RotateCW,
            Action::HardDrop,
            Action::MoveRight,
            Action::HardDrop,
        ];
        let mut a = Game::new(7);
        let mut b = Game::new(7);
        for act in actions {
            a.input(act);
            b.input(act);
        }
        assert_eq!(a.score(), b.score());
        assert_eq!(a.visible_board(), b.visible_board());
    }

    #[test]
    fn stacking_eventually_tops_out() {
        let mut g = Game::new(3);
        // Hard drop many pieces without moving; the well fills and tops out.
        for _ in 0..100 {
            g.input(Action::HardDrop);
            if g.is_done() {
                break;
            }
        }
        assert!(g.is_done(), "repeated center drops should top out");
    }
}
