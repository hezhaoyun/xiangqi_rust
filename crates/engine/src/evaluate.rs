//! Evaluates a board position and returns a score.

mod psts;

use crate::move_gen;
use crate::move_gen::sq_to_idx;
use crate::bitboard::{self, Board};
use crate::config::Config;
use crate::constants::{Piece, Player};


// --- Piece Values ---
pub const MATERIAL_VALUES: [i32; 8] = [0, 10000, 200, 200, 450, 900, 500, 100]; // Indexed by Piece type (abs value)

pub fn get_pst_mg(p: Piece) -> &'static [[i32; 9]; 10] {
    match p {
        Piece::RKing | Piece::BKing => &psts::KING_PST_MG,
        Piece::RGuard | Piece::BGuard => &psts::GUARD_PST_MG,
        Piece::RBishop | Piece::BBishop => &psts::BISHOP_PST_MG,
        Piece::RHorse | Piece::BHorse => &psts::HORSE_PST_MG,
        Piece::RRook | Piece::BRook => &psts::ROOK_PST_MG,
        Piece::RCannon | Piece::BCannon => &psts::CANNON_PST_MG,
        Piece::RPawn | Piece::BPawn => &psts::PAWN_PST_MG,
        Piece::Empty => &psts::KING_PST_MG, // Should be unreachable
    }
}

pub fn get_pst_eg(p: Piece) -> &'static [[i32; 9]; 10] {
    match p {
        Piece::RPawn | Piece::BPawn => &psts::PAWN_PST_EG,
        _ => get_pst_mg(p),
    }
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
    let material_score = calculate_material_score(board);
    let (mg_pst_score, eg_pst_score) = calculate_pst_scores(board);
    (material_score, mg_pst_score, eg_pst_score)
}

fn calculate_material_score(board: &Board) -> i32 {
    let mut material_score = 0;
    for i in 1..=7 {
        let piece = Piece::from_abs(i);
        let red_piece = piece;
        let black_piece = Piece::from_abs(-(i as i8));
        material_score += bitboard::popcount(board.piece_bitboards[red_piece.get_bb_index().unwrap()]) as i32 * MATERIAL_VALUES[i as usize];
        material_score -= bitboard::popcount(board.piece_bitboards[black_piece.get_bb_index().unwrap()]) as i32 * MATERIAL_VALUES[i as usize];
    }
    material_score
}

fn calculate_pst_scores(board: &Board) -> (i32, i32) {
    let mut mg_pst_score = 0;
    let mut eg_pst_score = 0;

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
    (mg_pst_score, eg_pst_score)
}


pub fn evaluate(board: &Board, config: &Config) -> i32 {
    // --- Tapered Evaluation ---
    // This blends the midgame and endgame scores based on the material on the board.
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
    let mobility_score = calculate_mobility_score(board, config);
    let pattern_score = calculate_pattern_score(board, config);
    let king_safety_score = calculate_king_safety_score(board, config);
    let dynamic_bonus_score = calculate_dynamic_bonus_score(board, config);

    let final_score = material_score + pst_score + mobility_score + pattern_score + king_safety_score + dynamic_bonus_score;
    if board.player_to_move == Player::Red { final_score } else { -final_score }
}

/// Calculates a score bonus for specific piece patterns.
fn calculate_pattern_score(board: &Board, config: &Config) -> i32 {
    let mut pattern_score = 0;

    // --- Red Player Patterns ---
    // Bottom Cannon: A cannon on the bottom rank is a strong attacking piece.
    let red_cannons = board.piece_bitboards[Piece::RCannon.get_bb_index().unwrap()];
    if (red_cannons & bitboard::RANK_MASKS[0]) != 0 {
        pattern_score += config.bonus_bottom_cannon;
    }
    // Palace Heart Horse: A horse in the center of the palace is a strong defensive and offensive piece.
    let red_horses = board.piece_bitboards[Piece::RHorse.get_bb_index().unwrap()];
    if (red_horses & bitboard::SQUARE_MASKS[4]) != 0 {
        pattern_score += config.bonus_palace_heart_horse;
    }

    // --- Black Player Patterns ---
    // Bottom Cannon
    let black_cannons = board.piece_bitboards[Piece::BCannon.get_bb_index().unwrap()];
    if (black_cannons & bitboard::RANK_MASKS[9]) != 0 {
        pattern_score -= config.bonus_bottom_cannon;
    }
    // Palace Heart Horse
    let black_horses = board.piece_bitboards[Piece::BHorse.get_bb_index().unwrap()];
    if (black_horses & bitboard::SQUARE_MASKS[85]) != 0 {
        pattern_score -= config.bonus_palace_heart_horse;
    }

    pattern_score
}

/// Calculates a score based on the safety of each player's king.
fn calculate_king_safety_score(board: &Board, config: &Config) -> i32 {
    let mut king_safety_score = 0;

    // Red player's king safety: Penalize for each missing guard.
    let red_guard_count =
        bitboard::popcount(board.piece_bitboards[Piece::RGuard.get_bb_index().unwrap()]);
    if red_guard_count < 2 {
        king_safety_score -= (2 - red_guard_count as i32) * config.king_safety_penalty_per_guard;
    }

    // Black player's king safety: Penalize for each missing guard.
    let black_guard_count =
        bitboard::popcount(board.piece_bitboards[Piece::BGuard.get_bb_index().unwrap()]);
    if black_guard_count < 2 {
        king_safety_score += (2 - black_guard_count as i32) * config.king_safety_penalty_per_guard;
    }

    king_safety_score
}

/// Calculates a dynamic score bonus for attacking a weakened palace.
fn calculate_dynamic_bonus_score(board: &Board, config: &Config) -> i32 {
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
            red_attackers * missing_black_defenders * config.dynamic_bonus_attack_per_missing_defender;
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
            black_attackers * missing_red_defenders * config.dynamic_bonus_attack_per_missing_defender;
    }

    dynamic_score
}

/// Calculates a score based on the mobility of each player's pieces.
fn calculate_mobility_score(board: &Board, config: &Config) -> i32 {
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
                bitboard::popcount(moves_bb) as i32 * config.mobility_bonus_rook * player_sign;
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
            mobility_score += count * config.mobility_bonus_horse * player_sign;
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
                bitboard::popcount(moves_bb) as i32 * config.mobility_bonus_cannon * player_sign;
            cannons_bb &= !bitboard::SQUARE_MASKS[sq];
        }
    }
    mobility_score
}
