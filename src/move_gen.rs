//! Move generation logic, including pre-computed attack tables.

use crate::bitboard::{Bitboard, SQUARE_MASKS};
use once_cell::sync::Lazy;

// Helper functions matching the C implementation
pub const fn sq_to_idx(r: usize, c: usize) -> usize { r * 9 + c }
const fn is_valid(r: isize, c: isize) -> bool { r >= 0 && r < 10 && c >= 0 && c < 9 }


/// A struct to hold all the pre-computed attack tables.
/// The tables are initialized once and then accessed globally.
pub struct AttackTables {
    pub king: [Bitboard; 90],
    pub guard: [Bitboard; 90],
    pub bishop: [Bitboard; 90],
    pub bishop_legs: [[usize; 90]; 90],
    pub horse: [Bitboard; 90],
    pub horse_legs: [[usize; 90]; 90],
    pub pawn: [[Bitboard; 90]; 2], // [player_idx][square]
    pub rays: [[Bitboard; 90]; 4], // [direction][square]
    pub red_half_mask: Bitboard,
    pub black_half_mask: Bitboard,
}

impl AttackTables {
    fn new() -> Self {
        let mut tables = AttackTables {
            king: [0; 90],
            guard: [0; 90],
            bishop: [0; 90],
            bishop_legs: [[0; 90]; 90],
            horse: [0; 90],
            horse_legs: [[0; 90]; 90],
            pawn: [[0; 90]; 2],
            rays: [[0; 90]; 4],
            red_half_mask: 0,
            black_half_mask: 0,
        };

        // Precompute King and Guard attacks
        for r in 0..10 {
            for c in 0..9 {
                let sq = sq_to_idx(r, c);
                // King
                for (dr, dc) in [(0, 1), (0, -1), (1, 0), (-1, 0)] {
                    let (nr, nc) = (r as isize + dr, c as isize + dc);
                    if nc >= 3 && nc <= 5 && ((nr >= 0 && nr <= 2) || (nr >= 7 && nr <= 9)) {
                        tables.king[sq] |= SQUARE_MASKS[sq_to_idx(nr as usize, nc as usize)];
                    }
                }
                // Guard
                for (dr, dc) in [(1, 1), (1, -1), (-1, 1), (-1, -1)] {
                    let (nr, nc) = (r as isize + dr, c as isize + dc);
                    if nc >= 3 && nc <= 5 && ((nr >= 0 && nr <= 2) || (nr >= 7 && nr <= 9)) {
                        tables.guard[sq] |= SQUARE_MASKS[sq_to_idx(nr as usize, nc as usize)];
                    }
                }
            }
        }

        // Precompute Bishop and Horse attacks
        for r in 0..10 {
            for c in 0..9 {
                let from_sq = sq_to_idx(r, c);
                // Bishop
                for (dr, dc) in [(2, 2), (2, -2), (-2, 2), (-2, -2)] {
                    let (nr, nc) = (r as isize + dr, c as isize + dc);
                    if is_valid(nr, nc) {
                        let to_sq = sq_to_idx(nr as usize, nc as usize);
                        tables.bishop[from_sq] |= SQUARE_MASKS[to_sq];
                        let leg_sq = sq_to_idx((r as isize + dr / 2) as usize, (c as isize + dc / 2) as usize);
                        tables.bishop_legs[from_sq][to_sq] = leg_sq;
                    }
                }
                // Horse
                for (dr, dc) in [(2, 1), (2, -1), (-2, 1), (-2, -1), (1, 2), (1, -2), (-1, 2), (-1, -2)] {
                    let (nr, nc) = (r as isize + dr, c as isize + dc);
                    if is_valid(nr, nc) {
                        let to_sq = sq_to_idx(nr as usize, nc as usize);
                        tables.horse[from_sq] |= SQUARE_MASKS[to_sq];
                        let (leg_r, leg_c) = if dr.abs() == 2 { (r as isize + dr/2, c as isize) } else { (r as isize, c as isize + dc/2) };
                        tables.horse_legs[from_sq][to_sq] = sq_to_idx(leg_r as usize, leg_c as usize);
                    }
                }
            }
        }

        // Precompute Pawn attacks
        for r in 0..10 {
            for c in 0..9 {
                let sq = sq_to_idx(r, c);
                // Red Pawn (player_idx 0)
                if is_valid(r as isize - 1, c as isize) { tables.pawn[0][sq] |= SQUARE_MASKS[sq_to_idx(r - 1, c)]; }
                if r < 5 { // Crossed river
                    if is_valid(r as isize, c as isize - 1) { tables.pawn[0][sq] |= SQUARE_MASKS[sq_to_idx(r, c - 1)]; }
                    if is_valid(r as isize, c as isize + 1) { tables.pawn[0][sq] |= SQUARE_MASKS[sq_to_idx(r, c + 1)]; }
                }
                // Black Pawn (player_idx 1)
                if is_valid(r as isize + 1, c as isize) { tables.pawn[1][sq] |= SQUARE_MASKS[sq_to_idx(r + 1, c)]; }
                if r > 4 { // Crossed river
                    if is_valid(r as isize, c as isize - 1) { tables.pawn[1][sq] |= SQUARE_MASKS[sq_to_idx(r, c - 1)]; }
                    if is_valid(r as isize, c as isize + 1) { tables.pawn[1][sq] |= SQUARE_MASKS[sq_to_idx(r, c + 1)]; }
                }
            }
        }

        // Precompute Rays for sliding pieces
        for sq in 0..90 {
            let (r, c) = (sq / 9, sq % 9);
            for i in (0..r).rev() { tables.rays[0][sq] |= SQUARE_MASKS[sq_to_idx(i, c)]; } // North
            for i in (c + 1)..9 { tables.rays[1][sq] |= SQUARE_MASKS[sq_to_idx(r, i)]; } // East
            for i in (r + 1)..10 { tables.rays[2][sq] |= SQUARE_MASKS[sq_to_idx(i, c)]; } // South
            for i in (0..c).rev() { tables.rays[3][sq] |= SQUARE_MASKS[sq_to_idx(r, i)]; } // West
        }

        // Precompute side masks
        for i in 0..45 { tables.black_half_mask |= SQUARE_MASKS[i]; } // Ranks 9-5 (Black's side)
        for i in 45..90 { tables.red_half_mask |= SQUARE_MASKS[i]; } // Ranks 4-0 (Red's side)

        tables
    }
}

// The global static instance of the attack tables, initialized lazily and only once.
pub static ATTACK_TABLES: Lazy<AttackTables> = Lazy::new(AttackTables::new);

/// Generates the attack bitboard for a rook on a given square.
pub fn get_rook_moves_bb(sq: usize, occupied: Bitboard) -> Bitboard {
    let mut final_attacks = 0;

    // North
    let ray = ATTACK_TABLES.rays[0][sq];
    let blockers = occupied & ray;
    if blockers != 0 {
        let first_blocker = 127 - blockers.leading_zeros() as usize;
        final_attacks |= (ray ^ ATTACK_TABLES.rays[0][first_blocker]) | SQUARE_MASKS[first_blocker];
    } else {
        final_attacks |= ray;
    }

    // East
    let ray = ATTACK_TABLES.rays[1][sq];
    let blockers = occupied & ray;
    if blockers != 0 {
        let first_blocker = blockers.trailing_zeros() as usize;
        final_attacks |= (ray ^ ATTACK_TABLES.rays[1][first_blocker]) | SQUARE_MASKS[first_blocker];
    } else {
        final_attacks |= ray;
    }

    // South
    let ray = ATTACK_TABLES.rays[2][sq];
    let blockers = occupied & ray;
    if blockers != 0 {
        let first_blocker = blockers.trailing_zeros() as usize;
        final_attacks |= (ray ^ ATTACK_TABLES.rays[2][first_blocker]) | SQUARE_MASKS[first_blocker];
    } else {
        final_attacks |= ray;
    }

    // West
    let ray = ATTACK_TABLES.rays[3][sq];
    let blockers = occupied & ray;
    if blockers != 0 {
        let first_blocker = 127 - blockers.leading_zeros() as usize;
        final_attacks |= (ray ^ ATTACK_TABLES.rays[3][first_blocker]) | SQUARE_MASKS[first_blocker];
    } else {
        final_attacks |= ray;
    }

    final_attacks
}

/// Generates the attack bitboard for a cannon on a given square.
pub fn get_cannon_moves_bb(sq: usize, occupied: Bitboard) -> Bitboard {
    let mut attacks = 0;

    // North
    let ray = ATTACK_TABLES.rays[0][sq];
    let blockers = occupied & ray;
    if blockers != 0 {
        let screen = 127 - blockers.leading_zeros() as usize;
        attacks |= (ray ^ ATTACK_TABLES.rays[0][screen]) ^ SQUARE_MASKS[screen];
        let remaining_blockers = blockers ^ SQUARE_MASKS[screen];
        if remaining_blockers != 0 {
            let target = 127 - remaining_blockers.leading_zeros() as usize;
            attacks |= SQUARE_MASKS[target];
        }
    } else {
        attacks |= ray;
    }

    // East
    let ray = ATTACK_TABLES.rays[1][sq];
    let blockers = occupied & ray;
    if blockers != 0 {
        let screen = blockers.trailing_zeros() as usize;
        attacks |= (ray ^ ATTACK_TABLES.rays[1][screen]) ^ SQUARE_MASKS[screen];
        let remaining_blockers = blockers ^ SQUARE_MASKS[screen];
        if remaining_blockers != 0 {
            let target = remaining_blockers.trailing_zeros() as usize;
            attacks |= SQUARE_MASKS[target];
        }
    } else {
        attacks |= ray;
    }

    // South
    let ray = ATTACK_TABLES.rays[2][sq];
    let blockers = occupied & ray;
    if blockers != 0 {
        let screen = blockers.trailing_zeros() as usize;
        attacks |= (ray ^ ATTACK_TABLES.rays[2][screen]) ^ SQUARE_MASKS[screen];
        let remaining_blockers = blockers ^ SQUARE_MASKS[screen];
        if remaining_blockers != 0 {
            let target = remaining_blockers.trailing_zeros() as usize;
            attacks |= SQUARE_MASKS[target];
        }
    } else {
        attacks |= ray;
    }

    // West
    let ray = ATTACK_TABLES.rays[3][sq];
    let blockers = occupied & ray;
    if blockers != 0 {
        let screen = 127 - blockers.leading_zeros() as usize;
        attacks |= (ray ^ ATTACK_TABLES.rays[3][screen]) ^ SQUARE_MASKS[screen];
        let remaining_blockers = blockers ^ SQUARE_MASKS[screen];
        if remaining_blockers != 0 {
            let target = 127 - remaining_blockers.leading_zeros() as usize;
            attacks |= SQUARE_MASKS[target];
        }
    } else {
        attacks |= ray;
    }

    attacks
}

/// Checks if a given square is attacked by the specified player.
pub fn is_square_attacked_by(board: &crate::bitboard::Board, sq: usize, attacker_player: crate::constants::Player) -> bool {
    let occupied = board.occupied_bitboard();
    let attacker_idx = attacker_player.get_bb_idx();
    let defender_idx = 1 - attacker_idx;

    // Attacked by Pawns (using reverse lookup)
    let pawn_type = if attacker_player == crate::constants::Player::Red { crate::constants::Piece::RPawn } else { crate::constants::Piece::BPawn };
    if (ATTACK_TABLES.pawn[defender_idx][sq] & board.piece_bitboards[pawn_type.get_bb_index().unwrap()]) != 0 {
        return true;
    }

    // Attacked by King
    let king_type = if attacker_player == crate::constants::Player::Red { crate::constants::Piece::RKing } else { crate::constants::Piece::BKing };
    if (ATTACK_TABLES.king[sq] & board.piece_bitboards[king_type.get_bb_index().unwrap()]) != 0 {
        return true;
    }

    // Attacked by Horse
    let horse_type = if attacker_player == crate::constants::Player::Red { crate::constants::Piece::RHorse } else { crate::constants::Piece::BHorse };
    let mut potential_horses = ATTACK_TABLES.horse[sq] & board.piece_bitboards[horse_type.get_bb_index().unwrap()];
    while potential_horses != 0 {
        let from_sq = potential_horses.trailing_zeros() as usize;
        let leg_sq = ATTACK_TABLES.horse_legs[from_sq][sq];
        if (occupied & SQUARE_MASKS[leg_sq]) == 0 {
            return true;
        }
        potential_horses &= !SQUARE_MASKS[from_sq];
    }

    // Attacked by Bishop
    let bishop_type = if attacker_player == crate::constants::Player::Red { crate::constants::Piece::RBishop } else { crate::constants::Piece::BBishop };
    let mut potential_bishops = ATTACK_TABLES.bishop[sq] & board.piece_bitboards[bishop_type.get_bb_index().unwrap()];
    if potential_bishops != 0 {
        let side_mask = if attacker_player == crate::constants::Player::Red { ATTACK_TABLES.red_half_mask } else { ATTACK_TABLES.black_half_mask };
        if (side_mask & SQUARE_MASKS[sq]) != 0 { // Bishops can only attack on their own side
            while potential_bishops != 0 {
                let from_sq = potential_bishops.trailing_zeros() as usize;
                let leg_sq = ATTACK_TABLES.bishop_legs[from_sq][sq];
                if (occupied & SQUARE_MASKS[leg_sq]) == 0 {
                    return true;
                }
                potential_bishops &= !SQUARE_MASKS[from_sq];
            }
        }
    }

    // Attacked by Rook
    let rook_type = if attacker_player == crate::constants::Player::Red { crate::constants::Piece::RRook } else { crate::constants::Piece::BRook };
    if (get_rook_moves_bb(sq, occupied) & board.piece_bitboards[rook_type.get_bb_index().unwrap()]) != 0 {
        return true;
    }

    // Attacked by Cannon
    let cannon_type = if attacker_player == crate::constants::Player::Red { crate::constants::Piece::RCannon } else { crate::constants::Piece::BCannon };
    if (get_cannon_moves_bb(sq, occupied) & board.piece_bitboards[cannon_type.get_bb_index().unwrap()]) != 0 {
        return true;
    }

    false
}

pub fn is_king_in_check(board: &crate::bitboard::Board, player: crate::constants::Player) -> bool {
    let king_piece = if player == crate::constants::Player::Red { crate::constants::Piece::RKing } else { crate::constants::Piece::BKing };
    let king_bb = board.piece_bitboards[king_piece.get_bb_index().unwrap()];
    if king_bb == 0 { return true; } // Should not happen
    let king_sq = king_bb.trailing_zeros() as usize;

    // 1. Check if attacked by opponent's pieces
    if is_square_attacked_by(board, king_sq, player.opponent()) {
        return true;
    }

    // 2. Check for "flying general"
    let opponent_king_piece = if player == crate::constants::Player::Red { crate::constants::Piece::BKing } else { crate::constants::Piece::RKing };
    let opponent_king_bb = board.piece_bitboards[opponent_king_piece.get_bb_index().unwrap()];
    if opponent_king_bb == 0 { return false; } // No opponent king, no check
    let opponent_king_sq = opponent_king_bb.trailing_zeros() as usize;

    if (king_sq % 9) != (opponent_king_sq % 9) {
        return false;
    }

    let occupied = board.occupied_bitboard();
    let min_sq = king_sq.min(opponent_king_sq);
    let max_sq = king_sq.max(opponent_king_sq);
    
    let mut between_mask = 0;
    for s in (min_sq + 9)..max_sq {
        if s % 9 == king_sq % 9 { // Ensure it's on the same file
            between_mask |= crate::bitboard::SQUARE_MASKS[s];
        }
    }

    if (occupied & between_mask) == 0 {
        return true; // Flying general check
    }

    false
}



