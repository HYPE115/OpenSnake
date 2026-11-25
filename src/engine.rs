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

const PHASE_MAX: i32 = 24; // 2 dames (4) + 2 tours (2) + 2 fous (1) + 2 cavaliers (1)

const BISHOP_MG: [i32; 8] = [0, 0, 15, 25, 25, 15, 0, 0];
const BISHOP_EG: [i32; 8] = [0, 8, 16, 24, 24, 16, 8, 0];

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

fn kind_index(kind: PieceType) -> usize {
    match kind {
        PieceType::Pawn => 0,
        PieceType::Knight => 1,
        PieceType::Bishop => 2,
        PieceType::Rook => 3,
        PieceType::Queen => 4,
        PieceType::King => 5,
    }
}

fn color_index(color: Color) -> usize {
    if color == Color::White {
        0
    } else {
        1
    }
}

fn sign_of(color: Color) -> i32 {
    if color == Color::White {
        1
    } else {
        -1
    }
}

fn material_value(kind: PieceType, endgame: bool) -> i32 {
    match (kind, endgame) {
        (PieceType::Pawn, false) => 100,
        (PieceType::Pawn, true) => 120,
        (PieceType::Knight, false) => 320,
        (PieceType::Knight, true) => 310,
        (PieceType::Bishop, false) => 330,
        (PieceType::Bishop, true) => 340,
        (PieceType::Rook, false) => 500,
        (PieceType::Rook, true) => 540,
        (PieceType::Queen, false) => 900,
        (PieceType::Queen, true) => 980,
        // Le roi est toujours présent des deux côtés : sa valeur s'annule.
        (PieceType::King, _) => 0,
    }
}

fn phase_value(kind: PieceType) -> i32 {
    match kind {
        PieceType::Knight | PieceType::Bishop => 1,
        PieceType::Rook => 2,
        PieceType::Queen => 4,
        _ => 0,
    }
}

/// Tables case-pièce générées une fois pour toutes (orientation Blanche).
/// Les Noirs utilisent la case miroir `sq ^ 56`.
const fn build_pst(kind: u8, endgame: bool) -> [i32; 64] {
    let mut table = [0i32; 64];
    let mut sq = 0usize;
    while sq < 64 {
        let r = (sq / 8) as i32;
        let f = (sq % 8) as i32;
        let center_file = if f < 4 { f } else { 7 - f };
        let center_rank = if r < 4 { r } else { 7 - r };
        let king_dist = if f > 4 { f - 4 } else { 4 - f };

        table[sq] = match kind {
            // Pions : avancer et occuper le centre vaut de plus en plus en finale.
            0 => {
                if endgame {
                    r * 11 + center_file * 3
                } else {
                    r * 5 + center_file * 5
                }
            }
            // Cavaliers : le centre est roi, les bords sont mauvais.
            1 => {
                if endgame {
                    2 + center_file * 5 + center_rank * 3
                } else {
                    4 + center_file * 7 + center_rank * 4 - if center_file == 0 { 8 } else { 0 }
                }
            }
            // Fous : un peu moins dépendants du centre que les cavaliers.
            2 => {
                if endgame {
                    3 + center_file * 4 + center_rank * 4
                } else {
                    4 + center_file * 4 + center_rank * 3
                }
            }
            // Tours : rester sur la première rangée au milieu de jeu, se centraliser en finale.
            3 => {
                if endgame {
                    center_file * 3 + r
                } else {
                    (if r == 0 { 8 } else { 0 }) + center_file * 2
                }
            }
            // Dames : centre modéré, activité en finale.
            4 => {
                if endgame {
                    center_file * 3 + center_rank * 3
                } else {
                    center_file * 2 + center_rank
                }
            }
            // Rois : abri au milieu de jeu, activité en finale.
            _ => {
                if endgame {
                    r * 7 + center_file * 4
                } else if r == 0 {
                    12 - king_dist * 2
                } else {
                    -(r * 10 + center_file * 5)
                }
            }
        };
        sq += 1;
    }
    table
}

static PST_MG: [[i32; 64]; 6] = [
    build_pst(0, false),
    build_pst(1, false),
    build_pst(2, false),
    build_pst(3, false),
    build_pst(4, false),
    build_pst(5, false),
];

static PST_EG: [[i32; 64]; 6] = [
    build_pst(0, true),
    build_pst(1, true),
    build_pst(2, true),
    build_pst(3, true),
    build_pst(4, true),
    build_pst(5, true),
];

fn pst_value(kind: PieceType, sq: u8, color: Color, endgame: bool) -> i32 {
    let idx = kind_index(kind);
    let table = if endgame { &PST_EG[idx] } else { &PST_MG[idx] };
    let table_sq = if color == Color::White {
        sq as usize
    } else {
        (sq as usize) ^ 56
    };
    table[table_sq]
}

const KNIGHT_DIRS: [(i8, i8); 8] = [
    (-2, -1),
    (-2, 1),
    (-1, -2),
    (-1, 2),
    (1, -2),
    (1, 2),
    (2, -1),
    (2, 1),
];
const BISHOP_DIRS: [(i8, i8); 4] = [(1, 1), (1, -1), (-1, 1), (-1, -1)];
const ROOK_DIRS: [(i8, i8); 4] = [(1, 0), (-1, 0), (0, 1), (0, -1)];
const QUEEN_DIRS: [(i8, i8); 8] = [
    (1, 0),
    (-1, 0),
    (0, 1),
    (0, -1),
    (1, 1),
    (1, -1),
    (-1, 1),
    (-1, -1),
];

fn ray_mobility(board: &Board, sq: u8, dirs: &[(i8, i8)]) -> i32 {
    let mut count = 0;
    for &(dr, df) in dirs {
        let mut r = board.row(sq) as i8 + dr;
        let mut f = board.col(sq) as i8 + df;
        while (0..8).contains(&r) && (0..8).contains(&f) {
            let to = board.square_index(r as u8, f as u8);
            match board.get(to) {
                None => count += 1,
                Some(p) => {
                    if p.color != board.get(sq).map(|own| own.color).unwrap_or(board.turn) {
                        count += 1;
                    }
                    break;
                }
            }
            r += dr;
            f += df;
        }
    }
    count
}

fn mobility_count(board: &Board, sq: u8, piece: crate::board::Piece) -> i32 {
    match piece.kind {
        PieceType::Pawn | PieceType::King => 0,
        PieceType::Knight => KNIGHT_DIRS
            .iter()
            .filter(|&&(dr, df)| {
                let r = board.row(sq) as i8 + dr;
                let f = board.col(sq) as i8 + df;
                (0..8).contains(&r) && (0..8).contains(&f)
                    && board
                        .get(board.square_index(r as u8, f as u8))
                        .map(|p| p.color != piece.color)
                        .unwrap_or(true)
            })
            .count() as i32,
        PieceType::Bishop => ray_mobility(board, sq, &BISHOP_DIRS),
        PieceType::Rook => ray_mobility(board, sq, &ROOK_DIRS),
        PieceType::Queen => ray_mobility(board, sq, &QUEEN_DIRS),
    }
}

fn pawn_structure(board: &Board, pawn_files: &[[i32; 8]; 2]) -> (i32, i32) {
    let mut mg = 0;
    let mut eg = 0;

    for color in [Color::White, Color::Black] {
        let ci = color_index(color);
        let enemy = color.opposite();
        let s = sign_of(color);
        let mut color_mg = 0;
        let mut color_eg = 0;

        for sq in 0..64u8 {
            let Some(piece) = board.get(sq) else {
                continue;
            };
            if piece.color != color || piece.kind != PieceType::Pawn {
                continue;
            }

            let file = board.col(sq) as usize;
            let row = board.row(sq);

            // Pions doublés.
            if pawn_files[ci][file] > 1 {
                color_mg -= 10;
                color_eg -= 15;
            }

            // Pions isolés.
            let neighbors = (file > 0 && pawn_files[ci][file - 1] > 0)
                || (file < 7 && pawn_files[ci][file + 1] > 0);
            if !neighbors {
                color_mg -= 12;
                color_eg -= 18;
            }

            // Pions passés : aucun pion adverse sur les colonnes voisines devant lui.
            let mut passed = true;
            for other in 0..64u8 {
                if let Some(other_piece) = board.get(other) {
                    if other_piece.color == enemy && other_piece.kind == PieceType::Pawn {
                        let other_file = board.col(other) as i32;
                        let other_row = board.row(other);
                        let adjacent_file = (other_file - file as i32).abs() <= 1;
                        let ahead = if color == Color::White {
                            other_row > row
                        } else {
                            other_row < row
                        };
                        if adjacent_file && ahead {
                            passed = false;
                            break;
                        }
                    }
                }
            }
            if passed {
                let relative = if color == Color::White {
                    row as usize
                } else {
                    7 - row as usize
                };
                color_mg += BISHOP_MG[relative];
                color_eg += BISHOP_EG[relative];
            }
        }

        mg += s * color_mg;
        eg += s * color_eg;
    }

    (mg, eg)
}

fn rook_file_bonus(board: &Board, pawn_files: &[[i32; 8]; 2]) -> (i32, i32) {
    let mut mg = 0;
    let mut eg = 0;

    for sq in 0..64u8 {
        if let Some(piece) = board.get(sq) {
            if piece.kind != PieceType::Rook {
                continue;
            }
            let ci = color_index(piece.color);
            let enemy = 1 - ci;
            let file = board.col(sq) as usize;
            let s = sign_of(piece.color);
            let bonus = if pawn_files[ci][file] == 0 {
                if pawn_files[enemy][file] == 0 {
                    (25, 10) // colonne ouverte
                } else {
                    (10, 5) // colonne semi-ouverte
                }
            } else {
                (0, 0)
            };
            mg += s * bonus.0;
            eg += s * bonus.1;
        }
    }
    (mg, eg)
}

fn bishop_pair_bonus(board: &Board, bishops: &[i32; 2]) -> (i32, i32) {
    let mut mg = 0;
    let mut eg = 0;
    for color in [Color::White, Color::Black] {
        if bishops[color_index(color)] >= 2 {
            let s = sign_of(color);
            mg += s * 30;
            eg += s * 45;
        }
    }
    let _ = board;
    (mg, eg)
}

fn mobility_bonus(board: &Board) -> (i32, i32) {
    let mut mg = 0;
    let mut eg = 0;

    for sq in 0..64u8 {
        if let Some(piece) = board.get(sq) {
            let mobility = mobility_count(board, sq, piece);
            let (bonus_mg, bonus_eg) = match piece.kind {
                PieceType::Knight => (mobility * 4, mobility * 5),
                PieceType::Bishop => (mobility * 4, mobility * 5),
                PieceType::Rook => (mobility * 2, mobility * 3),
                PieceType::Queen => (mobility, mobility * 2),
                _ => (0, 0),
            };
            let s = sign_of(piece.color);
            mg += s * bonus_mg;
            eg += s * bonus_eg;
        }
    }
    (mg, eg)
}

fn king_safety(board: &Board, queens: &[i32; 2]) -> (i32, i32) {
    let mut mg = 0;
    let mut eg = 0;

    for color in [Color::White, Color::Black] {
        let enemy = color.opposite();
        let s = sign_of(color);
        let king_sq = board.king_square(color);
        let king_row = board.row(king_sq) as i8;
        let king_file = board.col(king_sq) as i8;
        let dir = if color == Color::White { 1 } else { -1 };

        let mut missing_shield = 0;
        for df in [-1i8, 0, 1] {
            let file = king_file + df;
            if !(0..8).contains(&file) {
                continue;
            }
            let mut shielded = false;
            for step in 1..=2i8 {
                let row = king_row + dir * step;
                if !(0..8).contains(&row) {
                    continue;
                }
                let sq = board.square_index(row as u8, file as u8);
                if board
                    .get(sq)
                    .map(|p| p.color == color && p.kind == PieceType::Pawn)
                    .unwrap_or(false)
                {
                    shielded = true;
                    break;
                }
            }
            if !shielded {
                missing_shield += 1;
            }
        }

        let enemy_queen = queens[color_index(enemy)] > 0;
        let mg_penalty = missing_shield * 8 + if enemy_queen { missing_shield * 8 } else { 0 };
        let eg_penalty = missing_shield * 2;
        let home_square = if color == Color::White { 4 } else { 60 };
        let home_bonus = if king_sq == home_square { 8 } else { 0 };

        mg += s * (home_bonus - mg_penalty);
        eg += s * (home_bonus / 2 - eg_penalty);
    }
    (mg, eg)
}

fn threat_bonus(board: &Board) -> (i32, i32) {
    let mut mg = 0;
    let mut eg = 0;

    for sq in 0..64u8 {
        if let Some(victim) = board.get(sq) {
            if matches!(victim.kind, PieceType::Pawn | PieceType::King) {
                continue;
            }
            let attacker = victim.color.opposite();
            if board.is_attacked(sq, attacker) {
                let base = match victim.kind {
                    PieceType::Knight | PieceType::Bishop => 8,
                    PieceType::Rook => 12,
                    PieceType::Queen => 20,
                    _ => 0,
                };
                let s = sign_of(attacker);
                mg += s * base;
                eg += s * (base / 2);
            }
        }
    }
    (mg, eg)
}

/// Raw evaluation in centipawns (positive = good for White).
/// Tapered evaluation : melange milieu de jeu / finale selon la phase de la partie.
pub fn evaluate(board: &Board) -> f64 {
    let mut mg = 0i32;
    let mut eg = 0i32;
    let mut phase = 0i32;
    let mut pawn_files = [[0i32; 8]; 2];
    let mut bishops = [0i32; 2];
    let mut queens = [0i32; 2];

    // Matériel + tables case-pièce + comptage pour les termes positionnels.
    for sq in 0..64u8 {
        if let Some(piece) = board.get(sq) {
            let ci = color_index(piece.color);
            let s = sign_of(piece.color);
            phase += phase_value(piece.kind);

            mg += s * (material_value(piece.kind, false) + pst_value(piece.kind, sq, piece.color, false));
            eg += s * (material_value(piece.kind, true) + pst_value(piece.kind, sq, piece.color, true));

            match piece.kind {
                PieceType::Pawn => pawn_files[ci][board.col(sq) as usize] += 1,
                PieceType::Bishop => bishops[ci] += 1,
                PieceType::Queen => queens[ci] += 1,
                _ => {}
            }
        }
    }

    // Structure de pions (isolés, doublés, passés).
    let (pawn_mg, pawn_eg) = pawn_structure(board, &pawn_files);
    mg += pawn_mg;
    eg += pawn_eg;

    // Colonnes ouvertes / semi-ouvertes pour les tours.
    let (rook_mg, rook_eg) = rook_file_bonus(board, &pawn_files);
    mg += rook_mg;
    eg += rook_eg;

    // Couple de fous, mobilité, sécurité du roi, menaces.
    let (bishop_mg, bishop_eg) = bishop_pair_bonus(board, &bishops);
    let (mob_mg, mob_eg) = mobility_bonus(board);
    let (king_mg, king_eg) = king_safety(board, &queens);
    let (threat_mg, threat_eg) = threat_bonus(board);
    mg += bishop_mg + mob_mg + king_mg + threat_mg;
    eg += bishop_eg + mob_eg + king_eg + threat_eg;

    let phase = phase.min(PHASE_MAX);
    ((mg * phase + eg * (PHASE_MAX - phase)) / PHASE_MAX) as f64
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
