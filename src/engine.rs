use crate::board::{Board, Color, Move, PieceType};
use crate::move_gen::legal_moves;

pub const SCORE_MIN: f64 = 1.0;
pub const SCORE_MAX: f64 = 100.0;
pub const NEUTRAL_SCORE: f64 = 50.0;

const PAWN_VALUE: f64 = 100.0;
const KNIGHT_VALUE: f64 = 320.0;
const BISHOP_VALUE: f64 = 330.0;
const ROOK_VALUE: f64 = 500.0;
const QUEEN_VALUE: f64 = 900.0;
const KING_VALUE: f64 = 20000.0;

pub fn piece_value(kind: PieceType) -> f64 {
    match kind {
        PieceType::Pawn => PAWN_VALUE,
        PieceType::Knight => KNIGHT_VALUE,
        PieceType::Bishop => BISHOP_VALUE,
        PieceType::Rook => ROOK_VALUE,
        PieceType::Queen => QUEEN_VALUE,
        PieceType::King => KING_VALUE,
    }
}

/// Raw evaluation in centipawns (positive = good for White).
pub fn evaluate(board: &Board) -> f64 {
    let mut score = 0.0;
    for sq in 0..64u8 {
        if let Some(p) = board.get(sq) {
            let v = piece_value(p.kind);
            score += if p.color == Color::White { v } else { -v };
        }
    }
    score
}

/// Convert a centipawn evaluation into the 1-100 score scale.
/// Neutral positions score 50, +100 score means one pawn advantage for the side evaluated.
pub fn to_score(eval_centipawns: f64) -> f64 {
    let pawns = eval_centipawns / 100.0;
    (NEUTRAL_SCORE + NEUTRAL_SCORE * pawns).clamp(SCORE_MIN, SCORE_MAX)
}

/// Evaluation in the 1-100 score scale, from the point of view of `color`.
pub fn evaluate_for(board: &Board, color: Color) -> f64 {
    let eval = evaluate(board);
    let perspective = if color == Color::White { eval } else { -eval };
    to_score(perspective)
}

pub struct ScoredMove {
    pub move_: Move,
    pub score: f64,
    pub eval_centipawns: f64,
}

impl std::fmt::Display for ScoredMove {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} (score {:.6}/100, {:.6} centipawns)",
            self.move_, self.score, self.eval_centipawns
        )
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum GameStatus {
    Ongoing,
    Checkmate,
    Stalemate,
}

pub fn status(board: &Board) -> GameStatus {
    if !legal_moves(board).is_empty() {
        return GameStatus::Ongoing;
    }
    if board.is_in_check(board.turn) {
        GameStatus::Checkmate
    } else {
        GameStatus::Stalemate
    }
}

pub struct Engine;

impl Engine {
    pub fn new() -> Self {
        Engine
    }

    /// Score the current position on the 1-100 scale for the given color.
    pub fn score(&self, board: &Board, color: Color) -> f64 {
        evaluate_for(board, color)
    }

    /// Find the best move in the current position.
    pub fn best_move(&self, board: &Board) -> Option<ScoredMove> {
        self.best_moves(board, 1).into_iter().next()
    }

    /// Find the best moves in the current position, sorted by score (best first).
    pub fn best_moves(&self, board: &Board, count: usize) -> Vec<ScoredMove> {
        let color = board.turn;
        let mut scored: Vec<ScoredMove> = Vec::new();

        for m in legal_moves(board) {
            let mut next = *board;
            next.make_move(&m);
            let eval = evaluate(&next);
            let mover_eval = if color == Color::White { eval } else { -eval };
            scored.push(ScoredMove {
                move_: m,
                score: to_score(mover_eval),
                eval_centipawns: mover_eval,
            });
        }

        scored.sort_by(|a, b| {
            // Critère principal : centipawns bruts (précision maximale, ex: 64.532168)
            // Cela permet de départager deux coups qui auraient le même score 1-100 arrondi.
            b.eval_centipawns
                .partial_cmp(&a.eval_centipawns)
                .unwrap()
                .then(b.score.partial_cmp(&a.score).unwrap())
        });
        scored.truncate(count);
        scored
    }
}

impl Default for Engine {
    fn default() -> Self {
        Engine::new()
    }
}
