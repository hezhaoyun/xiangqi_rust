//! The core board representation for the Xiangqi engine.

use crate::constants::{Piece, Player};
use crate::zobrist;
use std::fmt;

// C's __int128_t is u128 in Rust.
pub type Bitboard = u128;

const MAX_HISTORY: usize = 256;

// --- Pre-computed Masks ---
pub const SQUARE_MASKS: [Bitboard; 90] = {
    let mut masks = [0; 90];
    let mut i = 0;
    while i < 90 {
        masks[i] = 1 << i;
        i += 1;
    }
    masks
};

pub const RANK_MASKS: [Bitboard; 10] = {
    let mut masks = [0; 10];
    let mut i = 0;
    while i < 10 {
        masks[i] = 0x1FF << (i * 9);
        i += 1;
    }
    masks
};

/// Represents the state of the Xiangqi board at any point in time.
#[derive(Debug, Clone)]
pub struct Board {
    pub piece_bitboards: [Bitboard; 14],
    pub color_bitboards: [Bitboard; 2],
    pub board: [Piece; 90],
    pub player_to_move: Player,
    pub hash_key: u64,
    pub history: [u64; MAX_HISTORY],
    pub history_ply: usize,
}

impl Board {
    pub fn new() -> Self {
        Board {
            piece_bitboards: [0; 14],
            color_bitboards: [0; 2],
            board: [Piece::Empty; 90],
            player_to_move: Player::Red,
            hash_key: 0,
            history: [0; MAX_HISTORY],
            history_ply: 0,
        }
    }

    pub fn from_fen(fen: &str) -> Self {
        let mut board = Board::new();
        let mut parts = fen.split_whitespace();

        let layout = parts.next().unwrap();
        let mut rank = 0;
        let mut file = 0;
        for ch in layout.chars() {
            if ch == '/' {
                rank += 1;
                file = 0;
            } else if let Some(digit) = ch.to_digit(10) {
                file += digit as usize;
            } else {
                let piece = Piece::from_fen_char(ch).unwrap();
                let sq = rank * 9 + file;
                board.set_piece(sq, piece);
                file += 1;
            }
        }

        let player = parts.next().unwrap();
        board.player_to_move = if player == "w" {
            Player::Red
        } else {
            Player::Black
        };
        if board.player_to_move == Player::Black {
            board.hash_key ^= zobrist::ZOBRIST_PLAYER;
        }

        board.history[board.history_ply] = board.hash_key;
        board
    }

    fn set_piece(&mut self, sq: usize, piece: Piece) {
        let mask = SQUARE_MASKS[sq];
        let player = piece.player().unwrap();
        let r = sq / 9;
        let c = sq % 9;

        self.board[sq] = piece;
        self.piece_bitboards[piece.get_bb_index().unwrap()] |= mask;
        self.color_bitboards[player.get_bb_idx()] |= mask;
        self.hash_key ^= zobrist::ZOBRIST_KEYS[piece.get_zobrist_idx().unwrap()][r][c];
    }

    pub fn occupied_bitboard(&self) -> Bitboard {
        self.color_bitboards[0] | self.color_bitboards[1]
    }

    pub fn generate_pseudo_legal_moves(&self) -> Vec<crate::r#move::Move> {
        let mut moves = Vec::with_capacity(128);
        let player_idx = self.player_to_move.get_bb_idx();
        let own_pieces_bb = self.color_bitboards[player_idx];
        let opponent_pieces_bb = self.color_bitboards[1 - player_idx];
        let occupied = own_pieces_bb | opponent_pieces_bb;

        let (piece_start_idx, piece_end_idx) = if self.player_to_move == Player::Red {
            (0, 7)
        } else {
            (7, 14)
        };

        for i in piece_start_idx..piece_end_idx {
            let mut piece_bb = self.piece_bitboards[i];
            if piece_bb == 0 {
                continue;
            }
            let piece_type = self.board[piece_bb.trailing_zeros() as usize];

            while piece_bb != 0 {
                let from_sq = piece_bb.trailing_zeros() as usize;
                let mut moves_bb: Bitboard = 0;

                match piece_type {
                    Piece::RKing | Piece::BKing => {
                        moves_bb = crate::move_gen::ATTACK_TABLES.king[from_sq]
                    }
                    Piece::RGuard | Piece::BGuard => {
                        moves_bb = crate::move_gen::ATTACK_TABLES.guard[from_sq]
                    }
                    Piece::RBishop => {
                        let mut potential_moves = crate::move_gen::ATTACK_TABLES.bishop[from_sq];
                        potential_moves &= crate::move_gen::ATTACK_TABLES.red_half_mask;
                        while potential_moves != 0 {
                            let to_sq = potential_moves.trailing_zeros() as usize;
                            let leg_sq = crate::move_gen::ATTACK_TABLES.bishop_legs[from_sq][to_sq];
                            if (occupied & SQUARE_MASKS[leg_sq]) == 0 {
                                moves_bb |= SQUARE_MASKS[to_sq];
                            }
                            potential_moves &= !SQUARE_MASKS[to_sq];
                        }
                    }
                    Piece::BBishop => {
                        let mut potential_moves = crate::move_gen::ATTACK_TABLES.bishop[from_sq];
                        potential_moves &= crate::move_gen::ATTACK_TABLES.black_half_mask;
                        while potential_moves != 0 {
                            let to_sq = potential_moves.trailing_zeros() as usize;
                            let leg_sq = crate::move_gen::ATTACK_TABLES.bishop_legs[from_sq][to_sq];
                            if (occupied & SQUARE_MASKS[leg_sq]) == 0 {
                                moves_bb |= SQUARE_MASKS[to_sq];
                            }
                            potential_moves &= !SQUARE_MASKS[to_sq];
                        }
                    }
                    Piece::RHorse | Piece::BHorse => {
                        let mut potential_moves = crate::move_gen::ATTACK_TABLES.horse[from_sq];
                        while potential_moves != 0 {
                            let to_sq = potential_moves.trailing_zeros() as usize;
                            let leg_sq = crate::move_gen::ATTACK_TABLES.horse_legs[from_sq][to_sq];
                            if (occupied & SQUARE_MASKS[leg_sq]) == 0 {
                                moves_bb |= SQUARE_MASKS[to_sq];
                            }
                            potential_moves &= !SQUARE_MASKS[to_sq];
                        }
                    }
                    Piece::RPawn | Piece::BPawn => {
                        moves_bb = crate::move_gen::ATTACK_TABLES.pawn[player_idx][from_sq]
                    }
                    Piece::RRook | Piece::BRook => {
                        moves_bb = crate::move_gen::get_rook_moves_bb(from_sq, occupied)
                    }
                    Piece::RCannon | Piece::BCannon => {
                        moves_bb = crate::move_gen::get_cannon_moves_bb(from_sq, occupied)
                    }
                    _ => {}
                }

                let mut valid_moves_bb = moves_bb & !own_pieces_bb;
                while valid_moves_bb != 0 {
                    let to_sq = valid_moves_bb.trailing_zeros() as usize;
                    let captured_piece = if (opponent_pieces_bb & SQUARE_MASKS[to_sq]) != 0 {
                        Some(self.board[to_sq])
                    } else {
                        None
                    };
                    moves.push(crate::r#move::Move::new(from_sq, to_sq, captured_piece));
                    valid_moves_bb &= !SQUARE_MASKS[to_sq];
                }
                piece_bb &= !SQUARE_MASKS[from_sq];
            }
        }
        moves
    }

    pub fn move_piece(&mut self, mv: crate::r#move::Move) -> Piece {
        let from_sq = mv.from_sq();
        let to_sq = mv.to_sq();
        let moving_piece = self.board[from_sq];
        let captured_piece = self.board[to_sq];

        let r_from = from_sq / 9;
        let c_from = from_sq % 9;
        let r_to = to_sq / 9;
        let c_to = to_sq % 9;

        self.board[from_sq] = Piece::Empty;
        self.board[to_sq] = moving_piece;

        let moving_z_idx = moving_piece.get_zobrist_idx().unwrap();
        self.hash_key ^= zobrist::ZOBRIST_KEYS[moving_z_idx][r_from][c_from];
        self.hash_key ^= zobrist::ZOBRIST_KEYS[moving_z_idx][r_to][c_to];

        let move_mask = SQUARE_MASKS[from_sq] | SQUARE_MASKS[to_sq];
        self.piece_bitboards[moving_piece.get_bb_index().unwrap()] ^= move_mask;
        self.color_bitboards[self.player_to_move.get_bb_idx()] ^= move_mask;

        if captured_piece != Piece::Empty {
            let captured_z_idx = captured_piece.get_zobrist_idx().unwrap();
            self.hash_key ^= zobrist::ZOBRIST_KEYS[captured_z_idx][r_to][c_to];
            let captured_player = captured_piece.player().unwrap();
            self.piece_bitboards[captured_piece.get_bb_index().unwrap()] &= !SQUARE_MASKS[to_sq];
            self.color_bitboards[captured_player.get_bb_idx()] &= !SQUARE_MASKS[to_sq];
        }

        self.player_to_move = self.player_to_move.opponent();
        self.hash_key ^= zobrist::ZOBRIST_PLAYER;

        self.history_ply += 1;
        self.history[self.history_ply] = self.hash_key;

        captured_piece
    }

    pub fn unmove_piece(&mut self, mv: crate::r#move::Move, captured_piece: Piece) {
        self.history_ply -= 1;
        let from_sq = mv.from_sq();
        let to_sq = mv.to_sq();
        let moving_piece = self.board[to_sq];

        let r_from = from_sq / 9;
        let c_from = from_sq % 9;
        let r_to = to_sq / 9;
        let c_to = to_sq % 9;

        self.player_to_move = self.player_to_move.opponent();
        self.hash_key ^= zobrist::ZOBRIST_PLAYER;

        self.board[from_sq] = moving_piece;
        self.board[to_sq] = captured_piece;

        let move_mask = SQUARE_MASKS[from_sq] | SQUARE_MASKS[to_sq];
        self.piece_bitboards[moving_piece.get_bb_index().unwrap()] ^= move_mask;
        self.color_bitboards[moving_piece.player().unwrap().get_bb_idx()] ^= move_mask;
        let moving_z_idx = moving_piece.get_zobrist_idx().unwrap();
        self.hash_key ^= zobrist::ZOBRIST_KEYS[moving_z_idx][r_from][c_from];
        self.hash_key ^= zobrist::ZOBRIST_KEYS[moving_z_idx][r_to][c_to];

        if captured_piece != Piece::Empty {
            let captured_player = captured_piece.player().unwrap();
            self.piece_bitboards[captured_piece.get_bb_index().unwrap()] |= SQUARE_MASKS[to_sq];
            self.color_bitboards[captured_player.get_bb_idx()] |= SQUARE_MASKS[to_sq];
            let captured_z_idx = captured_piece.get_zobrist_idx().unwrap();
            self.hash_key ^= zobrist::ZOBRIST_KEYS[captured_z_idx][r_to][c_to];
        }
    }

    pub fn generate_legal_moves(&mut self) -> Vec<crate::r#move::Move> {
        let pseudo_legal_moves = self.generate_pseudo_legal_moves();
        let mut legal_moves = Vec::new();
        let player = self.player_to_move;

        for &mv in &pseudo_legal_moves {
            let captured = self.move_piece(mv);
            if !crate::move_gen::is_king_in_check(self, player) {
                legal_moves.push(mv);
            }
            self.unmove_piece(mv, captured);
        }
        legal_moves
    }

    /// Generates all pseudo-legal capture moves for the current player.
    pub fn generate_capture_moves(&self) -> Vec<crate::r#move::Move> {
        let mut moves = Vec::with_capacity(32);
        let player_idx = self.player_to_move.get_bb_idx();
        let own_pieces_bb = self.color_bitboards[player_idx];
        let opponent_pieces_bb = self.color_bitboards[1 - player_idx];
        let occupied = own_pieces_bb | opponent_pieces_bb;

        let (piece_start_idx, piece_end_idx) = if self.player_to_move == Player::Red {
            (0, 7)
        } else {
            (7, 14)
        };

        for i in piece_start_idx..piece_end_idx {
            let mut piece_bb = self.piece_bitboards[i];
            if piece_bb == 0 {
                continue;
            }
            let piece_type = self.board[piece_bb.trailing_zeros() as usize];

            while piece_bb != 0 {
                let from_sq = piece_bb.trailing_zeros() as usize;
                let mut moves_bb: Bitboard = 0;

                match piece_type {
                    Piece::RKing | Piece::BKing => {
                        moves_bb = crate::move_gen::ATTACK_TABLES.king[from_sq]
                    }
                    Piece::RGuard | Piece::BGuard => {
                        moves_bb = crate::move_gen::ATTACK_TABLES.guard[from_sq]
                    }
                    Piece::RBishop => {
                        let mut potential_moves = crate::move_gen::ATTACK_TABLES.bishop[from_sq];
                        potential_moves &= crate::move_gen::ATTACK_TABLES.red_half_mask;
                        while potential_moves != 0 {
                            let to_sq = potential_moves.trailing_zeros() as usize;
                            let leg_sq = crate::move_gen::ATTACK_TABLES.bishop_legs[from_sq][to_sq];
                            if (occupied & SQUARE_MASKS[leg_sq]) == 0 { moves_bb |= SQUARE_MASKS[to_sq]; }
                            potential_moves &= !SQUARE_MASKS[to_sq];
                        }
                    }
                    Piece::BBishop => {
                        let mut potential_moves = crate::move_gen::ATTACK_TABLES.bishop[from_sq];
                        potential_moves &= crate::move_gen::ATTACK_TABLES.black_half_mask;
                        while potential_moves != 0 {
                            let to_sq = potential_moves.trailing_zeros() as usize;
                            let leg_sq = crate::move_gen::ATTACK_TABLES.bishop_legs[from_sq][to_sq];
                            if (occupied & SQUARE_MASKS[leg_sq]) == 0 { moves_bb |= SQUARE_MASKS[to_sq]; }
                            potential_moves &= !SQUARE_MASKS[to_sq];
                        }
                    }
                    Piece::RHorse | Piece::BHorse => {
                        let mut potential_moves = crate::move_gen::ATTACK_TABLES.horse[from_sq];
                        while potential_moves != 0 {
                            let to_sq = potential_moves.trailing_zeros() as usize;
                            let leg_sq = crate::move_gen::ATTACK_TABLES.horse_legs[from_sq][to_sq];
                            if (occupied & SQUARE_MASKS[leg_sq]) == 0 {
                                moves_bb |= SQUARE_MASKS[to_sq];
                            }
                            potential_moves &= !SQUARE_MASKS[to_sq];
                        }
                    }
                    Piece::RPawn | Piece::BPawn => {
                        moves_bb = crate::move_gen::ATTACK_TABLES.pawn[player_idx][from_sq]
                    }
                    Piece::RRook | Piece::BRook => {
                        moves_bb = crate::move_gen::get_rook_moves_bb(from_sq, occupied)
                    }
                    Piece::RCannon | Piece::BCannon => {
                        moves_bb = crate::move_gen::get_cannon_moves_bb(from_sq, occupied)
                    }
                    _ => {}
                }

                let mut capture_moves_bb = moves_bb & opponent_pieces_bb;

                while capture_moves_bb != 0 {
                    let to_sq = capture_moves_bb.trailing_zeros() as usize;
                    moves.push(crate::r#move::Move::new(
                        from_sq,
                        to_sq,
                        Some(self.board[to_sq]),
                    ));
                    capture_moves_bb &= !SQUARE_MASKS[to_sq];
                }

                piece_bb &= !SQUARE_MASKS[from_sq];
            }
        }
        moves
    }
}

impl fmt::Display for Board {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(
            f,
            "(Player: {:?}, Hash: {:016x})",
            self.player_to_move, self.hash_key
        )?;
        writeln!(f, "  +-------------------+")?;
        for r in 0..10 {
            write!(f, "{} | ", 9 - r)?;
            for c in 0..9 {
                let piece = self.board[r * 9 + c];
                write!(f, "{} ", piece.to_fen_char())?;
            }
            writeln!(f, "|")?;
        }
        writeln!(f, "  +-------------------+")?;
        writeln!(f, "    a b c d e f g h i")
    }
}

// --- Bitboard Helper Functions ---

#[inline]
pub fn popcount(bb: Bitboard) -> u32 {
    bb.count_ones()
}

#[inline]
pub fn get_lsb_index(bb: Bitboard) -> i32 {
    if bb == 0 {
        -1
    } else {
        bb.trailing_zeros() as i32
    }
}
