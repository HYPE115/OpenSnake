## OpenSnake

OpenSnake is an open source chess engine like stockfish (but worst than it use stockfish bro this repo suck tbh)

# Why?

I had found a chess game writen in python but the ai suck, they taked 30 seconds or more to make a move, so I made this to see if it's **THAT** hard to make one

# Key feature

- Scoring system ---> On a scale to 1 up to 100 (plain number only rn, there no way u get a good scaling system rn)
- Best moove finder ---> List the good move ig? (use the scoring system, so maybe i need to improve it with decimal number....)
- Writed in Rust ---> Rust is fast so why to not use it? (probably a bad idea but atleast it's fast if the code doesn't break...)
- Chess ai compatibility ---> Idk how to say that, it's supposed to be an AI chessbot/engine yk. Ima try to be the easier possible to use it in your project for chessbot (don't use it as a cheat you gonna loose elo at this point #_#)
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

Optional but cool: `setoption name Elo value 1500` before the `go` if u want it weaker/stronger (1 to 9999).

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
