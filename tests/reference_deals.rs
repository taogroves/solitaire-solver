use solitaire_solver::{solve_game, SolveOptions};

#[test]
fn solves_reference_draw_three_deal() {
    let deal = "122021053133044042092074131071132062123061011022101013064091114073063082034041014024103121094113102031033134072111084032023052012081112124043104083093051054";
    let result = solve_game(
        deal,
        SolveOptions {
            max_states: 300_000,
            max_moves: 250,
            max_recycles: 10,
            terminate_early: true,
        },
    );

    assert!(result.solved, "{result:?}");
    assert!(!result.moves.is_empty());
}

#[test]
fn reports_invalid_game_strings() {
    let result = solve_game("not-a-deal", SolveOptions::default());

    assert!(!result.solved);
    assert!(result.error.is_some());
}
