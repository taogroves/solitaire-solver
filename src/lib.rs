use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap, HashSet};
use wasm_bindgen::prelude::*;

const DRAW_COUNT: usize = 3;
const TABLEAU_COUNT: usize = 7;
const DECK_SIZE: usize = 52;
type StateKey = Vec<u8>;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct Card(u8);

impl Card {
    fn new(suit: u8, rank: u8) -> Self {
        debug_assert!(suit < 4);
        debug_assert!((1..=13).contains(&rank));
        Self(suit * 13 + (rank - 1))
    }

    fn suit(self) -> u8 {
        self.0 / 13
    }

    fn rank(self) -> u8 {
        self.0 % 13 + 1
    }

    fn is_red(self) -> bool {
        matches!(self.suit(), 0 | 1)
    }

    fn code(self) -> String {
        let rank = match self.rank() {
            1 => "A".to_string(),
            2..=9 => self.rank().to_string(),
            10 => "T".to_string(),
            11 => "J".to_string(),
            12 => "Q".to_string(),
            13 => "K".to_string(),
            _ => unreachable!(),
        };
        let suit = match self.suit() {
            0 => "H",
            1 => "D",
            2 => "C",
            3 => "S",
            _ => unreachable!(),
        };
        format!("{rank}{suit}")
    }

    fn numeric_code(self) -> String {
        let suit = match self.suit() {
            0 => 3, // hearts
            1 => 2, // diamonds
            2 => 1, // clubs
            3 => 4, // spades
            _ => unreachable!(),
        };
        format!("{:02}{suit}", self.rank())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
struct StackCard {
    card: Card,
    face_up: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Board {
    tableau: [Vec<StackCard>; TABLEAU_COUNT],
    stock: Vec<Card>,
    waste: Vec<Card>,
    foundations: [u8; 4],
    recycles: u8,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SolveStatus {
    Solved,
    Minimal,
    Impossible,
    Unknown,
    Invalid,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SolveOptions {
    #[serde(default = "default_max_states")]
    pub max_states: usize,
    #[serde(default = "default_max_moves")]
    pub max_moves: usize,
    #[serde(default = "default_max_recycles")]
    pub max_recycles: u8,
    #[serde(default)]
    pub terminate_early: bool,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ParallelSolveOptions {
    #[serde(default = "default_parallel_threads")]
    pub threads: usize,
    #[serde(default = "default_parallel_max_states")]
    pub max_states: usize,
    #[serde(default = "default_max_moves")]
    pub max_moves: usize,
    #[serde(default = "default_max_recycles")]
    pub max_recycles: u8,
    #[serde(default = "default_seen_shards")]
    pub seen_shards: usize,
}

#[cfg(not(target_arch = "wasm32"))]
impl Default for ParallelSolveOptions {
    fn default() -> Self {
        Self {
            threads: default_parallel_threads(),
            max_states: default_parallel_max_states(),
            max_moves: default_max_moves(),
            max_recycles: default_max_recycles(),
            seen_shards: default_seen_shards(),
        }
    }
}

impl Default for SolveOptions {
    fn default() -> Self {
        Self {
            max_states: default_max_states(),
            max_moves: default_max_moves(),
            max_recycles: default_max_recycles(),
            terminate_early: false,
        }
    }
}

fn default_max_states() -> usize {
    250_000
}

fn default_max_moves() -> usize {
    250
}

fn default_max_recycles() -> u8 {
    15
}

#[cfg(not(target_arch = "wasm32"))]
fn default_parallel_threads() -> usize {
    std::thread::available_parallelism()
        .map(usize::from)
        .unwrap_or(8)
}

#[cfg(not(target_arch = "wasm32"))]
fn default_parallel_max_states() -> usize {
    20_000_000
}

#[cfg(not(target_arch = "wasm32"))]
fn default_seen_shards() -> usize {
    256
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SolveResponse {
    pub status: SolveStatus,
    pub solved: bool,
    pub minimal: bool,
    pub moves: Vec<String>,
    pub move_count: usize,
    pub states: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_solution_moves: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_solution_states: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub move_gap: Option<usize>,
    pub error: Option<String>,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ParallelSolveResponse {
    #[serde(flatten)]
    pub response: SolveResponse,
    pub threads: usize,
    pub candidates: usize,
    pub shared_seen_entries: usize,
}

impl SolveResponse {
    fn invalid(error: impl Into<String>) -> Self {
        Self {
            status: SolveStatus::Invalid,
            solved: false,
            minimal: false,
            moves: Vec::new(),
            move_count: 0,
            states: 0,
            first_solution_moves: None,
            first_solution_states: None,
            move_gap: None,
            error: Some(error.into()),
        }
    }

    fn with_first_solution(mut self, first: Option<(usize, usize)>, optimal_moves: usize) -> Self {
        if let Some((first_moves, first_states)) = first {
            self.first_solution_moves = Some(first_moves);
            self.first_solution_states = Some(first_states);
            self.move_gap = Some(first_moves.saturating_sub(optimal_moves));
        }
        self
    }
}

#[derive(Clone)]
struct Action {
    text: String,
    cost: usize,
    priority: u8,
}

struct ParentNode {
    parent: Option<usize>,
    action: Option<String>,
}

#[derive(Clone)]
struct QueueEntry {
    estimate: usize,
    cost: usize,
    order: usize,
    parent: Option<usize>,
    action: Option<String>,
    board: Board,
}

impl Ord for QueueEntry {
    fn cmp(&self, other: &Self) -> Ordering {
        // BinaryHeap is max-first, so invert comparisons for a min-priority queue.
        other
            .estimate
            .cmp(&self.estimate)
            .then_with(|| other.cost.cmp(&self.cost))
            .then_with(|| other.order.cmp(&self.order))
    }
}

impl PartialOrd for QueueEntry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for QueueEntry {
    fn eq(&self, other: &Self) -> bool {
        self.estimate == other.estimate && self.cost == other.cost && self.order == other.order
    }
}

impl Eq for QueueEntry {}

impl Board {
    fn from_deck(deck: &[Card]) -> Result<Self, String> {
        if deck.len() != DECK_SIZE {
            return Err(format!("expected {DECK_SIZE} cards, got {}", deck.len()));
        }

        let mut seen = [false; DECK_SIZE];
        for card in deck {
            let idx = usize::from(card.0);
            if seen[idx] {
                return Err(format!("duplicate card {}", card.code()));
            }
            seen[idx] = true;
        }

        let mut tableau: [Vec<StackCard>; TABLEAU_COUNT] = std::array::from_fn(|_| Vec::new());
        let mut idx = 0;
        for (col, pile) in tableau.iter_mut().enumerate() {
            for row in 0..=col {
                pile.push(StackCard {
                    card: deck[idx],
                    face_up: row == col,
                });
                idx += 1;
            }
        }

        Ok(Self {
            tableau,
            stock: deck[idx..].to_vec(),
            waste: Vec::new(),
            foundations: [0; 4],
            recycles: 0,
        })
    }

    fn solved(&self) -> bool {
        self.foundations.iter().all(|&rank| rank == 13)
    }

    fn foundation_total(&self) -> usize {
        self.foundations.iter().map(|&n| usize::from(n)).sum()
    }

    fn heuristic(&self) -> usize {
        DECK_SIZE - self.foundation_total()
    }

    fn face_down_count(&self) -> usize {
        self.tableau
            .iter()
            .flat_map(|pile| pile.iter())
            .filter(|item| !item.face_up)
            .count()
    }

    fn quick_solution_score(&self, cost: usize) -> usize {
        (DECK_SIZE - self.foundation_total()) * 2
            + self.face_down_count() * 8
            + self.stock.len() / DRAW_COUNT
            + usize::from(self.recycles) * 4
            + cost
    }

    fn key(&self) -> StateKey {
        let mut key = Vec::with_capacity(128);
        for rank in self.foundations {
            key.push(rank);
        }
        key.push(0xff);
        key.push(self.recycles);
        key.push(0xff);
        append_cards(&mut key, &self.stock);
        key.push(0xff);
        append_cards(&mut key, &self.waste);
        key.push(0xff);
        for pile in &self.tableau {
            for item in pile {
                key.push(u8::from(item.face_up));
                push_card_key(&mut key, item.card);
            }
            key.push(0xff);
        }
        key
    }

    fn legal_moves(&self, options: &SolveOptions) -> Vec<(Action, Board)> {
        let mut moves = Vec::new();
        self.add_waste_moves(&mut moves);
        self.add_tableau_moves(&mut moves);
        self.add_foundation_moves(&mut moves);
        self.add_talon_moves(&mut moves, options);
        if !options.terminate_early {
            self.add_stock_click_move(&mut moves, options);
        }
        moves.sort_by_key(|(action, _)| (action.priority, action.cost));
        moves
    }

    fn add_waste_moves(&self, moves: &mut Vec<(Action, Board)>) {
        let Some(&card) = self.waste.last() else {
            return;
        };

        if self.can_place_on_foundation(card) {
            let mut next = self.clone();
            next.waste.pop();
            next.foundations[usize::from(card.suit())] += 1;
            moves.push((
                Action {
                    text: format!("W>{}", foundation_name(card.suit())),
                    cost: 1,
                    priority: 1,
                },
                next,
            ));
        }

        for col in 0..TABLEAU_COUNT {
            if self.can_place_on_tableau(card, col) {
                let mut next = self.clone();
                next.waste.pop();
                next.tableau[col].push(StackCard {
                    card,
                    face_up: true,
                });
                moves.push((
                    Action {
                        text: format!("W>T{}", col + 1),
                        cost: 1,
                        priority: 3,
                    },
                    next,
                ));
            }
        }
    }

    fn add_tableau_moves(&self, moves: &mut Vec<(Action, Board)>) {
        for from_col in 0..TABLEAU_COUNT {
            let pile = &self.tableau[from_col];
            let Some(first_face_up) = pile.iter().position(|item| item.face_up) else {
                continue;
            };

            if let Some(top) = pile.last() {
                if top.face_up && self.can_place_on_foundation(top.card) {
                    let reveals_card = pile.len() >= 2 && !pile[pile.len() - 2].face_up;
                    let mut next = self.clone();
                    next.tableau[from_col].pop();
                    next.flip_tableau_top(from_col);
                    next.foundations[usize::from(top.card.suit())] += 1;
                    moves.push((
                        Action {
                            text: format!("T{}>{}", from_col + 1, foundation_name(top.card.suit())),
                            cost: 1,
                            priority: if reveals_card { 0 } else { 1 },
                        },
                        next,
                    ));
                }
            }

            for from_index in first_face_up..pile.len() {
                if !valid_run(&pile[from_index..]) {
                    continue;
                }
                let moving_card = pile[from_index].card;
                for to_col in 0..TABLEAU_COUNT {
                    if from_col == to_col || !self.can_place_on_tableau(moving_card, to_col) {
                        continue;
                    }
                    if self.tableau[to_col].is_empty()
                        && moving_card.rank() == 13
                        && from_index == 0
                    {
                        continue;
                    }
                    let reveals_card = from_index == first_face_up && first_face_up > 0;
                    let mut next = self.clone();
                    let run = next.tableau[from_col].split_off(from_index);
                    next.flip_tableau_top(from_col);
                    next.tableau[to_col].extend(run);
                    moves.push((
                        Action {
                            text: format!("T{}:{}>T{}", from_col + 1, from_index, to_col + 1),
                            cost: 1,
                            priority: if reveals_card { 2 } else { 5 },
                        },
                        next,
                    ));
                }
            }
        }
    }

    fn add_foundation_moves(&self, moves: &mut Vec<(Action, Board)>) {
        for suit in 0..4_u8 {
            let rank = self.foundations[usize::from(suit)];
            if rank == 0 {
                continue;
            }
            if self.foundation_card_is_safe(rank) {
                continue;
            }
            let card = Card::new(suit, rank);
            for col in 0..TABLEAU_COUNT {
                if self.can_place_on_tableau(card, col) {
                    let mut next = self.clone();
                    next.foundations[usize::from(suit)] -= 1;
                    next.tableau[col].push(StackCard {
                        card,
                        face_up: true,
                    });
                    moves.push((
                        Action {
                            text: format!("{}>T{}", foundation_name(suit), col + 1),
                            cost: 1,
                            priority: 4,
                        },
                        next,
                    ));
                }
            }
        }
    }

    fn add_talon_moves(&self, moves: &mut Vec<(Action, Board)>, options: &SolveOptions) {
        let mut cursor = self.clone();
        let mut clicks = 0;
        let mut prefix = String::new();
        let mut seen = HashSet::new();

        loop {
            if cursor.stock.is_empty() {
                if cursor.waste.is_empty() || cursor.recycles >= options.max_recycles {
                    break;
                }
                cursor.stock = cursor.waste.clone();
                cursor.waste.clear();
                cursor.recycles += 1;
                clicks += 1;
                prefix.push('R');
            } else {
                let draw = DRAW_COUNT.min(cursor.stock.len());
                for _ in 0..draw {
                    if let Some(card) = cursor.stock.pop() {
                        cursor.waste.push(card);
                    }
                }
                clicks += 1;
                prefix.push('@');

                if !seen.insert(cursor.talon_key()) {
                    break;
                }
                cursor.add_top_waste_destinations(moves, clicks, &prefix);
            }
        }
    }

    fn add_stock_click_move(&self, moves: &mut Vec<(Action, Board)>, options: &SolveOptions) {
        if !self.stock.is_empty() {
            let mut next = self.clone();
            let draw = DRAW_COUNT.min(next.stock.len());
            for _ in 0..draw {
                if let Some(card) = next.stock.pop() {
                    next.waste.push(card);
                }
            }
            moves.push((
                Action {
                    text: "@".to_string(),
                    cost: 1,
                    priority: 6,
                },
                next,
            ));
        } else if !self.waste.is_empty() && self.recycles < options.max_recycles {
            let mut next = self.clone();
            next.stock = next.waste.clone();
            next.waste.clear();
            next.recycles += 1;
            moves.push((
                Action {
                    text: "R".to_string(),
                    cost: 1,
                    priority: 6,
                },
                next,
            ));
        }
    }

    fn talon_key(&self) -> StateKey {
        let mut key = Vec::with_capacity(56);
        key.push(self.recycles);
        key.push(0xff);
        append_cards(&mut key, &self.stock);
        key.push(0xff);
        append_cards(&mut key, &self.waste);
        key
    }

    fn add_top_waste_destinations(
        &self,
        moves: &mut Vec<(Action, Board)>,
        draw_clicks: usize,
        prefix: &str,
    ) {
        let Some(&card) = self.waste.last() else {
            return;
        };

        if self.can_place_on_foundation(card) {
            let mut next = self.clone();
            next.waste.pop();
            next.foundations[usize::from(card.suit())] += 1;
            moves.push((
                Action {
                    text: format!("{prefix} W>{}", foundation_name(card.suit())),
                    cost: draw_clicks + 1,
                    priority: 1,
                },
                next,
            ));
        }

        for col in 0..TABLEAU_COUNT {
            if self.can_place_on_tableau(card, col) {
                let mut next = self.clone();
                next.waste.pop();
                next.tableau[col].push(StackCard {
                    card,
                    face_up: true,
                });
                moves.push((
                    Action {
                        text: format!("{prefix} W>T{}", col + 1),
                        cost: draw_clicks + 1,
                        priority: 3,
                    },
                    next,
                ));
            }
        }
    }

    fn can_place_on_foundation(&self, card: Card) -> bool {
        self.foundations[usize::from(card.suit())] + 1 == card.rank()
    }

    fn can_place_on_tableau(&self, card: Card, col: usize) -> bool {
        let pile = &self.tableau[col];
        let Some(top) = pile.last() else {
            return card.rank() == 13 && self.leftmost_empty_tableau() == Some(col);
        };
        top.face_up && top.card.is_red() != card.is_red() && top.card.rank() == card.rank() + 1
    }

    fn foundation_card_is_safe(&self, rank: u8) -> bool {
        rank <= 2 || self.foundations.iter().all(|&foundation_rank| foundation_rank >= rank - 2)
    }

    fn leftmost_empty_tableau(&self) -> Option<usize> {
        self.tableau.iter().position(Vec::is_empty)
    }

    fn flip_tableau_top(&mut self, col: usize) {
        if let Some(top) = self.tableau[col].last_mut() {
            top.face_up = true;
        }
    }
}

fn valid_run(run: &[StackCard]) -> bool {
    if run.is_empty() || !run[0].face_up {
        return false;
    }
    for pair in run.windows(2) {
        let upper = pair[0];
        let lower = pair[1];
        if !lower.face_up {
            return false;
        }
        if upper.card.is_red() == lower.card.is_red() || upper.card.rank() != lower.card.rank() + 1
        {
            return false;
        }
    }
    true
}

fn append_cards(key: &mut StateKey, cards: &[Card]) {
    for card in cards {
        push_card_key(key, *card);
    }
}

fn push_card_key(key: &mut StateKey, card: Card) {
    key.push(card.0);
}

fn foundation_name(suit: u8) -> &'static str {
    match suit {
        0 => "FH",
        1 => "FD",
        2 => "FC",
        3 => "FS",
        _ => unreachable!(),
    }
}

pub fn parse_game_string(input: &str) -> Result<Vec<Card>, String> {
    let trimmed = input.trim();
    let compact: String = trimmed.chars().filter(|c| !c.is_whitespace()).collect();
    if compact.len() == DECK_SIZE * 3 && compact.chars().all(|c| c.is_ascii_digit()) {
        return parse_numeric_deal(&compact);
    }

    let parts: Vec<&str> = trimmed.split_whitespace().collect();
    if parts.len() == DECK_SIZE {
        return parts.into_iter().map(parse_card_code).collect();
    }

    Err("expected a 156-character numeric deal string or 52 spaced card codes".to_string())
}

fn parse_numeric_deal(input: &str) -> Result<Vec<Card>, String> {
    let mut encoded = Vec::with_capacity(DECK_SIZE);
    for idx in 0..DECK_SIZE {
        let start = idx * 3;
        let rank = input[start..start + 2]
            .parse::<u8>()
            .map_err(|_| format!("invalid rank at card {}", idx + 1))?;
        let suit_digit = input[start + 2..start + 3]
            .parse::<u8>()
            .map_err(|_| format!("invalid suit at card {}", idx + 1))?;
        let suit = match suit_digit {
            1 => 2, // clubs
            2 => 1, // diamonds
            3 => 0, // hearts
            4 => 3, // spades
            _ => return Err(format!("invalid suit {suit_digit} at card {}", idx + 1)),
        };
        if !(1..=13).contains(&rank) {
            return Err(format!("invalid rank {rank} at card {}", idx + 1));
        }
        encoded.push(Card::new(suit, rank));
    }

    let mut deck = vec![Card::new(0, 1); DECK_SIZE];
    let mut index = 0;
    let mut m = 0;
    for k in 1..=TABLEAU_COUNT {
        let mut j = m;
        for i in k..=TABLEAU_COUNT {
            deck[j] = encoded[index];
            index += 1;
            j += i;
        }
        m += k + 1;
    }

    for i in (DECK_SIZE - 24..DECK_SIZE).rev() {
        deck[i] = encoded[index];
        index += 1;
    }

    Ok(deck)
}

fn parse_card_code(input: &str) -> Result<Card, String> {
    let upper = input.trim().to_ascii_uppercase();
    if upper.len() < 2 || upper.len() > 3 {
        return Err(format!("invalid card code {input}"));
    }
    let (rank_part, suit_part) = upper.split_at(upper.len() - 1);
    let rank = match rank_part {
        "A" => 1,
        "2" => 2,
        "3" => 3,
        "4" => 4,
        "5" => 5,
        "6" => 6,
        "7" => 7,
        "8" => 8,
        "9" => 9,
        "10" | "T" => 10,
        "J" => 11,
        "Q" => 12,
        "K" => 13,
        _ => return Err(format!("invalid rank in {input}")),
    };
    let suit = match suit_part {
        "H" => 0,
        "D" => 1,
        "C" => 2,
        "S" => 3,
        _ => return Err(format!("invalid suit in {input}")),
    };
    Ok(Card::new(suit, rank))
}

pub fn deck_to_numeric_string(deck: &[Card]) -> String {
    let mut encoded = Vec::with_capacity(DECK_SIZE);
    let mut m = 0;
    for k in 1..=TABLEAU_COUNT {
        let mut j = m;
        for i in k..=TABLEAU_COUNT {
            encoded.push(deck[j]);
            j += i;
        }
        m += k + 1;
    }
    for i in (DECK_SIZE - 24..DECK_SIZE).rev() {
        encoded.push(deck[i]);
    }
    encoded.iter().map(|card| card.numeric_code()).collect()
}

pub fn solve_game(input: &str, options: SolveOptions) -> SolveResponse {
    let deck = match parse_game_string(input) {
        Ok(deck) => deck,
        Err(err) => return SolveResponse::invalid(err),
    };
    let board = match Board::from_deck(&deck) {
        Ok(board) => board,
        Err(err) => return SolveResponse::invalid(err),
    };
    solve_board(board, options)
}

fn solve_board(initial: Board, options: SolveOptions) -> SolveResponse {
    let mut best_solution: Option<(usize, Vec<String>, usize)> = None;
    let mut first_solution: Option<(usize, usize)> = None;

    if !options.terminate_early {
        let quick = solve_board(
            initial.clone(),
            SolveOptions {
                terminate_early: true,
                ..options.clone()
            },
        );
        if quick.solved {
            let first_moves = quick.first_solution_moves.unwrap_or(quick.move_count);
            let first_states = quick.first_solution_states.unwrap_or(quick.states);
            first_solution = Some((first_moves, first_states));
            best_solution = Some((quick.move_count, quick.moves, quick.states));
        }
    }

    let mut open = BinaryHeap::new();
    let mut visited: HashMap<StateKey, usize> = HashMap::new();
    let mut parents = Vec::new();

    visited.insert(initial.key(), 0);
    let initial_estimate = if options.terminate_early {
        initial.quick_solution_score(0)
    } else {
        initial.heuristic()
    };
    open.push(QueueEntry {
        estimate: initial_estimate,
        cost: 0,
        order: 0,
        parent: None,
        action: None,
        board: initial,
    });

    let mut states = 0;
    let mut sequence = 1;

    let mut note_first_solution = |moves: usize, seen: usize| {
        first_solution = Some(match first_solution {
            None => (moves, seen),
            Some((best_moves, _)) if moves < best_moves => (moves, seen),
            Some(pair) => pair,
        });
    };

    while let Some(entry) = open.pop() {
        states += 1;
        let current_node = parents.len();
        parents.push(ParentNode {
            parent: entry.parent,
            action: entry.action,
        });
        if !options.terminate_early {
            if let Some((best_cost, best_moves, quick_states)) = &best_solution {
                if entry.estimate >= *best_cost {
                    return SolveResponse {
                        status: SolveStatus::Minimal,
                        solved: true,
                        minimal: true,
                        move_count: *best_cost,
                        moves: best_moves.clone(),
                        states: states + quick_states,
                        first_solution_moves: None,
                        first_solution_states: None,
                        move_gap: None,
                        error: None,
                    }
                    .with_first_solution(first_solution, *best_cost);
                }
            }
        }

        if entry.board.solved() {
            let moves = reconstruct_path(&parents, current_node);
            let move_count = entry.cost;
            note_first_solution(move_count, states);
            return SolveResponse {
                status: if options.terminate_early {
                    SolveStatus::Solved
                } else {
                    SolveStatus::Minimal
                },
                solved: true,
                minimal: !options.terminate_early,
                move_count,
                moves,
                states,
                first_solution_moves: None,
                first_solution_states: None,
                move_gap: None,
                error: None,
            }
            .with_first_solution(first_solution, move_count);
        }

        if states >= options.max_states {
            break;
        }
        if entry.cost >= options.max_moves {
            continue;
        }

        for (action, next) in entry.board.legal_moves(&options) {
            let next_cost = entry.cost + action.cost;
            if next.solved() {
                note_first_solution(next_cost, states);
            }
            let key = next.key();
            if visited.get(&key).is_some_and(|&best| best <= next_cost) {
                continue;
            }
            visited.insert(key, next_cost);

            let estimate = if options.terminate_early {
                next.quick_solution_score(next_cost)
            } else {
                next_cost + next.heuristic()
            };
            if !options.terminate_early
                && best_solution
                    .as_ref()
                    .is_some_and(|(best_cost, _, _)| estimate >= *best_cost)
            {
                continue;
            }
            open.push(QueueEntry {
                estimate,
                cost: next_cost,
                order: sequence,
                parent: Some(current_node),
                action: Some(action.text),
                board: next,
            });
            sequence += 1;
        }
    }

    if !options.terminate_early {
        if let Some((best_cost, best_moves, quick_states)) = best_solution {
            return SolveResponse {
                status: SolveStatus::Solved,
                solved: true,
                minimal: false,
                move_count: best_cost,
                moves: best_moves,
                states: states + quick_states,
                first_solution_moves: None,
                first_solution_states: None,
                move_gap: None,
                error: None,
            }
            .with_first_solution(first_solution, best_cost);
        }
    }

    SolveResponse {
        status: if states < options.max_states {
            SolveStatus::Impossible
        } else {
            SolveStatus::Unknown
        },
        solved: false,
        minimal: false,
        moves: Vec::new(),
        move_count: 0,
        states,
        first_solution_moves: None,
        first_solution_states: None,
        move_gap: None,
        error: None,
    }
}

fn reconstruct_path(parents: &[ParentNode], mut node: usize) -> Vec<String> {
    let mut moves = Vec::new();
    while let Some(parent) = parents[node].parent {
        if let Some(action) = parents[node].action.clone() {
            moves.push(action);
        }
        node = parent;
    }
    moves.reverse();
    moves
}

#[cfg(not(target_arch = "wasm32"))]
mod native_parallel {
    use super::*;
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering as AtomicOrdering};
    use std::sync::{Arc, Mutex};

    struct SharedSeen {
        shards: Vec<Mutex<HashMap<StateKey, usize>>>,
    }

    impl SharedSeen {
        fn new(shard_count: usize) -> Self {
            let count = shard_count.max(1).next_power_of_two();
            Self {
                shards: (0..count).map(|_| Mutex::new(HashMap::new())).collect(),
            }
        }

        fn shard_index(&self, key: &[u8]) -> usize {
            let mut hasher = DefaultHasher::new();
            key.hash(&mut hasher);
            (hasher.finish() as usize) & (self.shards.len() - 1)
        }

        fn visit(&self, key: StateKey, cost: usize) -> bool {
            let idx = self.shard_index(&key);
            let mut shard = self.shards[idx].lock().unwrap();
            if shard.get(&key).is_some_and(|&best| best <= cost) {
                return false;
            }
            shard.insert(key, cost);
            true
        }

        fn len(&self) -> usize {
            self.shards
                .iter()
                .map(|shard| shard.lock().unwrap().len())
                .sum()
        }
    }

    #[derive(Clone)]
    struct StartCandidate {
        action: Action,
        board: Board,
        cost: usize,
        estimate: usize,
        order: usize,
    }

    #[derive(Clone)]
    struct BestSolution {
        cost: usize,
        moves: Vec<String>,
        states: usize,
    }

    struct WorkerContext {
        id: usize,
        options: ParallelSolveOptions,
        seen: Arc<SharedSeen>,
        states: Arc<AtomicUsize>,
        stopped_by_cap: Arc<AtomicBool>,
        best_cost: Arc<AtomicUsize>,
        best_solution: Arc<Mutex<Option<BestSolution>>>,
        starts: Vec<StartCandidate>,
    }

    pub(super) fn solve(input: &str, options: ParallelSolveOptions) -> ParallelSolveResponse {
        let deck = match parse_game_string(input) {
            Ok(deck) => deck,
            Err(err) => {
                return ParallelSolveResponse {
                    response: SolveResponse::invalid(err),
                    threads: 0,
                    candidates: 0,
                    shared_seen_entries: 0,
                }
            }
        };
        let initial = match Board::from_deck(&deck) {
            Ok(board) => board,
            Err(err) => {
                return ParallelSolveResponse {
                    response: SolveResponse::invalid(err),
                    threads: 0,
                    candidates: 0,
                    shared_seen_entries: 0,
                }
            }
        };

        solve_board(initial, options)
    }

    fn solve_board(initial: Board, options: ParallelSolveOptions) -> ParallelSolveResponse {
        let threads = options.threads.max(1);
        let solve_options = SolveOptions {
            max_states: options.max_states,
            max_moves: options.max_moves,
            max_recycles: options.max_recycles,
            terminate_early: false,
        };
        let seen = Arc::new(SharedSeen::new(options.seen_shards));
        seen.visit(initial.key(), 0);

        let quick = super::solve_board(
            initial.clone(),
            SolveOptions {
                terminate_early: true,
                max_states: options.max_states.min(1_000_000).max(250_000),
                max_moves: options.max_moves,
                max_recycles: options.max_recycles,
            },
        );

        let best_cost = Arc::new(AtomicUsize::new(usize::MAX));
        let best_solution = Arc::new(Mutex::new(None::<BestSolution>));
        if quick.solved {
            best_cost.store(quick.move_count, AtomicOrdering::Relaxed);
            *best_solution.lock().unwrap() = Some(BestSolution {
                cost: quick.move_count,
                moves: quick.moves,
                states: quick.states,
            });
        }

        let mut starts = Vec::new();
        for (idx, (action, board)) in initial.legal_moves(&solve_options).into_iter().enumerate() {
            let cost = action.cost;
            let estimate = cost + board.heuristic();
            if estimate >= best_cost.load(AtomicOrdering::Relaxed) {
                continue;
            }
            if seen.visit(board.key(), cost) {
                starts.push(StartCandidate {
                    action,
                    board,
                    cost,
                    estimate,
                    order: idx,
                });
            }
        }

        if initial.solved() {
            return ParallelSolveResponse {
                response: SolveResponse {
                    status: SolveStatus::Minimal,
                    solved: true,
                    minimal: true,
                    moves: Vec::new(),
                    move_count: 0,
                    states: 0,
                    first_solution_moves: Some(0),
                    first_solution_states: Some(0),
                    move_gap: Some(0),
                    error: None,
                },
                threads,
                candidates: 0,
                shared_seen_entries: seen.len(),
            };
        }

        let candidates = starts.len();
        let states = Arc::new(AtomicUsize::new(0));
        let stopped_by_cap = Arc::new(AtomicBool::new(false));
        let mut buckets = vec![Vec::new(); threads];
        for (idx, start) in starts.into_iter().enumerate() {
            buckets[idx % threads].push(start);
        }

        std::thread::scope(|scope| {
            for (id, starts) in buckets.into_iter().enumerate() {
                let ctx = WorkerContext {
                    id,
                    options: options.clone(),
                    seen: Arc::clone(&seen),
                    states: Arc::clone(&states),
                    stopped_by_cap: Arc::clone(&stopped_by_cap),
                    best_cost: Arc::clone(&best_cost),
                    best_solution: Arc::clone(&best_solution),
                    starts,
                };
                scope.spawn(move || worker(ctx));
            }
        });

        let total_states = states.load(AtomicOrdering::Relaxed)
            + best_solution
                .lock()
                .unwrap()
                .as_ref()
                .map(|best| best.states)
                .unwrap_or(0);
        let shared_seen_entries = seen.len();

        let best = best_solution.lock().unwrap().clone();
        let response = match best {
            Some(best) => SolveResponse {
                status: if stopped_by_cap.load(AtomicOrdering::Relaxed) {
                    SolveStatus::Solved
                } else {
                    SolveStatus::Minimal
                },
                solved: true,
                minimal: !stopped_by_cap.load(AtomicOrdering::Relaxed),
                move_count: best.cost,
                moves: best.moves,
                states: total_states,
                first_solution_moves: Some(best.cost),
                first_solution_states: Some(best.states),
                move_gap: Some(0),
                error: None,
            },
            None => SolveResponse {
                status: if stopped_by_cap.load(AtomicOrdering::Relaxed) {
                    SolveStatus::Unknown
                } else {
                    SolveStatus::Impossible
                },
                solved: false,
                minimal: false,
                moves: Vec::new(),
                move_count: 0,
                states: total_states,
                first_solution_moves: None,
                first_solution_states: None,
                move_gap: None,
                error: None,
            },
        };

        ParallelSolveResponse {
            response,
            threads,
            candidates,
            shared_seen_entries,
        }
    }

    fn worker(ctx: WorkerContext) {
        let mut open = BinaryHeap::new();
        let mut parents = vec![ParentNode {
            parent: None,
            action: None,
        }];
        let mut order = ctx.id;

        for start in ctx.starts {
            open.push(QueueEntry {
                estimate: start.estimate,
                cost: start.cost,
                order: start.order,
                parent: Some(0),
                action: Some(start.action.text),
                board: start.board,
            });
        }

        while let Some(entry) = open.pop() {
            let state_idx = ctx.states.fetch_add(1, AtomicOrdering::Relaxed);
            if state_idx >= ctx.options.max_states {
                ctx.stopped_by_cap.store(true, AtomicOrdering::Relaxed);
                return;
            }

            let bound = ctx.best_cost.load(AtomicOrdering::Relaxed);
            if entry.estimate >= bound || entry.cost >= bound {
                continue;
            }

            let current_node = parents.len();
            parents.push(ParentNode {
                parent: entry.parent,
                action: entry.action,
            });

            if entry.board.solved() {
                let moves = reconstruct_path(&parents, current_node);
                update_best(
                    &ctx.best_cost,
                    &ctx.best_solution,
                    BestSolution {
                        cost: entry.cost,
                        moves,
                        states: state_idx + 1,
                    },
                );
                continue;
            }

            if entry.cost >= ctx.options.max_moves {
                continue;
            }

            let solve_options = SolveOptions {
                max_states: ctx.options.max_states,
                max_moves: ctx.options.max_moves,
                max_recycles: ctx.options.max_recycles,
                terminate_early: false,
            };

            for (action, next) in entry.board.legal_moves(&solve_options) {
                let next_cost = entry.cost + action.cost;
                let estimate = next_cost + next.heuristic();
                if estimate >= ctx.best_cost.load(AtomicOrdering::Relaxed) {
                    continue;
                }
                let key = next.key();
                if !ctx.seen.visit(key, next_cost) {
                    continue;
                }
                order += ctx.options.threads;
                open.push(QueueEntry {
                    estimate,
                    cost: next_cost,
                    order,
                    parent: Some(current_node),
                    action: Some(action.text),
                    board: next,
                });
            }
        }
    }

    fn update_best(
        best_cost: &AtomicUsize,
        best_solution: &Mutex<Option<BestSolution>>,
        candidate: BestSolution,
    ) {
        let mut current = best_cost.load(AtomicOrdering::Relaxed);
        while candidate.cost < current {
            match best_cost.compare_exchange(
                current,
                candidate.cost,
                AtomicOrdering::SeqCst,
                AtomicOrdering::Relaxed,
            ) {
                Ok(_) => {
                    *best_solution.lock().unwrap() = Some(candidate);
                    return;
                }
                Err(next) => current = next,
            }
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn solve_game_parallel(input: &str, options: ParallelSolveOptions) -> ParallelSolveResponse {
    native_parallel::solve(input, options)
}

fn parse_options_json(options_json: Option<String>) -> SolveOptions {
    options_json
        .as_deref()
        .filter(|s| !s.trim().is_empty())
        .and_then(|s| serde_json::from_str::<SolveOptions>(s).ok())
        .unwrap_or_default()
}

fn response_json(response: SolveResponse) -> String {
    serde_json::to_string(&response).unwrap_or_else(|err| {
        serde_json::to_string(&SolveResponse::invalid(err.to_string())).unwrap()
    })
}

#[wasm_bindgen]
pub fn solve_game_string(game_string: &str, options_json: Option<String>) -> String {
    response_json(solve_game(game_string, parse_options_json(options_json)))
}

#[wasm_bindgen]
pub fn is_solvable_game_string(game_string: &str, options_json: Option<String>) -> bool {
    solve_game(game_string, parse_options_json(options_json)).solved
}

#[wasm_bindgen]
pub fn normalize_game_string(game_string: &str) -> Result<String, JsValue> {
    parse_game_string(game_string)
        .map(|deck| deck_to_numeric_string(&deck))
        .map_err(|err| JsValue::from_str(&err))
}

#[wasm_bindgen]
pub fn is_solvable_seed(seed: &str, options_json: Option<String>) -> bool {
    // The browser adapter converts app seeds to exact deal strings with the JS engine
    // before calling Rust. If a numeric game string is passed here directly, solve it.
    solve_game(seed, parse_options_json(options_json)).solved
}

#[wasm_bindgen]
pub fn first_solvable_seed(base_seed: &str, options_json: Option<String>) -> String {
    if is_solvable_seed(base_seed, options_json) {
        base_seed.to_string()
    } else {
        String::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ordered_deck_string() -> String {
        let mut cards = Vec::new();
        for suit in [0, 1, 2, 3] {
            for rank in 1..=13 {
                cards.push(Card::new(suit, rank));
            }
        }
        deck_to_numeric_string(&cards)
    }

    fn reference_draw_three_deal() -> &'static str {
        "122021053133044042092074131071132062123061011022101013064091114073063082034041014024103121094113102031033134072111084032023052012081112124043104083093051054"
    }

    #[test]
    fn parses_numeric_deal_strings() {
        let deck = parse_game_string(&ordered_deck_string()).unwrap();
        assert_eq!(deck.len(), DECK_SIZE);
        assert_eq!(deck[0], Card::new(0, 1));
        assert_eq!(deck[51], Card::new(3, 13));
    }

    #[test]
    fn rejects_duplicate_cards() {
        let mut deck = parse_game_string(&ordered_deck_string()).unwrap();
        deck[1] = deck[0];
        let board = Board::from_deck(&deck);
        assert!(board.is_err());
    }

    #[test]
    fn solves_reference_draw_three_game_string() {
        let response = solve_game(
            reference_draw_three_deal(),
            SolveOptions {
                max_states: 300_000,
                max_moves: 120,
                max_recycles: 10,
                terminate_early: true,
            },
        );
        assert!(response.solved, "{response:?}");
        assert!(!response.moves.is_empty());
    }

    #[test]
    fn records_first_discovered_solution_profile() {
        let response = solve_game(
            reference_draw_three_deal(),
            SolveOptions {
                max_states: 300_000,
                max_moves: 120,
                max_recycles: 10,
                terminate_early: true,
            },
        );
        assert!(response.solved, "{response:?}");
        assert!(response.first_solution_moves.is_some());
        assert!(response.first_solution_states.is_some());
        assert!(response.move_gap.is_some());
        assert!(response.move_count <= response.first_solution_moves.unwrap());
    }

    #[test]
    fn caps_unsolved_search_as_unknown() {
        let response = solve_game(
            &ordered_deck_string(),
            SolveOptions {
                max_states: 1,
                max_moves: 5,
                max_recycles: 0,
                terminate_early: true,
            },
        );
        assert_eq!(response.status, SolveStatus::Unknown);
    }
}

#[cfg(all(test, target_arch = "wasm32"))]
mod wasm_tests {
    use super::*;
    use wasm_bindgen_test::*;

    #[wasm_bindgen_test]
    fn normalizes_game_string_export() {
        let input = "122021053133044042092074131071132062123061011022101013064091114073063082034041014024103121094113102031033134072111084032023052012081112124043104083093051054";
        assert_eq!(normalize_game_string(input).unwrap(), input);
    }
}
