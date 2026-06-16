# Tetris — Rust + WebAssembly

A modern-guideline Tetris implementation featuring:

- **Pure Rust core engine** — deterministic and seedable (SRS rotation, 7-bag, hold, ghost, lock delay, T-spins, back-to-back/combo scoring)
- **WebAssembly build** for browser gameplay
- **Responsive Canvas web UI** that works on desktop (keyboard) and mobile (on-screen buttons)
- **AI player** — a heuristic "Smart" bot (El-Tetris evaluation) plus a "Random" bot, watchable at adjustable speed

```
┌──────────────────────────────────────────────────────────┐
│                     Tetris Repository                     │
├──────────────────────────────────────────────────────────┤
│                                                           │
│   ┌──────────┐         ┌──────────────────────────────┐  │
│   │   Core   │         │            Web UI            │  │
│   │  (Rust)  │         │   (TypeScript + Canvas)      │  │
│   └────┬─────┘         └───────────────▲──────────────┘  │
│        │                               │                 │
│        │        ┌──────────┐           │                 │
│        └───────▶│   WASM   │───────────┘                 │
│                 │ (bindgen)│                             │
│                 └──────────┘                             │
└──────────────────────────────────────────────────────────┘
```

## 📁 Repository Structure

```
/tetris
├── Cargo.toml              # Workspace root
├── core/                   # Pure Rust game engine
│   └── src/
│       ├── lib.rs          # Public API
│       ├── rng.rs          # Seedable SplitMix64 PRNG
│       ├── bag.rs          # 7-bag randomizer
│       ├── piece.rs        # Tetrominoes + SRS shapes & wall-kick tables
│       ├── board.rs        # 10x40 playfield, line clears
│       └── game.rs         # Gravity, lock delay, hold, scoring, T-spins
├── wasm/                   # WebAssembly bindings (wasm-bindgen)
│   └── src/lib.rs
├── web/                    # Static web application (Vite + TS)
│   ├── index.html
│   └── src/
│       ├── main.ts         # Game loop, wiring
│       ├── render.ts       # Responsive Canvas renderer
│       ├── input.ts        # Keyboard + on-screen button backends
│       └── style.css
└── .github/workflows/
    ├── ci.yml              # fmt + clippy + tests + web build
    └── deploy.yml          # GitHub Pages deployment
```

## 🚀 Quick Start

### Prerequisites

- **Rust** (stable): https://rustup.rs/
- **wasm-pack**: `cargo install wasm-pack`
- **Node.js** (v18+)

### Run the Web UI (development)

```bash
# 1. Build the WASM module
cd wasm
wasm-pack build --target web --out-dir ../web/src/wasm-pkg

# 2. Start the dev server
cd ../web
npm install
npm run dev          # http://localhost:5173
```

`npm run wasm` (from `web/`) is a shortcut for step 1.

### Build for production

```bash
cd web
npm run wasm && npm run build   # output in web/dist/
```

## 🎮 Controls

**Desktop (keyboard)**

| Key | Action |
| --- | --- |
| ← / → or A / D | Move |
| ↓ or S | Soft drop |
| Space | Hard drop |
| ↑ or X | Rotate CW |
| Z / Ctrl | Rotate CCW |
| C / Shift | Hold |
| P / Esc | Pause |

**Mobile** — on-screen D-pad and rotate/hold/drop buttons appear automatically on touch devices.

A full **[User Guide](./USER_GUIDE.md)** covers the rules, mechanics, and scoring.
The same guide is available in-game via the **？ Help** button.

## 🕹️ Game Rules (modern guideline)

- **SRS** rotation with wall kicks (enables T-spins)
- **7-bag** randomizer (every 7 pieces is one of each)
- **Hold** piece, **ghost** preview, **lock delay**
- Scoring: single/double/triple/Tetris, T-spins, **back-to-back** (+50%), and **combos**
- Speed increases every 10 lines (level up)

## 🧪 Testing

```bash
cargo test --workspace      # core engine tests
cargo fmt --all -- --check  # formatting
cargo clippy --all-targets -- -D warnings
```

## 📖 Core API

```rust
use tetris_core::{Game, Action};

let mut game = Game::new(42);          // seedable & deterministic
game.input(Action::RotateCW);          // discrete inputs
let result = game.tick(16.7);          // advance time (ms); gravity, lock delay
// queries:
game.visible_board();                  // Vec<u8>, 10x20 row-major (0 = empty)
game.active_cells();                   // [(x, y); 4]
game.ghost_cells();
game.score(); game.lines(); game.level();
game.is_done();
```

The engine is **deterministic**: the same `seed` + the same sequence of inputs
and `dt` values always reproduces the same game — handy for replays and any
future RL work (a placement-based `step` API can be layered on `core`).

## 🤖 AI player

The top bar has an **AI** selector (`Off` / `Smart` / `Random`) and a speed slider.

- **Smart** — a heuristic bot in the Rust core. For each piece it enumerates every
  rotation × column placement (plus a one-piece Hold), scores the resulting board
  with the [El-Tetris](https://imake.ninja/el-tetris-an-improvement-on-pierre-dellacheries-algorithm/)
  feature set (landing height, lines cleared, row/column transitions, holes,
  wells), and plays the best one. It keeps a flat, hole-free stack and clears
  lines indefinitely.
- **Random** — drops pieces with random rotation and position; useful as a baseline.

The bot is exposed from the core as `Game::bot_plan()` (and `WasmGame.botPlan()`),
returning the action sequence to reach its chosen placement — so it reuses the
exact same rules the human plays by.

## 🌐 Deploy to GitHub Pages

`.github/workflows/deploy.yml` builds the WASM + web bundle and publishes
`web/dist` to a **`gh-pages` branch** on every push to `main` (via
[JamesIves/github-pages-deploy-action](https://github.com/JamesIves/github-pages-deploy-action)).

One-time setup: in **Settings → Pages**, set the source to **"Deploy from a
branch"** and choose **`gh-pages` / `root`**. The site then serves from
`https://<user>.github.io/tetris/`. The Vite `base` is set to `/tetris/` in
`web/vite.config.ts` — adjust if you rename the repo.

## 📄 License

MIT — see [LICENSE](./LICENSE).
