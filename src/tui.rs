
//! The Textual User Interface for the Xiangqi engine.

use crate::bitboard::Board;
use crate::constants::Player;
use crate::engine::Engine;
use crate::r#move::Move;
use crossterm::{
    execute,
    style::{Color, Print, ResetColor, SetBackgroundColor, SetForegroundColor},
};
use std::io::{self, stdout, Write};

/// Runs the main game loop for the text-based UI.
pub fn run() {
    let fen = "rnbakabnr/9/1c5c1/p1p1p1p1p/9/9/P1P1P1P1P/1C5C1/9/RNBAKABNR w - - 0 1";
    let mut board = Board::from_fen(fen);
    let mut engine = Engine::new(16); // 16MB TT
    let mut last_move: Option<Move> = None;

    println!("--- Xiangqi Engine in Rust ---");
    println!("Enter moves in algebraic notation (e.g., h2e2). Type 'exit' to quit.");

    loop {
        println!();
        draw_board(&board, last_move.as_ref());

        let legal_moves = board.generate_legal_moves();
        if legal_moves.is_empty() {
            if crate::move_gen::is_king_in_check(&board, board.player_to_move) {
                println!("Checkmate! {:?} wins.", board.player_to_move.opponent());
            } else {
                println!("Stalemate! It's a draw.");
            }
            break;
        }

        if board.player_to_move == Player::Red {
            // --- Player's Turn ---
            print!("Your move: ");
            io::Write::flush(&mut io::stdout()).expect("flush failed!");

            let mut input = String::new();
            io::stdin().read_line(&mut input).unwrap();
            let input = input.trim();

            if input == "exit" {
                break;
            }

            match parse_move_string(input, &legal_moves) {
                Some(mv) => {
                    board.move_piece(mv);
                    last_move = Some(mv);
                }
                None => {
                    println!("Invalid or illegal move. Please try again.");
                    last_move = None; // Clear last move on invalid input
                    continue;
                }
            }
        } else {
            // --- Computer's Turn ---
            println!("Computer is thinking...");
            let (best_move, score) = engine.search(&mut board, 6);

            if best_move.from_sq() == 0 && best_move.to_sq() == 0 {
                println!("Engine returned null move. Game over?");
                break;
            }

            let from_notation = get_square_notation(best_move.from_sq());
            let to_notation = get_square_notation(best_move.to_sq());
            println!(
                "Computer moves: {}{} (Score: {})",
                from_notation, to_notation, score
            );
            board.move_piece(best_move);
            last_move = Some(best_move);
        }
    }
}

/// Draws the board to the console with highlighting for the last move.
fn draw_board(board: &Board, last_move: Option<&Move>) {
    let from_sq = last_move.map(|m| m.from_sq());
    let to_sq = last_move.map(|m| m.to_sq());

    println!(
        "(Player: {:?}, Hash: {:016x})",
        board.player_to_move, board.hash_key
    );
    println!("  +---------------------------+");

    for r in 0..10 {
        print!("{} |", 9 - r);
        for c in 0..9 {
            let sq = r * 9 + c;
            let piece = board.board[sq];

            let is_from = from_sq.map_or(false, |s| s == sq);
            let is_to = to_sq.map_or(false, |s| s == sq);

            let bg_color = if is_from {
                Color::DarkYellow
            } else if is_to {
                Color::DarkGreen
            } else {
                Color::Reset // Use Reset to go back to terminal default
            };

            let piece_color = if piece.player() == Some(Player::Red) {
                Color::DarkRed
            } else if piece.player() == Some(Player::Black) {
                Color::White
            } else {
                Color::DarkGrey
            };

            execute!(
                stdout(),
                SetBackgroundColor(bg_color),
                SetForegroundColor(piece_color),
                Print(format!(" {} ", piece.to_fen_char())),
                ResetColor
            )
            .unwrap();
        }
        println!("|");
    }

    println!("  +---------------------------+");
    println!("     a  b  c  d  e  f  g  h  i");
}

/// Parses a move from algebraic notation (e.g., "h2e2") and checks if it's legal.
fn parse_move_string(move_str: &str, legal_moves: &[Move]) -> Option<Move> {
    if move_str.len() != 4 {
        return None;
    }
    let mut chars = move_str.chars();
    let from_c = (chars.next()? as u8) - b'a';
    let from_r = 9 - ((chars.next()? as u8) - b'0');
    let to_c = (chars.next()? as u8) - b'a';
    let to_r = 9 - ((chars.next()? as u8) - b'0');

    if from_c > 8 || from_r > 9 || to_c > 8 || to_r > 9 {
        return None;
    }

    let from_sq = (from_r * 9 + from_c) as usize;
    let to_sq = (to_r * 9 + to_c) as usize;

    // Find the move in the list of legal moves
    for &legal_move in legal_moves {
        if legal_move.from_sq() == from_sq && legal_move.to_sq() == to_sq {
            return Some(legal_move);
        }
    }

    None
}

/// Gets algebraic notation from a square index.
fn get_square_notation(sq: usize) -> String {
    if sq >= 90 {
        return "??".to_string();
    }
    let r = sq / 9;
    let c = sq % 9;
    format!("{}{}", (b'a' + c as u8) as char, 9 - r)
}