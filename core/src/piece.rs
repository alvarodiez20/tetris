//! Tetromino shapes, rotation states, and SRS (Super Rotation System) wall-kick
//! data. Coordinates use (x = column, y = row) with `y` increasing downward to
//! match the board's row-major storage.

/// The seven tetrominoes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Kind {
    I = 0,
    J = 1,
    L = 2,
    O = 3,
    S = 4,
    T = 5,
    Z = 6,
}

impl Kind {
    pub const ALL: [Kind; 7] = [
        Kind::I,
        Kind::J,
        Kind::L,
        Kind::O,
        Kind::S,
        Kind::T,
        Kind::Z,
    ];

    /// Stable 1-based color/cell id (0 is reserved for "empty").
    #[inline]
    pub fn cell_id(self) -> u8 {
        self as u8 + 1
    }
}

/// Rotation state. SRS labels these 0 (spawn), R (one CW), 2 (180), L (one CCW).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rotation(pub u8); // 0..=3

impl Rotation {
    pub const SPAWN: Rotation = Rotation(0);

    #[inline]
    pub fn cw(self) -> Rotation {
        Rotation((self.0 + 1) & 3)
    }
    #[inline]
    pub fn ccw(self) -> Rotation {
        Rotation((self.0 + 3) & 3)
    }
    #[inline]
    pub fn index(self) -> usize {
        self.0 as usize
    }
}

/// The four occupied cells of a piece, as (x, y) offsets within its bounding box.
pub type Cells = [(i8, i8); 4];

/// Local cell offsets for every (kind, rotation). JLSTZ use a 3x3 box, I uses a
/// 4x4 box, O is fixed.
#[inline]
pub fn cells(kind: Kind, rot: Rotation) -> Cells {
    SHAPES[kind as usize][rot.index()]
}

// Each entry: SHAPES[kind][rotation] = four (x, y) cells.
#[rustfmt::skip]
const SHAPES: [[Cells; 4]; 7] = [
    // I (4x4 box)
    [
        [(0,1),(1,1),(2,1),(3,1)],
        [(2,0),(2,1),(2,2),(2,3)],
        [(0,2),(1,2),(2,2),(3,2)],
        [(1,0),(1,1),(1,2),(1,3)],
    ],
    // J
    [
        [(0,0),(0,1),(1,1),(2,1)],
        [(1,0),(2,0),(1,1),(1,2)],
        [(0,1),(1,1),(2,1),(2,2)],
        [(1,0),(1,1),(0,2),(1,2)],
    ],
    // L
    [
        [(2,0),(0,1),(1,1),(2,1)],
        [(1,0),(1,1),(1,2),(2,2)],
        [(0,1),(1,1),(2,1),(0,2)],
        [(0,0),(1,0),(1,1),(1,2)],
    ],
    // O (does not rotate)
    [
        [(1,0),(2,0),(1,1),(2,1)],
        [(1,0),(2,0),(1,1),(2,1)],
        [(1,0),(2,0),(1,1),(2,1)],
        [(1,0),(2,0),(1,1),(2,1)],
    ],
    // S
    [
        [(1,0),(2,0),(0,1),(1,1)],
        [(1,0),(1,1),(2,1),(2,2)],
        [(1,1),(2,1),(0,2),(1,2)],
        [(0,0),(0,1),(1,1),(1,2)],
    ],
    // T
    [
        [(1,0),(0,1),(1,1),(2,1)],
        [(1,0),(1,1),(2,1),(1,2)],
        [(0,1),(1,1),(2,1),(1,2)],
        [(1,0),(0,1),(1,1),(1,2)],
    ],
    // Z
    [
        [(0,0),(1,0),(1,1),(2,1)],
        [(2,0),(1,1),(2,1),(1,2)],
        [(0,1),(1,1),(1,2),(2,2)],
        [(1,0),(0,1),(1,1),(0,2)],
    ],
];

/// SRS wall-kick offsets, in board coordinates (y down). Indexed by the
/// *current* rotation state. Five tests are attempted in order.
type KickSet = [[(i8, i8); 5]; 4];

#[rustfmt::skip]
const JLSTZ_CW: KickSet = [
    [(0,0),(-1,0),(-1,-1),(0,2),(-1,2)],   // 0 -> R
    [(0,0),(1,0),(1,1),(0,-2),(1,-2)],     // R -> 2
    [(0,0),(1,0),(1,-1),(0,2),(1,2)],      // 2 -> L
    [(0,0),(-1,0),(-1,1),(0,-2),(-1,-2)],  // L -> 0
];
#[rustfmt::skip]
const JLSTZ_CCW: KickSet = [
    [(0,0),(1,0),(1,-1),(0,2),(1,2)],      // 0 -> L
    [(0,0),(1,0),(1,1),(0,-2),(1,-2)],     // R -> 0
    [(0,0),(-1,0),(-1,-1),(0,2),(-1,2)],   // 2 -> R
    [(0,0),(-1,0),(-1,1),(0,-2),(-1,-2)],  // L -> 2
];
#[rustfmt::skip]
const I_CW: KickSet = [
    [(0,0),(-2,0),(1,0),(-2,1),(1,-2)],    // 0 -> R
    [(0,0),(-1,0),(2,0),(-1,-2),(2,1)],    // R -> 2
    [(0,0),(2,0),(-1,0),(2,-1),(-1,2)],    // 2 -> L
    [(0,0),(1,0),(-2,0),(1,2),(-2,-1)],    // L -> 0
];
#[rustfmt::skip]
const I_CCW: KickSet = [
    [(0,0),(1,0),(-2,0),(1,2),(-2,-1)],    // 0 -> L
    [(0,0),(2,0),(-1,0),(2,-1),(-1,2)],    // R -> 0
    [(0,0),(1,0),(-2,0),(1,2),(-2,-1)],    // 2 -> R
    [(0,0),(-2,0),(1,0),(-2,1),(1,-2)],    // L -> 2
];

/// Wall-kick tests for rotating `kind` from `from`, in `cw` (else ccw) direction.
#[inline]
pub fn kicks(kind: Kind, from: Rotation, cw: bool) -> [(i8, i8); 5] {
    // O never needs kicks (it does not rotate visually).
    match (kind, cw) {
        (Kind::I, true) => I_CW[from.index()],
        (Kind::I, false) => I_CCW[from.index()],
        (_, true) => JLSTZ_CW[from.index()],
        (_, false) => JLSTZ_CCW[from.index()],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_piece_has_four_cells() {
        for k in Kind::ALL {
            for r in 0..4 {
                let c = cells(k, Rotation(r));
                assert_eq!(c.len(), 4);
                // All offsets fit within a 4x4 box.
                for (x, y) in c {
                    assert!((0..4).contains(&x) && (0..4).contains(&y));
                }
            }
        }
    }

    #[test]
    fn rotation_cycles() {
        let r = Rotation::SPAWN;
        assert_eq!(r.cw().cw().cw().cw(), Rotation::SPAWN);
        assert_eq!(r.ccw(), Rotation(3));
    }
}
