//! `tetris-core` — a deterministic, seedable modern-guideline Tetris engine.
//!
//! Features: 7-bag randomizer, SRS rotation with wall kicks, hold piece, ghost
//! preview, lock delay, T-spin detection, and back-to-back / combo scoring.
//!
//! The engine is frontend-agnostic. Drive it with discrete [`Action`]s via
//! [`Game::input`] and advance time with [`Game::tick`]. Given the same seed and
//! the same sequence of inputs and `dt` values, it always produces the same game.
//!
//! ```
//! use tetris_core::{Game, Action};
//! let mut game = Game::new(42);
//! game.input(Action::MoveLeft);
//! let result = game.input(Action::HardDrop);
//! assert!(result.locked);
//! ```

mod bag;
mod board;
mod game;
mod piece;
mod rng;

pub use board::{Board, HEIGHT, VISIBLE_HEIGHT, WIDTH};
pub use game::{Action, Game, StepResult};
pub use piece::{Kind, Rotation};
