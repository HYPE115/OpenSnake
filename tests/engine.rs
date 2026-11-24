use opensnake::board::{Board, Color, Move};
use opensnake::engine::{Engine, GameStatus, status};
use opensnake::move_gen::legal_moves;

fn play(board: &mut Board, from: u8, to: u8) {
    board.make_move(&Move::simple(from, to));
}

#[test]
fn initial_position_has_20_legal_moves() {
    let board = Board::new();
    assert_eq!(legal_moves(&board).len(), 20);
}

#[test]
fn initial_position_score_is_neutral() {
    let engine = Engine::new();
    let board = Board::new();
    let score = engine.score(&board, Color::White);
    assert!((score - 50.0).abs() < 1e-9);
}

#[test]
fn initial_position_has_a_best_move() {
    let engine = Engine::new();
    let board = Board::new();
    let best = engine.best_move(&board).expect("expected at least one legal move");
    assert!(legal_moves(&board).contains(&best.move_));
}

#[test]
fn fooly_mate_is_checkmate() {
    let mut board = Board::new();
    play(&mut board, 13, 21); // f2-f3
    play(&mut board, 52, 36); // e7-e5
    play(&mut board, 14, 30); // g2-g4
    play(&mut board, 59, 31); // d8-h4#
    assert_eq!(status(&board), GameStatus::Checkmate);
    assert!(legal_moves(&board).is_empty());
}
