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
}
