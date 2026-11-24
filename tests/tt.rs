use opensnake::board::Board;
use opensnake::search::Searcher;

#[test]
fn tt_probes_and_hits_are_recorded() {
    let board = Board::new();
    let mut searcher = Searcher::new();
    searcher.elo = 2000; // profondeur 7
    let result = searcher.search(&board, None);

    // La TT doit avoir été sondée au moins une fois
    assert!(result.tt_probes > 0, "Aucun probe TT enregistré");
    println!("TT probes: {}, TT hits: {}, hit rate: {:.2}%", result.tt_probes, result.tt_hits, result.tt_hits as f64 / result.tt_probes as f64 * 100.0);
}

#[test]
fn tt_deterministic_same_position_same_move() {
    let board = Board::new();
    let mut searcher1 = Searcher::new();
    searcher1.elo = 1800;
    let r1 = searcher1.search(&board, None);

    let mut searcher2 = Searcher::new();
    searcher2.elo = 1800;
    let r2 = searcher2.search(&board, None);

    // Même position, même ELO → même meilleur coup
    assert_eq!(r1.best_move, r2.best_move, "La recherche doit être déterministe");
    assert_eq!(r1.depth, r2.depth, "Même profondeur attendue");
}

#[test]
fn tt_depth_override_respected() {
    let board = Board::new();
    let mut searcher = Searcher::new();
    searcher.elo = 100;  // normalement profondeur 1
    searcher.forced_depth = Some(5); // mais on force 5
    let result = searcher.search(&board, None);
    assert_eq!(result.depth, 5, "La profondeur forcée doit être respectée");
}

#[test]
fn tt_speeds_up_repeated_subtree() {
    // Une position avec beaucoup de transpositions possibles (finale de pions)
    let fen = "8/8/8/8/8/5k2/5PPP/4K3 w - - 0 1";
    let board = Board::from_fen(fen).expect("FEN invalide");
    let mut searcher = Searcher::new();
    searcher.elo = 2000;
    let result = searcher.search(&board, None);

    // Avec une recherche à profondeur 5+, la TT doit avoir des hits
    assert!(result.tt_probes > 0, "La TT devrait être utilisée");
    // Le taux de hit devrait être significatif sur une position avec transpositions
    if result.tt_probes > 100 {
        assert!(result.tt_hits as f64 / result.tt_probes as f64 > 0.05,
            "Taux de hit TT trop faible: {:.2}%", result.tt_hits as f64 / result.tt_probes as f64 * 100.0);
    }
    println!("Finale pions: probes={}, hits={}, hit_rate={:.2}%", result.tt_probes, result.tt_hits, result.tt_hits as f64 / result.tt_probes as f64 * 100.0);
}
