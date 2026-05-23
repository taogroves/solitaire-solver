use solitaire_solver::{solve_game, SolveOptions};
use std::env;

fn main() {
    let Some(game_string) = env::args().nth(1) else {
        eprintln!(
            "usage: cargo run --example solve -- <game-string>"
        );
        std::process::exit(2);
    };

    let options = env::args()
        .nth(2)
        .and_then(|json| serde_json::from_str::<SolveOptions>(&json).ok())
        .unwrap_or(SolveOptions {
            max_states: 500_000,
            max_moves: 250,
            max_recycles: 15,
            terminate_early: false,
        });

    let response = solve_game(&game_string, options);

    println!("{}", serde_json::to_string_pretty(&response).unwrap());
    if !response.solved {
        std::process::exit(1);
    }
}
