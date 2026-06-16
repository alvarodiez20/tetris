//! Heuristic AI ("logic bot"). For the active piece (and the held/next piece via
//! a single Hold), it enumerates every straight-drop placement — each rotation ×
//! each column — scores the resulting board with a well-known feature set, and
//! returns the move sequence that reaches the best one.
//!
//! The evaluation features and weights are the El-Tetris set
//! (<https://imake.ninja/el-tetris-an-improvement-on-pierre-dellacheries-algorithm/>),
//! which plays strongly with only one-piece lookahead.

use crate::board::{Board, HEIGHT, WIDTH};
use crate::piece::{cells, Kind, Rotation};

const W_LANDING_HEIGHT: f64 = -4.500_158_825_082_766;
const W_LINES: f64 = 3.418_126_810_139_269_4;
const W_ROW_TRANSITIONS: f64 = -3.217_888_286_848_775_3;
const W_COL_TRANSITIONS: f64 = -9.348_695_305_445_199;
const W_HOLES: f64 = -7.899_265_427_351_652;
const W_WELLS: f64 = -3.385_597_224_726_362_6;

/// A chosen straight-drop placement for a piece.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Placement {
    pub rotation: u8, // number of CW rotations from spawn (0..=3)
    pub x: i32,       // bounding-box origin column
    pub score: f64,
}

#[inline]
fn filled(b: &Board, x: i32, y: i32) -> bool {
    // Cells above the top are empty; walls and the floor (out of bounds) are solid.
    if y < 0 {
        return false;
    }
    b.get(x, y) != 0
}

fn collides(b: &Board, kind: Kind, rot: Rotation, x: i32, y: i32) -> bool {
    cells(kind, rot)
        .iter()
        .any(|(cx, cy)| !b.is_empty_cell(x + *cx as i32, y + *cy as i32))
}

/// Distinct rotation states worth trying per piece (avoids redundant symmetries).
fn rotation_count(kind: Kind) -> u8 {
    match kind {
        Kind::O => 1,
        Kind::I | Kind::S | Kind::Z => 2,
        _ => 4,
    }
}

/// Evaluate dropping `kind` at rotation `rot`, box origin column `x`.
fn evaluate(b: &Board, kind: Kind, rot: Rotation, x: i32) -> Option<f64> {
    let local = cells(kind, rot);
    // Must sit within the horizontal bounds.
    for (cx, _) in local {
        let bx = x + cx as i32;
        if bx < 0 || bx >= WIDTH as i32 {
            return None;
        }
    }
    let min_cy = local.iter().map(|(_, cy)| *cy as i32).min().unwrap();
    let mut y = -min_cy; // place the top of the piece at row 0
    if collides(b, kind, rot, x, y) {
        return None; // column is blocked all the way up
    }
    while !collides(b, kind, rot, x, y + 1) {
        y += 1;
    }

    let mut nb = b.clone();
    let id = kind.cell_id();
    let (mut min_y, mut max_y) = (i32::MAX, i32::MIN);
    for (cx, cy) in local {
        let by = y + cy as i32;
        nb.set(x + cx as i32, by, id);
        min_y = min_y.min(by);
        max_y = max_y.max(by);
    }
    let lines = nb.clear_lines();

    let landing_height = HEIGHT as f64 - (min_y + max_y) as f64 / 2.0;
    let score = W_LANDING_HEIGHT * landing_height
        + W_LINES * lines as f64
        + W_ROW_TRANSITIONS * row_transitions(&nb) as f64
        + W_COL_TRANSITIONS * col_transitions(&nb) as f64
        + W_HOLES * count_holes(&nb) as f64
        + W_WELLS * well_sums(&nb) as f64;
    Some(score)
}

/// Best straight-drop placement for `kind` on `board`, if any fits.
pub(crate) fn best_placement(board: &Board, kind: Kind) -> Option<Placement> {
    let mut best: Option<Placement> = None;
    for r in 0..rotation_count(kind) {
        let rot = Rotation(r);
        for x in -2..=WIDTH as i32 {
            if let Some(score) = evaluate(board, kind, rot, x) {
                if best.is_none_or(|b| score > b.score) {
                    best = Some(Placement {
                        rotation: r,
                        x,
                        score,
                    });
                }
            }
        }
    }
    best
}

// --- board features ---------------------------------------------------------

fn count_holes(b: &Board) -> i32 {
    let mut holes = 0;
    for x in 0..WIDTH as i32 {
        let mut seen = false;
        for y in 0..HEIGHT as i32 {
            if filled(b, x, y) {
                seen = true;
            } else if seen {
                holes += 1;
            }
        }
    }
    holes
}

fn row_transitions(b: &Board) -> i32 {
    let mut t = 0;
    for y in 0..HEIGHT as i32 {
        let mut prev = true; // left wall is solid
        for x in 0..WIDTH as i32 {
            let cur = filled(b, x, y);
            if cur != prev {
                t += 1;
            }
            prev = cur;
        }
        if !prev {
            t += 1; // right wall is solid
        }
    }
    t
}

fn col_transitions(b: &Board) -> i32 {
    let mut t = 0;
    for x in 0..WIDTH as i32 {
        let mut prev = false; // above the top is empty
        for y in 0..HEIGHT as i32 {
            let cur = filled(b, x, y);
            if cur != prev {
                t += 1;
            }
            prev = cur;
        }
        if !prev {
            t += 1; // floor is solid
        }
    }
    t
}

/// Cumulative well depth: a depth-`d` well contributes 1 + 2 + ... + d.
fn well_sums(b: &Board) -> i32 {
    let mut s = 0;
    for x in 0..WIDTH as i32 {
        let mut depth = 0;
        for y in 0..HEIGHT as i32 {
            if !filled(b, x, y) && filled(b, x - 1, y) && filled(b, x + 1, y) {
                depth += 1;
                s += depth;
            } else {
                depth = 0;
            }
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_a_placement_on_empty_board() {
        let b = Board::default();
        for kind in Kind::ALL {
            assert!(best_placement(&b, kind).is_some());
        }
    }

    #[test]
    fn empty_board_has_no_holes_or_wells() {
        let b = Board::default();
        assert_eq!(count_holes(&b), 0);
        assert_eq!(well_sums(&b), 0);
    }
}
