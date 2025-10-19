pub mod constants;
pub mod zobrist;
pub mod bitboard;
pub mod r#move;
pub mod move_gen;
pub mod tt;
pub mod evaluate;
pub mod engine;
pub mod opening_book;
pub mod gui;

fn main() {
    gui::run().expect("Failed to run GUI");
}
