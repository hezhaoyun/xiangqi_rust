//! Evaluates a board position and returns a score.

use crate::move_gen;
use crate::move_gen::sq_to_idx;
use crate::bitboard::{self, Board};
use crate::constants::{Piece, Player};

// --- Piece Values ---
pub const MATERIAL_VALUES: [i32; 8] = [0, 10000, 200, 200, 450, 900, 500, 100]; // Indexed by Piece type (abs value)

// --- Piece-Square Tables (Midgame) ---
// From Red's perspective (bottom of the board)

const KING_PST_MG: [[i32; 9]; 10] = [
    [0, 0, 0, 8, 8, 8, 0, 0, 0],
    [0, 0, 0, 8, 8, 8, 0, 0, 0],
    [0, 0, 0, 6, 6, 6, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 6, 6, 6, 0, 0, 0],
    [0, 0, 0, 8, 8, 8, 0, 0, 0],
    [0, 0, 0, 8, 8, 8, 0, 0, 0],
];

const GUARD_PST_MG: [[i32; 9]; 10] = [
    [0, 0, 0, 20, 0, 20, 0, 0, 0],
    [0, 0, 0, 0, 23, 0, 0, 0, 0],
    [0, 0, 0, 20, 0, 20, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 20, 0, 20, 0, 0, 0],
    [0, 0, 0, 0, 23, 0, 0, 0, 0],
    [0, 0, 0, 20, 0, 20, 0, 0, 0],
];

const BISHOP_PST_MG: [[i32; 9]; 10] = [
    [0, 0, 20, 0, 0, 0, 20, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 23, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 20, 0, 0, 0, 20, 0, 0],
    [0, 0, 20, 0, 0, 0, 20, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 23, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 20, 0, 0, 0, 20, 0, 0],
];

const HORSE_PST_MG: [[i32; 9]; 10] = [
    [90, 90, 90, 96, 90, 96, 90, 90, 90],
    [90, 96, 103, 97, 94, 97, 103, 96, 90],
    [92, 98, 99, 103, 99, 103, 99, 98, 92],
    [93, 108, 100, 107, 100, 107, 100, 108, 93],
    [90, 100, 99, 103, 104, 103, 99, 100, 90],
    [90, 98, 101, 102, 103, 102, 101, 98, 90],
    [92, 94, 98, 95, 98, 95, 98, 94, 92],
    [93, 92, 94, 95, 92, 95, 94, 92, 93],
    [85, 90, 92, 93, 78, 93, 92, 90, 85],
    [88, 85, 90, 88, 90, 88, 90, 85, 88],
];

const ROOK_PST_MG: [[i32; 9]; 10] = [
    [206, 208, 207, 213, 214, 213, 207, 208, 206],
    [206, 212, 209, 216, 233, 216, 209, 212, 206],
    [206, 208, 207, 214, 216, 214, 207, 208, 206],
    [206, 213, 213, 216, 216, 216, 213, 213, 206],
    [208, 211, 211, 214, 215, 214, 211, 211, 208],
    [208, 212, 212, 214, 215, 214, 212, 212, 208],
    [204, 209, 204, 212, 214, 212, 204, 209, 204],
    [198, 208, 204, 212, 212, 212, 204, 208, 198],
    [200, 208, 206, 212, 200, 212, 206, 208, 200],
    [194, 206, 204, 212, 200, 212, 204, 206, 194],
];

const CANNON_PST_MG: [[i32; 9]; 10] = [
    [100, 100, 96, 91, 90, 91, 96, 100, 100],
    [98, 98, 96, 92, 89, 92, 96, 98, 98],
    [97, 97, 96, 91, 92, 91, 96, 97, 97],
    [96, 99, 99, 98, 100, 98, 99, 99, 96],
    [96, 96, 96, 96, 100, 96, 96, 96, 96],
    [95, 96, 99, 96, 100, 96, 99, 96, 95],
    [96, 96, 96, 96, 96, 96, 96, 96, 96],
    [97, 96, 100, 99, 101, 99, 100, 96, 97],
    [96, 97, 98, 98, 98, 98, 98, 97, 96],
    [96, 96, 97, 99, 99, 99, 97, 96, 96],
];

const PAWN_PST_MG: [[i32; 9]; 10] = [
    [9, 9, 9, 11, 13, 11, 9, 9, 9],
    [19, 24, 34, 42, 44, 42, 34, 24, 19],
    [19, 24, 32, 37, 37, 37, 32, 24, 19],
    [19, 23, 27, 29, 30, 29, 27, 23, 19],
    [14, 18, 20, 27, 29, 27, 20, 18, 14],
    [7, 0, 13, 0, 16, 0, 13, 0, 7],
    [7, 0, 7, 0, 15, 0, 7, 0, 7],
    [0, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 0],
];

const PAWN_PST_EG: [[i32; 9]; 10] = [
    [20, 20, 20, 25, 30, 25, 20, 20, 20],
    [40, 50, 60, 70, 75, 70, 60, 50, 40],
    [40, 50, 60, 65, 70, 65, 60, 50, 40],
    [40, 50, 55, 60, 60, 60, 55, 50, 40],
    [30, 40, 45, 50, 50, 50, 45, 40, 30],
    [15, 20, 25, 30, 30, 30, 25, 20, 15],
    [10, 15, 20, 20, 20, 20, 20, 15, 10],
    [5, 5, 5, 5, 5, 5, 5, 5, 5],
    [0, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 0],
];

pub fn get_pst_mg(p: Piece) -> &'static [[i32; 9]; 10] {
    match p.abs_val() as i8 {
        1 => &KING_PST_MG,
        2 => &GUARD_PST_MG,
        3 => &BISHOP_PST_MG,
        4 => &HORSE_PST_MG,
        5 => &ROOK_PST_MG,
        6 => &CANNON_PST_MG,
        7 => &PAWN_PST_MG,
        _ => &KING_PST_MG, // Should be unreachable
    }
}

pub fn get_pst_eg(p: Piece) -> &'static [[i32; 9]; 10] {
    if p.abs_val() == 7 { &PAWN_PST_EG } else { get_pst_mg(p) }
}

/// Returns the midgame and endgame PST scores for a given piece at a given square.
pub fn get_pst_scores(piece: Piece, sq: usize) -> (i32, i32) {
    let player = piece.player().unwrap();
    let r = sq / 9;
    let c = sq % 9;

    // Map board coordinates to PST coordinates (from Red's perspective)
    let pst_r = 9 - r;
    let pst_c = 8 - c;

    let mg_table = get_pst_mg(piece);
    let eg_table = get_pst_eg(piece);

    let mg_pst = mg_table[pst_r][pst_c];
    let eg_pst = eg_table[pst_r][pst_c];

    // Return score from Red's perspective
    if player == Player::Red {
        (mg_pst, eg_pst)
    } else {
        (-mg_pst, -eg_pst)
    }
}

/// Calculates the full material and PST scores from scratch.
/// This is intended to be called only once when the board is set up.
pub fn calculate_full_scores(board: &Board) -> (i32, i32, i32) {
    let mut material_score = 0;
    let mut mg_pst_score = 0;
    let mut eg_pst_score = 0;

    // 1. Material Score
    for i in 1..=7 {
        let piece = Piece::from_abs(i);
        let red_piece = piece;
        let black_piece = Piece::from_abs(-(i as i8));
        material_score += bitboard::popcount(board.piece_bitboards[red_piece.get_bb_index().unwrap()]) as i32 * MATERIAL_VALUES[i as usize];
        material_score -= bitboard::popcount(board.piece_bitboards[black_piece.get_bb_index().unwrap()]) as i32 * MATERIAL_VALUES[i as usize];
    }

    // 2. PST Scores (Midgame and Endgame)
    for i in 0..14 {
        let mut piece_bb = board.piece_bitboards[i];
        if piece_bb == 0 { continue; }
        let piece_type = board.board[piece_bb.trailing_zeros() as usize];
        let player = piece_type.player().unwrap();

        let mg_table = get_pst_mg(piece_type);
        let eg_table = get_pst_eg(piece_type);

        while piece_bb != 0 {
            let sq = piece_bb.trailing_zeros() as usize;
            let r = sq / 9; let c = sq % 9;

            let (pst_r, pst_c) = if player == Player::Red { (9 - r, 8 - c) } else { (r, c) };

            let mg_pst = mg_table[pst_r][pst_c];
            let eg_pst = eg_table[pst_r][pst_c];

            if player == Player::Red {
                mg_pst_score += mg_pst;
                eg_pst_score += eg_pst;
            } else {
                mg_pst_score -= mg_pst;
                eg_pst_score -= eg_pst;
            }
            piece_bb &= !crate::bitboard::SQUARE_MASKS[sq];
        }
    }

    (material_score, mg_pst_score, eg_pst_score)
}

pub fn evaluate(board: &Board) -> i32 {
    // --- Tapered Evaluation ---
    const OPENING_PHASE_MATERIAL: i32 = (900 + 450 + 500) * 2 + (200 + 200) * 2;
    let mut current_phase_material = 0;
    for i in 2..=6 { // Major pieces
        let red_piece = Piece::from_abs(i);
        let black_piece = Piece::from_abs(-(i as i8));
        current_phase_material += bitboard::popcount(board.piece_bitboards[red_piece.get_bb_index().unwrap()]) as i32 * MATERIAL_VALUES[i as usize];
        current_phase_material += bitboard::popcount(board.piece_bitboards[black_piece.get_bb_index().unwrap()]) as i32 * MATERIAL_VALUES[i as usize];
    }
    let phase_weight = (current_phase_material as f64 / OPENING_PHASE_MATERIAL as f64).min(1.0);

    let pst_score = (board.mg_pst_score as f64 * phase_weight + board.eg_pst_score as f64 * (1.0 - phase_weight)) as i32;
    let material_score = board.material_score;

    // The less expensive, dynamic scores are still calculated on the fly.
    let mobility_score = calculate_mobility_score(board);
    let pattern_score = calculate_pattern_score(board);
    let king_safety_score = calculate_king_safety_score(board);
    let dynamic_bonus_score = calculate_dynamic_bonus_score(board);

    let final_score = material_score + pst_score + mobility_score + pattern_score + king_safety_score + dynamic_bonus_score;
    if board.player_to_move == Player::Red { final_score } else { -final_score }
}

const BONUS_BOTTOM_CANNON: i32 = 80;
const BONUS_PALACE_HEART_HORSE: i32 = 70;

fn calculate_pattern_score(board: &Board) -> i32 {
    let mut pattern_score = 0;

    // --- Red Player Patterns ---
    // Bottom Cannon
    let red_cannons = board.piece_bitboards[Piece::RCannon.get_bb_index().unwrap()];
    if (red_cannons & bitboard::RANK_MASKS[0]) != 0 {
        pattern_score += BONUS_BOTTOM_CANNON;
    }
    // Palace Heart Horse
    let red_horses = board.piece_bitboards[Piece::RHorse.get_bb_index().unwrap()];
    if (red_horses & bitboard::SQUARE_MASKS[4]) != 0 {
        pattern_score += BONUS_PALACE_HEART_HORSE;
    }

    // --- Black Player Patterns ---
    // Bottom Cannon
    let black_cannons = board.piece_bitboards[Piece::BCannon.get_bb_index().unwrap()];
    if (black_cannons & bitboard::RANK_MASKS[9]) != 0 {
        pattern_score -= BONUS_BOTTOM_CANNON;
    }
    // Palace Heart Horse
    let black_horses = board.piece_bitboards[Piece::BHorse.get_bb_index().unwrap()];
    if (black_horses & bitboard::SQUARE_MASKS[85]) != 0 {
        pattern_score -= BONUS_PALACE_HEART_HORSE;
    }

    pattern_score
}

const KING_SAFETY_PENALTY_PER_GUARD: i32 = 50;

fn calculate_king_safety_score(board: &Board) -> i32 {
    let mut king_safety_score = 0;

    // Red player's king safety
    let red_guard_count =
        bitboard::popcount(board.piece_bitboards[Piece::RGuard.get_bb_index().unwrap()]);
    if red_guard_count < 2 {
        king_safety_score -= (2 - red_guard_count as i32) * KING_SAFETY_PENALTY_PER_GUARD;
    }

    // Black player's king safety
    let black_guard_count =
        bitboard::popcount(board.piece_bitboards[Piece::BGuard.get_bb_index().unwrap()]);
    if black_guard_count < 2 {
        king_safety_score += (2 - black_guard_count as i32) * KING_SAFETY_PENALTY_PER_GUARD;
    }

    king_safety_score
}

const DYNAMIC_BONUS_ATTACK_PER_MISSING_DEFENDER: i32 = 15;

fn calculate_dynamic_bonus_score(board: &Board) -> i32 {
    let mut dynamic_score = 0;

    // --- Red attacking Black's Palace ---
    let black_defenders =
        bitboard::popcount(board.piece_bitboards[Piece::BGuard.get_bb_index().unwrap()]);
    let missing_black_defenders = 2 - black_defenders as i32;
    if missing_black_defenders > 0 {
        let mut red_attackers = 0;
        // Define black palace zone
        for r in 0..=2 {
            for c in 3..=5 {
                if move_gen::is_square_attacked_by(board, sq_to_idx(r, c), Player::Red) {
                    red_attackers += 1;
                }
            }
        }
        dynamic_score +=
            red_attackers * missing_black_defenders * DYNAMIC_BONUS_ATTACK_PER_MISSING_DEFENDER;
    }

    // --- Black attacking Red's Palace ---
    let red_defenders =
        bitboard::popcount(board.piece_bitboards[Piece::RGuard.get_bb_index().unwrap()]);
    let missing_red_defenders = 2 - red_defenders as i32;
    if missing_red_defenders > 0 {
        let mut black_attackers = 0;
        // Define red palace zone
        for r in 7..=9 {
            for c in 3..=5 {
                if move_gen::is_square_attacked_by(board, sq_to_idx(r, c), Player::Black) {
                    black_attackers += 1;
                }
            }
        }
        dynamic_score -=
            black_attackers * missing_red_defenders * DYNAMIC_BONUS_ATTACK_PER_MISSING_DEFENDER;
    }

    dynamic_score
}

const MOBILITY_BONUS_ROOK: i32 = 1;
const MOBILITY_BONUS_HORSE: i32 = 3;
const MOBILITY_BONUS_CANNON: i32 = 1;

fn calculate_mobility_score(board: &Board) -> i32 {
    let mut mobility_score = 0;
    let occupied = board.occupied_bitboard();

    for player in [Player::Red, Player::Black] {
        let player_sign = if player == Player::Red { 1 } else { -1 };
        let own_pieces_bb = board.color_bitboards[player.get_bb_idx()];

        // Rook mobility
        let rook_type = if player == Player::Red {
            Piece::RRook
        } else {
            Piece::BRook
        };
        let mut rooks_bb = board.piece_bitboards[rook_type.get_bb_index().unwrap()];
        while rooks_bb != 0 {
            let sq = rooks_bb.trailing_zeros() as usize;
            let moves_bb = move_gen::get_rook_moves_bb(sq, occupied) & !own_pieces_bb;
            mobility_score +=
                bitboard::popcount(moves_bb) as i32 * MOBILITY_BONUS_ROOK * player_sign;
            rooks_bb &= !bitboard::SQUARE_MASKS[sq];
        }

        // Horse mobility
        let horse_type = if player == Player::Red {
            Piece::RHorse
        } else {
            Piece::BHorse
        };
        let mut horses_bb = board.piece_bitboards[horse_type.get_bb_index().unwrap()];
        while horses_bb != 0 {
            let sq = horses_bb.trailing_zeros() as usize;
            let mut potential_moves = move_gen::ATTACK_TABLES.horse[sq] & !own_pieces_bb;
            let mut count = 0;
            while potential_moves != 0 {
                let to_sq = potential_moves.trailing_zeros() as usize;
                let leg_sq = move_gen::ATTACK_TABLES.horse_legs[sq][to_sq];
                if (occupied & bitboard::SQUARE_MASKS[leg_sq]) == 0 {
                    count += 1;
                }
                potential_moves &= !bitboard::SQUARE_MASKS[to_sq];
            }
            mobility_score += count * MOBILITY_BONUS_HORSE * player_sign;
            horses_bb &= !bitboard::SQUARE_MASKS[sq];
        }

        // Cannon mobility
        let cannon_type = if player == Player::Red {
            Piece::RCannon
        } else {
            Piece::BCannon
        };
        let mut cannons_bb = board.piece_bitboards[cannon_type.get_bb_index().unwrap()];
        while cannons_bb != 0 {
            let sq = cannons_bb.trailing_zeros() as usize;
            let moves_bb = move_gen::get_cannon_moves_bb(sq, occupied) & !own_pieces_bb;
            mobility_score +=
                bitboard::popcount(moves_bb) as i32 * MOBILITY_BONUS_CANNON * player_sign;
            cannons_bb &= !bitboard::SQUARE_MASKS[sq];
        }
    }
    mobility_score
}
