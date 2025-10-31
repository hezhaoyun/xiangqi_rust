//! Transposition Table for caching search results.

use crate::r#move::Move;

// Transposition Table Entry Flags
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TtFlag {
    Exact,
    LowerBound, // Alpha
    UpperBound, // Beta
}

/// A single entry in the transposition table.
#[derive(Debug, Clone, Copy)]
pub struct TtEntry {
    pub hash_key: u64,
    pub depth: i32,
    pub score: i32,
    pub flag: TtFlag,
    pub best_move: Move,
}

impl TtEntry {
    pub fn new_empty() -> Self {
        Self {
            hash_key: 0,
            depth: 0,
            score: 0,
            flag: TtFlag::Exact,
            best_move: Move::new(0, 0, None), // Represents a null move
        }
    }
}

/// The transposition table itself.
pub struct TranspositionTable {
    entries: Vec<TtEntry>,
}

impl TranspositionTable {
    /// Creates a new transposition table with a given size in MB.
    pub fn new(size_mb: usize) -> Self {
        let num_entries = size_mb * 1024 * 1024 / std::mem::size_of::<TtEntry>();
        Self {
            entries: vec![TtEntry::new_empty(); num_entries],
        }
    }

    /// Probes the transposition table for a given hash key.
    pub fn probe(&self, hash_key: u64) -> Option<&TtEntry> {
        let index = hash_key as usize % self.entries.len();
        let entry = &self.entries[index];
        if entry.hash_key == hash_key {
            Some(entry)
        } else {
            None
        }
    }

    /// Stores an entry in the transposition table.
    pub fn store(&mut self, hash_key: u64, depth: i32, score: i32, flag: TtFlag, best_move: Move) {
        let index = hash_key as usize % self.entries.len();
        let entry = &self.entries[index];

        // Depth-preferred replacement strategy
        if depth >= entry.depth {
            self.entries[index] = TtEntry { hash_key, depth, score, flag, best_move };
        }
    }

    pub fn clear(&mut self) {
        self.entries.iter_mut().for_each(|entry| *entry = TtEntry::new_empty());
    }
}
