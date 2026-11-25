#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Color {
    White,
    Black,
}

impl Color {
    pub fn opposite(self) -> Color {
        match self {
            Color::White => Color::Black,
            Color::Black => Color::White,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PieceType {
    Pawn,
    Knight,
    Bishop,
    Rook,
    Queen,
    King,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Piece {
    pub color: Color,
    pub kind: PieceType,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Move {
    pub from: u8,
    pub to: u8,
    pub promotion: Option<PieceType>,
}

impl Move {
    pub fn new(from: u8, to: u8, promotion: Option<PieceType>) -> Self {
        Move { from, to, promotion }
    }

    pub fn simple(from: u8, to: u8) -> Self {
        Move::new(from, to, None)
    }

    fn col_of(&self, sq: u8) -> u8 {
        sq % 8
    }

    fn row_of(&self, sq: u8) -> u8 {
        sq / 8
    }

    fn square_name(&self, sq: u8) -> String {
        format!(
            "{}{}",
            char::from(b'a' + self.col_of(sq)),
            self.row_of(sq) + 1
        )
    }
}

impl std::fmt::Display for Move {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}-{}",
            self.square_name(self.from),
            self.square_name(self.to)
        )?;
        if let Some(promotion) = self.promotion {
            write!(f, "={}", promotion_letter(promotion))?;
        }
        Ok(())
    }
}

fn promotion_letter(p: PieceType) -> char {
    match p {
        PieceType::Queen => 'Q',
        PieceType::Rook => 'R',
        PieceType::Bishop => 'B',
        PieceType::Knight => 'N',
        _ => 'Q',
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct CastlingRights {
    pub white_kingside: bool,
    pub white_queenside: bool,
    pub black_kingside: bool,
    pub black_queenside: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Board {
    squares: [Option<Piece>; 64],
    pub turn: Color,
    pub castling: CastlingRights,
    pub en_passant: Option<u8>,
}

pub(crate) const KNIGHT_OFFSETS: [(i8, i8); 8] = [
    (-2, -1),
    (-2, 1),
    (-1, -2),
    (-1, 2),
    (1, -2),
    (1, 2),
    (2, -1),
    (2, 1),
];

pub(crate) const KING_OFFSETS: [(i8, i8); 8] = [
    (-1, -1),
    (-1, 0),
    (-1, 1),
    (0, -1),
    (0, 1),
    (1, -1),
    (1, 0),
    (1, 1),
];

pub(crate) const ALL_DIRS: [(i8, i8); 8] = [
    (1, 0),
    (0, 1),
    (-1, 0),
    (0, -1),
    (1, 1),
    (1, -1),
    (-1, 1),
    (-1, -1),
];

impl Board {
    pub fn new() -> Self {
        Self::initial_position()
    }

    pub fn initial_position() -> Self {
        let mut board = Board {
            squares: [None; 64],
            turn: Color::White,
            castling: CastlingRights::default(),
            en_passant: None,
        };
        board.castling = CastlingRights {
            white_kingside: true,
            white_queenside: true,
            black_kingside: true,
            black_queenside: true,
        };

        // Row 0 = rank 1 (white back rank), row 7 = rank 8 (black back rank).
        board.set(0, Some(Piece {
            color: Color::White,
            kind: PieceType::Rook,
        }));
        board.set(1, Some(Piece {
            color: Color::White,
            kind: PieceType::Knight,
        }));
        board.set(2, Some(Piece {
            color: Color::White,
            kind: PieceType::Bishop,
        }));
        board.set(3, Some(Piece {
            color: Color::White,
            kind: PieceType::Queen,
        }));
        board.set(4, Some(Piece {
            color: Color::White,
            kind: PieceType::King,
        }));
        board.set(5, Some(Piece {
            color: Color::White,
            kind: PieceType::Bishop,
        }));
        board.set(6, Some(Piece {
            color: Color::White,
            kind: PieceType::Knight,
        }));
        board.set(7, Some(Piece {
            color: Color::White,
            kind: PieceType::Rook,
        }));

        for f in 0..8u8 {
            board.set(8 + f, Some(Piece {
                color: Color::White,
                kind: PieceType::Pawn,
            }));
        }

        for f in 0..8u8 {
            board.set(48 + f, Some(Piece {
                color: Color::Black,
                kind: PieceType::Pawn,
            }));
        }

        board.set(56, Some(Piece {
            color: Color::Black,
            kind: PieceType::Rook,
        }));
        board.set(57, Some(Piece {
            color: Color::Black,
            kind: PieceType::Knight,
        }));
        board.set(58, Some(Piece {
            color: Color::Black,
            kind: PieceType::Bishop,
        }));
        board.set(59, Some(Piece {
            color: Color::Black,
            kind: PieceType::Queen,
        }));
        board.set(60, Some(Piece {
            color: Color::Black,
            kind: PieceType::King,
        }));
        board.set(61, Some(Piece {
            color: Color::Black,
            kind: PieceType::Bishop,
        }));
        board.set(62, Some(Piece {
            color: Color::Black,
            kind: PieceType::Knight,
        }));
        board.set(63, Some(Piece {
            color: Color::Black,
            kind: PieceType::Rook,
        }));

        board
    }

    pub fn empty() -> Self {
        Board {
            squares: [None; 64],
            turn: Color::White,
            castling: CastlingRights::default(),
            en_passant: None,
        }
    }

    pub fn get(&self, sq: u8) -> Option<Piece> {
        self.squares[sq as usize].clone()
    }

    pub fn set(&mut self, sq: u8, piece: Option<Piece>) {
        self.squares[sq as usize] = piece;
    }

    pub fn row(&self, sq: u8) -> u8 {
        sq / 8
    }

    pub fn col(&self, sq: u8) -> u8 {
        sq % 8
    }

    pub fn square_index(&self, row: u8, col: u8) -> u8 {
        row * 8 + col
    }

    pub fn square_name(&self, sq: u8) -> String {
        format!(
            "{}{}",
            char::from(b'a' + self.col(sq)),
            self.row(sq) + 1
        )
    }

    pub fn king_square(&self, color: Color) -> u8 {
        for sq in 0..64u8 {
            if let Some(p) = self.get(sq) {
                if p.color == color && p.kind == PieceType::King {
                    return sq;
                }
            }
        }
        0
    }

    pub fn is_attacked(&self, square: u8, by: Color) -> bool {
        let r = self.row(square) as i8;
        let f = self.col(square) as i8;

        // Pawn attacks: a `by` pawn attacks from one row behind `square`.
        let pawn_row = if by == Color::White { r - 1 } else { r + 1 };
        if pawn_row >= 0 && pawn_row <= 7 {
            let pr = pawn_row as u8;
            for df in [-1i8, 1] {
                let nf = f + df;
                if nf >= 0 && nf <= 7 {
                    if let Some(p) = self.squares[(pr * 8 + nf as u8) as usize] {
                        if p.color == by && p.kind == PieceType::Pawn {
                            return true;
                        }
                    }
                }
            }
        }

        // Knight attacks.
        for (dr, df) in KNIGHT_OFFSETS {
            let nr = r + dr;
            let nf = f + df;
            if nr < 0 || nr > 7 || nf < 0 || nf > 7 {
                continue;
            }
            if let Some(p) = self.squares[(nr as u8 * 8 + nf as u8) as usize] {
                if p.color == by && p.kind == PieceType::Knight {
                    return true;
                }
            }
        }

        // King attacks.
        for (dr, df) in KING_OFFSETS {
            let nr = r + dr;
            let nf = f + df;
            if nr < 0 || nr > 7 || nf < 0 || nf > 7 {
                continue;
            }
            if let Some(p) = self.squares[(nr as u8 * 8 + nf as u8) as usize] {
                if p.color == by && p.kind == PieceType::King {
                    return true;
                }
            }
        }

        // Sliding attacks (rook / bishop / queen).
        for (dr, df) in ALL_DIRS {
            let mut nr = r + dr;
            let mut nf = f + df;
            while nr >= 0 && nr <= 7 && nf >= 0 && nf <= 7 {
                let idx = (nr as u8 * 8 + nf as u8) as usize;
                if let Some(p) = self.squares[idx] {
                    let orthogonal = dr == 0 || df == 0;
                    let is_attack = if orthogonal {
                        matches!(p.kind, PieceType::Rook | PieceType::Queen)
                    } else {
                        matches!(p.kind, PieceType::Bishop | PieceType::Queen)
                    };
                    if is_attack && p.color == by {
                        return true;
                    }
                    break;
                }
                nr += dr;
                nf += df;
            }
        }

        false
    }

    pub fn is_in_check(&self, color: Color) -> bool {
        self.is_attacked(self.king_square(color), color.opposite())
    }

    pub fn make_move(&mut self, m: &Move) {
        let mover = self.turn;
        let moving = self.squares[m.from as usize].unwrap();

        let is_en_passant = moving.kind == PieceType::Pawn
            && self.en_passant == Some(m.to)
            && self.col(m.to) != self.col(m.from);

        if is_en_passant {
            let captured = self.square_index(self.row(m.from), self.col(m.to));
            self.squares[captured as usize] = None;
        }

        self.squares[m.to as usize] = self.squares[m.from as usize];
        self.squares[m.from as usize] = None;

        if moving.kind == PieceType::Pawn && (self.row(m.to) == 7 || self.row(m.to) == 0) {
            if let Some(promotion) = m.promotion {
                self.squares[m.to as usize] = Some(Piece {
                    color: mover,
                    kind: promotion,
                });
            }
        }

        if moving.kind == PieceType::King && m.to.abs_diff(m.from) == 2 {
            let row = self.row(m.from);
            if m.to % 8 == (m.from % 8) + 2 {
                let rook_to = self.square_index(row, 5);
                let rook_from = self.square_index(row, 7);
                self.squares[rook_to as usize] = self.squares[rook_from as usize];
                self.squares[rook_from as usize] = None;
            } else {
                let rook_to = self.square_index(row, 3);
                let rook_from = self.square_index(row, 0);
                self.squares[rook_to as usize] = self.squares[rook_from as usize];
                self.squares[rook_from as usize] = None;
            }
        }

        // On ne touche aux droits de roque que si le roi ou une tour d'origine
        // bouge (ou est capturée). Les réinitialiser à chaque coup empêchait
        // tout roque et désynchronisait le moteur des GUI comme Cute Chess.
        if moving.kind == PieceType::King {
            if mover == Color::White {
                self.castling.white_kingside = false;
                self.castling.white_queenside = false;
            } else {
                self.castling.black_kingside = false;
                self.castling.black_queenside = false;
            }
        }
        if m.from == 7 {
            self.castling.white_kingside = false;
        }
        if m.from == 0 {
            self.castling.white_queenside = false;
        }
        if m.from == 63 {
            self.castling.black_kingside = false;
        }
        if m.from == 56 {
            self.castling.black_queenside = false;
        }
        if m.to == 7 {
            self.castling.white_kingside = false;
        }
        if m.to == 0 {
            self.castling.white_queenside = false;
        }
        if m.to == 63 {
            self.castling.black_kingside = false;
        }
        if m.to == 56 {
            self.castling.black_queenside = false;
        }

        let row_diff = self.row(m.to) as i8 - self.row(m.from) as i8;
        self.en_passant = if moving.kind == PieceType::Pawn
            && (row_diff == 2 || row_diff == -2)
        {
            Some(if row_diff == 2 { m.from + 8 } else { m.from - 8 })
        } else {
            None
        };

        self.turn = mover.opposite();
    }

    /// Charge une position depuis une chaîne FEN (ex: `rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1`).
    pub fn from_fen(fen: &str) -> Result<Board, String> {
        let parts: Vec<&str> = fen.split_whitespace().collect();
        if parts.len() != 6 {
            return Err(format!("FEN invalide: attendu 6 champs, trouvé {}", parts.len()));
        }

        let mut board = Board::empty();

        // 1. Placement des pièces (rang 8 d'abord → rang 1 à la fin).
        let rows: Vec<&str> = parts[0].split('/').collect();
        if rows.len() != 8 {
            return Err("FEN invalide: le placement doit contenir 8 rangs séparés par '/'".to_string());
        }
        for (ri, row) in rows.iter().enumerate() {
            let board_row = 7 - ri as u8;
            let mut col: u8 = 0;
            for ch in row.chars() {
                if ch.is_ascii_digit() {
                    col += ch.to_digit(10).unwrap() as u8;
                    if col > 8 {
                        return Err("FEN invalide: trop de cases dans un rang".to_string());
                    }
                } else {
                    let piece = match ch {
                        'P' => Some(Piece { color: Color::White, kind: PieceType::Pawn }),
                        'N' => Some(Piece { color: Color::White, kind: PieceType::Knight }),
                        'B' => Some(Piece { color: Color::White, kind: PieceType::Bishop }),
                        'R' => Some(Piece { color: Color::White, kind: PieceType::Rook }),
                        'Q' => Some(Piece { color: Color::White, kind: PieceType::Queen }),
                        'K' => Some(Piece { color: Color::White, kind: PieceType::King }),
                        'p' => Some(Piece { color: Color::Black, kind: PieceType::Pawn }),
                        'n' => Some(Piece { color: Color::Black, kind: PieceType::Knight }),
                        'b' => Some(Piece { color: Color::Black, kind: PieceType::Bishop }),
                        'r' => Some(Piece { color: Color::Black, kind: PieceType::Rook }),
                        'q' => Some(Piece { color: Color::Black, kind: PieceType::Queen }),
                        'k' => Some(Piece { color: Color::Black, kind: PieceType::King }),
                        _ => return Err(format!("FEN invalide: caractère pièce inconnu '{}'", ch)),
                    };
                    let sq = board_row * 8 + col;
                    board.set(sq, piece);
                    col += 1;
                }
            }
            if col != 8 {
                return Err(format!("FEN invalide: le rang {} ne contient pas 8 cases", 8 - ri));
            }
        }

        // 2. Couleur à jouer.
        board.turn = match parts[1] {
            "w" => Color::White,
            "b" => Color::Black,
            _ => return Err("FEN invalide: le champ à jouer doit être 'w' ou 'b'".to_string()),
        };

        // 3. Droits de roque.
        let cr = parts[2];
        board.castling.white_kingside = cr.contains('K');
        board.castling.white_queenside = cr.contains('Q');
        board.castling.black_kingside = cr.contains('k');
        board.castling.black_queenside = cr.contains('q');

        // 4. En passant.
        board.en_passant = if parts[3] == "-" {
            None
        } else {
            match alg_to_square(parts[3]) {
                Some(sq) => Some(sq),
                None => return Err(format!("FEN invalide: case en passant inconnue '{}'", parts[3])),
            }
        };

        // Les champs halfmove (5) et fullmove (6) sont ignorés (Board ne les stocke pas).
        let _ = (parts[4], parts[5]);

        Ok(board)
    }

    /// Exporte la position courante au format FEN.
    pub fn to_fen(&self) -> String {
        let mut fen = String::new();
        for row in (0..8).rev() {
            if row < 7 {
                fen.push('/');
            }
            let mut empty = 0u8;
            for col in 0..8u8 {
                let sq = row * 8 + col;
                match self.squares[sq as usize] {
                    None => empty += 1,
                    Some(p) => {
                        if empty > 0 {
                            fen.push(char::from_digit(empty as u32, 10).unwrap());
                            empty = 0;
                        }
                        fen.push(piece_char(p));
                    }
                }
            }
            if empty > 0 {
                fen.push(char::from_digit(empty as u32, 10).unwrap());
            }
        }

        fen.push(' ');
        fen.push(if self.turn == Color::White { 'w' } else { 'b' });
        fen.push(' ');

        let mut castling = String::new();
        if self.castling.white_kingside { castling.push('K'); }
        if self.castling.white_queenside { castling.push('Q'); }
        if self.castling.black_kingside { castling.push('k'); }
        if self.castling.black_queenside { castling.push('q'); }
        if castling.is_empty() { castling.push('-'); }
        fen.push_str(&castling);
        fen.push(' ');

        match self.en_passant {
            Some(sq) => fen.push_str(&square_to_alg(sq)),
            None => fen.push('-'),
        }

        fen.push_str(" 0 1");
        fen
    }
}

impl Default for Board {
    fn default() -> Self {
        Self::empty()
    }
}

/// Notation algébrique (ex: `e4`, `a1`) <-> index de case (0-64).
pub fn square_to_alg(sq: u8) -> String {
    format!("{}{}", char::from(b'a' + sq % 8), sq / 8 + 1)
}

pub fn alg_to_square(s: &str) -> Option<u8> {
    let bytes = s.as_bytes();
    if bytes.len() != 2 {
        return None;
    }
    let file = bytes[0] as i8 - b'a' as i8;
    let rank = bytes[1] as i8 - b'1' as i8;
    if file < 0 || file > 7 || rank < 0 || rank > 7 {
        return None;
    }
    Some(rank as u8 * 8 + file as u8)
}

fn piece_char(p: Piece) -> char {
    let base = match p.kind {
        PieceType::Pawn => 'P',
        PieceType::Knight => 'N',
        PieceType::Bishop => 'B',
        PieceType::Rook => 'R',
        PieceType::Queen => 'Q',
        PieceType::King => 'K',
    };
    if p.color == Color::White {
        base
    } else {
        base.to_ascii_lowercase()
    }
}

impl std::fmt::Display for Board {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for row in (0..8).rev() {
            let mut line = String::new();
            line.push_str(&format!("{:>2}  ", row + 1));
            for col in 0..8u8 {
                let sq = row * 8 + col;
                let ch = match self.squares[sq as usize] {
                    None => '.',
                    Some(p) => piece_char(p),
                };
                line.push(ch);
                line.push(' ');
            }
            writeln!(f, "{}", line.trim_end())?;
        }
        Ok(())
    }
}
