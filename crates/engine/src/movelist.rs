//! A move list implementation that avoids heap allocations.

use crate::r#move::Move;
use std::ops::{Index, IndexMut};

const MAX_MOVES: usize = 256;

#[derive(Debug, Clone)]
pub struct MoveList {
    moves: [Move; MAX_MOVES],
    count: usize,
}

impl MoveList {
    pub fn new() -> Self {
        Self {
            moves: [Move::new(0, 0, None); MAX_MOVES],
            count: 0,
        }
    }

    pub fn add(&mut self, mv: Move) {
        if self.count < MAX_MOVES {
            self.moves[self.count] = mv;
            self.count += 1;
        }
    }

    pub fn len(&self) -> usize {
        self.count
    }

    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    pub fn as_slice(&self) -> &[Move] {
        &self.moves[0..self.count]
    }

    pub fn as_mut_slice(&mut self) -> &mut [Move] {
        &mut self.moves[0..self.count]
    }
}

impl Index<usize> for MoveList {
    type Output = Move;

    fn index(&self, index: usize) -> &Self::Output {
        &self.moves[index]
    }
}

impl IndexMut<usize> for MoveList {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.moves[index]
    }
}