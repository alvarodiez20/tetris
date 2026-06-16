//! 7-bag randomizer: each "bag" is a shuffled permutation of all seven pieces,
//! so droughts are bounded and the sequence stays fair. Deterministic given the
//! RNG seed.

use crate::piece::Kind;
use crate::rng::Rng;

pub struct Bag {
    rng: Rng,
    /// Upcoming pieces, front = next to spawn. Refilled a bag at a time.
    queue: Vec<Kind>,
}

impl Bag {
    pub fn new(seed: u64) -> Self {
        let mut bag = Bag {
            rng: Rng::new(seed),
            queue: Vec::with_capacity(14),
        };
        bag.refill();
        bag
    }

    fn refill(&mut self) {
        let mut pieces = Kind::ALL;
        // Fisher–Yates shuffle.
        for i in (1..pieces.len()).rev() {
            let j = self.rng.below(i + 1);
            pieces.swap(i, j);
        }
        self.queue.extend_from_slice(&pieces);
    }

    /// Pop the next piece, refilling when the current bag runs low so callers
    /// can always preview several pieces ahead.
    pub fn next(&mut self) -> Kind {
        if self.queue.len() <= 7 {
            self.refill();
        }
        self.queue.remove(0)
    }

    /// Peek the next `n` upcoming pieces without consuming them.
    pub fn preview(&mut self, n: usize) -> Vec<Kind> {
        while self.queue.len() < n {
            self.refill();
        }
        self.queue[..n].to_vec()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_bag_is_a_permutation() {
        let mut bag = Bag::new(123);
        let mut counts = [0u32; 7];
        for _ in 0..7 {
            counts[bag.next() as usize] += 1;
        }
        assert_eq!(counts, [1; 7], "first 7 pieces must be one of each");
    }

    #[test]
    fn deterministic_sequence() {
        let mut a = Bag::new(99);
        let mut b = Bag::new(99);
        for _ in 0..50 {
            assert_eq!(a.next(), b.next());
        }
    }
}
