## OpenSnake

OpenSnake is an open source chess engine like stockfish (but worst than it use stockfish bro this repo suck tbh)

# Why?

I had found a chess game writen in python but the ai suck, they taked 30 seconds or more to make a move, so I made this to see if it's **THAT** hard to make one

# Key feature

- Scoring system ---> On a scale to 1 up to 100 but now with decimals (64.532168 type shi) so 2 moves with the same score can still be compared and u actually pick the best one
- Best moove finder ---> List the good move ig? (now it list the top N moves sorted by their exact score, not just the best one)
- Writed in Rust ---> Rust is fast so why to not use it? (probably a bad idea but atleast it's fast if the code doesn't break...)
- Chess ai compatibility ---> Idk how to say that, it's supposed to be an AI chessbot/engine yk. Ima try to be the easier possible to use it in your project for chessbot (don't use it as a cheat you gonna loose elo at this point #_#)
- UCI protocol ---> The universal standard so Arena, SCID, Cute Chess and any bot can talk to it properly (u send position, it's send move, verry simple yk)
- ELO selector ---> U can set the bot ELO from 1 to 9999 (low elo = it play like me, high elo = it tryhard but he probably suck too bc i made it)
- Time management ---> Handle wtime, btime and movetime so it don't lose on time in blitz/rapid (u just told him to speed up, so he not gonna take 1day to play in a 5 mins blitz)
- Transposition table ---> It remember positions it already calculated instead of recalculating everything (that the ram of the sofware, probably)
- Alpha-beta + pruning ---> It look at the good moves first and throw away the trash ones (I hope it's work, i'm too lazy to test it, i don't even know how to test that..)
- FEN support ---> Load and export any position with FEN (for all the weird custom positions u want, try to not break the engine please)
- Bench command ---> Perft benchmark to see how many nodes/sec it can do (nerd stuff)
- Build script ---> Just run build.bat and u get a .exe (why did i even explain it?)
- Better eval ---> It's not just "count the pieces" anymore, it also looks at where they stand, pawns, king safety, mobility and all that nerdy shi (still no neural network tho)
- Move ordering ---> It tries TT moves, captures with SEE, killer moves and history first so it can throw away the trash faster
- MultiPV / Hash / AnalyseMode ---> U can ask for many lines, set the table size and make it think without a time limit (handy for analysis)
- Cute Chess ready ---> Full UCI handshake and async `isready` / `stop` / `ucinewgame` (even while it's thinking), so u can just point Cute Chess at the .exe and run games
- Human timing ---> Low ELO don't play instantly anymore (delay from ELO: 200 ≈ 2,3 s, 1500 ≈ 1 s, 9999 ≈ 0,2 s), configurable in ms with `MoveDelay` + `MoveDelayJitter` for a time range
- Other things ig? idk what other things to add rn, ima see later

# Build

- Step 1: Run build.bat on windows
- Step 2: Wait a bit
- Step 3:  Find the .exe in `.\OpenSnake\target\release`

# How To Use

To use it at an ai bot, you jsut need to use the `OpenSnake.exe` in your code. U talk to it with **UCI** (the chess standard), so u start the exe, u write commands in its stdin and u read its answers from its stdout. That's it, no api key, no server, no internet.

The 3 commands u actually need:

- `position startpos moves e2e4 e7e5` ---> tell it the position (or `position fen ...` for a custom one)
- `go movetime 100` ---> tell it to think max 100 ms (or `go wtime 10000 btime 10000` in a real game)
- `bestmove g1f3` ---> what it answer, that's the move u play

Optional but cool: `setoption name Elo value 1500` before the `go` if u want it weaker/stronger (1 to 9999). U can also use `setoption name Hash value 64`, `setoption name MultiPV value 3` or `setoption name UCI_AnalyseMode value true`.

**Exemple:**
*Python:*
```python
import subprocess

engine = subprocess.Popen(
    ["OpenSnake.exe"],  # put the full path if it's not in the same folder
    stdin=subprocess.PIPE,
    stdout=subprocess.PIPE,
    text=True,
    bufsize=1,
)

def send(cmd):
    engine.stdin.write(cmd + "\n")
    engine.stdin.flush()

send("uci")
send("isready")
send("setoption name Elo value 1500")  # optional, 1 to 9999
send("position startpos moves e2e4 e7e5")
send("go movetime 100")

while True:
    line = engine.stdout.readline().strip()
    if line.startswith("bestmove"):
        print(line)  # bestmove g1f3
        break

send("quit")
engine.wait()
```

*Javascript (node):*
```javascript
const { spawn } = require("child_process");

const engine = spawn("OpenSnake.exe"); // put the full path if needed
let buffer = "";

engine.stdout.on("data", (data) => {
  buffer += data.toString();
  const lines = buffer.split("\n");
  buffer = lines.pop(); // keep the last incomplete line for next chunk
  for (const line of lines) {
    if (line.startsWith("bestmove")) {
      console.log(line); // bestmove g1f3
      engine.stdin.write("quit\n");
      engine.kill();
    }
  }
});

const send = (cmd) => engine.stdin.write(cmd + "\n");

send("uci");
send("isready");
send("setoption name Elo value 1500"); // optional, 1 to 9999
send("position startpos moves e2e4 e7e5");
send("go movetime 100");
```

*Rust:*
```rust
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};

fn main() {
    let mut engine = Command::new("OpenSnake.exe") // full path if needed
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("OpenSnake.exe not found");

    let mut stdin = engine.stdin.take().unwrap();
    let stdout = engine.stdout.take().unwrap();

    for cmd in [
        "uci",
        "isready",
        "setoption name Elo value 1500", // optional, 1 to 9999
        "position startpos moves e2e4 e7e5",
        "go movetime 100",
    ] {
        writeln!(stdin, "{}", cmd).unwrap();
        stdin.flush().unwrap();
    }

    for line in BufReader::new(stdout).lines() {
        let line = line.unwrap();
        if line.starts_with("bestmove") {
            println!("{}", line); // bestmove g1f3
            break;
        }
    }

    writeln!(stdin, "quit").unwrap();
    stdin.flush().unwrap();
    engine.wait().unwrap();
}
```

Some stuff to know so u don't get stuck:

- If the exe is not in ur project folder, use the full path (`C:\...\OpenSnake.exe` on Windows)
- Don't send `quit` before reading `bestmove`, otherwise the engine can close before answering
- The engine answer `bestmove 0000` if there is no legal move (mate/stalemate), so check that in ur bot
- For a real game, send `go wtime X btime X winc Y binc Y` with the remaining time in ms, the engine handle the rest
- By default the delay before playing depends on `UCI_Elo` (weak ELO = slower, like a real human). Set `setoption name MoveDelay value 3000` for a fixed 3 s delay, or `setoption name MoveDelayJitter value 1000` to add a random 0–1000 ms range on top. `MoveDelay 0` = back to ELO-based auto
- The delay can never make you lose on time: it's capped at 40% of the remaining time in a timed game, and `go movetime X` always wins over `MoveDelay`. `go infinite` / analyse mode ignore it (they wait for `stop`)

# Cute Chess

U can add OpenSnake as a normal UCI engine, nothing special to install. Example `engines.json`:

```json
[
  {
    "name": "OpenSnake",
    "command": "C:\\...\\OpenSnake.exe",
    "protocol": "uci"
  }
]
```

Then just start a match/tournament from the GUI or CLI (`cutechess-cli -engine cmd=OpenSnake.exe proto=uci -engine cmd=Other.exe proto=uci -each tc=5+0.05`). The engine only answers after `uci`, handles `isready` during search, reacts to `stop`, and sends `bestmove 0000` when there's no legal move. Options exposed: `UCI_Elo` + `UCI_LimitStrength`, `Hash`, `MultiPV`, `UCI_AnalyseMode`, `MoveDelay`, `MoveDelayJitter`.

Self-play against itself, with `baron30.bin` and colors swapped (`-repeat`) so the games don't always start the same way:

```bat
cutechess-cli -engine cmd="OpenSnake.exe" proto=uci name=OpenSnake-A -engine cmd="OpenSnake.exe" proto=uci name=OpenSnake-B -each tc=5+0.3 book="C:\Program Files (x86)\Cute Chess\baron30.bin" bookdepth=8 -games 2 -rounds 1 -repeat -concurrency 1 -pgnout selfplay_OpenSnake.pgn
```

To get an Elo ranking from that PGN, use [Ordo](https://github.com/jhaglund/ordo) (not included in this repo):

```bat
ordo-win64.exe -p selfplay_OpenSnake.pgn -s 1000 -o rating.txt
```

`-s 1000` adds the statistical error margin. Two instances of the same engine should come out at the same rating (écart ≈ 0), but with only a few games the ± error is huge — play more rounds to narrow it.
