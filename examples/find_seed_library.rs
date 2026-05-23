use serde::Serialize;
use solitaire_solver::{solve_game, SolveOptions, SolveResponse};
use std::io::{self, BufRead};
use std::sync::{Arc, Mutex};
use std::time::Instant;

const TARGET_PER_TIER: usize = 10;
const TIERS: [&str; 4] = ["easy", "medium", "hard", "expert"];

#[derive(Clone)]
struct Candidate {
    order: usize,
    seed: String,
    game_string: String,
}

#[derive(Clone, Serialize)]
struct LibraryHit {
    seed: String,
    score: f64,
    moves: usize,
    states: usize,
    first_states: usize,
    move_gap: usize,
    elapsed_ms: u128,
}

#[derive(Serialize)]
struct LibraryOutput {
    elapsed_ms: u128,
    candidates_started: usize,
    candidates_solved: usize,
    buckets: [Vec<LibraryHit>; 4],
}

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

    let candidates: Vec<Candidate> = io::stdin()
        .lock()
        .lines()
        .enumerate()
        .filter_map(|(order, line)| {
            let line = line.ok()?;
            let (seed, game_string) = line.split_once('\t')?;
            Some(Candidate {
                order,
                seed: seed.to_string(),
                game_string: game_string.to_string(),
            })
        })
        .collect();

    let started = Instant::now();
    let work = Arc::new(Mutex::new(candidates.into_iter()));
    let buckets = Arc::new(Mutex::new(std::array::from_fn::<_, 4, _>(|_| Vec::new())));
    let stats = Arc::new(Mutex::new((0usize, 0usize)));

    std::thread::scope(|scope| {
        for _ in 0..threads {
            let options = options.clone();
            let work = Arc::clone(&work);
            let buckets = Arc::clone(&buckets);
            let stats = Arc::clone(&stats);
            scope.spawn(move || loop {
                if full(&buckets.lock().unwrap()) {
                    break;
                }

                let Some(candidate) = work.lock().unwrap().next() else {
                    break;
                };
                stats.lock().unwrap().0 += 1;

                let deal_started = Instant::now();
                let response = solve_game(&candidate.game_string, options.clone());
                let elapsed_ms = deal_started.elapsed().as_millis();
                if !response.solved {
                    continue;
                }
                stats.lock().unwrap().1 += 1;

                let Some((tier_idx, hit)) = make_hit(candidate, response, elapsed_ms) else {
                    continue;
                };
                let mut buckets = buckets.lock().unwrap();
                if buckets[tier_idx].len() < TARGET_PER_TIER {
                    buckets[tier_idx].push(hit);
                    eprintln!(
                        "found {} {}/{}",
                        TIERS[tier_idx],
                        buckets[tier_idx].len(),
                        TARGET_PER_TIER
                    );
                }
            });
        }
    });

    let (candidates_started, candidates_solved) = *stats.lock().unwrap();
    let mut buckets = buckets.lock().unwrap().clone();
    for bucket in &mut buckets {
        bucket.sort_by(|a, b| a.score.total_cmp(&b.score));
        bucket.truncate(TARGET_PER_TIER);
    }

    println!(
        "{}",
        serde_json::to_string_pretty(&LibraryOutput {
            elapsed_ms: started.elapsed().as_millis(),
            candidates_started,
            candidates_solved,
            buckets,
        })
        .unwrap()
    );
}

fn full(buckets: &[Vec<LibraryHit>; 4]) -> bool {
    buckets.iter().all(|bucket| bucket.len() >= TARGET_PER_TIER)
}

fn make_hit(
    candidate: Candidate,
    response: SolveResponse,
    elapsed_ms: u128,
) -> Option<(usize, LibraryHit)> {
    let moves = response.move_count;
    let first_moves = response.first_solution_moves.unwrap_or(moves);
    let first_states = response.first_solution_states.unwrap_or(response.states);
    let move_gap = response
        .move_gap
        .unwrap_or_else(|| first_moves.saturating_sub(moves));
    let score = difficulty_score(moves, first_states, move_gap);
    let tier_idx = if score < 0.36 {
        0
    } else if score < 0.60 {
        1
    } else if score < 0.82 {
        2
    } else {
        3
    };
    Some((
        tier_idx,
        LibraryHit {
            seed: candidate.seed,
            score,
            moves,
            states: response.states,
            first_states,
            move_gap,
            elapsed_ms: elapsed_ms + candidate.order as u128 * 0,
        },
    ))
}

fn difficulty_score(solution_moves: usize, first_states: usize, move_gap: usize) -> f64 {
    let clamp = |value: f64| value.clamp(0.0, 1.0);
    let move_score = clamp((solution_moves as f64 - 75.0) / 70.0);
    let obscure_score = clamp(((first_states as f64) + 1.0).log10() / 4.2);
    let gap_score = clamp(move_gap as f64 / 25.0);
    0.45 * move_score + 0.35 * obscure_score + 0.2 * gap_score
}
