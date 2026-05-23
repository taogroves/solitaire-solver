use solitaire_solver::{solve_game, SolveOptions};
use std::io::{self, BufRead};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

fn main() {
    let options = std::env::args()
        .nth(1)
        .and_then(|json| serde_json::from_str::<SolveOptions>(&json).ok())
        .unwrap_or(SolveOptions {
            max_states: 250_000,
            max_moves: 250,
            max_recycles: 15,
            terminate_early: true,
        });
    let threads = std::env::args()
        .nth(2)
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or_else(|| {
            std::thread::available_parallelism()
                .map(usize::from)
                .unwrap_or(8)
        })
        .max(1);

    let deals: Vec<(usize, String, String)> = io::stdin()
        .lock()
        .lines()
        .enumerate()
        .filter_map(|(idx, line)| {
            let line = line.ok()?;
            let (seed, game_string) = line.split_once('\t')?;
            Some((idx, seed.to_string(), game_string.to_string()))
        })
        .collect();

    let deals = Arc::new(deals);
    let cursor = Arc::new(AtomicUsize::new(0));
    let output = Arc::new(Mutex::new(Vec::new()));

    std::thread::scope(|scope| {
        for _ in 0..threads {
            let deals = Arc::clone(&deals);
            let cursor = Arc::clone(&cursor);
            let output = Arc::clone(&output);
            let options = options.clone();
            scope.spawn(move || loop {
                let idx = cursor.fetch_add(1, Ordering::Relaxed);
                let Some((order, seed, game_string)) = deals.get(idx) else {
                    break;
                };
                let started = std::time::Instant::now();
                let response = solve_game(game_string, options.clone());
                let elapsed_ms = started.elapsed().as_millis();
                output.lock().unwrap().push(serde_json::json!({
                    "order": order,
                    "seed": seed,
                    "game_string": game_string,
                    "elapsed_ms": elapsed_ms,
                    "result": response,
                }));
            });
        }
    });

    let mut output = output.lock().unwrap();
    output.sort_by_key(|item| item["order"].as_u64().unwrap_or(u64::MAX));
    for item in output.iter() {
        println!("{}", serde_json::to_string(item).unwrap());
    }
}
