use engine::bitboard::Board;
use engine::engine::Engine;
use engine::r#move::Move;
use std::fs::File;
use std::io::{self, BufRead, Write};
use std::sync::{Arc, Mutex};

fn parse_uci_move(board: &Board, move_str: &str) -> Option<Move> {
    if move_str.len() != 4 {
        return None;
    }
    let from_file = move_str.chars().nth(0).unwrap() as u8 - b'a';
    let from_rank = move_str.chars().nth(1).unwrap() as u8 - b'0';
    let to_file = move_str.chars().nth(2).unwrap() as u8 - b'a';
    let to_rank = move_str.chars().nth(3).unwrap() as u8 - b'0';

    let from_sq = (9 - from_rank) as usize * 9 + from_file as usize;
    let to_sq = (9 - to_rank) as usize * 9 + to_file as usize;

    let captured_piece = board.board[to_sq];

    Some(Move::new(
        from_sq,
        to_sq,
        if captured_piece == engine::constants::Piece::Empty {
            None
        } else {
            Some(captured_piece)
        },
    ))
}

pub fn parse_go_command(parts: &[&str], board: &Board) -> (i32, Option<u128>) {
    let mut depth = 64; // Default depth
    let mut time_limit_ms = None;

    if parts.contains(&"infinite") {
        return (i32::MAX, None);
    }

    if let Some(depth_idx) = parts.iter().position(|&x| x == "depth") {
        if let Some(depth_val) = parts.get(depth_idx + 1) {
            if let Ok(d) = depth_val.parse() {
                depth = d;
            }
        }
    }

    if let Some(movetime_idx) = parts.iter().position(|&x| x == "movetime") {
        if let Some(movetime_val) = parts.get(movetime_idx + 1) {
            if let Ok(t) = movetime_val.parse() {
                time_limit_ms = Some(t);
            }
        }
    }

    if time_limit_ms.is_none() {
        let mut wtime: Option<u128> = None;
        let mut btime: Option<u128> = None;
        let mut winc: Option<u128> = Some(0);
        let mut binc: Option<u128> = Some(0);
        let mut movestogo: Option<u128> = None;

        let mut i = 0;
        while i < parts.len() {
            match parts[i] {
                "wtime" => {
                    wtime = parts.get(i + 1).and_then(|s| s.parse().ok());
                    i += 2;
                }
                "btime" => {
                    btime = parts.get(i + 1).and_then(|s| s.parse().ok());
                    i += 2;
                }
                "winc" => {
                    winc = parts.get(i + 1).and_then(|s| s.parse().ok());
                    i += 2;
                }
                "binc" => {
                    binc = parts.get(i + 1).and_then(|s| s.parse().ok());
                    i += 2;
                }
                "movestogo" => {
                    movestogo = parts.get(i + 1).and_then(|s| s.parse().ok());
                    i += 2;
                }
                _ => i += 1,
            }
        }

        let (time_to_use, increment) = if board.player_to_move == engine::constants::Player::Red {
            (wtime, winc.unwrap_or(0))
        } else {
            (btime, binc.unwrap_or(0))
        };

        if let Some(t) = time_to_use {
            if let Some(moves) = movestogo {
                time_limit_ms = Some(t / moves as u128 + increment);
            } else {
                time_limit_ms = Some(t / 20u128 + increment);
            }
        }
    }
    (depth, time_limit_ms)
}

fn main() {
    let mut log_file = File::create("uci.log").unwrap();
    let engine = Arc::new(Mutex::new(Engine::new(128)));
    let mut board: Option<Board> = None;

    let stdin = io::stdin();
    for line in stdin.lock().lines() {
        let line = line.unwrap();
        writeln!(log_file, "Received: {}", line).unwrap();
        let parts: Vec<&str> = line.split_whitespace().collect();
        if let Some(command) = parts.get(0) {
            match *command {
                "uci" => {
                    println!("id name Xiangqi");
                    println!("id author Hezhaoyun");
                    println!("uciok");
                }
                "isready" => {
                    println!("readyok");
                }
                "ucinewgame" => {
                    let mut engine_lock = engine.lock().unwrap();
                    engine_lock.clear_history();
                    engine_lock.tt.clear();
                }
                "position" => {
                    let mut new_board = if parts.get(1) == Some(&"startpos") {
                        Board::from_fen(
                            "rnbakabnr/9/1c5c1/p1p1p1p1p/9/9/P1P1P1P1P/1C5C1/9/RNBAKABNR w - - 0 1",
                        )
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
                        let (depth, time_limit_ms) = parse_go_command(&parts, b);
                        // writeln!(log_file, "depth: {}, time_limit_ms: {:?}", depth, time_limit_ms).unwrap();

                        let mut engine_lock = engine.lock().unwrap();
                        engine_lock.stop_search = false;

                        let (best_move, _) = engine_lock.search(b, depth, time_limit_ms);
                        writeln!(log_file, "bestmove {}", best_move.to_uci_string()).unwrap();
                    }
                }
                "stop" => {
                    let mut engine_lock = engine.lock().unwrap();
                    engine_lock.stop_search = true;
                }
                "quit" => {
                    break;
                }
                _ => {}
            }
        }
    }
}
