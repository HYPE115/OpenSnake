use crate::board::{Board, Color, Move, PieceType};
use crate::engine::evaluate;
use crate::move_gen::{ordered_moves, is_capture, is_promotion};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

const MATE_SCORE: f64 = 20000.0;
const TT_SIZE: usize = 1_000_000; // 1 million d'entrées max

// Marge de futility pruning par profondeur (centipawns).
fn futility_margin(depth: u32) -> f64 {
    (depth as f64) * 80.0
}

/// Type de borne stocké dans la Transposition Table.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum BoundType {
    Exact,    // Le score est exact
    Lower,    // Le score est >= valeur (alpha cut)
    Upper,    // Le score est <= valeur (beta cut)
}

/// Entrée de la Transposition Table.
#[derive(Clone, Copy, Debug)]
struct TTEntry {
    depth: u32,
    score: f64,
    bound: BoundType,
    best_move: Option<Move>,
}

/// Table de transposition : cache les positions déjà évaluées.
/// Évite de recalculer les mêmes nœuds → accélération x10 à x100.
struct TranspositionTable {
    table: HashMap<u64, TTEntry>,
    probes: u64,
    hits: u64,
}

impl TranspositionTable {
    fn new() -> Self {
        TranspositionTable {
            table: HashMap::with_capacity(TT_SIZE),
            probes: 0,
            hits: 0,
        }
    }

    /// Sonde la table pour une position donnée.
    /// Retourne (score, meilleur_coup, type_de_borne) si l'entrée est assez profonde.
    fn probe(&mut self, key: u64, depth: u32, alpha: f64, beta: f64) -> Option<(f64, Option<Move>, BoundType)> {
        self.probes += 1;
        self.table.get(&key).and_then(|entry| {
            if entry.depth < depth {
                return None; // Pas assez profond pour être utilisé
            }
            let usable = match entry.bound {
                BoundType::Exact => true,
                BoundType::Lower => entry.score >= beta,
                BoundType::Upper => entry.score <= alpha,
            };
            if usable {
                self.hits += 1;
                Some((entry.score, entry.best_move, entry.bound))
            } else {
                None
            }
        })
    }

    fn clear(&mut self) {
        self.table.clear();
        self.probes = 0;
        self.hits = 0;
    }

    /// Enregistre un résultat dans la table.
    fn record(&mut self, key: u64, depth: u32, score: f64, bound: BoundType, best_move: Option<Move>) {
        // Si la table est pleine, on nettoie les entrées peu profondes
        if self.table.len() >= TT_SIZE {
            let keys_to_remove: Vec<u64> = self.table
                .iter()
                .filter(|(_, v)| v.depth < 2)
                .map(|(k, _)| *k)
                .take(TT_SIZE / 10)
                .collect();
            for k in keys_to_remove {
                self.table.remove(&k);
            }
        }
        self.table.insert(key, TTEntry { depth, score, bound, best_move });
    }
}

/// Génère une clé Zobrist pour une position (hash 64 bits).
fn zobrist_key(board: &Board) -> u64 {
    let mut seed: u64 = 0x9E3779B97F4A7C15;
    let mut hash: u64 = 0;

    // Mélange les pièces sur chaque case.
    for sq in 0..64u8 {
        if let Some(p) = board.get(sq) {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            let piece_index = match p.kind {
                PieceType::Pawn => 0,
                PieceType::Knight => 1,
                PieceType::Bishop => 2,
                PieceType::Rook => 3,
                PieceType::Queen => 4,
                PieceType::King => 5,
            } + if p.color == Color::Black { 6 } else { 0 };
            hash ^= seed.wrapping_add((piece_index as u64) * 0x100000001b3 + sq as u64);
        }
    }

    // Côté à jouer.
    seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    if board.turn == Color::Black {
        hash ^= seed;
    }

    // Droits de roque.
    seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    if board.castling.white_kingside { hash ^= seed; }
    seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    if board.castling.white_queenside { hash ^= seed; }
    seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    if board.castling.black_kingside { hash ^= seed; }
    seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    if board.castling.black_queenside { hash ^= seed; }

    // Case en passant.
    if let Some(ep) = board.en_passant {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        hash ^= seed.wrapping_add(ep as u64);
    }

    hash
}

/// Convertit un ELO (1-9999) en profondeur de recherche maximale.
fn elo_to_depth(elo: u32) -> u32 {
    match elo {
        1..=300 => 1,
        301..=600 => 2,
        601..=900 => 3,
        901..=1200 => 4,
        1201..=1500 => 5,
        1501..=1800 => 6,
        1801..=2100 => 7,
        _ => 8, // 2101-9999
    }
}

/// Nombre de coups parmi lesquels un bot de cet ELO peut choisir au hasard (erreurs).
fn elo_top_n(elo: u32) -> u32 {
    match elo {
        1..=400 => 10,
        401..=800 => 7,
        801..=1200 => 5,
        1201..=1600 => 3,
        _ => 1, // 1601+ : toujours le meilleur coup
    }
}

/// Probabilité (en pour-mille, 0-1000) qu'un bot de cet ELO fasse une erreur
/// en ne jouant pas le meilleur coup.
fn elo_error_chance(elo: u32) -> u32 {
    match elo {
        1..=400 => 750,
        401..=600 => 600,
        601..=800 => 450,
        801..=1000 => 350,
        1001..=1200 => 250,
        1201..=1400 => 150,
        1401..=1600 => 80,
        1601..=1800 => 30,
        1801..=2000 => 10,
        _ => 0, // 2001+ : jamais d'erreur intentionnelle
    }
}

/// Petit générateur de nombres aléatoires (LCG) pour choisir les erreurs.
struct SimpleRng {
    state: u64,
}

impl SimpleRng {
    fn new() -> Self {
        let seed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(1)
            .max(1);
        SimpleRng { state: seed }
    }

    fn next(&mut self) -> u64 {
        self.state = self
            .state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.state
    }

    /// Retourne un entier dans [min, max] inclus.
    fn range(&mut self, min: u32, max: u32) -> u32 {
        if min == max {
            return min;
        }
        min + (self.next() % (max as u64 - min as u64 + 1)) as u32
    }
}

/// Résultat d'une recherche.
pub struct SearchResult {
    pub best_move: Option<Move>,
    pub best_score: f64, // du point de vue du côté à jouer à la racine
    pub depth: u32,
    pub nodes: u64,
    pub time_ms: u128,
    /// Tous les coups de la racine avec leur score (triés du meilleur au pire),
    /// utilisés pour introduire des erreurs chez les bas ELO.
    pub root_scored: Vec<(Move, f64)>,
    /// Statistiques de la Transposition Table.
    pub tt_probes: u64,
    pub tt_hits: u64,
}

/// Moteur de recherche : minimax + alpha-beta + iterative deepening + pruning + transposition table.
///
/// Le champ `elo` contrôle la profondeur et la probabilité de faire une erreur.
/// `forced_depth` permet de surcharger la profondeur ELO (commande UCI `go depth X`).
pub struct Searcher {
    start_time: std::time::Instant,
    time_limit_ms: Option<u64>,
    pub stop: bool,
    pub elo: u32,
    pub forced_depth: Option<u32>,
    nodes: u64,
    tt: TranspositionTable,
}

impl Searcher {
    pub fn new() -> Self {
        Searcher {
            start_time: std::time::Instant::now(),
            time_limit_ms: None,
            stop: false,
            elo: 1500,
            forced_depth: None,
            nodes: 0,
            tt: TranspositionTable::new(),
        }
    }

    /// Recherche le meilleur coup pour `board`.
    /// `time_ms` : limite temps optionnelle (ms).
    pub fn search(&mut self, board: &Board, time_ms: Option<u64>) -> SearchResult {
        self.start_time = std::time::Instant::now();
        self.time_limit_ms = time_ms;
        self.stop = false;
        self.nodes = 0;
        self.tt.clear(); // Nouvelle recherche = nouvelle table

        let max_depth = self.forced_depth.unwrap_or_else(|| elo_to_depth(self.elo));
        let mut rng = SimpleRng::new();

        let mut best_move: Option<Move> = None;
        let mut best_score: f64 = 0.0;
        let mut completed_depth: u32 = 0;
        let mut root_scored: Vec<(Move, f64)> = Vec::new();

        let maximizing = board.turn == Color::White;

        // Iterative deepening : profondeur 1, 2, 3... jusqu'à max_depth ou temps écoulé.
        for depth in 1..=max_depth {
            if self.stop {
                break;
            }
            let mut root_best: Option<(Move, f64)> = None;
            let mut alpha = -f64::INFINITY;
            let mut beta = f64::INFINITY;

            // Coups triés : bons coups d'abord (captures MVV-LVA, puis silencieux).
            let moves = ordered_moves(board);
            if moves.is_empty() {
                break;
            }

            for (idx, m) in moves.iter().enumerate() {
                if self.stop {
                    break;
                }
                let mut next = *board;
                next.make_move(m);

                // Late Move Reduction : les coups tardifs (peu prometteurs) sont explorés
                // à profondeur réduite d'abord.
                let lmr_depth = if depth >= 3 && idx >= 3 && !is_capture(board, m) && !is_promotion(m) {
                    let reduction = 1 + (idx / 4) as u32;
                    depth.saturating_sub(reduction).max(1)
                } else {
                    depth - 1
                };

                let val = self.alphabeta(&next, lmr_depth, alpha, beta, !maximizing, idx == 0);

                // Si le coup à profondeur réduite dépasse alpha/beta, on re-explore à fond.
                let val = if lmr_depth < depth - 1
                    && ((maximizing && val > alpha) || (!maximizing && val < beta))
                {
                    self.alphabeta(&next, depth - 1, alpha, beta, !maximizing, false)
                } else {
                    val
                };

                if maximizing {
                    if root_best.is_none() || val > root_best.as_ref().unwrap().1 {
                        root_best = Some((*m, val));
                    }
                    alpha = alpha.max(val);
                } else {
                    if root_best.is_none() || val < root_best.as_ref().unwrap().1 {
                        root_best = Some((*m, val));
                    }
                    beta = beta.min(val);
                }

                // On garde toutes les évaluations de la racine pour le choix final.
                root_scored.push((*m, val));

                if beta <= alpha {
                    break;
                }
            }

            // Trie les coups de la racine du meilleur au pire.
            root_scored.sort_by(|a, b| {
                b.1.partial_cmp(&a.1)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });

            if let Some((mv, score)) = root_best {
                best_move = Some(mv);
                best_score = score;
                completed_depth = depth;
            }

            // Vérification temps.
            if let Some(limit) = self.time_limit_ms {
                let elapsed = self.start_time.elapsed().as_millis();
                if elapsed >= limit as u128 {
                    self.stop = true;
                }
            }
        }

        // --- Introduire des erreurs intentionnelles selon l'ELO ---
        if let Some(ref mut best) = best_move {
            if !root_scored.is_empty() {
                let chance = elo_error_chance(self.elo);
                if chance > 0 && rng.range(0, 1000) < chance {
                    let top_n = elo_top_n(self.elo).min(root_scored.len() as u32);
                    let idx = if top_n == 1 {
                        0
                    } else {
                        let roll = rng.range(0, top_n * 2); // 0 .. top_n*2-1
                        if roll < top_n {
                            roll as usize
                        } else {
                            ((top_n - 1) as usize).min((roll - top_n) as usize)
                        }
                    };
                    let chosen_idx = idx.min((top_n - 1) as usize);
                    *best = root_scored[chosen_idx].0;
                    best_score = root_scored[chosen_idx].1;
                }
            }
        }

        SearchResult {
            best_move,
            best_score,
            depth: completed_depth,
            nodes: self.nodes,
            time_ms: self.start_time.elapsed().as_millis(),
            root_scored,
            tt_probes: self.tt.probes,
            tt_hits: self.tt.hits,
        }
    }

    /// Alpha-beta récursif avec futility pruning, late move reduction et transposition table.
    fn alphabeta(
        &mut self,
        board: &Board,
        depth: u32,
        mut alpha: f64,
        mut beta: f64,
        maximizing: bool,
        is_root_move: bool,
    ) -> f64 {
        self.nodes += 1;

        // Vérification temps périodique pendant la recherche.
        if self.nodes % 1024 == 0 {
            if let Some(limit) = self.time_limit_ms {
                if self.start_time.elapsed().as_millis() >= limit as u128 {
                    self.stop = true;
                    return evaluate(board);
                }
            }
        }

        if self.stop {
            return evaluate(board);
        }

        // Feuille : évaluation statique.
        if depth == 0 {
            return evaluate(board);
        }

        // --- Transposition Table : probe ---
        let tt_key = zobrist_key(board);
        if let Some((tt_score, _tt_move, tt_bound)) = self.tt.probe(tt_key, depth, alpha, beta) {
            match tt_bound {
                BoundType::Exact => return tt_score,
                BoundType::Lower => alpha = alpha.max(tt_score),
                BoundType::Upper => beta = beta.min(tt_score),
            }
            if alpha >= beta {
                return tt_score;
            }
        }

        let moves = ordered_moves(board);
        if moves.is_empty() {
            if board.is_in_check(board.turn) {
                // Échec-et-mat : score très mauvais pour le côté à jouer.
                return if maximizing { -MATE_SCORE } else { MATE_SCORE };
            } else {
                // Pat : nul.
                return 0.0;
            }
        }

        let in_check = board.is_in_check(board.turn);

        // Futility pruning à faible profondeur.
        if !in_check && !is_root_move && depth <= 3 {
            let static_eval = evaluate(board);
            let margin = futility_margin(depth);

            if maximizing {
                if static_eval + margin <= alpha {
                    return static_eval;
                }
            } else {
                if static_eval - margin >= beta {
                    return static_eval;
                }
            }
        }

        let original_alpha = alpha;

        if maximizing {
            let mut best = -f64::INFINITY;
            let mut best_move_local: Option<Move> = None;
            for (idx, m) in moves.iter().enumerate() {
                if !in_check && depth <= 3 && !is_capture(board, m) && !is_promotion(m) {
                    let gain = piece_move_value(m.promotion.unwrap_or(PieceType::Pawn)) as f64;
                    if evaluate(board) + gain + futility_margin(depth) <= alpha {
                        continue;
                    }
                }

                let mut next = *board;
                next.make_move(m);

                let lmr_depth = if depth >= 3 && idx >= 3 && !is_capture(board, m) && !is_promotion(m) {
                    let reduction = 1 + (idx / 4) as u32;
                    depth.saturating_sub(reduction).max(1)
                } else {
                    depth - 1
                };

                let val = self.alphabeta(&next, lmr_depth, alpha, beta, false, idx == 0);

                let val = if lmr_depth < depth - 1 && val > alpha {
                    self.alphabeta(&next, depth - 1, alpha, beta, false, false)
                } else {
                    val
                };

                if val > best {
                    best = val;
                    best_move_local = Some(*m);
                }
                alpha = alpha.max(best);
                if beta <= alpha {
                    break;
                }
            }
            let bound = if best <= original_alpha {
                BoundType::Upper
            } else if best >= beta {
                BoundType::Lower
            } else {
                BoundType::Exact
            };
            self.tt.record(tt_key, depth, best, bound, best_move_local);
            best
        } else {
            let mut best = f64::INFINITY;
            let mut best_move_local: Option<Move> = None;
            for (idx, m) in moves.iter().enumerate() {
                if !in_check && depth <= 3 && !is_capture(board, m) && !is_promotion(m) {
                    let gain = piece_move_value(m.promotion.unwrap_or(PieceType::Pawn)) as f64;
                    if evaluate(board) - gain - futility_margin(depth) >= beta {
                        continue;
                    }
                }

                let mut next = *board;
                next.make_move(m);

                let lmr_depth = if depth >= 3 && idx >= 3 && !is_capture(board, m) && !is_promotion(m) {
                    let reduction = 1 + (idx / 4) as u32;
                    depth.saturating_sub(reduction).max(1)
                } else {
                    depth - 1
                };

                let val = self.alphabeta(&next, lmr_depth, alpha, beta, true, idx == 0);

                let val = if lmr_depth < depth - 1 && val < beta {
                    self.alphabeta(&next, depth - 1, alpha, beta, true, false)
                } else {
                    val
                };

                if val < best {
                    best = val;
                    best_move_local = Some(*m);
                }
                beta = beta.min(best);
                if beta <= alpha {
                    break;
                }
            }
            let bound = if best <= original_alpha {
                BoundType::Upper
            } else if best >= beta {
                BoundType::Lower
            } else {
                BoundType::Exact
            };
            self.tt.record(tt_key, depth, best, bound, best_move_local);
            best
        }
    }
}

impl Default for Searcher {
    fn default() -> Self {
        Searcher::new()
    }
}

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
