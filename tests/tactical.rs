use opensnake::board::{Board, Move};
use opensnake::engine::evaluate;
use opensnake::move_gen::{legal_moves, see};
use opensnake::search::Searcher;

#[test]
fn see_detects_losing_and_winning_captures() {
    // e4xd4 perd la tour : la tour noire d8 recapture sans riposte.
    let bad = Board::from_fen("3rk3/8/8/8/3pR3/8/8/4K3 w - - 0 1").expect("FEN invalide");
    let rxd4 = Move::new(28, 27, None); // e4 -> d4
    let bad_see = see(&bad, &rxd4);
    assert!(bad_see < 0, "SEE devrait voir que e4xd4 perd la tour");

    // d4xe5 gagne un pion non défendu.
    let good = Board::from_fen("4k3/8/8/4p3/3Q4/8/8/4K3 w - - 0 1").expect("FEN invalide");
    let qxe5 = Move::new(27, 36, None);
    assert!(
        see(&good, &qxe5) > 0,
        "SEE devrait voir que d4xe5 gagne un pion"
    );
}

#[test]
fn search_finds_back_rank_mate_in_one() {
    let board = Board::from_fen("6k1/5ppp/8/8/8/8/8/R5K1 w - - 0 1").expect("FEN invalide");
    let mut searcher = Searcher::new();
    searcher.elo = 2000;
    searcher.forced_depth = Some(3);
    let result = searcher.search(&board, None);

    let best = result.best_move.expect("un coup doit être trouvé");
    assert_eq!(best.from, 0, "Ra1 doit jouer le mat");
    assert_eq!(best.to, 56, "Ra8# attendu");
}

#[test]
fn evaluation_prefers_centralized_knight() {
    let center = Board::from_fen("4k3/8/8/8/4N3/8/8/4K3 w - - 0 1").expect("FEN invalide");
    let corner = Board::from_fen("4k3/8/8/8/8/8/8/N3K3 w - - 0 1").expect("FEN invalide");
    assert!(
        evaluate(&center) > evaluate(&corner),
        "Un cavalier central doit mieux valoir qu'un cavalier de coin"
    );
}

#[test]
fn evaluation_prefers_passed_pawn_and_bishop_pair() {
    let advanced =
        Board::from_fen("4k3/8/4P3/8/8/8/8/4K3 w - - 0 1").expect("FEN invalide");
    let backward =
        Board::from_fen("4k3/8/8/8/8/8/4P3/4K3 w - - 0 1").expect("FEN invalide");
    assert!(
        evaluate(&advanced) > evaluate(&backward),
        "Un pion passé avancé doit mieux valoir qu'un pion sur sa case initiale"
    );

    let pair = Board::from_fen("4k3/8/8/8/8/8/8/2B1KB2 w - - 0 1").expect("FEN invalide");
    let mixed = Board::from_fen("4k3/8/8/8/8/8/8/1NB1K3 w - - 0 1").expect("FEN invalide");
    assert!(
        evaluate(&pair) > evaluate(&mixed),
        "Le couple de fous doit être valorisé"
    );
}

#[test]
fn self_play_stays_legal() {
    let mut board = Board::new();
    let mut searcher = Searcher::new();
    searcher.elo = 100; // erreurs intentionnelles : on vérifie surtout la légalité
    searcher.forced_depth = Some(2);

    for _ in 0..8 {
        let legal = legal_moves(&board);
        if legal.is_empty() {
            break;
        }
        let result = searcher.search(&board, None);
        let best = result.best_move.expect("un coup doit être trouvé");
        assert!(legal.contains(&best), "Le moteur a proposé un coup illégal");
        board.make_move(&best);
    }
}
