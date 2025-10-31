pub mod bitboard;
pub mod config;
pub mod constants;
pub mod engine;
pub mod evaluate;
pub mod move_gen;
pub mod movelist;
pub mod r#move;
pub mod opening_book;
pub mod tt;
pub mod zobrist;

#[cfg(test)]
mod tests {
    use super::bitboard::Board;
    use super::constants::Piece;

    #[test]
    fn test_make_move() {
        let mut board = Board::from_fen("rnbakabnr/9/1c5c1/p1p1p1p1p/9/9/P1P1P1P1P/1C5C1/9/RNBAKABNR w - - 0 1");
        let moves = board.generate_legal_moves();
        let first_move = moves[0];
        let from_sq = first_move.from_sq();
        let to_sq = first_move.to_sq();
        let moving_piece = board.board[from_sq];

        board.move_piece(first_move);

        assert_eq!(board.board[to_sq], moving_piece);
        assert_eq!(board.board[from_sq], Piece::Empty);
    }

    #[test]
    fn test_unmake_move() {
        let mut board = Board::from_fen("rnbakabnr/9/1c5c1/p1p1p1p1p/9/9/P1P1P1P1P/1C5C1/9/RNBAKABNR w - - 0 1");
        let original_fen = board.to_fen();
        let moves = board.generate_legal_moves();
        let first_move = moves[0];

        let captured_piece = board.move_piece(first_move);
        board.unmove_piece(first_move, captured_piece);

        assert_eq!(board.to_fen(), original_fen);
    }
}