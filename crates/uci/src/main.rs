
use std::fs::File;
use std::io::{self, BufRead, Write};
use std::sync::{Arc, Mutex};
use engine::bitboard::Board;
use engine::engine::Engine;
use engine::r#move::Move;

fn parse_uci_move(board: &Board, move_str: &str) -> Option<Move> {
    if move_str.len() != 4 {
        return None;
    }
    let from_file = move_str.chars().nth(0)? as u8 - b'a';
    let from_rank = move_str.chars().nth(1)? as u8 - b'0';
    let to_file = move_str.chars().nth(2)? as u8 - b'a';
    let to_rank = move_str.chars().nth(3)? as u8 - b'0';

    let from_sq = (9 - from_rank) as usize * 9 + from_file as usize;
    let to_sq = (9 - to_rank) as usize * 9 + to_file as usize;

    let captured_piece = board.board[to_sq];

    Some(Move::new(from_sq, to_sq, if captured_piece == engine::constants::Piece::Empty { None } else { Some(captured_piece) }))
}

fn main() {
    let mut log_file = File::create("uci.log").unwrap();
    let engine = Arc::new(Mutex::new(Engine::new(16)));
    let mut board: Option<Board> = None;

    let stdin = io::stdin();
    for line in stdin.lock().lines() {
        let line = line.unwrap();
        writeln!(log_file, "Received: {}", line).unwrap();
        let parts: Vec<&str> = line.split_whitespace().collect();
        if let Some(command) = parts.get(0) {
            match *command {
                "uci" => {
                    println!("id name XiangqiEngine");
        println!("id author Gemini CLI");
                    println!("uciok");
                }
                "isready" => {
                    println!("readyok");
                }
                "position" => {
                    let mut new_board = if parts.get(1) == Some(&"startpos") {
                        Board::from_fen("rnbakabnr/9/1c5c1/p1p1p1p1p/9/9/P1P1P1P1P/1C5C1/9/RNBAKABNR w - - 0 1")
                    } else if parts.get(1) == Some(&"fen") {
                        let fen = parts[2..].join(" ");
                        Board::from_fen(&fen)
                    } else {
                        continue;
                    };

                    if let Some(moves_idx) = parts.iter().position(|&x| x == "moves") {
                        for move_str in &parts[moves_idx + 1..] {
                            if let Some(mv) = parse_uci_move(&new_board, move_str) {
                                new_board.move_piece(mv);
                            }
                        }
                    }
                    board = Some(new_board);
                }
                "go" => {
                    if let Some(ref mut b) = board {
                        let mut engine_lock = engine.lock().unwrap();
                        let (best_move, _) = engine_lock.search(b, 5); // Search for 5 ply
                        let from_sq = best_move.from_sq();
                        let to_sq = best_move.to_sq();
                        let from_file = (from_sq % 9) as u8 + b'a';
                        let from_rank = 9 - (from_sq / 9) as u8;
                        let to_file = (to_sq % 9) as u8 + b'a';
                        let to_rank = 9 - (to_sq / 9) as u8;

                        println!("bestmove {}{}{}{}", from_file as char, from_rank, to_file as char, to_rank);
                    }
                }
                "quit" => {
                    break;
                }
                _ => {}
            }
        }
    }
}
