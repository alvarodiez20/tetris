//! The playfield: a 10-wide grid with 20 visible rows plus a hidden buffer above
//! for spawning and rotation. Cells store a color id (0 = empty).

pub const WIDTH: usize = 10;
pub const VISIBLE_HEIGHT: usize = 20;
/// Hidden rows above the visible field (pieces spawn here).
pub const BUFFER_HEIGHT: usize = 20;
pub const HEIGHT: usize = VISIBLE_HEIGHT + BUFFER_HEIGHT; // 40

#[derive(Clone)]
pub struct Board {
    /// Row-major: `cells[y * WIDTH + x]`, y = 0 at the top of the buffer.
    cells: [u8; WIDTH * HEIGHT],
}

impl Default for Board {
    fn default() -> Self {
        Board {
            cells: [0; WIDTH * HEIGHT],
        }
    }
}

impl Board {
    #[inline]
    pub fn get(&self, x: i32, y: i32) -> u8 {
        if x < 0 || x >= WIDTH as i32 || y < 0 || y >= HEIGHT as i32 {
            // Treat out of bounds as solid so collision checks are simple.
            return u8::MAX;
        }
        self.cells[y as usize * WIDTH + x as usize]
    }

    #[inline]
    pub fn set(&mut self, x: i32, y: i32, v: u8) {
        if (0..WIDTH as i32).contains(&x) && (0..HEIGHT as i32).contains(&y) {
            self.cells[y as usize * WIDTH + x as usize] = v;
        }
    }

    #[inline]
    pub fn is_empty_cell(&self, x: i32, y: i32) -> bool {
        self.get(x, y) == 0
    }

    /// Clear all full rows, returning how many were cleared. Rows above shift down.
    pub fn clear_lines(&mut self) -> u32 {
        let mut cleared = 0;
        let mut write = HEIGHT as i32 - 1;
        for read in (0..HEIGHT as i32).rev() {
            let full = (0..WIDTH as i32).all(|x| !self.is_empty_cell(x, read));
            if full {
                cleared += 1;
                continue;
            }
            if write != read {
                for x in 0..WIDTH as i32 {
                    let v = self.get(x, read);
                    self.set(x, write, v);
                }
            }
            write -= 1;
        }
        // Zero out the rows left at the top.
        while write >= 0 {
            for x in 0..WIDTH as i32 {
                self.set(x, write, 0);
            }
            write -= 1;
        }
        cleared
    }

    /// Raw view of all cells (including buffer), row-major.
    pub fn cells(&self) -> &[u8; WIDTH * HEIGHT] {
        &self.cells
    }

    /// Copy the visible portion (bottom 20 rows), row-major, into a fresh vec.
    pub fn visible(&self) -> Vec<u8> {
        let start = BUFFER_HEIGHT * WIDTH;
        self.cells[start..].to_vec()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clears_a_full_bottom_row() {
        let mut b = Board::default();
        let y = HEIGHT as i32 - 1;
        for x in 0..WIDTH as i32 {
            b.set(x, y, 1);
        }
        // Put a single block one row above so we can confirm it shifts down.
        b.set(0, y - 1, 2);
        let cleared = b.clear_lines();
        assert_eq!(cleared, 1);
        assert_eq!(b.get(0, y), 2, "block above should fall into cleared row");
    }

    #[test]
    fn out_of_bounds_is_solid() {
        let b = Board::default();
        assert_eq!(b.get(-1, 0), u8::MAX);
        assert_eq!(b.get(0, HEIGHT as i32), u8::MAX);
    }
}
