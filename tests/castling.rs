use opensnake::alg_to_square;
use opensnake::board::{Board, Move, PieceType};
use opensnake::move_gen::legal_moves;

fn apply(board: &mut Board, uci: &str) {
    let from = alg_to_square(&uci[0..2]).unwrap_or_else(|| panic!("case invalide: {}", uci));
    let to = alg_to_square(&uci[2..4]).unwrap_or_else(|| panic!("case invalide: {}", uci));
    let promotion = match uci.chars().nth(4) {
        Some('q') => Some(PieceType::Queen),
        Some('r') => Some(PieceType::Rook),
        Some('b') => Some(PieceType::Bishop),
        Some('n') => Some(PieceType::Knight),
        _ => None,
    };
    let mv = Move::new(from, to, promotion);
    assert!(
        legal_moves(board).contains(&mv),
        "coup {} illégal dans {}",
        uci,
        board.to_fen()
    );
    board.make_move(&mv);
}

#[test]
fn castling_rights_survive_ordinary_moves() {
    let mut board = Board::new();
    apply(&mut board, "e2e4");
    apply(&mut board, "e7e5");
    apply(&mut board, "g1f3");
    let fen = board.to_fen();
    assert!(
        fen.contains("KQkq"),
        "les droits de roque doivent survivre aux coups normaux: {}",
        fen
    );
}

#[test]
fn uci_castling_is_legal_and_moves_the_rook() {
    let mut board = Board::new();
    for uci in [
        "e2e4", "e7e5", "g1f3", "b8c6", "f1c4", "g8f6", "e1g1",
    ] {
        apply(&mut board, uci);
    }
    let fen = board.to_fen();
    assert!(fen.contains("RNBQ1RK1"), "la tour doit suivre le roi: {}", fen);
    assert!(fen.contains("kq"), "les droits noirs restent: {}", fen);
}

#[test]
fn black_castling_from_cutechess_book_line_is_legal() {
    // Position jouée par Cute Chess avec baron30.bin qui déclenchait un
    // "illegal move e8d8" : le roque du livre n'était pas appliqué.
    let mut board = Board::new();
    for uci in [
        "d2d4", "e7e6", "e2e4", "d7d5", "b1c3", "g8f6", "c1g5", "f8e7", "e4e5", "f6d7", "h2h4",
        "e8g8",
    ] {
        apply(&mut board, uci);
    }
    let fen = board.to_fen();
    assert!(fen.contains("rnbq1rk1"), "le roi noir doit être en g8: {}", fen);
}

#[test]
fn rook_and_king_moves_clear_only_their_own_rights() {
    let mut board = Board::new();
    apply(&mut board, "a2a3");
    apply(&mut board, "a7a6");
    apply(&mut board, "h2h3");
    apply(&mut board, "h7h6");
    apply(&mut board, "h1h2");
    let fen = board.to_fen();
    assert!(
        fen.contains("Qkq") && !fen.contains("KQkq"),
        "bouger la tour h1 n'efface que le droit K: {}",
        fen
    );
}
