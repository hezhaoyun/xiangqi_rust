//! Defines the representation of a move in the engine.

use crate::constants::Piece;

/// Represents a single move.
///
/// A move is encoded as a 16-bit integer:
/// - Bits 0-6:   from_sq (0-89)
/// - Bits 7-13:  to_sq (0-89)
/// - Bits 14-15: flags (e.g., capture, promotion - though Xiangqi has no promotion)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Move(u16);

impl Move {
    /// Creates a new move.
    pub fn new(from_sq: usize, to_sq: usize, captured_piece: Option<Piece>) -> Self {
        let mut move_val = (from_sq as u16) | ((to_sq as u16) << 7);
        if captured_piece.is_some() {
            // For now, we can use a simple flag for captures.
            // A more robust system could store the captured piece type.
            move_val |= 1 << 14;
        }
        Move(move_val)
    }

    /// Gets the source square.
    pub fn from_sq(&self) -> usize {
        (self.0 & 0x7F) as usize
    }

    /// Gets the destination square.
    pub fn to_sq(&self) -> usize {
        ((self.0 >> 7) & 0x7F) as usize
    }

    /// Checks if the move is a capture.
    pub fn is_capture(&self) -> bool {
        (self.0 >> 14) & 1 != 0
    }

    /// Returns a mirrored version of the move.
    pub fn mirrored(&self) -> Self {
        let from = self.from_sq();
        let to = self.to_sq();

        let mirrored_from = (from / 9) * 9 + (8 - (from % 9));
        let mirrored_to = (to / 9) * 9 + (8 - (to % 9));

        let mut move_val = (mirrored_from as u16) | ((mirrored_to as u16) << 7);
        if self.is_capture() {
            move_val |= 1 << 14;
        }
        Move(move_val)
    }

    pub fn to_uci_string(&self) -> String {
        let from_sq = self.from_sq();
        let to_sq = self.to_sq();
        let from_file = (from_sq % 9) as u8 + b'a';
        let from_rank = 9 - (from_sq / 9) as u8;
        let to_file = (to_sq % 9) as u8 + b'a';
        let to_rank = 9 - (to_sq / 9) as u8;
        format!("{}{}{}{}", from_file as char, from_rank, to_file as char, to_rank)
    }
}

