use crate::{bitboard::Board, engine::Engine};

pub mod constants;
pub mod zobrist;
pub mod bitboard;
pub mod r#move;
pub mod move_gen;
pub mod tt;
pub mod evaluate;
pub mod engine;
pub mod opening_book;
pub mod tui;

fn main() {
    tui::run();
    // profile();
}

#[allow(dead_code)]
fn profile() {
    let fen = "rnbakabnr/9/1c5c1/p1p1p1p1p/9/9/P1P1P1P1P/1C5C1/9/RNBAKABNR w - - 0 1";
    let mut board = Board::from_fen(fen);
    let mut engine = Engine::new(16);
    // 16MB TT
    let (best_move, score) = engine.search(&mut board, 10);
    // Search to depth 6 for now
    println!("Best move: {:?} -> {}", best_move, score);
}

