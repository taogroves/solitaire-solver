# Klondike Solitaire Solver

Perfect-information **draw-three Klondike** solver used by the [solitaire-web](../solitaire-web) app. The crate compiles to native Rust (examples, tests, seed-library tooling) and to WebAssembly via `wasm-pack`.

## Game string format

Deals are encoded as a **156-digit numeric string** (52 cards × 3 digits each: rank + suit). The encoding captures the initial tableau triangle and stock pile order used by the web app.

Full specification: **[docs/GAME_STRING.md](docs/GAME_STRING.md)**.

```bash
# Pretty-print a deal in the terminal
cargo run --example visualize -- '<156-character-game-string>'

# Machine-readable layout
cargo run --example visualize -- --json '<game-string>'
```

## Build and test

```bash
cargo test
cargo run --example solve -- '<156-character-game-string>'
```

## WebAssembly for the web app

Install [wasm-pack](https://rustwasm.github.io/wasm-pack/), then:

```bash
wasm-pack build . --target web --out-dir ../solitaire-web/wasm/pkg
```

## Examples

| Example | Purpose |
|---------|---------|
| `visualize` | Print initial tableau/stock for a game string |
| `solve` | Solve one game string (JSON result) |
| `profile_deals` | Batch profile deals for difficulty metrics |
| `find_seed_library` | Build curated seed libraries |
| `native_parallel_solve` | Parallel native solve benchmark |

## License

MIT
