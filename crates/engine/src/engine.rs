//! The main search engine.

use crate::r#move::Move;
use crate::bitboard::{self, Board};
use crate::constants::{MATE_VALUE, Piece, Player};
use crate::evaluate;
use crate::movelist::MoveList;
use crate::move_generator;
use crate::opening_book;
use crate::tt::{TranspositionTable, TtFlag};
use std::time::Instant;

/// A struct to hold a move and its score for move ordering.

#[derive(Debug, Clone, Copy)]
pub struct ScoredMove {
    pub mv: Move,
    pub score: i32,
}

/// The search engine.

const MAX_PLY: usize = 128;

pub struct Engine {
    pub tt: TranspositionTable,
    pub history_table: [[i32; 90]; 14],
    pub killer_moves: [[Move; 2]; MAX_PLY],
    pub nodes_searched: u64,
    pub stop_search: bool,
    pub start_time: Instant,
    pub time_limit_ms: Option<u128>,
    pub config: crate::config::Config,
}

impl Engine {
    /// Creates a new `Engine` instance with a transposition table of the specified size.
    pub fn new(tt_size_mb: usize) -> Self {
        Self {
            tt: TranspositionTable::new(tt_size_mb),
            history_table: [[0; 90]; 14],
            killer_moves: [[Move::new(0, 0, None); 2]; MAX_PLY],
            nodes_searched: 0,
            stop_search: false,
            start_time: Instant::now(),
            time_limit_ms: None,
            config: crate::config::Config::default(),
        }
    }

    /// Clears the killer moves table.
    fn clear_killers(&mut self) {
        self.killer_moves = [[Move::new(0, 0, None); 2]; MAX_PLY];
    }

    /// Clears the history table, resetting all move scores to zero.
    pub fn clear_history(&mut self) {
        self.history_table = [[0; 90]; 14];
    }

    /// Counts the number of major pieces (Rook, Horse, Cannon) for a given player.
    /// This is used for null move pruning.
    fn get_major_piece_count(&self, board: &Board, player: Player) -> u32 {
        let mut count = 0;
        let (start_idx, end_idx) = if player == Player::Red {
            (0, 7)
        } else {
            (7, 14)
        };

        for i in start_idx..end_idx {
            let piece = Piece::from_abs((i + 1) as i8);
            if piece.is_major() {
                count += bitboard::popcount(board.piece_bitboards[i]);
            }
        }
        count
    }

    /// The main search function, using iterative deepening.
    ///
    /// This function iteratively deepens the search depth, starting from 1 up to `max_depth`.
    /// It also handles opening book moves and time management.
    pub fn search(
        &mut self,
        board: &mut Board,
        max_depth: i32,
        time_limit_ms: Option<u128>,
    ) -> (Move, i32, i32) {
        self.clear_history();
        self.clear_killers();
        self.tt.clear();
        self.nodes_searched = 0;
        self.stop_search = false;
        self.start_time = Instant::now();
        self.time_limit_ms = time_limit_ms;

        let mut best_move_overall = Move::new(0, 0, None);
        let mut best_score_overall = -MATE_VALUE;
        let mut searched_depth = 1;

        for current_depth in 1..=max_depth {
            // Query the opening book
            if let Some(book_move) = opening_book::query_opening_book(board) {
                println!(
                    "Move from opening book: {} -> {}",
                    book_move.from_sq(),
                    book_move.to_sq()
                );

                return (book_move, 0, current_depth); // Return book move with a neutral score
            }

            let (best_move_this_depth, best_score_this_depth) =
                self.negamax(board, current_depth, -MATE_VALUE, MATE_VALUE, 0);

            if self.stop_search {
                break;
            }

            if best_move_this_depth.from_sq() != 0 || best_move_this_depth.to_sq() != 0 {
                best_move_overall = best_move_this_depth;
                best_score_overall = best_score_this_depth;
                searched_depth = current_depth;
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

        (best_move_overall, best_score_overall, searched_depth)
    }

    fn negamax(
        &mut self,
        board: &mut Board,
        depth: i32,
        mut alpha: i32,
        mut beta: i32,
        ply: usize,
    ) -> (Move, i32) {
        if self.check_time_limit() {
            return (Move::new(0, 0, None), 0);
        }

        self.nodes_searched += 1;

        if ply > 0 {
            if let Some(draw_score) = self.handle_repetition(board) {
                return (Move::new(0, 0, None), draw_score);
            }
        }

        let mut tt_best_move = Move::new(0, 0, None);
        let original_alpha = alpha;
        if let Some(tt_result) = self.probe_tt_table(
            board.hash_key,
            depth,
            &mut alpha,
            &mut beta,
            &mut tt_best_move,
        ) {
            return tt_result;
        }

        if depth <= 0 {
            return (
                Move::new(0, 0, None),
                self.quiescence_search(board, alpha, beta, ply),
            );
        }

        let is_in_check = move_generator::is_king_in_check(board, board.player_to_move);

        // Check extension
        let mut current_depth = depth;
        if is_in_check {
            current_depth += 1;
        }

        if let Some(pruning_result) =
            self.perform_null_move_pruning(board, current_depth, beta, is_in_check, ply)
        {
            return pruning_result;
        }

        let mut legal_moves_found = 0;
        let mut best_score = -MATE_VALUE;
        let mut best_move = Move::new(0, 0, None);

        let mut moves = MoveList::new();
        board.generate_capture_moves(&mut moves);
        board.generate_quiet_moves(&mut moves);

        let mut scored_moves: Vec<ScoredMove> = moves
            .as_slice()
            .iter()
            .map(|mv| ScoredMove {
                mv: *mv,
                score: self.score_move(board, *mv, tt_best_move, ply),
            })
            .collect();
        scored_moves.sort_by(|a, b| b.score.cmp(&a.score));

            for sm in scored_moves {
                let captured = board.move_piece(sm.mv);
                if move_generator::is_king_in_check(board, board.player_to_move.opponent()) {
                    board.unmove_piece(sm.mv, captured);
                    continue;
                }
                legal_moves_found += 1;

                let mut score;
                if legal_moves_found == 1 {
                    // Full window search for the first move
                    score = -self
                        .negamax(board, current_depth - 1, -beta, -alpha, ply + 1)
                        .1;
                } else {
                    // --- Late Move Reduction (LMR) ---
                    let reduction = if current_depth >= 3
                        && legal_moves_found > 3
                        && !is_in_check
                        && !sm.mv.is_capture()
                    {
                        1
                    } else {
                        0
                    };

                    score = -self
                        .negamax(
                            board,
                            current_depth - 1 - reduction,
                            -alpha - 1,
                            -alpha,
                            ply + 1,
                        )
                        .1;

                    // Re-search if LMR was too aggressive
                    if score > alpha && reduction > 0 {
                        score = -self
                            .negamax(board, current_depth - 1, -beta, -alpha, ply + 1)
                            .1;
                    }
                }

                board.unmove_piece(sm.mv, captured);

                if score > best_score {
                    best_score = score;
                    best_move = sm.mv;
                }
                if best_score > alpha {
                    alpha = best_score;
                }
                if alpha >= beta {
                    if !sm.mv.is_capture() {
                        self.store_killer_move(sm.mv, ply);
                        let moving_piece = board.board[sm.mv.from_sq()];
                        if let Some(idx) = moving_piece.get_bb_index() {
                            self.history_table[idx][sm.mv.to_sq()] += depth * depth;
                        }
                    }
                    break; // Beta cutoff
                }
            }

            if legal_moves_found == 0 {
                return (
                    Move::new(0, 0, None),
                    if is_in_check {
                        -MATE_VALUE + ply as i32
                    } else {
                        0
                    },
                );
            }

            self.store_in_tt_table(
                board.hash_key,
                depth,
                best_score,
                original_alpha,
                beta,
                best_move,
            );

            (best_move, best_score)
        }

        /// Checks if the time limit for the search has been exceeded.
        fn check_time_limit(&mut self) -> bool {
            if self.nodes_searched % 2048 == 0 {
                if let Some(limit) = self.time_limit_ms {
                    if self.start_time.elapsed().as_millis() >= limit {
                        self.stop_search = true;
                    }
                }
            }
            self.stop_search
        }

        /// Detects if the current position is a draw by repetition.
        fn handle_repetition(&self, board: &Board) -> Option<i32> {
            if board.history_ply >= 4 {
                let mut repetitions = 0;
                for i in (0..board.history_ply - 1).rev().step_by(2) {
                    if board.history[i] == board.hash_key {
                        repetitions += 1;
                        if repetitions >= 2 {
                            return Some(0); // Draw
                        }
                    }
                }
            }
            None
        }

        /// Probes the transposition table for the current position.
        fn probe_tt_table(
            &mut self,
            hash_key: u64,
            depth: i32,
            alpha: &mut i32,
            beta: &mut i32,
            tt_best_move: &mut Move,
        ) -> Option<(Move, i32)> {
            if let Some(tt_entry) = self.tt.probe(hash_key) {
                *tt_best_move = tt_entry.best_move;
                if tt_entry.depth >= depth {
                    let score = tt_entry.score;
                    match tt_entry.flag {
                        TtFlag::Exact => return Some((tt_entry.best_move, score)),
                        TtFlag::LowerBound => *alpha = (*alpha).max(score),
                        TtFlag::UpperBound => *beta = (*beta).min(score),
                    }
                    if *alpha >= *beta {
                        return Some((tt_entry.best_move, score));
                    }
                }
            }
            None
        }

        /// Performs null move pruning.
        fn perform_null_move_pruning(
            &mut self,
            board: &mut Board,
            depth: i32,
            beta: i32,
            is_in_check: bool,
            ply: usize,
        ) -> Option<(Move, i32)> {
            if !is_in_check && depth >= 3 && self.get_major_piece_count(board, board.player_to_move) > 1
            {
                let r = if depth > 6 { 3 } else { 2 };
                board.player_to_move = board.player_to_move.opponent();
                board.hash_key ^= crate::zobrist::ZOBRIST_PLAYER;
                board.history_ply += 1;
                board.history[board.history_ply] = board.hash_key;

                let (_, null_move_score) =
                    self.negamax(board, depth - 1 - r, -beta, -beta + 1, ply + 1);
                let score = -null_move_score;

                board.history_ply -= 1;
                board.hash_key ^= crate::zobrist::ZOBRIST_PLAYER;
                board.player_to_move = board.player_to_move.opponent();

                if score >= beta {
                    return Some((Move::new(0, 0, None), beta));
                }
            }
            None
        }

        fn store_in_tt_table(
            &mut self,
            hash_key: u64,
            depth: i32,
            best_score: i32,
            original_alpha: i32,
            beta: i32,
            best_move: Move,
        ) {
            let flag = if best_score >= beta {
                TtFlag::LowerBound
            } else if best_score > original_alpha {
                TtFlag::Exact
            } else {
                TtFlag::UpperBound
            };
            self.tt.store(hash_key, depth, best_score, flag, best_move);
        }

        fn store_killer_move(&mut self, mv: Move, ply: usize) {
            if ply < MAX_PLY {
                self.killer_moves[ply][1] = self.killer_moves[ply][0];
                self.killer_moves[ply][0] = mv;
            }
        }

        /// Helper to score a move for move ordering.
        fn score_move(&self, board: &Board, mv: Move, tt_best_move: Move, ply: usize) -> i32 {
            const TT_BEST_MOVE_SCORE: i32 = 1_000_000;
            const KILLER_MOVE_SCORE: i32 = 500_000;
            const CAPTURE_BONUS: i32 = 800_000;

            if mv.from_sq() == tt_best_move.from_sq() && mv.to_sq() == tt_best_move.to_sq() {
                return TT_BEST_MOVE_SCORE;
            }

            // MVV-LVA (Most Valuable Victim - Least Valuable Aggressor)
            let captured_piece = board.board[mv.to_sq()];
            if captured_piece != Piece::Empty {
                let moving_piece = board.board[mv.from_sq()];
                return CAPTURE_BONUS + captured_piece.value() - moving_piece.value();
            }

            // Killer moves
            if ply < MAX_PLY {
                if self.killer_moves[ply][0] == mv {
                    return KILLER_MOVE_SCORE;
                }
                if self.killer_moves[ply][1] == mv {
                    return KILLER_MOVE_SCORE - 10;
                }
            }

            // History heuristic
            let moving_piece = board.board[mv.from_sq()];
            if let Some(idx) = moving_piece.get_bb_index() {
                return self.history_table[idx][mv.to_sq()];
            }
            0 // Default if piece not found (should not happen)
        }

        /// Quiescence search to evaluate noisy positions.
        fn quiescence_search(
            &mut self,
            board: &mut Board,
            mut alpha: i32,
            beta: i32,
            ply: usize,
        ) -> i32 {
            const Q_SEARCH_DEPTH: i32 = 8;
            if ply >= MAX_PLY || (ply as i32) > Q_SEARCH_DEPTH {
                return evaluate::evaluate(board, &self.config);
            }

            if self.check_time_limit() {
                return 0;
            }
            self.nodes_searched += 1;

            let stand_pat = evaluate::evaluate(board, &self.config);
            if stand_pat >= beta {
                return beta;
            }
            if stand_pat > alpha {
                alpha = stand_pat;
            }

            let mut moves = MoveList::new();
            board.generate_capture_moves(&mut moves);
            
            let mut quiet_moves = MoveList::new();
            board.generate_quiet_moves(&mut quiet_moves);
            for i in 0..quiet_moves.len() {
                let mv = quiet_moves[i];
                let captured = board.move_piece(mv);
                if move_generator::is_king_in_check(board, board.player_to_move) {
                    moves.add(mv);
                }
                board.unmove_piece(mv, captured);
            }

            let mut scored_moves: Vec<ScoredMove> = moves
                .as_slice()
                .iter()
                .map(|mv| ScoredMove {
                    mv: *mv,
                    score: self.score_move(board, *mv, Move::new(0, 0, None), ply),
                })
                .collect();
            scored_moves.sort_by(|a, b| b.score.cmp(&a.score));

            for sm in scored_moves {
                let captured = board.move_piece(sm.mv);
                if !move_generator::is_king_in_check(board, board.player_to_move.opponent()) {
                    let score = -self.quiescence_search(board, -beta, -alpha, ply + 1);
                    board.unmove_piece(sm.mv, captured);

                    if score >= beta {
                        return beta;
                    }
                    if score > alpha {
                        alpha = score;
                    }
                } else {
                    board.unmove_piece(sm.mv, captured);
                }
            }
            alpha
        }
    }
