//! WebAssembly bindings for `tetris-core`.
//!
//! ```js
//! import init, { WasmGame } from './wasm-pkg/tetris_wasm.js';
//! await init();
//! const game = new WasmGame(42n);     // BigInt seed (u64)
//! game.input(4);                       // 4 = RotateCW
//! const lines = game.tick(16.7);       // advance ~one frame
//! const cells = game.visibleBoard();   // Uint8Array, 200 cells (10x20)
//! ```

use tetris_core::{Action, Game};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct WasmGame {
    inner: Game,
}

fn action_from_u8(a: u8) -> Option<Action> {
    Some(match a {
        0 => Action::MoveLeft,
        1 => Action::MoveRight,
        2 => Action::SoftDrop,
        3 => Action::HardDrop,
        4 => Action::RotateCW,
        5 => Action::RotateCCW,
        6 => Action::Hold,
        _ => return None,
    })
}

#[wasm_bindgen]
impl WasmGame {
    #[wasm_bindgen(constructor)]
    pub fn new(seed: u64) -> WasmGame {
        WasmGame {
            inner: Game::new(seed),
        }
    }

    pub fn reset(&mut self, seed: u64) {
        self.inner.reset(seed);
    }

    /// Apply a discrete action (0..=6). Returns lines cleared (only > 0 on hard drop).
    pub fn input(&mut self, action: u8) -> u32 {
        match action_from_u8(action) {
            Some(a) => self.inner.input(a).lines_cleared,
            None => 0,
        }
    }

    /// Advance time by `dt_ms`. Returns lines cleared this tick.
    pub fn tick(&mut self, dt_ms: f64) -> u32 {
        self.inner.tick(dt_ms).lines_cleared
    }

    #[wasm_bindgen(js_name = setSoftDrop)]
    pub fn set_soft_drop(&mut self, on: bool) {
        self.inner.set_soft_drop(on);
    }

    /// Visible playfield, row-major, 10 x 20 = 200 cells. 0 = empty, else color id.
    #[wasm_bindgen(js_name = visibleBoard)]
    pub fn visible_board(&self) -> Vec<u8> {
        self.inner.visible_board()
    }

    /// Active piece absolute cells as flat `[x0,y0, x1,y1, ...]` in *visible*
    /// coordinates (rows above the visible field are negative).
    #[wasm_bindgen(js_name = activeCells)]
    pub fn active_cells(&self) -> Vec<i32> {
        flatten_visible(&self.inner.active_cells())
    }

    #[wasm_bindgen(js_name = ghostCells)]
    pub fn ghost_cells(&self) -> Vec<i32> {
        flatten_visible(&self.inner.ghost_cells())
    }

    #[wasm_bindgen(js_name = activeKind)]
    pub fn active_kind(&self) -> u8 {
        self.inner.active_kind() as u8
    }

    /// Held piece kind, or -1 if none.
    pub fn hold(&self) -> i32 {
        self.inner.hold_piece().map(|k| k as i32).unwrap_or(-1)
    }

    /// Next `n` piece kinds (front = soonest).
    pub fn preview(&mut self, n: usize) -> Vec<u8> {
        self.inner.preview(n).iter().map(|k| *k as u8).collect()
    }

    /// AI: the action sequence (0..=6 codes) the heuristic bot would play for the
    /// current piece. Ends with a hard drop; call again after the piece locks.
    #[wasm_bindgen(js_name = botPlan)]
    pub fn bot_plan(&mut self) -> Vec<u8> {
        self.inner.bot_plan().iter().map(|a| *a as u8).collect()
    }

    pub fn score(&self) -> u32 {
        self.inner.score()
    }
    pub fn lines(&self) -> u32 {
        self.inner.lines()
    }
    pub fn level(&self) -> u32 {
        self.inner.level()
    }
    #[wasm_bindgen(js_name = isDone)]
    pub fn is_done(&self) -> bool {
        self.inner.is_done()
    }
}

/// Convert board cells (40-row coordinates) to visible coordinates by shifting
/// the hidden buffer off the top.
fn flatten_visible(cells: &[(i32, i32); 4]) -> Vec<i32> {
    let buffer = (tetris_core::HEIGHT - tetris_core::VISIBLE_HEIGHT) as i32;
    let mut out = Vec::with_capacity(8);
    for (x, y) in cells {
        out.push(*x);
        out.push(*y - buffer);
    }
    out
}
