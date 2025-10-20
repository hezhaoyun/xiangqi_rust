//! The main search engine.

use crate::bitboard::{self, Board};
use crate::constants::{MATE_VALUE, Piece, Player};
use crate::evaluate;
use crate::r#move::Move;
use crate::move_gen;
use crate::opening_book;
use crate::tt::{TranspositionTable, TtFlag};
use crate::zobrist;
use std::time::Instant;

/// A struct to hold a move and its score for move ordering.

#[derive(Debug, Clone, Copy)]
pub struct ScoredMove {
    pub mv: Move,
    pub score: i32,
}

/// The search engine.

pub struct Engine {
    pub tt: TranspositionTable,
    pub history_table: [[i32; 90]; 14],
    pub nodes_searched: u64,
    pub stop_search: bool,
    pub start_time: Instant,
    pub time_limit_ms: Option<u128>,
}

impl Engine {
    pub fn new(tt_size_mb: usize) -> Self {
        Self {
            tt: TranspositionTable::new(tt_size_mb),
            history_table: [[0; 90]; 14],
            nodes_searched: 0,
            stop_search: false,
            start_time: Instant::now(),
            time_limit_ms: None,
        }
    }

    pub fn clear_history(&mut self) {
        self.history_table = [[0; 90]; 14];
    }

    /// Helper to count major pieces for null move pruning

    fn get_major_piece_count(&self, board: &Board, player: Player) -> u32 {
        let mut count = 0;

        count += bitboard::popcount(
            board.piece_bitboards[(if player == Player::Red {
                Piece::RRook
            } else {
                Piece::BRook
            })
            .get_bb_index()
            .unwrap()],
        );

        count += bitboard::popcount(
            board.piece_bitboards[(if player == Player::Red {
                Piece::RHorse
            } else {
                Piece::BHorse
            })
            .get_bb_index()
            .unwrap()],
        );

        count += bitboard::popcount(
            board.piece_bitboards[(if player == Player::Red {
                Piece::RCannon
            } else {
                Piece::BCannon
            })
            .get_bb_index()
            .unwrap()],
        );

        count
    }

    /// The main search function using iterative deepening.

    pub fn search(
        &mut self,
        board: &mut Board,
        max_depth: i32,
        time_limit_ms: Option<u128>,
    ) -> (Move, i32) {
        self.clear_history();
        self.tt.clear();
        self.nodes_searched = 0;
        self.stop_search = false;
        self.start_time = Instant::now();
        self.time_limit_ms = time_limit_ms;

        let mut best_move_overall = Move::new(0, 0, None);
        let mut best_score_overall = -MATE_VALUE;

        for current_depth in 1..=max_depth {
            // Query the opening book
            if let Some(book_move) = opening_book::query_opening_book(board) {
                println!(
                    "Move from opening book: {} -> {}",
                    book_move.from_sq(),
                    book_move.to_sq()
                );

                return (book_move, 0); // Return book move with a neutral score
            }

            let (best_move_this_depth, best_score_this_depth) =
                self.negamax(board, current_depth, -MATE_VALUE, MATE_VALUE);

            if self.stop_search {
                break;
            }

            if best_move_this_depth.from_sq() != 0 || best_move_this_depth.to_sq() != 0 {
                best_move_overall = best_move_this_depth;

                best_score_overall = best_score_this_depth;
            }

            // The score from negamax is from the perspective of the player whose turn it is.
            // To display it consistently from Red's perspective (assuming Red is the human player),
            // we check whose turn it was at the root of the search.
            let display_score = if board.player_to_move == Player::Red {
                best_score_overall
            } else {
                // If it was Black's turn, a positive score means Black is winning.
                // To show this from Red's perspective, we negate it.
                -best_score_overall
            };

            println!(
                "info depth {} score cp {} nodes {} time {} pv {}",
                current_depth,
                display_score,
                self.nodes_searched,
                self.start_time.elapsed().as_millis(),
                best_move_overall.to_uci_string()
            );

            if best_score_overall.abs() > MATE_VALUE - 100 {
                break;
            }
        }

        (best_move_overall, best_score_overall)
    }

    /// The core negamax search function with alpha-beta pruning.

    fn negamax(
        &mut self,
        board: &mut Board,
        depth: i32,
        mut alpha: i32,
        mut beta: i32,
    ) -> (Move, i32) {
        if self.nodes_searched % 2048 == 0 {
            if let Some(limit) = self.time_limit_ms {
                if self.start_time.elapsed().as_millis() >= limit {
                    self.stop_search = true;
                }
            }
        }

        if self.stop_search {
            return (Move::new(0, 0, None), 0);
        }

        self.nodes_searched += 1;
        let hash_key = board.hash_key;
        let player = board.player_to_move;

        // --- Repetition Detection ---
        if board.history_ply >= 4 {
            let mut repetitions = 0;
            for i in (0..board.history_ply - 1).rev().step_by(2) {
                if board.history[i] == hash_key {
                    repetitions += 1;
                }
            }
            if repetitions >= 2 {
                return (Move::new(0, 0, None), 0); // Draw
            }
        }

        // --- Transposition Table Probe ---
        let mut tt_best_move = Move::new(0, 0, None);
        let original_alpha = alpha;
        if let Some(tt_entry) = self.tt.probe(hash_key) {
            if tt_entry.depth >= depth {
                match tt_entry.flag {
                    TtFlag::Exact => return (tt_entry.best_move, tt_entry.score),
                    TtFlag::LowerBound => alpha = alpha.max(tt_entry.score),
                    TtFlag::UpperBound => beta = beta.min(tt_entry.score),
                }
                if alpha >= beta {
                    return (tt_entry.best_move, tt_entry.score);
                }
            }
            tt_best_move = tt_entry.best_move;
        }

        if depth == 0 {
            return (
                Move::new(0, 0, None),
                self.quiescence_search(board, alpha, beta),
            );
        }

        let is_in_check = move_gen::is_king_in_check(board, player);

        // --- Null Move Pruning ---
        if !is_in_check
            && depth >= 3
            && self.get_major_piece_count(board, player) > 1
        {
            board.player_to_move = board.player_to_move.opponent();
            board.hash_key ^= zobrist::ZOBRIST_PLAYER;
            board.history_ply += 1;
            board.history[board.history_ply] = board.hash_key;
            let (_, null_move_score) = self.negamax(board, depth - 1 - 2, -beta, -beta + 1); // R = 2
            board.history_ply -= 1;
            board.hash_key ^= zobrist::ZOBRIST_PLAYER;
            board.player_to_move = board.player_to_move.opponent();

            if -null_move_score >= beta {
                return (Move::new(0, 0, None), beta);
            }
        }

        // --- Staged Move Generation and Search ---
        let mut best_score = -MATE_VALUE;
        let mut best_move = Move::new(0, 0, None);
        let mut moves_searched = 0;

        // Stage 1: TT Move
        if tt_best_move.from_sq() != 0 || tt_best_move.to_sq() != 0 {
            let captured = board.move_piece(tt_best_move);
            if !move_gen::is_king_in_check(board, player) {
                moves_searched += 1;
                let (_, mut score) = self.negamax(board, depth - 1, -beta, -alpha);
                score = -score;

                if score > best_score {
                    best_score = score;
                    best_move = tt_best_move;
                }
                if best_score > alpha {
                    alpha = best_score;
                }
            }
            board.unmove_piece(tt_best_move, captured);
            if alpha >= beta {
                self.tt.store(hash_key, depth, best_score, TtFlag::LowerBound, best_move);
                return (best_move, best_score);
            }
        }

        // Stage 2: Captures
        let captures = board.generate_capture_moves();
        let mut scored_captures: Vec<ScoredMove> = captures.into_iter().map(|mv| ScoredMove { mv, score: self.score_move(board, mv) }).collect();
        scored_captures.sort_by(|a, b| b.score.cmp(&a.score));

        for sm in scored_captures {
            if sm.mv.from_sq() == tt_best_move.from_sq() && sm.mv.to_sq() == tt_best_move.to_sq() { continue; } // Skip TT move
            let captured = board.move_piece(sm.mv);
            if !move_gen::is_king_in_check(board, player) {
                moves_searched += 1;
                let score = -self.negamax(board, depth - 1, -beta, -alpha).1;

                if score > best_score {
                    best_score = score;
                    best_move = sm.mv;
                }
                if best_score > alpha {
                    alpha = best_score;
                }
                if alpha >= beta {
                    board.unmove_piece(sm.mv, captured);
                    self.tt.store(hash_key, depth, best_score, TtFlag::LowerBound, best_move);
                    return (best_move, best_score);
                }
            }
            board.unmove_piece(sm.mv, captured);
        }

        // Stage 3: Quiet Moves
        let quiets = board.generate_quiet_moves();
        let mut scored_quiets: Vec<ScoredMove> = quiets.into_iter().map(|mv| ScoredMove { mv, score: self.score_move(board, mv) }).collect();
        scored_quiets.sort_by(|a, b| b.score.cmp(&a.score));

        for (_i, sm) in scored_quiets.iter().enumerate() {
            if sm.mv.from_sq() == tt_best_move.from_sq() && sm.mv.to_sq() == tt_best_move.to_sq() { continue; } // Skip TT move
            
            let captured = board.move_piece(sm.mv);
            if !move_gen::is_king_in_check(board, player) {
                moves_searched += 1;

                // --- Late Move Reduction (LMR) ---
                let mut score;
                if depth >= 3 && _i > 3 && !is_in_check {
                    let reduction = 1;
                    score = -self.negamax(board, depth - 1 - reduction, -beta, -alpha).1;

                    // Re-search if LMR was too aggressive
                    if score > alpha {
                        score = -self.negamax(board, depth - 1, -beta, -alpha).1;
                    }
                } else {
                    score = -self.negamax(board, depth - 1, -beta, -alpha).1;
                }

                if score > best_score {
                    best_score = score;
                    best_move = sm.mv;
                }
                if best_score > alpha {
                    alpha = best_score;
                }
                if alpha >= beta {
                    board.unmove_piece(sm.mv, captured);
                    let moving_piece = board.board[sm.mv.from_sq()];
                    if let Some(idx) = moving_piece.get_bb_index() {
                        self.history_table[idx][sm.mv.to_sq()] += depth * depth;
                    }
                    self.tt.store(hash_key, depth, best_score, TtFlag::LowerBound, best_move);
                    return (best_move, best_score);
                }
            }
            board.unmove_piece(sm.mv, captured);
        }

        // --- Checkmate and Stalemate Detection ---
        if moves_searched == 0 {
            if is_in_check {
                return (Move::new(0, 0, None), -MATE_VALUE + depth); // Checkmate
            } else {
                return (Move::new(0, 0, None), 0); // Stalemate
            }
        }

        // --- Transposition Table Store ---
        let flag = if best_score >= beta {
            TtFlag::LowerBound
        } else if best_score > original_alpha {
            TtFlag::Exact
        } else {
            TtFlag::UpperBound
        };
        self.tt.store(hash_key, depth, best_score, flag, best_move);

        (best_move, best_score)
    }

    /// Helper to score a move for move ordering.

    fn score_move(&self, board: &Board, mv: Move) -> i32 {
        // MVV-LVA (Most Valuable Victim - Least Valuable Aggressor)
        let captured_piece = board.board[mv.to_sq()];
        if captured_piece != Piece::Empty {
            let moving_piece = board.board[mv.from_sq()];
            return 1000 * captured_piece.value() - moving_piece.value();
        }

        // History heuristic
        let moving_piece = board.board[mv.from_sq()];
        if let Some(idx) = moving_piece.get_bb_index() {
            return self.history_table[idx][mv.to_sq()];
        }
        0 // Default if piece not found (should not happen)
    }

    /// Quiescence search to evaluate noisy positions.

    fn quiescence_search(&mut self, board: &mut Board, mut alpha: i32, beta: i32) -> i32 {
        if self.nodes_searched % 2048 == 0 {
            if let Some(limit) = self.time_limit_ms {
                if self.start_time.elapsed().as_millis() >= limit {
                    self.stop_search = true;
                }
            }
        }

        if self.stop_search {
            return 0;
        }
        self.nodes_searched += 1;
        // Evaluate the current position statically
        let stand_pat = evaluate::evaluate(board);
        if stand_pat >= beta {
            return beta;
        }

        if stand_pat > alpha {
            alpha = stand_pat;
        }

        let capture_moves = board.generate_capture_moves();

        let mut scored_capture_moves: Vec<ScoredMove> = capture_moves
            .into_iter()
            .map(|mv| ScoredMove {
                mv,
                score: self.score_move(board, mv),
            })
            .collect();

        scored_capture_moves.sort_by(|a, b| b.score.cmp(&a.score)); // Descending order

        for sm in scored_capture_moves {
            let captured = board.move_piece(sm.mv);
            let score = -self.quiescence_search(board, -beta, -alpha);

            board.unmove_piece(sm.mv, captured);

            if score >= beta {
                return beta;
            }

            if score > alpha {
                alpha = score;
            }
        }
        alpha
    }
}
