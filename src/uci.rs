use crate::board::{Board, Color, Move};
use crate::move_gen::legal_moves;
use crate::search::Searcher;
use crate::{alg_to_square, square_to_alg};
use std::io::{self, BufRead, Write};

/// Protocole UCI (Universal Chess Interface).
/// Un bot communique avec le moteur via stdin/stdout :
///   `position ...` puis `go` → le moteur répond `bestmove <coup>`
pub struct Uci {
    board: Board,
    searcher: Searcher,
    elo: u32,
}

impl Uci {
    pub fn new() -> Self {
        Uci {
            board: Board::new(),
            searcher: Searcher::new(),
            elo: 1500,
        }
    }

    pub fn run(&mut self) {
        let stdin = io::stdin();
        let stdout = io::stdout();
        let mut out = stdout.lock();

        // Envoi de l'identification UCI au démarrage.
        self.send(&mut out, "id name OpenSnake");
        self.send(&mut out, "id author OpenSnake");
        self.send(&mut out, "uciok");

        for line in stdin.lock().lines() {
            let line = match line {
                Ok(l) => l,
                Err(_) => break,
            };
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            match self.handle_line(line) {
                UciAction::Quit => break,
                UciAction::None => {}
            }
        }
    }

    fn send(&self, out: &mut io::StdoutLock, text: &str) {
        let _ = writeln!(out, "{}", text);
        let _ = out.flush();
    }

    fn handle_line(&mut self, line: &str) -> UciAction {
        let mut parts = line.split_whitespace();
        let cmd = match parts.next() {
            Some(c) => c,
            None => return UciAction::None,
        };

        match cmd {
            "uci" => {
                let _ = std::io::stdout();
                // Déjà envoyé au démarrage, mais on renvoie uciok.
                println!("id name OpenSnake");
                println!("id author OpenSnake");
                println!("uciok");
            }
            "debug" => {
                // Ignoré (pas de debug détaillé).
            }
            "isready" => {
                println!("readyok");
            }
            "setoption" => {
                self.cmd_setoption(parts);
            }
            "ucinewgame" => {
                self.board = Board::new();
                self.searcher = Searcher::new();
            }
            "position" => {
                self.cmd_position(parts);
            }
            "go" => {
                self.cmd_go(parts);
            }
            "stop" => {
                self.searcher.stop = true;
            }
            "ponderhit" => {
                // Non supporté pour l'instant.
            }
            "quit" => {
                return UciAction::Quit;
            }
            _ => {
                // Commande inconnue, ignorée.
            }
        }
        UciAction::None
    }

    fn cmd_position(&mut self, parts: std::str::SplitWhitespace) {
        let mut parts = parts.peekable();
        let first = match parts.next() {
            Some(f) => f,
            None => return,
        };

        match first {
            "startpos" => {
                self.board = Board::new();
                self.cmd_moves(&mut parts);
            }
            "fen" => {
                let fen_parts: Vec<String> = parts.by_ref().take(6).map(|s| s.to_string()).collect();
                if fen_parts.len() < 6 {
                    return;
                }
                let fen = fen_parts.join(" ");
                match Board::from_fen(&fen) {
                    Ok(b) => self.board = b,
                    Err(_) => return,
                }
                self.cmd_moves(&mut parts);
            }
            _ => {}
        }
    }

    /// Parse la liste de coups après `startpos` / `fen ... moves`.
    fn cmd_moves(&mut self, parts: &mut std::iter::Peekable<std::str::SplitWhitespace>) {
        // On consomme le mot-clé "moves" s'il est présent.
        if parts.peek().map(|s| *s == "moves").unwrap_or(false) {
            parts.next(); // consomme "moves"
        }

        while let Some(mv_str) = parts.next() {
            if mv_str.len() < 4 {
                continue;
            }
            let from_sq = match alg_to_square(&mv_str[0..2]) {
                Some(s) => s,
                None => continue,
            };
            let to_sq = match alg_to_square(&mv_str[2..4]) {
                Some(s) => s,
                None => continue,
            };
            let promotion = if mv_str.len() >= 5 {
                match mv_str.chars().nth(4) {
                    Some('q') => Some(crate::board::PieceType::Queen),
                    Some('r') => Some(crate::board::PieceType::Rook),
                    Some('b') => Some(crate::board::PieceType::Bishop),
                    Some('n') => Some(crate::board::PieceType::Knight),
                    _ => None,
                }
            } else {
                None
            };

            let mv = Move::new(from_sq, to_sq, promotion);
            let legal = legal_moves(&self.board);
            if legal.contains(&mv) {
                self.board.make_move(&mv);
            }
        }
    }

    /// Gère les options UCI (ex: `setoption name Elo value 1500`).
    fn cmd_setoption(&mut self, parts: std::str::SplitWhitespace) {
        let parts: Vec<&str> = parts.collect();
        let mut i = 0;
        while i + 2 < parts.len() {
            if parts[i] == "name" && parts[i + 2] == "value" {
                let name = parts[i + 1];
                let value = parts[i + 3];
                match name {
                    "Elo" => {
                        if let Ok(v) = value.parse::<u32>() {
                            self.elo = v.clamp(1, 9999);
                            self.searcher.elo = self.elo;
                        }
                    }
                    _ => {}
                }
                i += 4;
            } else {
                i += 1;
            }
        }
    }

    fn cmd_go(&mut self, parts: std::str::SplitWhitespace) {
        let parts: Vec<&str> = parts.collect();

        let mut depth: Option<u32> = None; // None = profondeur déterminée par l'ELO
        let mut time_remaining: Option<u64> = None; // wtime ou btime selon le côté à jouer
        let mut increment: u64 = 0;
        let mut movetime: Option<u64> = None;

        let mut i = 0;
        while i + 1 < parts.len() || i < parts.len() {
            match parts[i] {
                "depth" => {
                    if let Some(d) = parts.get(i + 1).and_then(|s| s.parse().ok()) {
                        depth = Some(d);
                    }
                    i += 2;
                }
                "wtime" => {
                    if self.board.turn == Color::White {
                        if let Some(t) = parts.get(i + 1).and_then(|s| s.parse().ok()) {
                            time_remaining = Some(t);
                        }
                    }
                    i += 2;
                }
                "btime" => {
                    if self.board.turn == Color::Black {
                        if let Some(t) = parts.get(i + 1).and_then(|s| s.parse().ok()) {
                            time_remaining = Some(t);
                        }
                    }
                    i += 2;
                }
                "winc" => {
                    if self.board.turn == Color::White {
                        increment = parts.get(i + 1).and_then(|s| s.parse().ok()).unwrap_or(0);
                    }
                    i += 2;
                }
                "binc" => {
                    if self.board.turn == Color::Black {
                        increment = parts.get(i + 1).and_then(|s| s.parse().ok()).unwrap_or(0);
                    }
                    i += 2;
                }
                "movetime" => {
                    if let Some(t) = parts.get(i + 1).and_then(|s| s.parse().ok()) {
                        movetime = Some(t);
                    }
                    i += 2;
                }
                _ => {
                    i += 1;
                }
            }
        }

        // --- Gestion intelligente du temps UCI ---
        // Priorité 1 : movetime (forcé par la GUI, on doit s'y tenir)
        // Priorité 2 : calcul basé sur wtime/btime + incrément
        let limit = if let Some(mt) = movetime {
            // Marge de sécurité : on ne consomme pas 100% du movetime pour éviter le timeout
            // à cause du délai d'envoi/réception (network latency, flush stdout, etc.).
            let safety_margin_ms = 50u64;
            if mt > safety_margin_ms {
                Some(mt - safety_margin_ms)
            } else {
                Some(mt)
            }
        } else if let Some(tr) = time_remaining {
            // Nombre de coups estimés restants avant le prochain contrôle de temps.
            // 30 est une valeur standard pour les parties rapides/blitz.
            let moves_to_go: u64 = 30;

            // Temps de base : on répartit le temps restant sur moves_to_go coups.
            let base_time = tr / moves_to_go;

            // On ajoute une partie de l'incrément (on ne compte pas sur 100% de l'incrément
            // car on pourrait ne pas en avoir besoin sur tous les coups).
            let increment_portion = increment / 2;

            let mut time_for_move = base_time.saturating_add(increment_portion);

            // Plafond : on n'utilise jamais plus de 40% du temps restant sur un seul coup.
            let max_time = tr * 40 / 100;
            if time_for_move > max_time {
                time_for_move = max_time;
            }

            // Plancher minimal : même en Zeitnot, on réfléchit un minimum.
            let min_time = 10u64; // 10 ms minimum
            if time_for_move < min_time {
                time_for_move = min_time;
            }

            // Marge de sécurité pour ne pas dépasser le hard limit (perdre au temps).
            // On garde 5% de temps de marge, ou au moins 100ms.
            let safety_margin = (time_for_move * 5 / 100).max(100);
            if time_for_move > safety_margin {
                Some(time_for_move - safety_margin)
            } else {
                Some(time_for_move)
            }
        } else {
            // Aucun contrôle de temps : pas de limite.
            None
        };

        // On applique l'ELO configuré au searcher.
        self.searcher.elo = self.elo;

        // Si `go depth X` est spécifié, on force cette profondeur (surcharge l'ELO).
        if let Some(d) = depth {
            self.searcher.forced_depth = Some(d);
        } else {
            self.searcher.forced_depth = None;
        }

        let result = self.searcher.search(&self.board, limit);

        match result.best_move {
            Some(mv) => {
                let from = square_to_alg(mv.from);
                let to = square_to_alg(mv.to);
                let promo = match mv.promotion {
                    Some(crate::board::PieceType::Queen) => "q",
                    Some(crate::board::PieceType::Rook) => "r",
                    Some(crate::board::PieceType::Bishop) => "b",
                    Some(crate::board::PieceType::Knight) => "n",
                    _ => "",
                };
                println!("bestmove {}{}{}", from, to, promo);
            }
            None => {
                println!("bestmove 0000");
            }
        }
    }
}

enum UciAction {
    None,
    Quit,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uci_position_startpos_creates_initial_board() {
        let mut uci = Uci::new();
        uci.cmd_position("startpos".split_whitespace());
        assert_eq!(uci.board.to_fen(), "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1");
    }

    #[test]
    fn uci_position_with_moves_plays_them() {
        let mut uci = Uci::new();
        uci.cmd_position("startpos moves e2e4 e7e5".split_whitespace());
        let fen = uci.board.to_fen();
        assert!(fen.contains("4P3"), "FEN après e4/e5: {}", fen);
        assert!(fen.contains(" w "), "C'est aux Blancs après e4/e5: {}", fen);
    }

    #[test]
    fn uci_position_from_fen() {
        let mut uci = Uci::new();
        uci.cmd_position(
            "fen r3k2r/p1ppqpb1/bnp2p1p/3N4/1B6/P4N2/1PPPQPPP/R3K2R w KQkq - 0 1".split_whitespace()
        );
        assert_eq!(
            uci.board.to_fen(),
            "r3k2r/p1ppqpb1/bnp2p1p/3N4/1B6/P4N2/1PPPQPPP/R3K2R w KQkq - 0 1"
        );
    }
}
