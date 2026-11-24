use crate::board::{Board, Color, Move, PieceType};

const ROOK_DIRS: [(i8, i8); 4] = [(1, 0), (-1, 0), (0, 1), (0, -1)];
const BISHOP_DIRS: [(i8, i8); 4] = [(1, 1), (1, -1), (-1, 1), (-1, -1)];
const QUEEN_DIRS: [(i8, i8); 8] = [
    (1, 0),
    (0, 1),
    (-1, 0),
    (0, -1),
    (1, 1),
    (1, -1),
    (-1, 1),
    (-1, -1),
];

pub fn pseudo_legal_moves(board: &Board) -> Vec<Move> {
    let mut moves = Vec::new();
    let color = board.turn;

    for sq in 0..64u8 {
        let Some(piece) = board.get(sq) else {
            continue;
        };
        if piece.color != color {
            continue;
        }

        match piece.kind {
            PieceType::Pawn => pawn_moves(board, sq, color, &mut moves),
            PieceType::Knight => add_moves_for(board, sq, color, &mut moves, &crate::board::KNIGHT_OFFSETS, false),
            PieceType::Bishop => add_slider_moves(board, sq, color, &mut moves, &BISHOP_DIRS),
            PieceType::Rook => add_slider_moves(board, sq, color, &mut moves, &ROOK_DIRS),
            PieceType::Queen => add_slider_moves(board, sq, color, &mut moves, &QUEEN_DIRS),
            PieceType::King => add_moves_for(board, sq, color, &mut moves, &crate::board::KING_OFFSETS, true),
        }
    }

    moves
}

pub fn legal_moves(board: &Board) -> Vec<Move> {
    let color = board.turn;
    let enemy = color.opposite();
    let mut legal = Vec::new();

    for m in pseudo_legal_moves(board) {
        let mut next = *board;
        next.make_move(&m);
        if !next.is_attacked(next.king_square(color), enemy) {
            legal.push(m);
        }
    }

    legal
}

fn add_moves_for(
    board: &Board,
    sq: u8,
    color: Color,
    moves: &mut Vec<Move>,
    offsets: &[(i8, i8)],
    include_castling: bool,
) {
    let r = board.row(sq) as i8;
    let f = board.col(sq) as i8;

    for (dr, df) in offsets {
        let nr = r + dr;
        let nf = f + df;
        if nr < 0 || nr > 7 || nf < 0 || nf > 7 {
            continue;
        }
        let to = board.square_index(nr as u8, nf as u8);
        if let Some(p) = board.get(to) {
            if p.color != color {
                moves.push(Move::new(sq, to, None));
            }
        } else {
            moves.push(Move::new(sq, to, None));
        }
    }

    if include_castling {
        add_castling_moves(board, sq, color, moves);
    }
}

fn add_castling_moves(board: &Board, sq: u8, color: Color, moves: &mut Vec<Move>) {
    let enemy = color.opposite();
    let expected_row: u8 = if color == Color::White { 0 } else { 7 };
    if board.row(sq) != expected_row || board.col(sq) != 4 {
        return;
    }
    if !board
        .get(sq)
        .map(|p| p.kind == PieceType::King && p.color == color)
        .unwrap_or(false)
    {
        return;
    }
    if board.is_attacked(sq, enemy) {
        return;
    }

    let right_kingside = if color == Color::White {
        board.castling.white_kingside
    } else {
        board.castling.black_kingside
    };
    let right_queenside = if color == Color::White {
        board.castling.white_queenside
    } else {
        board.castling.black_queenside
    };

    let f_sq = board.square_index(expected_row, 5);
    let g_sq = board.square_index(expected_row, 6);
    if right_kingside
        && board.get(f_sq).is_none()
        && board.get(g_sq).is_none()
        && !board.is_attacked(f_sq, enemy)
        && !board.is_attacked(g_sq, enemy)
    {
        moves.push(Move::new(sq, g_sq, None));
    }

    let d_sq = board.square_index(expected_row, 3);
    let c_sq = board.square_index(expected_row, 2);
    let b_sq = board.square_index(expected_row, 1);
    if right_queenside
        && board.get(b_sq).is_none()
        && board.get(c_sq).is_none()
        && board.get(d_sq).is_none()
        && !board.is_attacked(d_sq, enemy)
        && !board.is_attacked(c_sq, enemy)
    {
        moves.push(Move::new(sq, c_sq, None));
    }
}

fn add_slider_moves(
    board: &Board,
    sq: u8,
    color: Color,
    moves: &mut Vec<Move>,
    dirs: &[(i8, i8)],
) {
    let r = board.row(sq) as i8;
    let f = board.col(sq) as i8;

    for (dr, df) in dirs {
        let mut nr = r + dr;
        let mut nf = f + df;
        while nr >= 0 && nr <= 7 && nf >= 0 && nf <= 7 {
            let to = board.square_index(nr as u8, nf as u8);
            if let Some(p) = board.get(to) {
                if p.color != color {
                    moves.push(Move::new(sq, to, None));
                }
                break;
            }
            moves.push(Move::new(sq, to, None));
            nr += dr;
            nf += df;
        }
    }
}

fn pawn_moves(board: &Board, sq: u8, color: Color, moves: &mut Vec<Move>) {
    let dir: i8 = if color == Color::White { 1 } else { -1 };
    let start_row: i8 = if color == Color::White { 1 } else { 6 };
    let promo_row: u8 = if color == Color::White { 7 } else { 0 };

    let r = board.row(sq) as i8;
    let f = board.col(sq) as i8;

    let one_r = r + dir;
    if one_r >= 0 && one_r <= 7
        && board.get(board.square_index(one_r as u8, f as u8)).is_none()
    {
        add_pawn_move(board, sq, board.square_index(one_r as u8, f as u8), promo_row, moves);

        let two_r = r + dir * 2;
        if r == start_row
            && board.get(board.square_index(two_r as u8, f as u8)).is_none()
        {
            moves.push(Move::new(sq, board.square_index(two_r as u8, f as u8), None));
        }
    }

    for df in [-1i8, 1] {
        let nr = r + dir;
        let nf = f + df;
        if nr < 0 || nr > 7 || nf < 0 || nf > 7 {
            continue;
        }
        let to = board.square_index(nr as u8, nf as u8);
        let is_capture = board.get(to).map(|p| p.color != color).unwrap_or(false)
            || board.en_passant == Some(to);
        if is_capture {
            add_pawn_move(board, sq, to, promo_row, moves);
        }
    }
}

fn add_pawn_move(
    board: &Board,
    sq: u8,
    to: u8,
    promo_row: u8,
    moves: &mut Vec<Move>,
) {
    if board.row(to) == promo_row {
        for promotion in [
            PieceType::Queen,
            PieceType::Rook,
            PieceType::Bishop,
            PieceType::Knight,
        ] {
            moves.push(Move::new(sq, to, Some(promotion)));
        }
    } else {
        moves.push(Move::new(sq, to, None));
    }
}

/// Valeur matérielle d'une pièce (pour l'ordonnancement MVV-LVA).
fn piece_move_value(kind: PieceType) -> i32 {
    match kind {
        PieceType::Pawn => 100,
        PieceType::Knight => 320,
        PieceType::Bishop => 330,
        PieceType::Rook => 500,
        PieceType::Queen => 900,
        PieceType::King => 20000,
    }
}

/// Score d'ordonnancement d'un coup (plus élevé = plus prioritaire).
/// MVV-LVA : on veut capturer la victime la plus précieuse avec l'agresseur le moins précieux.
fn move_order_score(board: &Board, m: &Move) -> i32 {
    let victim = board.get(m.to).map(|p| piece_move_value(p.kind)).unwrap_or(0);
    let attacker = board.get(m.from).map(|p| piece_move_value(p.kind)).unwrap_or(0);

    // Captures : score élevé selon MVV-LVA.
    if victim > 0 {
        1_000_000 + victim * 10 - attacker
    } else {
        // Coups silencieux : score faible, triés par position (from/to) pour stabilité.
        (m.from as i32) * 100 + (m.to as i32)
    }
}

/// Retourne les coups légaux triés par ordre de priorité (bons coups d'abord).
/// Cela permet à l'alpha-beta d'élaguer beaucoup plus efficacement.
pub fn ordered_moves(board: &Board) -> Vec<Move> {
    let mut moves = legal_moves(board);
    moves.sort_by(|a, b| {
        move_order_score(board, b)
            .cmp(&move_order_score(board, a))
            .then_with(|| a.from.cmp(&b.from).then(a.to.cmp(&b.to)))
    });
    moves
}

/// Indique si un coup est une capture (ou en passant).
pub fn is_capture(board: &Board, m: &Move) -> bool {
    board.get(m.to).is_some()
        || (board.en_passant == Some(m.to) && board.col(m.to) != board.col(m.from))
}

/// Indique si un coup est une promotion.
pub fn is_promotion(m: &Move) -> bool {
    m.promotion.is_some()
}
