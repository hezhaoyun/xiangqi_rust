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
    pub material_score: i32, // Score for material balance
    pub mg_pst_score: i32,   // Midgame score from piece-square tables
    pub eg_pst_score: i32,   // Endgame score from piece-square tables
}

impl Board {
    pub fn new() -> Self {
        Self {
            piece_bitboards: [0; 14],
            color_bitboards: [0; 2],
            board: [Piece::Empty; 90],
            player_to_move: Player::Red,
            hash_key: 0,
            history: [0; MAX_HISTORY],
            history_ply: 0,
            material_score: 0,
            mg_pst_score: 0,
            eg_pst_score: 0,
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

        // Calculate and store the initial evaluation scores
        let (material, mg_pst, eg_pst) = crate::evaluate::calculate_full_scores(&board);
        board.material_score = material;
        board.mg_pst_score = mg_pst;
        board.eg_pst_score = eg_pst;

        board.history[board.history_ply] = board.hash_key;
        board
    }

    pub fn to_fen(&self) -> String {
        let mut fen = String::with_capacity(128);
        for r in 0..10 {
            let mut empty_count = 0;
            for c in 0..9 {
                let piece = self.board[r * 9 + c];
                if piece == Piece::Empty {
                    empty_count += 1;
                } else {
                    if empty_count > 0 {
                        fen.push_str(&empty_count.to_string());
                        empty_count = 0;
                    }
                    fen.push(piece.to_fen_char());
                }
            }
            if empty_count > 0 {
                fen.push_str(&empty_count.to_string());
            }
            if r < 9 {
                fen.push('/');
            }
        }

        // Active color
        fen.push(' ');
        fen.push(if self.player_to_move == Player::Red { 'w' } else { 'b' });

        // Other fields (can be placeholders as they are not used by this engine)
        fen.push_str(" - - 0 1");

        fen
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

    pub fn move_piece(&mut self, mv: crate::r#move::Move) -> Piece {
        let from_sq = mv.from_sq();
        let to_sq = mv.to_sq();
        let moving_piece = self.board[from_sq];
        let captured_piece = self.board[to_sq];

        // --- Incremental Score Update (Remove moving piece from from_sq) ---
        let (mg_from, eg_from) = crate::evaluate::get_pst_scores(moving_piece, from_sq);
        self.mg_pst_score -= mg_from;
        self.eg_pst_score -= eg_from;

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
            // --- Incremental Score Update (Capture) ---
            let captured_value = crate::evaluate::MATERIAL_VALUES[captured_piece.abs_val() as usize];
            if captured_piece.player().unwrap() == Player::Black {
                self.material_score += captured_value;
            } else {
                self.material_score -= captured_value;
            }
            let (mg_cap, eg_cap) = crate::evaluate::get_pst_scores(captured_piece, to_sq);
            self.mg_pst_score -= mg_cap;
            self.eg_pst_score -= eg_cap;

            let captured_z_idx = captured_piece.get_zobrist_idx().unwrap();
            self.hash_key ^= zobrist::ZOBRIST_KEYS[captured_z_idx][r_to][c_to];
            let captured_player = captured_piece.player().unwrap();
            self.piece_bitboards[captured_piece.get_bb_index().unwrap()] &= !SQUARE_MASKS[to_sq];
            self.color_bitboards[captured_player.get_bb_idx()] &= !SQUARE_MASKS[to_sq];
        }

        // --- Incremental Score Update (Add moving piece to to_sq) ---
        let (mg_to, eg_to) = crate::evaluate::get_pst_scores(moving_piece, to_sq);
        self.mg_pst_score += mg_to;
        self.eg_pst_score += eg_to;

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

        // --- Incremental Score Update (Remove moving piece from to_sq) ---
        let (mg_to, eg_to) = crate::evaluate::get_pst_scores(moving_piece, to_sq);
        self.mg_pst_score -= mg_to;
        self.eg_pst_score -= eg_to;

        self.board[from_sq] = moving_piece;
        self.board[to_sq] = captured_piece;

        let move_mask = SQUARE_MASKS[from_sq] | SQUARE_MASKS[to_sq];
        self.piece_bitboards[moving_piece.get_bb_index().unwrap()] ^= move_mask;
        self.color_bitboards[moving_piece.player().unwrap().get_bb_idx()] ^= move_mask;
        let moving_z_idx = moving_piece.get_zobrist_idx().unwrap();
        self.hash_key ^= zobrist::ZOBRIST_KEYS[moving_z_idx][r_from][c_from];
        self.hash_key ^= zobrist::ZOBRIST_KEYS[moving_z_idx][r_to][c_to];

        // --- Incremental Score Update (Add moving piece to from_sq) ---
        let (mg_from, eg_from) = crate::evaluate::get_pst_scores(moving_piece, from_sq);
        self.mg_pst_score += mg_from;
        self.eg_pst_score += eg_from;

        if captured_piece != Piece::Empty {
            // --- Incremental Score Update (Restore captured piece) ---
            let captured_value = crate::evaluate::MATERIAL_VALUES[captured_piece.abs_val() as usize];
            if captured_piece.player().unwrap() == Player::Black {
                self.material_score -= captured_value;
            } else {
                self.material_score += captured_value;
            }
            let (mg_cap, eg_cap) = crate::evaluate::get_pst_scores(captured_piece, to_sq);
            self.mg_pst_score += mg_cap;
            self.eg_pst_score += eg_cap;

            let captured_player = captured_piece.player().unwrap();
            self.piece_bitboards[captured_piece.get_bb_index().unwrap()] |= SQUARE_MASKS[to_sq];
            self.color_bitboards[captured_player.get_bb_idx()] |= SQUARE_MASKS[to_sq];
            let captured_z_idx = captured_piece.get_zobrist_idx().unwrap();
            self.hash_key ^= zobrist::ZOBRIST_KEYS[captured_z_idx][r_to][c_to];
        }
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

    /// Generates all pseudo-legal quiet (non-capture) moves for the current player.
    pub fn generate_quiet_moves(&self) -> Vec<crate::r#move::Move> {
        let mut moves = Vec::with_capacity(64);
        let player_idx = self.player_to_move.get_bb_idx();
        let own_pieces_bb = self.color_bitboards[player_idx];
        let opponent_pieces_bb = self.color_bitboards[1 - player_idx];
        let occupied = own_pieces_bb | opponent_pieces_bb;
        let empty_squares = !occupied;

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

                let mut quiet_moves_bb = moves_bb & empty_squares;

                while quiet_moves_bb != 0 {
                    let to_sq = quiet_moves_bb.trailing_zeros() as usize;
                    moves.push(crate::r#move::Move::new(
                        from_sq,
                        to_sq,
                        None,
                    ));
                    quiet_moves_bb &= !SQUARE_MASKS[to_sq];
                }

                piece_bb &= !SQUARE_MASKS[from_sq];
            }
        }
        moves
    }

    pub fn generate_legal_moves(&mut self) -> Vec<crate::r#move::Move> {
        let mut legal_moves = Vec::with_capacity(128);
        let player = self.player_to_move;

        let mut pseudo_legal_moves = self.generate_capture_moves();
        pseudo_legal_moves.extend(self.generate_quiet_moves());

        for &mv in &pseudo_legal_moves {
            let captured = self.move_piece(mv);
            if !crate::move_gen::is_king_in_check(self, player) {
                legal_moves.push(mv);
            }
            self.unmove_piece(mv, captured);
        }
        legal_moves
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
