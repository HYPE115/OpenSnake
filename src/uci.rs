use crate::board::{Board, Color, Move};
use crate::move_gen::legal_moves;
use crate::search::{SearchResult, Searcher};
use crate::{alg_to_square, square_to_alg};
use std::io::{self, BufRead, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const INFINITE_DEPTH: u32 = 64;

/// Protocole UCI (Universal Chess Interface).
/// Cute Chess pilote le moteur via stdin/stdout : `position ...`, `go ...`, puis `bestmove`.
pub struct Uci {
    board: Board,
    searcher: Searcher,
    elo: u32,
    hash_mb: usize,
    multipv: usize,
    analyse_mode: bool,
    limit_strength: bool,
    /// Délai minimum avant de jouer (ms). 0 = calculé depuis l'ELO.
    move_delay_ms: u64,
    /// Variation aléatoire ajoutée au délai (plage de temps). 0 = fixe.
    move_delay_jitter_ms: u64,
    /// Recherche en cours, pour pouvoir répondre à `stop` / `isready` / `quit`.
    running: Option<RunningSearch>,
}

struct RunningSearch {
    result_rx: mpsc::Receiver<SearchResult>,
    abort: Arc<AtomicBool>,
    multipv: usize,
}

fn move_to_uci(m: &Move) -> String {
    let promo = match m.promotion {
        Some(crate::board::PieceType::Queen) => "q",
        Some(crate::board::PieceType::Rook) => "r",
        Some(crate::board::PieceType::Bishop) => "b",
        Some(crate::board::PieceType::Knight) => "n",
        _ => "",
    };
    format!("{}{}{}", square_to_alg(m.from), square_to_alg(m.to), promo)
}

fn parse_bool(value: &str) -> bool {
    matches!(
        value.trim().to_lowercase().as_str(),
        "true" | "1" | "yes" | "on"
    )
}

/// Délai de réflexion minimal (ms) déduit de l'ELO.
/// Un joueur faible met plus de temps qu'un fort joueur :
/// 200 ELO ≈ 2,3 s, 1500 ≈ 1 s, 2800 ≈ 0,6 s, 9999 ≈ 0,2 s.
fn elo_delay_ms(elo: u32) -> u64 {
    let ms = 2_100_000u64 / (700u64 + elo as u64);
    ms.clamp(100, 5_000)
}

/// Petit aléa dans `0..=max` pour une plage de temps, basé sur l'horloge.
fn jitter_ms(max: u64) -> u64 {
    if max == 0 {
        return 0;
    }
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos() as u64;
    nanos % (max + 1)
}

/// Calcule le temps de réflexion à partir des champs `go` envoyés par le GUI.
fn allocate_time_ms(
    movetime: Option<u64>,
    remaining: Option<u64>,
    increment: u64,
    moves_to_go: u64,
) -> Option<u64> {
    if let Some(mt) = movetime {
        // Marge de sécurité : on ne consomme pas 100% du movetime.
        return Some(if mt > 50 { mt - 50 } else { mt });
    }

    let tr = remaining?;
    let moves_to_go = moves_to_go.clamp(1, 60);
    let base_time = tr / moves_to_go;
    let increment_portion = increment / 2;
    let mut time_for_move = base_time.saturating_add(increment_portion);

    // Plancher : même en Zeitnot, on réfléchit un minimum.
    if time_for_move < 10 {
        time_for_move = 10;
    }
    // Plafond : jamais plus de 40% du temps restant sur un seul coup.
    let max_time = tr * 40 / 100;
    if time_for_move > max_time {
        time_for_move = max_time;
    }

    // Marge de sécurité : 5% du temps alloué, au moins 100 ms.
    let safety_margin = (time_for_move * 5 / 100).max(100);
    Some(if time_for_move > safety_margin {
        time_for_move - safety_margin
    } else {
        time_for_move
    })
}

impl Uci {
    pub fn new() -> Self {
        Uci {
            board: Board::new(),
            searcher: Searcher::new(),
            elo: 1500,
            hash_mb: 64,
            multipv: 1,
            analyse_mode: false,
            limit_strength: true,
            move_delay_ms: 0,
            move_delay_jitter_ms: 0,
            running: None,
        }
    }

    pub fn run(&mut self) {
        // Un thread lit stdin en continu : le moteur doit pouvoir traiter
        // `isready`, `stop` et `quit` pendant qu'il cherche (exigence UCI).
        let (line_tx, line_rx) = mpsc::channel::<String>();
        thread::spawn(move || {
            let stdin = io::stdin();
            for line in stdin.lock().lines() {
                match line {
                    Ok(l) => {
                        if line_tx.send(l).is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        });

        let stdout = io::stdout();
        let mut out = stdout.lock();

        // UCI : on attend la commande `uci` avant de s'identifier.
        loop {
            if self.running.is_some() {
                // Une recherche est en cours : on surveille son résultat et les commandes.
                let result = {
                    let task = self.running.as_ref().expect("running search");
                    task.result_rx.recv_timeout(Duration::from_millis(20))
                };
                match result {
                    Ok(result) => {
                        self.finish_search(result, &mut out);
                        continue;
                    }
                    Err(mpsc::RecvTimeoutError::Disconnected) => {
                        self.running = None;
                    }
                    Err(mpsc::RecvTimeoutError::Timeout) => {}
                }

                match line_rx.recv_timeout(Duration::from_millis(20)) {
                    Ok(line) => {
                        if let UciAction::Quit = self.handle_line(line.trim(), &mut out) {
                            break;
                        }
                    }
                    Err(mpsc::RecvTimeoutError::Timeout) => continue,
                    Err(mpsc::RecvTimeoutError::Disconnected) => break,
                }
            } else {
                match line_rx.recv() {
                    Ok(line) => {
                        if let UciAction::Quit = self.handle_line(line.trim(), &mut out) {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        }
    }

    fn send(&self, out: &mut io::StdoutLock, text: &str) {
        let _ = writeln!(out, "{}", text);
        let _ = out.flush();
    }

    fn send_identification(&self, out: &mut io::StdoutLock) {
        self.send(out, "id name OpenSnake");
        self.send(out, "id author OpenSnake");
        self.send(out, "option name UCI_Elo type spin default 1500 min 1 max 9999");
        self.send(
            out,
            "option name UCI_LimitStrength type check default true",
        );
        self.send(out, "option name Hash type spin default 64 min 1 max 1024");
        self.send(out, "option name MultiPV type spin default 1 min 1 max 10");
        self.send(out, "option name UCI_AnalyseMode type check default false");
        self.send(out, "option name MoveDelay type spin default 0 min 0 max 60000");
        self.send(out, "option name MoveDelayJitter type spin default 0 min 0 max 10000");
        self.send(out, "uciok");
    }

    fn abort_search(&self) {
        if let Some(task) = self.running.as_ref() {
            task.abort.store(true, Ordering::Relaxed);
        }
    }

    fn finish_search(&mut self, result: SearchResult, out: &mut io::StdoutLock) {
        let multipv = self
            .running
            .as_ref()
            .map(|task| task.multipv)
            .unwrap_or(1)
            .max(1);
        self.running = None;

        let lines: Vec<(Move, f64)> = if result.root_scored.is_empty() {
            result
                .best_move
                .map(|mv| vec![(mv, result.best_score)])
                .unwrap_or_default()
        } else {
            result.root_scored.clone()
        };

        let nps = if result.time_ms > 0 {
            result.nodes * 1000 / result.time_ms as u64
        } else {
            0
        };
        for (rank, (mv, score)) in lines.iter().take(multipv).enumerate() {
            self.send(
                out,
                &format!(
                    "info depth {} multipv {} score cp {} nodes {} nps {} time {} pv {}",
                    result.depth,
                    rank + 1,
                    score.round() as i32,
                    result.nodes,
                    nps,
                    result.time_ms,
                    move_to_uci(mv)
                ),
            );
        }

        match result.best_move {
            Some(mv) => self.send(out, &format!("bestmove {}", move_to_uci(&mv))),
            None => self.send(out, "bestmove 0000"),
        }
    }

    fn handle_line(&mut self, line: &str, out: &mut io::StdoutLock) -> UciAction {
        let mut parts = line.split_whitespace();
        let cmd = match parts.next() {
            Some(c) => c,
            None => return UciAction::None,
        };

        match cmd {
            "uci" => self.send_identification(out),
            "debug" => {
                // `debug on/off` : rien à afficher pour l'instant.
            }
            "isready" => {
                // Réponse immédiate, même pendant une recherche.
                self.send(out, "readyok");
            }
            "setoption" => self.cmd_setoption(parts),
            "ucinewgame" => {
                self.abort_search();
                self.board = Board::new();
                self.searcher = Searcher::new();
                self.searcher.elo = self.elo;
                self.searcher.set_hash_mb(self.hash_mb);
            }
            "position" => {
                self.abort_search();
                self.cmd_position(parts);
            }
            "go" => {
                self.abort_search();
                self.cmd_go(parts, out);
            }
            "stop" => self.abort_search(),
            "ponderhit" => {
                // Ponder non annoncé : on ignore sans casser la recherche en cours.
            }
            "quit" => {
                self.abort_search();
                return UciAction::Quit;
            }
            _ => {
                // Commande inconnue, ignorée (comme le demande la spec UCI).
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
        if parts.peek().map(|s| *s == "moves").unwrap_or(false) {
            parts.next();
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
            if legal_moves(&self.board).contains(&mv) {
                self.board.make_move(&mv);
            }
        }
    }

    /// Gère les options UCI (ex: `setoption name UCI_Elo value 1500`).
    fn cmd_setoption(&mut self, parts: std::str::SplitWhitespace) {
        let parts: Vec<&str> = parts.collect();
        let Some(name_at) = parts.iter().position(|p| *p == "name") else {
            return;
        };
        let Some(value_at) = parts[name_at + 1..]
            .iter()
            .position(|p| *p == "value")
            .map(|offset| name_at + 1 + offset)
        else {
            return;
        };
        let name = parts[name_at + 1..value_at].join(" ").to_lowercase();
        let value = parts.get(value_at + 1).copied().unwrap_or("");

        match name.as_str() {
            "elo" | "uci_elo" => {
                if let Ok(v) = value.parse::<u32>() {
                    self.elo = v.clamp(1, 9999);
                    self.searcher.elo = self.elo;
                }
            }
            "uci_limitstrength" => self.limit_strength = parse_bool(value),
            "hash" => {
                if let Ok(v) = value.parse::<usize>() {
                    self.hash_mb = v.clamp(1, 1024);
                    self.searcher.set_hash_mb(self.hash_mb);
                }
            }
            "multipv" => {
                if let Ok(v) = value.parse::<usize>() {
                    self.multipv = v.clamp(1, 10);
                }
            }
            "uci_analysemode" => self.analyse_mode = parse_bool(value),
            "movedelay" | "delay" | "thinktime" => {
                if let Ok(v) = value.parse::<u64>() {
                    self.move_delay_ms = v.min(60_000);
                }
            }
            "movedelayjitter" | "delayjitter" | "thinktimejitter" => {
                if let Ok(v) = value.parse::<u64>() {
                    self.move_delay_jitter_ms = v.min(10_000);
                }
            }
            // Accepté pour les GUI qui l'envoient, mais la recherche reste mono-thread.
            "threads" => {
                let _ = value;
            }
            _ => {}
        }
    }

    fn cmd_go(&mut self, parts: std::str::SplitWhitespace, out: &mut io::StdoutLock) {
        let parts: Vec<&str> = parts.collect();

        let mut depth: Option<u32> = None;
        let mut multipv = self.multipv;
        let mut time_remaining: Option<u64> = None;
        let mut increment: u64 = 0;
        let mut movetime: Option<u64> = None;
        let mut moves_to_go: u64 = 30;
        let mut infinite = false;

        let mut i = 0;
        while i < parts.len() {
            match parts[i] {
                "depth" => {
                    if let Some(d) = parts.get(i + 1).and_then(|s| s.parse().ok()) {
                        depth = Some(d);
                    }
                    i += 2;
                }
                "wtime" => {
                    if self.board.turn == Color::White {
                        time_remaining = parts.get(i + 1).and_then(|s| s.parse().ok());
                    }
                    i += 2;
                }
                "btime" => {
                    if self.board.turn == Color::Black {
                        time_remaining = parts.get(i + 1).and_then(|s| s.parse().ok());
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
                    movetime = parts.get(i + 1).and_then(|s| s.parse().ok());
                    i += 2;
                }
                "movestogo" => {
                    moves_to_go = parts.get(i + 1).and_then(|s| s.parse().ok()).unwrap_or(30);
                    i += 2;
                }
                "multipv" => {
                    if let Some(n) = parts.get(i + 1).and_then(|s| s.parse::<usize>().ok()) {
                        multipv = n.clamp(1, 10);
                    }
                    i += 2;
                }
                "infinite" => {
                    infinite = true;
                    i += 1;
                }
                // `nodes`, `mate`, `searchmoves`, `ponder` : ignorés sans casser le parser.
                _ => i += 1,
            }
        }

        if legal_moves(&self.board).is_empty() {
            self.send(out, "bestmove 0000");
            return;
        }

        let mut searcher = self.searcher.clone();
        searcher.stop = false;
        searcher.multi_pv = multipv;

        if self.analyse_mode || infinite {
            searcher.elo = 9999;
        } else if self.limit_strength {
            searcher.elo = self.elo;
        } else {
            searcher.elo = 9999;
        }

        searcher.forced_depth = if infinite {
            Some(INFINITE_DEPTH)
        } else {
            depth
        };

        // Délai de réflexion : dérivé de l'ELO par défaut, ou fixé par MoveDelay
        // (0 = auto). MoveDelayJitter ajoute une plage aléatoire au délai.
        let base_delay = if self.move_delay_ms == 0 {
            elo_delay_ms(searcher.elo)
        } else {
            self.move_delay_ms
        };
        let delay = base_delay.saturating_add(jitter_ms(self.move_delay_jitter_ms));

        let (limit, min_wait) = if infinite || self.analyse_mode {
            // Analyse / `go infinite` : on attend la commande `stop`, pas de délai.
            (None, None)
        } else if movetime.is_some() {
            // `movetime` explicite : on ne dépasse jamais le budget demandé.
            let budget = allocate_time_ms(movetime, None, 0, moves_to_go).unwrap_or(0);
            (Some(budget), Some(delay.min(budget)))
        } else if let Some(remaining) = time_remaining {
            let allocated =
                allocate_time_ms(None, Some(remaining), increment, moves_to_go).unwrap_or(0);
            // On peut prendre plus de temps que le budget serré, mais jamais plus de
            // 40 % du temps restant pour éviter de perdre au chrono.
            let cap = remaining.saturating_mul(40) / 100;
            let wait = delay.min(cap);
            (Some(allocated.max(wait)), Some(wait))
        } else {
            // `go depth X` sans temps : on applique juste le délai.
            (None, Some(delay))
        };
        searcher.min_time_ms = min_wait;

        let abort = Arc::new(AtomicBool::new(false));
        searcher.abort = Some(Arc::clone(&abort));

        let board = self.board;
        let (result_tx, result_rx) = mpsc::channel::<SearchResult>();
        thread::spawn(move || {
            let result = searcher.search(&board, limit);
            let _ = result_tx.send(result);
        });

        self.running = Some(RunningSearch {
            result_rx,
            abort,
            multipv,
        });
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
    fn uci_setoptions_are_parsed() {
        let mut uci = Uci::new();
        uci.cmd_setoption("name Elo value 2100".split_whitespace());
        uci.cmd_setoption("name Hash value 32".split_whitespace());
        uci.cmd_setoption("name MultiPV value 3".split_whitespace());
        uci.cmd_setoption("name UCI_AnalyseMode value true".split_whitespace());
        uci.cmd_setoption("name UCI_LimitStrength value false".split_whitespace());
        uci.cmd_setoption("name MoveDelay value 2500".split_whitespace());
        uci.cmd_setoption("name MoveDelayJitter value 500".split_whitespace());

        assert_eq!(uci.elo, 2100);
        assert_eq!(uci.hash_mb, 32);
        assert_eq!(uci.multipv, 3);
        assert!(uci.analyse_mode);
        assert!(!uci.limit_strength);
        assert_eq!(uci.move_delay_ms, 2500);
        assert_eq!(uci.move_delay_jitter_ms, 500);
    }

    #[test]
    fn uci_elo_delay_is_slower_for_weak_players() {
        assert_eq!(elo_delay_ms(200), 2333);
        assert!(elo_delay_ms(200) > elo_delay_ms(1500));
        assert!(elo_delay_ms(1500) > elo_delay_ms(2800));
        assert!(elo_delay_ms(2800) > elo_delay_ms(9999));
        assert!(elo_delay_ms(1) <= 5_000);
        assert!(elo_delay_ms(9999) >= 100);
        assert_eq!(jitter_ms(0), 0);
        assert!(jitter_ms(500) <= 500);
    }

    #[test]
    fn uci_elo_delay_is_monotonic_over_full_range() {
        let mut last = u64::MAX;
        for elo in 1..=9999u32 {
            let d = elo_delay_ms(elo);
            assert!((100..=5_000).contains(&d), "délai hors bornes à {}: {}", elo, d);
            assert!(d <= last, "le délai remonte à {}", elo);
            last = d;
        }
    }

    #[test]
    fn uci_elo_option_is_clamped() {
        let mut uci = Uci::new();
        uci.cmd_setoption("name UCI_Elo value 0".split_whitespace());
        assert_eq!(uci.elo, 1);
        uci.cmd_setoption("name UCI_Elo value 10000".split_whitespace());
        assert_eq!(uci.elo, 9999);
        uci.cmd_setoption("name UCI_Elo value nope".split_whitespace());
        assert_eq!(uci.elo, 9999);
    }

    #[test]
    fn uci_accepts_every_elo_from_1_to_9999() {
        let mut uci = Uci::new();
        for elo in 1..=9999u32 {
            let cmd = format!("name UCI_Elo value {}", elo);
            uci.cmd_setoption(cmd.split_whitespace());
            assert_eq!(uci.elo, elo, "UCI_Elo ignoré à {}", elo);
            assert_eq!(
                uci.searcher.elo, elo,
                "la profondeur/erreur n'a pas suivi UCI_Elo à {}",
                elo
            );
        }
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

    #[test]
    fn uci_time_allocation_uses_movetime_and_movestogo() {
        assert_eq!(allocate_time_ms(Some(1000), None, 0, 30), Some(950));
        let t = allocate_time_ms(None, Some(30_000), 1_000, 10).unwrap();
        assert!(t > 0 && t < 30_000 / 10 + 500);
        assert_eq!(allocate_time_ms(None, None, 0, 30), None);
    }
}
