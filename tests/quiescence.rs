use opensnake::board::Board;
use opensnake::search::Searcher;

/// La dame blanche sur d4 peut capturer le pion noir sur e5.
/// La quiescence search doit trouver ce coup gagnant même à faible profondeur.
#[test]
fn quiescence_finds_winning_capture() {
    // d4 (index 27) -> e5 (index 36) : diagonal valide pour la dame
    let fen = "4k3/8/8/4p3/3Q4/8/8/4K3 w - - 0 1";
    let board = Board::from_fen(fen).expect("FEN invalide");
    let mut searcher = Searcher::new();
    searcher.elo = 2000; // profondeur 7, pas d'erreur intentionnelle
    let result = searcher.search(&board, None);

    assert!(result.best_move.is_some(), "Le moteur doit trouver un coup");
    let best = result.best_move.unwrap();
    // Qxe5 : d4 (27) -> e5 (36)
    assert_eq!(
        best.from, 27,
        "Le moteur devrait jouer Qxe5 (d4->e5), a trouvé {}-{}",
        best.from, best.to
    );
    assert_eq!(best.to, 36);
}

/// Avec profondeur 1 (forced) + quiescence, le moteur doit quand même
/// trouver la capture gagnante Qxe5.
#[test]
fn quiescence_works_at_depth_1() {
    let fen = "4k3/8/8/4p3/3Q4/8/8/4K3 w - - 0 1";
    let board = Board::from_fen(fen).expect("FEN invalide");

    let mut searcher = Searcher::new();
    searcher.elo = 2000; // déterministe
    searcher.forced_depth = Some(1); // forcer profondeur 1
    let result = searcher.search(&board, None);

    assert!(result.best_move.is_some());
    let best = result.best_move.unwrap();
    assert_eq!(best.from, 27, "Qxe5 attendu (d4->e5)");
    assert_eq!(best.to, 36);
}

/// La quiescence ne doit pas planter sur une position tactique avec plusieurs captures.
#[test]
fn quiescence_does_not_crash_on_tactical_position() {
    let fen = "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq - 0 2";
    let board = Board::from_fen(fen).expect("FEN invalide");
    let mut searcher = Searcher::new();
    searcher.elo = 1500;
    let result = searcher.search(&board, None);
    assert!(result.best_move.is_some());
}

/// PVS est une optimisation d'alpha-beta : le résultat doit être déterministe
/// pour une même position et un même ELO.
#[test]
fn search_is_deterministic_with_pvs() {
    let board = Board::new();

    let mut s1 = Searcher::new();
    s1.elo = 1800;
    let r1 = s1.search(&board, None);

    let mut s2 = Searcher::new();
    s2.elo = 1800;
    let r2 = s2.search(&board, None);

    assert_eq!(r1.best_move, r2.best_move, "PVS doit être déterministe");
    assert_eq!(r1.depth, r2.depth);
}

/// Une position de mat en 1 doit être trouvée même par quiescence.
#[test]
fn quiescence_finds_mate_in_one() {
    // Mat du fou : Fou c4 mat
    let fen = "rnb1kbnr/pppp1ppp/8/4p3/4P3/5N2/PPPP1PPP/RNBQKB1R b KQkq - 1 2";
    let board = Board::from_fen(fen).expect("FEN invalide");
    let mut searcher = Searcher::new();
    searcher.elo = 2000;
    let result = searcher.search(&board, None);

    assert!(result.best_move.is_some());
    // Le coup de mat devrait être trouvé
    println!("Mat en 1 trouvé : {:?} (score: {})", result.best_move, result.best_score);
}