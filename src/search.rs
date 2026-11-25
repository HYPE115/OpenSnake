use crate::board::{Board, Color, Move, PieceType};
use crate::engine::evaluate;
use crate::move_gen::{is_capture, is_promotion, legal_moves, see};
use std::collections::HashMap;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const MATE_SCORE: f64 = 20000.0;
const TT_SIZE: usize = 1_000_000; // 1 million d'entrées max
const MAX_PLY: usize = 64;

// Marge de futility pruning par profondeur (centipawns).
fn futility_margin(depth: u32) -> f64 {
    (depth as f64) * 80.0
}

/// Type de borne stocké dans la Transposition Table.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum BoundType {
    Exact, // Le score est exact
    Lower, // Le score est >= valeur (alpha cut)
    Upper, // Le score est <= valeur (beta cut)
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
#[derive(Clone)]
struct TranspositionTable {
    table: HashMap<u64, TTEntry>,
    capacity: usize,
    probes: u64,
    hits: u64,
}

impl TranspositionTable {
    fn new() -> Self {
        TranspositionTable {
            table: HashMap::with_capacity(TT_SIZE),
            capacity: TT_SIZE,
            probes: 0,
            hits: 0,
        }
    }

    /// Sonde la table pour une position donnée.
    /// Retourne (score, meilleur_coup, type_de_borne) si l'entrée est assez profonde.
    fn probe(
        &mut self,
        key: u64,
        depth: u32,
        alpha: f64,
        beta: f64,
    ) -> Option<(f64, Option<Move>, BoundType)> {
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

    /// Récupère seulement le meilleur coup connu (pour l'ordonnancement).
    fn get_move(&self, key: u64) -> Option<Move> {
        self.table.get(&key).and_then(|entry| entry.best_move)
    }

    fn clear(&mut self) {
        self.table.clear();
        self.reset_stats();
    }

    /// Remet les compteurs à zéro sans vider la table (une recherche = des stats propres).
    fn reset_stats(&mut self) {
        self.probes = 0;
        self.hits = 0;
    }

    /// Redimensionne la table (option UCI `Hash`, en Mo).
    fn resize_mb(&mut self, mb: usize) {
        // Une entrée fait à peu près 40 octets ; on garde une marge pour la table elle-même.
        let entries = (mb.saturating_mul(1024 * 1024) / 64).max(1024);
        if entries != self.capacity {
            self.capacity = entries;
            self.table = HashMap::with_capacity(entries.min(TT_SIZE.max(entries)));
        }
    }

    /// Enregistre un résultat dans la table.
    fn record(
        &mut self,
        key: u64,
        depth: u32,
        score: f64,
        bound: BoundType,
        best_move: Option<Move>,
    ) {
        // Remplacement simple : on garde l'entrée existante si elle est plus profonde.
        if let Some(old) = self.table.get(&key) {
            if old.depth > depth && old.bound != BoundType::Exact {
                return;
            }
        }
        if self.table.len() >= self.capacity {
            let keys_to_remove: Vec<u64> = self
                .table
                .iter()
                .filter(|(_, v)| v.depth < 2)
                .map(|(k, _)| *k)
                .take((self.capacity / 10).max(1))
                .collect();
            for k in keys_to_remove {
                self.table.remove(&k);
            }
        }
        self.table.insert(
            key,
            TTEntry {
                depth,
                score,
                bound,
                best_move,
            },
        );
    }
}

/// Génère une clé Zobrist pour une position (hash 64 bits).
fn zobrist_key(board: &Board) -> u64 {
    let mut seed: u64 = 0x9E3779B97F4A7C15;
    let mut hash: u64 = 0;

    // Mélange les pièces sur chaque case.
    for sq in 0..64u8 {
        if let Some(p) = board.get(sq) {
            seed = seed
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
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
    seed = seed
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    if board.turn == Color::Black {
        hash ^= seed;
    }

    // Droits de roque.
    seed = seed
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    if board.castling.white_kingside {
        hash ^= seed;
    }
    seed = seed
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    if board.castling.white_queenside {
        hash ^= seed;
    }
    seed = seed
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    if board.castling.black_kingside {
        hash ^= seed;
    }
    seed = seed
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    if board.castling.black_queenside {
        hash ^= seed;
    }

    // Case en passant.
    if let Some(ep) = board.en_passant {
        seed = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
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

/// Contexte d'ordonnancement des coups (TT move, coups tueurs, heuristique d'historique).
struct MoveOrderContext<'a> {
    tt_move: Option<Move>,
    killers: &'a [Option<Move>; 2],
    history: &'a [[i32; 64]; 2],
}

fn color_index(color: Color) -> usize {
    if color == Color::White {
        0
    } else {
        1
    }
}

/// Score d'ordonnancement : TT move > captures MVV-LVA > promotions > killers > historique.
fn move_order_score(board: &Board, m: &Move, ctx: &MoveOrderContext) -> i32 {
    if ctx.tt_move == Some(*m) {
        return 10_000_000;
    }

    let victim = board.get(m.to).map(|p| piece_move_value(p.kind)).unwrap_or(0);
    let attacker = board.get(m.from).map(|p| piece_move_value(p.kind)).unwrap_or(0);

    if victim > 0 {
        // MVV-LVA + SEE : on privilégie les captures gagnantes, pas seulement les grosses victimes.
        let exchange = see(board, m);
        return 1_000_000 + victim * 10 - attacker + exchange;
    }
    if let Some(promo) = m.promotion {
        return 900_000 + piece_move_value(promo);
    }
    if ctx.killers.iter().any(|k| *k == Some(*m)) {
        return 100_000;
    }

    ctx.history[color_index(board.turn)][m.to as usize].min(90_000)
}

/// Coups légaux triés du meilleur au pire selon le contexte de recherche.
fn ordered_moves_ctx(board: &Board, ctx: &MoveOrderContext) -> Vec<Move> {
    // On calcule chaque score une seule fois : SEE peut être coûteux.
    let mut scored: Vec<(i32, Move)> = legal_moves(board)
        .into_iter()
        .map(|m| (move_order_score(board, &m, ctx), m))
        .collect();
    scored.sort_by(|(sa, ma), (sb, mb)| {
        sb.cmp(sa).then_with(|| ma.from.cmp(&mb.from).then(ma.to.cmp(&mb.to)))
    });
    scored.into_iter().map(|(_, m)| m).collect()
}

/// En quiescence, on ignore les captures franchement perdantes d'après SEE.
fn worth_quiescence(board: &Board, m: &Move) -> bool {
    if is_promotion(m) {
        return true;
    }
    is_capture(board, m) && see(board, m) >= -50
}

/// Contexte sans heuristiques (utilisé par la quiescence).
fn plain_ordered_moves(board: &Board) -> Vec<Move> {
    let no_killers = [None, None];
    let no_history = [[0i32; 64]; 2];
    let ctx = MoveOrderContext {
        tt_move: None,
        killers: &no_killers,
        history: &no_history,
    };
    ordered_moves_ctx(board, &ctx)
}

/// Vrai si le camp possède au moins une pièce autre que des pions
/// (condition nécessaire pour le null move pruning).
fn has_non_pawn_material(board: &Board, color: Color) -> bool {
    for sq in 0..64u8 {
        if let Some(p) = board.get(sq) {
            if p.color == color && !matches!(p.kind, PieceType::Pawn | PieceType::King) {
                return true;
            }
        }
    }
    false
}

/// Résultat d'une recherche.
pub struct SearchResult {
    pub best_move: Option<Move>,
    pub best_score: f64, // du point de vue du côté à jouer à la racine
    pub depth: u32,
    pub nodes: u64,
    pub time_ms: u128,
    /// Tous les coups de la racine avec leur score (triés du meilleur au pire),
    /// utilisés pour introduire des erreurs chez les bas ELO et pour MultiPV.
    pub root_scored: Vec<(Move, f64)>,
    /// Statistiques de la Transposition Table.
    pub tt_probes: u64,
    pub tt_hits: u64,
}

/// Moteur de recherche : alpha-beta + iterative deepening + PVS + quiescence +
/// pruning (futility, LMR, null move) + transposition table + move ordering.
///
/// Le champ `elo` contrôle la profondeur et la probabilité de faire une erreur.
/// `forced_depth` permet de surcharger la profondeur ELO (commande UCI `go depth X`).
#[derive(Clone)]
pub struct Searcher {
    start_time: std::time::Instant,
    time_limit_ms: Option<u64>,
    pub stop: bool,
    /// Drapeau partagé avec le thread UCI pour `stop` / `quit` pendant la recherche.
    pub abort: Option<Arc<AtomicBool>>,
    pub elo: u32,
    pub forced_depth: Option<u32>,
    /// Délai minimum avant de répondre (ms). Sert à ne pas jouer instantanément
    /// quand un faible ELO donne une recherche très courte.
    pub min_time_ms: Option<u64>,
    /// Nombre de variantes à garder à la racine (MultiPV > 1 désactive l'élagage racine).
    pub multi_pv: usize,
    nodes: u64,
    tt: TranspositionTable,
    killers: [[Option<Move>; 2]; MAX_PLY],
    history: [[i32; 64]; 2],
}

impl Searcher {
    pub fn new() -> Self {
        Searcher {
            start_time: std::time::Instant::now(),
            time_limit_ms: None,
            stop: false,
            abort: None,
            elo: 1500,
            forced_depth: None,
            min_time_ms: None,
            multi_pv: 1,
            nodes: 0,
            tt: TranspositionTable::new(),
            killers: [[None; 2]; MAX_PLY],
            history: [[0; 64]; 2],
        }
    }

    /// Nouvelle partie : on vide la table (les positions d'une partie précédente ne servent plus).
    pub fn new_game(&mut self) {
        self.tt.clear();
        self.killers = [[None; 2]; MAX_PLY];
        self.history = [[0; 64]; 2];
    }

    /// Taille de la table de transposition en Mo (option UCI `Hash`).
    pub fn set_hash_mb(&mut self, mb: usize) {
        self.tt.resize_mb(mb);
    }

    /// Vrai si l'utilisateur (UCI `stop`) ou le temps imparti demande l'arrêt.
    fn should_stop(&self) -> bool {
        self.stop
            || self
                .abort
                .as_ref()
                .map(|flag| flag.load(Ordering::Relaxed))
                .unwrap_or(false)
    }

    /// Recherche le meilleur coup pour `board`.
    /// `time_ms` : limite temps optionnelle (ms).
    pub fn search(&mut self, board: &Board, time_ms: Option<u64>) -> SearchResult {
        self.start_time = std::time::Instant::now();
        self.time_limit_ms = time_ms;
        self.stop = false;
        self.nodes = 0;
        self.tt.reset_stats(); // la table survit aux coups, les stats sont par recherche

        let max_depth = self.forced_depth.unwrap_or_else(|| elo_to_depth(self.elo));
        let mut rng = SimpleRng::new();

        let mut best_move: Option<Move> = None;
        let mut best_score: f64 = 0.0; // point de vue du côté à jouer
        let mut raw_best: f64 = 0.0; // point de vue des Blancs (pour les fenêtres)
        let mut completed_depth: u32 = 0;
        let mut root_scored: Vec<(Move, f64)> = Vec::new();
        let mut pv_move: Option<Move> = None;

        let maximizing = board.turn == Color::White;

        // Iterative deepening : profondeur 1, 2, 3... jusqu'à max_depth ou temps écoulé.
        for depth in 1..=max_depth {
            if self.should_stop() {
                break;
            }

            // Aspiration windows : à partir de la profondeur 3, on cherche dans une
            // petite fenêtre autour du score précédent. Si on sort de la fenêtre,
            // on relance avec la fenêtre complète.
            let use_aspiration = depth >= 3 && self.multi_pv <= 1;
            let mut window_alpha = if use_aspiration {
                (raw_best - 50.0).max(-MATE_SCORE - 100.0)
            } else {
                f64::NEG_INFINITY
            };
            let mut window_beta = if use_aspiration {
                (raw_best + 50.0).min(MATE_SCORE + 100.0)
            } else {
                f64::INFINITY
            };

            // MultiPV : on doit évaluer tous les coups racine avec la fenêtre complète,
            // sinon les coups élagués n'ont pas de score affichable.
            let root_full = self.multi_pv > 1;
            if root_full {
                window_alpha = f64::NEG_INFINITY;
                window_beta = f64::INFINITY;
            }

            let mut accepted = false;
            for _attempt in 0..2 {
                let moves = {
                    let ctx = MoveOrderContext {
                        tt_move: pv_move.or_else(|| self.tt.get_move(zobrist_key(board))),
                        killers: &self.killers[0],
                        history: &self.history,
                    };
                    ordered_moves_ctx(board, &ctx)
                };
                if moves.is_empty() {
                    break;
                }

                let mut root_best: Option<(Move, f64)> = None;
                let mut scored: Vec<(Move, f64)> = Vec::new();
                let mut alpha = window_alpha;
                let mut beta = window_beta;

                for (idx, m) in moves.iter().enumerate() {
                    if self.should_stop() {
                        break;
                    }
                    let mut next = *board;
                    next.make_move(m);

                    // Late Move Reduction : les coups tardifs sont d'abord explorés
                    // à profondeur réduite, puis rejoués à fond s'ils sont prometteurs.
                    let lmr_depth = if depth >= 3
                        && idx >= 3
                        && !is_capture(board, m)
                        && !is_promotion(m)
                    {
                        let reduction = 1 + (idx / 4) as u32;
                        depth.saturating_sub(reduction).max(1)
                    } else {
                        depth - 1
                    };

                    let val = self.alphabeta(
                        &next,
                        lmr_depth,
                        alpha,
                        beta,
                        !maximizing,
                        idx == 0,
                        1,
                    );
                    let val = if lmr_depth < depth - 1
                        && ((maximizing && val > alpha) || (!maximizing && val < beta))
                    {
                        self.alphabeta(&next, depth - 1, alpha, beta, !maximizing, false, 1)
                    } else {
                        val
                    };

                    if maximizing {
                        if root_best.map(|(_, v)| val > v).unwrap_or(true) {
                            root_best = Some((*m, val));
                        }
                        alpha = alpha.max(val);
                    } else {
                        if root_best.map(|(_, v)| val < v).unwrap_or(true) {
                            root_best = Some((*m, val));
                        }
                        beta = beta.min(val);
                    }

                    // Le score affiché/choisi est toujours du point de vue du joueur au trait.
                    let mover_score = if maximizing { val } else { -val };
                    scored.push((*m, mover_score));
                    if beta <= alpha && !root_full {
                        break;
                    }
                }

                let val = root_best.map(|(_, v)| v);
                let outside = match val {
                    Some(v) if maximizing => v <= window_alpha || v >= window_beta,
                    Some(v) => v >= window_beta || v <= window_alpha,
                    None => false,
                };
                let full_window = window_alpha == f64::NEG_INFINITY && window_beta == f64::INFINITY;

                if outside && !full_window && !self.should_stop() {
                    // Échec de la fenêtre : on élargit et on relance cette profondeur.
                    window_alpha = f64::NEG_INFINITY;
                    window_beta = f64::INFINITY;
                    continue;
                }

                // Si le temps est écoulé en pleine itération, on garde le résultat
                // de la dernière profondeur terminée, pas un score partiel.
                if self.should_stop() {
                    break;
                }

                if let Some((mv, score)) = root_best {
                    best_move = Some(mv);
                    raw_best = score;
                    best_score = if maximizing { score } else { -score };
                    completed_depth = depth;
                    pv_move = Some(mv);
                    // Tout est maintenant du point de vue du joueur au trait : meilleur = plus grand.
                    scored.sort_by(|a, b| {
                        b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal)
                    });
                    root_scored = scored;
                }
                accepted = true;
                break;
            }
            let _ = accepted;

            // Vérification temps.
            if let Some(limit) = self.time_limit_ms {
                if self.start_time.elapsed().as_millis() >= limit as u128 {
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
                        let roll = rng.range(0, top_n * 2);
                        if roll < top_n {
                            roll as usize
                        } else {
                            ((top_n - 1) as usize).min((roll - top_n) as usize)
                        }
                    };
                    let chosen_idx = idx.min((top_n - 1) as usize);
                    let chosen = root_scored[chosen_idx];
                    *best = chosen.0;
                    best_score = chosen.1;
                    // Le PV envoyé à l'UCI doit toujours commencer par le coup joué.
                    root_scored.remove(chosen_idx);
                    root_scored.insert(0, chosen);
                }
            }
        }

        // Délai minimum "humain" : à bas ELO la recherche finit en 1 ms, mais un
        // joueur de 200 ELO ne joue pas instantanément. On attend donc un peu,
        // tout en restant interrompable par `stop` / `quit`.
        if let Some(min_ms) = self.min_time_ms {
            while !self.should_stop() && self.start_time.elapsed().as_millis() < min_ms as u128 {
                thread::sleep(Duration::from_millis(5));
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

    /// Alpha-beta récursif avec PVS, quiescence, futility, LMR, null move,
    /// check extensions, killer moves, historique et transposition table.
    #[allow(clippy::too_many_arguments)]
    fn alphabeta(
        &mut self,
        board: &Board,
        depth: u32,
        mut alpha: f64,
        mut beta: f64,
        maximizing: bool,
        is_root_move: bool,
        ply: usize,
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
        if self.should_stop() {
            self.stop = true;
            return evaluate(board);
        }

        let in_check = board.is_in_check(board.turn);

        // Feuille : quiescence search sauf si le roi est en échec (dans ce cas on prolonge).
        if depth == 0 && !in_check {
            return self.quiesce(board, alpha, beta, maximizing);
        }

        // Check extension : approfondir d'un ply quand le roi est attaqué.
        let depth = if in_check && ply < MAX_PLY / 2 {
            depth + 1
        } else {
            depth
        };

        // --- Transposition Table : probe ---
        let tt_key = zobrist_key(board);
        if let Some((tt_score, _tt_move, tt_bound)) =
            self.tt.probe(tt_key, depth, alpha, beta)
        {
            match tt_bound {
                BoundType::Exact => return tt_score,
                BoundType::Lower => alpha = alpha.max(tt_score),
                BoundType::Upper => beta = beta.min(tt_score),
            }
            if alpha >= beta {
                return tt_score;
            }
        }

        // --- Null Move Pruning ---
        // Si on "passe" son tour et que la position reste aussi bonne, on peut couper.
        if !is_root_move
            && !in_check
            && depth >= 3
            && beta.abs() < MATE_SCORE - 100.0
            && has_non_pawn_material(board, board.turn)
        {
            let mut null_board = *board;
            null_board.turn = null_board.turn.opposite();
            null_board.en_passant = None;
            let reduction = 2 + depth / 4;
            let null_depth = depth.saturating_sub(1 + reduction);
            let null_val = if maximizing {
                self.alphabeta(
                    &null_board,
                    null_depth,
                    beta - 1.0,
                    beta,
                    false,
                    false,
                    ply + 1,
                )
            } else {
                self.alphabeta(
                    &null_board,
                    null_depth,
                    alpha,
                    alpha + 1.0,
                    true,
                    false,
                    ply + 1,
                )
            };
            if (maximizing && null_val >= beta) || (!maximizing && null_val <= alpha) {
                return if maximizing { beta } else { alpha };
            }
        }

        let moves = {
            let ctx = MoveOrderContext {
                tt_move: self.tt.get_move(tt_key),
                killers: &self.killers[ply.min(MAX_PLY - 1)],
                history: &self.history,
            };
            ordered_moves_ctx(board, &ctx)
        };
        if moves.is_empty() {
            if in_check {
                return if maximizing { -MATE_SCORE } else { MATE_SCORE };
            } else {
                return 0.0; // pat
            }
        }

        // Futility pruning à faible profondeur (sauf au premier coup et sous échec).
        if !in_check && !is_root_move && depth <= 3 {
            let static_eval = evaluate(board);
            let margin = futility_margin(depth);
            if (maximizing && static_eval + margin <= alpha)
                || (!maximizing && static_eval - margin >= beta)
            {
                return static_eval;
            }
        }

        let original_alpha = alpha;
        let color_idx = color_index(board.turn);

        if maximizing {
            let mut best = f64::NEG_INFINITY;
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

                let lmr_depth = if depth >= 3
                    && idx >= 3
                    && !is_capture(board, m)
                    && !is_promotion(m)
                {
                    let reduction = 1 + (idx / 4) as u32;
                    depth.saturating_sub(reduction).max(1)
                } else {
                    depth - 1
                };

                // PVS : premier coup avec fenêtre complète, suivants avec fenêtre nulle.
                let val = if idx == 0 {
                    self.alphabeta(&next, lmr_depth, alpha, beta, false, idx == 0, ply + 1)
                } else {
                    let v = self.alphabeta(
                        &next,
                        lmr_depth,
                        alpha,
                        alpha + 1.0,
                        false,
                        false,
                        ply + 1,
                    );
                    if v > alpha && v < beta {
                        self.alphabeta(&next, lmr_depth, alpha, beta, false, false, ply + 1)
                    } else {
                        v
                    }
                };
                let val = if lmr_depth < depth - 1 && val > alpha {
                    self.alphabeta(&next, depth - 1, alpha, beta, false, false, ply + 1)
                } else {
                    val
                };

                if val > best {
                    best = val;
                    best_move_local = Some(*m);
                }
                alpha = alpha.max(best);
                if beta <= alpha {
                    self.record_cutoff(board, ply, m, depth, color_idx);
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

                let lmr_depth = if depth >= 3
                    && idx >= 3
                    && !is_capture(board, m)
                    && !is_promotion(m)
                {
                    let reduction = 1 + (idx / 4) as u32;
                    depth.saturating_sub(reduction).max(1)
                } else {
                    depth - 1
                };

                let val = self.alphabeta(
                    &next,
                    lmr_depth,
                    alpha,
                    beta,
                    true,
                    idx == 0,
                    ply + 1,
                );
                let val = if lmr_depth < depth - 1 && val < beta {
                    self.alphabeta(&next, depth - 1, alpha, beta, true, false, ply + 1)
                } else {
                    val
                };

                if val < best {
                    best = val;
                    best_move_local = Some(*m);
                }
                beta = beta.min(best);
                if beta <= alpha {
                    self.record_cutoff(board, ply, m, depth, color_idx);
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

    /// Enregistre un coup tueur et renforce l'historique après une coupure beta.
    fn record_cutoff(
        &mut self,
        board: &Board,
        ply: usize,
        m: &Move,
        depth: u32,
        color_idx: usize,
    ) {
        // Les coups tueurs/historique ne concernent que les coups silencieux.
        if is_capture(board, m) || is_promotion(m) {
            return;
        }
        let slot = &mut self.killers[ply.min(MAX_PLY - 1)];
        if slot[0] != Some(*m) {
            slot[1] = slot[0];
            slot[0] = Some(*m);
        }
        self.history[color_idx][m.to as usize] += (depth * depth) as i32;
    }

    /// Quiescence search : explore uniquement les captures (et promotions)
    /// à partir d'une feuille (depth == 0) pour éviter l'effet d'horizon.
    ///
    /// Retourne le score de la position après avoir résolu les échanges tactiques immédiats.
    fn quiesce(&mut self, board: &Board, mut alpha: f64, mut beta: f64, maximizing: bool) -> f64 {
        self.nodes += 1;

        if self.should_stop() {
            self.stop = true;
            return evaluate(board);
        }

        // Stand pat : évaluation statique de la position courante.
        let stand_pat = evaluate(board);

        if maximizing {
            if stand_pat >= beta {
                return beta; // fail hard : l'adversaire ne permettra pas cette position
            }
            if stand_pat > alpha {
                alpha = stand_pat;
            }
        } else {
            if stand_pat <= alpha {
                return alpha;
            }
            if stand_pat < beta {
                beta = stand_pat;
            }
        }

        // Génère seulement les captures et promotions (coups tactiques).
        let moves = plain_ordered_moves(board);

        if maximizing {
            let mut best = stand_pat;
            for m in moves.iter().filter(|m| worth_quiescence(board, m)) {
                let mut next = *board;
                next.make_move(m);
                let val = self.quiesce(&next, alpha, beta, false);
                if val > best {
                    best = val;
                }
                alpha = alpha.max(best);
                if beta <= alpha {
                    break; // beta cutoff
                }
            }
            best
        } else {
            let mut best = stand_pat;
            for m in moves.iter().filter(|m| worth_quiescence(board, m)) {
                let mut next = *board;
                next.make_move(m);
                let val = self.quiesce(&next, alpha, beta, true);
                if val < best {
                    best = val;
                }
                beta = beta.min(best);
                if beta <= alpha {
                    break; // alpha cutoff
                }
            }
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Balayage de TOUTE la plage 1..=9999 : plus l'ELO monte, plus le moteur
    /// doit être fort (profondeur up, erreurs down), jamais l'inverse.
    #[test]
    fn elo_scaling_is_monotonic_over_full_range() {
        let mut last_depth = 0;
        let mut last_error = 1001;
        let mut last_top_n = 11;

        for elo in 1..=9999u32 {
            let depth = elo_to_depth(elo);
            let error = elo_error_chance(elo);
            let top_n = elo_top_n(elo);

            assert!((1..=8).contains(&depth), "profondeur hors bornes à {}: {}", elo, depth);
            assert!(error <= 1000, "probabilité d'erreur hors bornes à {}", elo);
            assert!((1..=10).contains(&top_n), "top_n hors bornes à {}: {}", elo, top_n);

            assert!(depth >= last_depth, "la profondeur régresse à {}", elo);
            assert!(error <= last_error, "la probabilité d'erreur remonte à {}", elo);
            assert!(top_n <= last_top_n, "top_n remonte à {}", elo);

            last_depth = depth;
            last_error = error;
            last_top_n = top_n;
        }

        assert_eq!(
            (elo_to_depth(1), elo_error_chance(1), elo_top_n(1)),
            (1, 750, 10)
        );
        assert_eq!(
            (elo_to_depth(9999), elo_error_chance(9999), elo_top_n(9999)),
            (8, 0, 1)
        );
    }

    #[test]
    fn elo_tiers_match_the_expected_steps() {
        let samples = [
            1, 300, 301, 600, 601, 900, 901, 1200, 1201, 1500, 1501, 1800, 1801, 2100, 2101, 9999,
        ];
        let depths: Vec<u32> = samples.iter().map(|&e| elo_to_depth(e)).collect();
        assert_eq!(depths, vec![1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8]);
        assert_eq!(elo_error_chance(2000), 10);
        assert_eq!(elo_error_chance(2001), 0);
        assert_eq!(elo_top_n(1600), 3);
        assert_eq!(elo_top_n(1601), 1);
    }
}
