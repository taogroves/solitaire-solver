#[cfg(not(target_arch = "wasm32"))]
use solitaire_solver::{solve_game_parallel, ParallelSolveOptions};
#[cfg(not(target_arch = "wasm32"))]
use std::env;

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    let Some(game_string) = env::args().nth(1) else {
        eprintln!(
            "usage: cargo run --release --example native_parallel_solve -- <game-string> [options-json]"
        );
        std::process::exit(2);
    };

    let options = env::args()
        .nth(2)
        .and_then(|json| serde_json::from_str::<ParallelSolveOptions>(&json).ok())
        .unwrap_or_default();

    let started = std::time::Instant::now();
    let response = solve_game_parallel(&game_string, options);
    let elapsed = started.elapsed();

    println!("{}", serde_json::to_string_pretty(&response).unwrap());
    eprintln!("elapsed_ms={}", elapsed.as_millis());
    if !response.response.solved {
        std::process::exit(1);
    }
}

#[cfg(target_arch = "wasm32")]
fn main() {}
