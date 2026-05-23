use solitaire_solver::{solve_game, SolveOptions};

#[test]
fn solves_reference_draw_three_deal() {
    let deal = "063101072122011034092104111073071051064024102094023124074123081044133012134061033013114021041112093022062121132082052043054131091014103113042031083032053084";
    let result = solve_game(
        deal,
        SolveOptions {
            max_states: 300_000,
            max_moves: 120,
            max_recycles: 10,
            terminate_early: true,
        },
    );

    assert!(result.solved, "{result:?}");
    assert_eq!(result.move_count, 98);
    assert!(!result.moves.is_empty());
}

#[test]
fn reports_invalid_game_strings() {
    let result = solve_game("not-a-deal", SolveOptions::default());

    assert!(!result.solved);
    assert!(result.error.is_some());
}
