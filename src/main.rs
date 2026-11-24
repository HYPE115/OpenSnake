use opensnake::board::Board;
use opensnake::engine::{Engine, evaluate, evaluate_for, status, to_score, GameStatus};
use opensnake::move_gen::legal_moves;
use opensnake::uci::Uci;
use opensnake::{alg_to_square, square_to_alg};

fn print_help() {
    println!("OpenSnake — Chess engine from scratch (Rust) — single binary, all features included");
    println!("Usage: OpenSnake.exe [commande] [arguments]");
    println!();
    println!("Mode moteur (par défaut, pour les bots et GUI) :");
    println!("  OpenSnake.exe                 Entre en mode UCI (attend des commandes sur stdin)");
    println!("  OpenSnake.exe uci             Idem, explicite");
    println!();
    println!("Commandes CLI (utilisation directe / tests) :");
    println!("  help                          Affiche cette aide");
    println!("  board                         Affiche le plateau actuel");
    println!("  fen                           Affiche la position au format FEN");
    println!("  loadfen <fen>                 Charge une position FEN");
    println!("  score                         Évalue la position (score 1-100 pour chaque côté)");
    println!("  best [nb]                     Liste les N meilleurs coups (défaut: 3)");
    println!("  moves                         Liste tous les coups légaux");
    println!("  play <from> <to>              Joue un coup (ex: play e2 e4)");
    println!("  status                        Statut de la partie (en cours, mat, pat)");
    println!("  bench [depth]                 Test de performance (perft), profondeur par défaut: 4");
    println!();
    println!("Exemple :");
    println!("  OpenSnake.exe loadfen rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1");
    println!("  OpenSnake.exe best");
    println!("  OpenSnake.exe play e2 e4");
}

fn print_board(board: &Board) {
    println!("{}", board);
    println!("Trait : {:?}", board.turn);
    println!("FEN   : {}", board.to_fen());
}

fn cmd_board(board: &Board) {
    print_board(board);
}

fn cmd_fen(board: &Board) {
    println!("{}", board.to_fen());
}

fn cmd_loadfen(board: &mut Board, fen: &str) {
    match Board::from_fen(fen) {
        Ok(b) => {
            *board = b;
            println!("Position chargée depuis FEN.");
            print_board(board);
        }
        Err(e) => {
            eprintln!("Erreur FEN : {}", e);
        }
    }
}

fn cmd_score(board: &Board) {
    let eval_cp = evaluate(board);
    let white_score = to_score(eval_cp);
    let black_score = to_score(-eval_cp);
    let side_score = evaluate_for(board, board.turn);

    println!("Évaluation : {:.1} centipawns (du point de vue des Blancs)", eval_cp);
    println!("Score  : Blancs {:.6}/100 | Noirs {:.6}/100", white_score, black_score);
    println!("Score ({:?} à jouer) : {:.6}/100", board.turn, side_score);

    let engine = Engine::new();
    match engine.best_move(board) {
        Some(best) => println!("Meilleur coup suggéré : {} (score {:.6}/100)", best.move_, best.score),
        None => println!("Aucun coup légal ({:?})", status(board)),
    }
}

fn cmd_best(board: &Board, count: usize) {
    let engine = Engine::new();
    let top = engine.best_moves(board, count);
    if top.is_empty() {
        println!("Aucun coup légal ({:?})", status(board));
        return;
    }
    println!("Top {} meilleur(s) coup(s) :", top.len());
    for (i, m) in top.iter().enumerate() {
        println!("  {}. {} | score {:.6}/100 | {:.6} centipawns", i + 1, m.move_, m.score, m.eval_centipawns);
    }
}

fn cmd_moves(board: &Board) {
    let moves = legal_moves(board);
    println!("{} coup(s) légal(s) :", moves.len());
    for m in moves {
        println!("  {}", m);
    }
}

fn cmd_play(board: &mut Board, from_str: &str, to_str: &str) {
    let from = match alg_to_square(from_str) {
        Some(s) => s,
        None => {
            eprintln!("Case 'from' invalide : '{}' (format attendu: e2)", from_str);
            return;
        }
    };
    let to = match alg_to_square(to_str) {
        Some(s) => s,
        None => {
            eprintln!("Case 'to' invalide : '{}' (format attendu: e4)", to_str);
            return;
        }
    };

    let legal = legal_moves(board);
    let mv = legal.iter().find(|m| m.from == from && m.to == to);

    match mv {
        Some(m) => {
            board.make_move(m);
            let side_score = evaluate_for(board, board.turn);
            println!("Coup joué : {} (score {:.2}/100 pour le nouveau côté à jouer)", m, side_score);
            print_board(board);
        }
        None => {
            eprintln!("Coup illégal : {}-{} (pas dans les coups légaux)", square_to_alg(from), square_to_alg(to));
        }
    }
}

fn cmd_status(board: &Board) {
    match status(board) {
        GameStatus::Ongoing => println!("Statut : partie en cours ({} coups légaux)", legal_moves(board).len()),
        GameStatus::Checkmate => println!("Statut : ÉCHEC ET MAT ({:?} a perdu)", board.turn),
        GameStatus::Stalemate => println!("Statut : PAT (nul)"),
    }
}

/// Compte les nœuds à une profondeur donnée (perft).
fn perft(board: &Board, depth: u32) -> u64 {
    if depth == 0 {
        return 1;
    }
    let mut nodes: u64 = 0;
    for m in legal_moves(board) {
        let mut next = *board;
        next.make_move(&m);
        nodes += perft(&next, depth - 1);
    }
    nodes
}

fn cmd_bench(board: &Board, depth: u32) {
    let start = std::time::Instant::now();
    let nodes = perft(board, depth);
    let elapsed = start.elapsed();
    let secs = elapsed.as_secs_f64();
    let nps = if secs > 0.0 { nodes as f64 / secs } else { 0.0 };

    println!("Perft profondeur {} :", depth);
    println!("  Nœuds       : {}", nodes);
    println!("  Temps       : {:.3} s", secs);
    println!("  Nœuds/seconde : {:.0}", nps as u64);
}

fn run_cli() -> Result<(), String> {
    let args: Vec<String> = std::env::args().collect();
    let command = args.get(1).map(|s| s.as_str()).unwrap_or("help");

    let mut board = Board::new();

    match command {
        "help" | "--help" | "-h" => {
            print_help();
        }
        "uci" => {
            // Bascule en mode UCI (géré par run_uci).
            return Err("uci".to_string()); // signal spécial pour main()
        }
        "board" => {
            cmd_board(&board);
        }
        "fen" => {
            cmd_fen(&board);
        }
        "loadfen" => {
            let fen = args.get(2..)
                .map(|parts| parts.join(" "))
                .ok_or_else(|| "Usage: OpenSnake.exe loadfen <fen>".to_string())?;
            cmd_loadfen(&mut board, &fen);
        }
        "score" | "eval" => {
            cmd_score(&board);
        }
        "best" => {
            let count: usize = args.get(2)
                .and_then(|s| s.parse().ok())
                .unwrap_or(3);
            cmd_best(&board, count);
        }
        "moves" => {
            cmd_moves(&board);
        }
        "play" => {
            let from = args.get(2).ok_or_else(|| "Usage: OpenSnake.exe play <from> <to>".to_string())?;
            let to = args.get(3).ok_or_else(|| "Usage: OpenSnake.exe play <from> <to>".to_string())?;
            cmd_play(&mut board, from, to);
        }
        "status" => {
            cmd_status(&board);
        }
        "bench" => {
            let depth: u32 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(4);
            cmd_bench(&board, depth);
        }
        other => {
            eprintln!("Commande inconnue : '{}'", other);
            print_help();
            return Err(format!("commande inconnue: {}", other));
        }
    }

    Ok(())
}

fn run_uci() {
    let mut uci = Uci::new();
    uci.run();
}

fn main() {
    let args: Vec<String> = std::env::args().collect();

    // Mode UCI par défaut : pas d'argument, ou premier argument "uci".
    if args.len() <= 1 || args.get(1).map(|s| s.as_str()) == Some("uci") {
        run_uci();
        return;
    }

    // Sinon, mode CLI avec commandes.
    if let Err(e) = run_cli() {
        // "uci" est un signal spécial (déjà géré ci-dessus), sinon c'est une vraie erreur.
        if e != "uci" {
            eprintln!("Erreur : {}", e);
            std::process::exit(1);
        }
    }
}
