//! Constants used in the Xiangqi engine.

// Using i8 to match the C implementation's enum values.
// Negative for Black, positive for Red.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i8)]
pub enum Piece {
    BKing = -1,
    BGuard = -2,
    BBishop = -3,
    BHorse = -4,
    BRook = -5,
    BCannon = -6,
    BPawn = -7,
    Empty = 0,
    RKing = 1,
    RGuard = 2,
    RBishop = 3,
    RHorse = 4,
    RRook = 5,
    RCannon = 6,
    RPawn = 7,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i8)]
pub enum Player {
    Red = 1,
    Black = -1,
}

// --- Search and Evaluation Constants ---
pub const MATE_VALUE: i32 = 10000;
pub const DRAW_VALUE: i32 = 0;

// --- Piece Base Values ---
// Indexed by `abs(piece as i8)`.
pub const PIECE_VALUES: [i32; 8] = [
    0,     // EMPTY
    0,     // KING (dummy value)
    100,   // GUARD
    100,   // BISHOP
    450,   // HORSE
    900,   // ROOK
    500,   // CANNON
    100,   // PAWN
];

impl Piece {
    /// Get the value of a piece.
    pub fn value(self) -> i32 {
        PIECE_VALUES[ (self as i8).abs() as usize ]
    }

    pub fn is_major(self) -> bool {
        matches!(self, Piece::RRook | Piece::BRook | Piece::RHorse | Piece::BHorse | Piece::RCannon | Piece::BCannon)
    }

    /// Get the player associated with a piece.
    /// Returns `None` if the piece is `Empty`.
    pub fn player(self) -> Option<Player> {
        if (self as i8) > 0 {
            Some(Player::Red)
        } else if (self as i8) < 0 {
            Some(Player::Black)
        } else {
            None
        }
    }
}

impl Player {
    /// Get the opponent of the current player.
    pub fn opponent(self) -> Player {
        match self {
            Player::Red => Player::Black,
            Player::Black => Player::Red,
        }
    }
}

// Add FEN character conversions to Piece
impl Piece {
    pub fn to_fen_char(self) -> char {
        match self {
            Piece::BKing => 'k',
            Piece::BGuard => 'a',
            Piece::BBishop => 'b',
            Piece::BHorse => 'n',
            Piece::BRook => 'r',
            Piece::BCannon => 'c',
            Piece::BPawn => 'p',
            Piece::Empty => '.',
            Piece::RKing => 'K',
            Piece::RGuard => 'A',
            Piece::RBishop => 'B',
            Piece::RHorse => 'N',
            Piece::RRook => 'R',
            Piece::RCannon => 'C',
            Piece::RPawn => 'P',
        }
    }

    pub fn from_fen_char(c: char) -> Option<Piece> {
        match c {
            'k' => Some(Piece::BKing),
            'a' => Some(Piece::BGuard),
            'b' => Some(Piece::BBishop),
            'n' => Some(Piece::BHorse),
            'r' => Some(Piece::BRook),
            'c' => Some(Piece::BCannon),
            'p' => Some(Piece::BPawn),
            'K' => Some(Piece::RKing),
            'A' => Some(Piece::RGuard),
            'B' => Some(Piece::RBishop),
            'N' => Some(Piece::RHorse),
            'R' => Some(Piece::RRook),
            'C' => Some(Piece::RCannon),
            'P' => Some(Piece::RPawn),
            _ => None,
        }
    }

    pub fn abs_val(self) -> u8 {
        (self as i8).abs() as u8
    }

    pub fn from_abs(val: i8) -> Self {
        match val {
            -1 => Piece::BKing, -2 => Piece::BGuard, -3 => Piece::BBishop, -4 => Piece::BHorse, -5 => Piece::BRook, -6 => Piece::BCannon, -7 => Piece::BPawn,
             1 => Piece::RKing,  2 => Piece::RGuard,  3 => Piece::RBishop,  4 => Piece::RHorse,  5 => Piece::RRook,  6 => Piece::RCannon,  7 => Piece::RPawn,
            _ => Piece::Empty,
        }
    }

    /// Gets the index into the `piece_bitboards` array for a given piece.
    pub fn get_bb_index(self) -> Option<usize> {
        let p_val = self as i8;
        if p_val > 0 {
            Some((p_val - 1) as usize)
        } else if p_val < 0 {
            Some((p_val.abs() - 1 + 7) as usize)
        } else {
            None
        }
    }

    /// Gets the index into the Zobrist key table for a given piece.
    pub fn get_zobrist_idx(self) -> Option<usize> {
        let p_val = self as i8;
        if p_val < 0 {
            Some((p_val.abs() - 1) as usize)
        } else if p_val > 0 {
            Some((p_val + 6) as usize)
        } else {
            None
        }
    }
}

impl Player {
    /// Gets the index for the color bitboard (0 for Red, 1 for Black).
    pub fn get_bb_idx(self) -> usize {
        if self == Player::Red { 0 } else { 1 }
    }
}

