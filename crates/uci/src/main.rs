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
    let from_file = move_str.chars().nth(0).unwrap() as u8 - b'a';
    let from_rank = move_str.chars().nth(1).unwrap() as u8 - b'0';
    let to_file = move_str.chars().nth(2).unwrap() as u8 - b'a';
    let to_rank = move_str.chars().nth(3).unwrap() as u8 - b'0';

    let from_sq = (9 - from_rank) as usize * 9 + from_file as usize;
    let to_sq = (9 - to_rank) as usize * 9 + to_file as usize;

    let captured_piece = board.board[to_sq];

    Some(Move::new(from_sq, to_sq, if captured_piece == engine::constants::Piece::Empty { None } else { Some(captured_piece) }))
}

pub fn parse_go_command(parts: &[&str], board: &Board) -> (i32, Option<u128>) {
    let mut depth = 10; // Default depth
    let mut time_limit_ms = None;

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
        let (wtime, btime) = parts.windows(2).fold((None::<u128>, None::<u128>), |(w, b), chunk| {
            match chunk[0] {
                "wtime" => (chunk[1].parse().ok(), b),
                "btime" => (w, chunk[1].parse().ok()),
                _ => (w, b),
            }
        });

        let time_to_use = if board.player_to_move == engine::constants::Player::Red {
            wtime
        } else {
            btime
        };
        if let Some(t) = time_to_use {
            time_limit_ms = Some(t / 20);
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
                        let (depth, time_limit_ms) = parse_go_command(&parts, b);
                        let mut engine_lock = engine.lock().unwrap();
                        let (best_move, _) = engine_lock.search(b, depth, time_limit_ms);

                        println!("bestmove {}", best_move.to_uci_string());
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

#[cfg(test)]
mod tests {
    use super::*;
    use engine::bitboard::Board;

    #[test]
    fn test_parse_go_command_depth() {
        let board = Board::from_fen("rnbakabnr/9/1c5c1/p1p1p1p1p/9/9/P1P1P1P1P/1C5C1/9/RNBAKABNR w - - 0 1");
        let parts = vec!["go", "depth", "5"];
        let (depth, time_limit) = parse_go_command(&parts, &board);
        assert_eq!(depth, 5);
        assert_eq!(time_limit, None);
    }

    #[test]
    fn test_parse_go_command_movetime() {
        let board = Board::from_fen("rnbakabnr/9/1c5c1/p1p1p1p1p/9/9/P1P1P1P1P/1C5C1/9/RNBAKABNR w - - 0 1");
        let parts = vec!["go", "movetime", "10000"];
        let (depth, time_limit) = parse_go_command(&parts, &board);
        assert_eq!(depth, 10); // default
        assert_eq!(time_limit, Some(10000));
    }

    #[test]
    fn test_parse_go_command_wtime_btime() {
        let board = Board::from_fen("rnbakabnr/9/1c5c1/p1p1p1p1p/9/9/P1P1P1P1P/1C5C1/9/RNBAKABNR w - - 0 1");
        let parts = vec!["go", "wtime", "20000", "btime", "30000"];
        let (depth, time_limit) = parse_go_command(&parts, &board);
        assert_eq!(depth, 10); // default
        assert_eq!(time_limit, Some(1000)); // 20000 / 20
    }

    #[test]
    fn test_parse_go_command_wtime_btime_black() {
        let mut board = Board::from_fen("rnbakabnr/9/1c5c1/p1p1p1p1p/9/9/P1P1P1P1P/1C5C1/9/RNBAKABNR w - - 0 1");
        board.player_to_move = engine::constants::Player::Black;
        let parts = vec!["go", "wtime", "20000", "btime", "30000"];
        let (depth, time_limit) = parse_go_command(&parts, &board);
        assert_eq!(depth, 10); // default
        assert_eq!(time_limit, Some(1500)); // 30000 / 20
    }
}