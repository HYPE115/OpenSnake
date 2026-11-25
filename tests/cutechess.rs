use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

struct Engine {
    child: Child,
    stdin: ChildStdin,
    reader: BufReader<ChildStdout>,
}

impl Engine {
    fn start() -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_OpenSnake"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("OpenSnake.exe should start");
        let stdin = child.stdin.take().expect("stdin");
        let stdout = child.stdout.take().expect("stdout");
        Engine {
            child,
            stdin,
            reader: BufReader::new(stdout),
        }
    }

    fn send(&mut self, cmd: &str) {
        writeln!(self.stdin, "{}", cmd).expect("write to engine stdin");
        self.stdin.flush().expect("flush engine stdin");
    }

    fn read_until(&mut self, prefix: &str) -> Vec<String> {
        let mut lines = Vec::new();
        loop {
            let mut line = String::new();
            let read = self.reader.read_line(&mut line).expect("read engine stdout");
            assert!(read > 0, "engine closed stdout before `{}`", prefix);
            let line = line.trim().to_string();
            let reached = line.starts_with(prefix);
            lines.push(line);
            if reached {
                return lines;
            }
        }
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        let _ = writeln!(self.stdin, "quit");
        let _ = self.stdin.flush();
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Session envoyée par Cute Chess avant une partie : uci / isready / ucinewgame / go.
#[test]
fn cutechess_handshake_and_timed_go() {
    let mut engine = Engine::start();

    // Le moteur ne doit rien envoyer avant la commande `uci`.
    engine.send("uci");
    let handshake = engine.read_until("uciok");
    assert!(handshake.iter().any(|l| l == "id name OpenSnake"));
    assert!(handshake.iter().any(|l| l.starts_with("option name UCI_Elo")));
    assert!(handshake.iter().any(|l| l.starts_with("option name Hash")));

    engine.send("isready");
    let ready = engine.read_until("readyok");
    assert_eq!(ready.last().map(String::as_str), Some("readyok"));

    engine.send("ucinewgame");
    engine.send("isready");
    let ready = engine.read_until("readyok");
    assert_eq!(ready.last().map(String::as_str), Some("readyok"));

    engine.send("position startpos moves e2e4 e7e5");
    engine.send("go wtime 5000 btime 5000 winc 100 binc 100 movestogo 40");
    let output = engine.read_until("bestmove");
    assert!(
        output.iter().any(|l| l.starts_with("info depth ")),
        "Cute Chess attend une ligne info avant bestmove"
    );
    let bestmove = output.last().expect("bestmove");
    assert!(bestmove.starts_with("bestmove "), "ligne: {}", bestmove);
    assert_ne!(bestmove, "bestmove 0000");
}

/// `stop` doit être traité pendant la recherche, sans attendre la fin de `go infinite`.
#[test]
fn cutechess_stop_is_handled_while_searching() {
    let mut engine = Engine::start();
    engine.send("uci");
    engine.read_until("uciok");
    engine.send("isready");
    engine.read_until("readyok");

    engine.send("position startpos");
    engine.send("go infinite");
    engine.send("isready");
    let ready = engine.read_until("readyok");
    assert_eq!(ready.last().map(String::as_str), Some("readyok"));

    engine.send("stop");
    let output = engine.read_until("bestmove");
    assert!(output.last().expect("bestmove").starts_with("bestmove "));
}

/// À bas ELO le moteur ne doit pas répondre instantanément, et MoveDelay doit
/// pouvoir remplacer la valeur dérivée de l'ELO.
#[test]
fn think_delay_follows_elo_and_can_be_configured() {
    let mut engine = Engine::start();
    engine.send("uci");
    engine.read_until("uciok");

    engine.send("setoption name UCI_Elo value 200");
    let start = std::time::Instant::now();
    engine.send("position startpos");
    engine.send("go depth 1"); // aucun temps imposé : seul le délai compte
    engine.read_until("bestmove");
    let weak_ms = start.elapsed().as_millis();
    assert!(
        weak_ms >= 1_500,
        "200 ELO doit réfléchir au moins ~1,5 s, obtenu {} ms",
        weak_ms
    );

    engine.send("setoption name UCI_Elo value 9999");
    engine.send("setoption name MoveDelay value 2500");
    let start = std::time::Instant::now();
    engine.send("go depth 1");
    engine.read_until("bestmove");
    let configured_ms = start.elapsed().as_millis();
    assert!(
        configured_ms >= 2_000,
        "MoveDelay 2500 doit attendre au moins ~2 s, obtenu {} ms",
        configured_ms
    );
}
