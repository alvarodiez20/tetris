//! The game state machine: active piece, gravity, lock delay, hold, line clears,
//! and guideline scoring (including T-spins, back-to-back, and combos).
//!
//! Time is fed in via [`Game::tick`] (milliseconds), keeping the engine
//! independent of frame rate and fully reproducible from `seed + inputs + dt`.

use crate::bag::Bag;
use crate::board::{Board, BUFFER_HEIGHT, WIDTH};
use crate::piece::{cells, kicks, Kind, Rotation};
use crate::rng::Rng;

const SPAWN_X: i32 = 3;
const SPAWN_Y: i32 = (BUFFER_HEIGHT as i32) - 2; // box top-left, so piece straddles the buffer/visible line
const LOCK_DELAY_MS: f64 = 500.0;
const MAX_LOCK_RESETS: u32 = 15;
const SOFT_DROP_FACTOR: f64 = 20.0; // soft drop is 20x gravity
const MAX_PENDING_GARBAGE: u32 = 20; // incoming garbage is capped at one screen
const MAX_GARBAGE_PER_LOCK: u32 = 8; // rows that can rise after a single lock
const GARBAGE_SEED_SALT: u64 = 0xD1B5_4A32_D192_ED03;

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
    /// Garbage lines to send to the opponent (already net of cancelled garbage).
    pub garbage_sent: u32,
    /// Garbage rows that rose into this player's board on this lock.
    pub garbage_received: u32,
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

    // Versus mode: garbage waiting to rise, and the RNG choosing its holes.
    pending_garbage: u32,
    garbage_rng: Rng,

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
            pending_garbage: 0,
            garbage_rng: Rng::new(seed ^ GARBAGE_SEED_SALT),
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
    /// Garbage lines queued by the opponent that will rise after the next lock
    /// that does not clear a line.
    pub fn pending_garbage(&self) -> u32 {
        self.pending_garbage
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

    /// Queue `n` garbage lines sent by an opponent (versus mode).
    pub fn receive_garbage(&mut self, n: u32) {
        self.pending_garbage = (self.pending_garbage + n).min(MAX_PENDING_GARBAGE);
    }

    // --- AI bot ------------------------------------------------------------

    /// Compute a sequence of [`Action`]s that places the current piece in the
    /// spot the heuristic likes best. Considers a single Hold (swapping in the
    /// held piece, or the next piece if Hold is empty). The plan ends with a
    /// hard drop; replan after the piece locks.
    pub fn bot_plan(&mut self) -> Vec<Action> {
        if self.done {
            return Vec::new();
        }

        let current = crate::bot::best_placement(&self.board, self.active.kind);

        // What Hold would give us this turn.
        let hold_kind = self.hold.unwrap_or_else(|| self.bag.preview(1)[0]);
        let held = if self.can_hold {
            crate::bot::best_placement(&self.board, hold_kind)
        } else {
            None
        };

        let use_hold = match (current, held) {
            (Some(c), Some(h)) => h.score > c.score,
            (None, Some(_)) => true,
            _ => false,
        };
        let chosen = if use_hold { held } else { current };

        let Some(p) = chosen else {
            // Board is full to the top; just drop and let it end.
            return vec![Action::HardDrop];
        };

        let mut actions = Vec::new();
        if use_hold {
            actions.push(Action::Hold);
        }
        // Rotations happen in the open spawn area, so the box origin stays at
        // SPAWN_X and a plain horizontal shift reaches the target column.
        for _ in 0..p.rotation {
            actions.push(Action::RotateCW);
        }
        let shift = p.x - SPAWN_X;
        let step = if shift < 0 {
            Action::MoveLeft
        } else {
            Action::MoveRight
        };
        for _ in 0..shift.unsigned_abs() {
            actions.push(step);
        }
        actions.push(Action::HardDrop);
        actions
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
        let b2b_before = self.back_to_back;
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

        // Versus: outgoing attack first cancels incoming garbage; if nothing was
        // cleared, whatever is still pending rises from the bottom.
        let mut garbage_sent = if cleared > 0 {
            attack_lines(
                cleared,
                t_spin,
                difficult && base > 0 && b2b_before,
                self.combo,
            )
        } else {
            0
        };
        let cancelled = garbage_sent.min(self.pending_garbage);
        garbage_sent -= cancelled;
        self.pending_garbage -= cancelled;
        let mut garbage_received = 0;
        if cleared == 0 && self.pending_garbage > 0 {
            garbage_received = self.pending_garbage.min(MAX_GARBAGE_PER_LOCK);
            self.pending_garbage -= garbage_received;
            let hole = self.garbage_rng.below(WIDTH);
            if self.board.push_garbage(garbage_received as usize, hole) {
                self.done = true;
            }
        }

        // Next piece.
        self.can_hold = true;
        let next = self.bag.next();
        self.spawn(next);

        StepResult {
            lines_cleared: cleared,
            score_delta: self.score - score_before,
            locked: true,
            done: self.done,
            garbage_sent,
            garbage_received,
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

/// Garbage lines an attack is worth (guideline-style table): singles send
/// nothing, T-spins hit harder, back-to-back adds one, and long combos add more.
fn attack_lines(cleared: u32, t_spin: bool, back_to_back: bool, combo: i32) -> u32 {
    let base = match (t_spin, cleared) {
        (true, n) => n * 2,
        (false, 2) => 1,
        (false, 3) => 2,
        (false, 4) => 4,
        _ => 0,
    };
    let b2b = u32::from(back_to_back);
    let combo_bonus = if combo >= 2 {
        (combo as u32 / 2).min(4)
    } else {
        0
    };
    base + b2b + combo_bonus
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
    fn bot_survives_and_clears_lines() {
        let mut g = Game::new(42);
        let mut pieces = 0;
        while !g.is_done() && pieces < 150 {
            let plan = g.bot_plan();
            assert!(!plan.is_empty());
            for act in plan {
                g.input(act);
            }
            pieces += 1;
        }
        // A competent heuristic should clear plenty of lines and not top out fast.
        assert!(!g.is_done(), "bot topped out within 150 pieces");
        assert!(g.lines() > 10, "bot cleared too few lines: {}", g.lines());
    }

    /// Bottom two rows full except columns 4..=5, with an O piece ready to drop
    /// into the gap: a guaranteed double (worth 1 garbage line).
    fn double_setup() -> Game {
        let mut g = Game::new(1);
        let last = crate::board::HEIGHT as i32 - 1;
        for y in [last, last - 1] {
            for x in (0..WIDTH as i32).filter(|x| *x != 4 && *x != 5) {
                g.board.set(x, y, 1);
            }
        }
        g.active = Active {
            kind: Kind::O,
            rot: Rotation::SPAWN,
            x: 3,
            y: SPAWN_Y,
        };
        g
    }

    #[test]
    fn attack_table() {
        assert_eq!(attack_lines(1, false, false, 0), 0);
        assert_eq!(attack_lines(2, false, false, 1), 1);
        assert_eq!(attack_lines(3, false, false, 1), 2);
        assert_eq!(attack_lines(4, false, false, 1), 4);
        assert_eq!(attack_lines(4, false, true, 1), 5, "back-to-back adds one");
        assert_eq!(attack_lines(2, true, false, 1), 4, "t-spin double");
        assert_eq!(attack_lines(1, false, false, 4), 2, "combo bonus");
        assert_eq!(
            attack_lines(1, false, false, 99),
            4,
            "combo bonus is capped"
        );
    }

    #[test]
    fn clearing_lines_sends_garbage() {
        let mut g = double_setup();
        let r = g.input(Action::HardDrop);
        assert_eq!(r.lines_cleared, 2);
        assert_eq!(r.garbage_sent, 1);
    }

    #[test]
    fn clearing_lines_cancels_incoming_garbage_first() {
        let mut g = double_setup();
        g.receive_garbage(3);
        let r = g.input(Action::HardDrop);
        assert_eq!(r.garbage_sent, 0, "the attack was spent cancelling");
        assert_eq!(r.garbage_received, 0, "no garbage rises on a clearing lock");
        assert_eq!(g.pending_garbage(), 2);
    }

    #[test]
    fn garbage_rises_after_a_non_clearing_lock() {
        let mut g = Game::new(5);
        g.receive_garbage(3);
        assert_eq!(g.pending_garbage(), 3);
        let r = g.input(Action::HardDrop);
        assert_eq!(r.lines_cleared, 0);
        assert_eq!(r.garbage_received, 3);
        assert_eq!(g.pending_garbage(), 0);
        let board = g.visible_board();
        for row in 17..20 {
            let garbage = board[row * WIDTH..(row + 1) * WIDTH]
                .iter()
                .filter(|&&c| c == crate::board::GARBAGE_ID)
                .count();
            assert_eq!(garbage, WIDTH - 1, "row {row} has exactly one hole");
        }
    }

    #[test]
    fn garbage_holes_are_deterministic_per_seed() {
        let play = |seed| {
            let mut g = Game::new(seed);
            g.receive_garbage(2);
            g.input(Action::HardDrop);
            g.visible_board()
        };
        assert_eq!(play(9), play(9));
    }

    #[test]
    fn pending_garbage_is_capped() {
        let mut g = Game::new(1);
        g.receive_garbage(500);
        assert_eq!(g.pending_garbage(), MAX_PENDING_GARBAGE);
    }

    #[test]
    fn garbage_can_top_the_player_out() {
        let mut g = Game::new(2);
        for _ in 0..40 {
            if g.is_done() {
                break;
            }
            g.receive_garbage(8);
            g.input(Action::HardDrop);
        }
        assert!(g.is_done(), "a wall of garbage should end the game");
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
