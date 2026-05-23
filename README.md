# Klondike Solitaire Solver

Perfect-information **draw-three Klondike** solver used by the [solitaire-web](../solitaire-web) app. The crate compiles to native Rust (examples, tests, seed-library tooling) and to WebAssembly via `wasm-pack`.

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
| `solve` | Solve one MinimalKlondike-style game string |
| `profile_deals` | Batch profile deals for difficulty metrics |
| `find_seed_library` | Build curated seed libraries |
| `native_parallel_solve` | Parallel native solve benchmark |

## License

MIT
