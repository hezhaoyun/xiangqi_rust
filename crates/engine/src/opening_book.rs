//! Implements an opening book for the Xiangqi engine.

use crate::r#move::Move;
use crate::bitboard::Board;
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::fs::File;
use std::io::{self, Read};

// Define the structure for a book entry
#[derive(Debug, Clone, Copy)]
pub struct BookEntry {
    pub hash: u64,
    pub mv: Move,
}

// The opening book, stored as a HashMap for quick lookup
pub static OPENING_BOOK: Lazy<HashMap<u64, Vec<Move>>> = Lazy::new(|| {
    let mut book = HashMap::new();
    // Attempt to load the book from a binary file
    if let Err(e) = load_opening_book_from_file(&mut book, "opening_book.bin") {
        eprintln!("Warning: Could not load opening book: {}", e);
    }
    book
});

fn load_opening_book_from_file(book: &mut HashMap<u64, Vec<Move>>, filename: &str) -> io::Result<()> {
    let mut file = File::open(filename)?;
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer)?;

    // Each entry is 16 bytes: u64 hash, u32 from_sq, u32 to_sq
    let entry_size = 16;
    if buffer.len() % entry_size != 0 {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "Invalid book file size"));
    }

    for chunk in buffer.chunks_exact(entry_size) {
        let hash = u64::from_le_bytes(chunk[0..8].try_into().unwrap());
        let from_sq = u32::from_le_bytes(chunk[8..12].try_into().unwrap()) as usize;
        let to_sq = u32::from_le_bytes(chunk[12..16].try_into().unwrap()) as usize;
        
        let mv = Move::new(from_sq, to_sq, None);
        book.entry(hash).or_default().push(mv);
    }

    Ok(())
}

/// Queries the opening book for a move in the current position.
/// Returns a random move from the book if found, otherwise None.
pub fn query_opening_book(board: &Board) -> Option<Move> {
    if let Some(moves) = OPENING_BOOK.get(&board.hash_key) {
        if !moves.is_empty() {
            // Return a random move from the list
            use rand::seq::SliceRandom;
            use rand::thread_rng;
            let mut rng = thread_rng();
            return moves.choose(&mut rng).copied();
        }
    }
    None
}
