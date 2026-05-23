# Game string format

A **game string** is a lossless encoding of the **initial** Klondike tableau and stock pile before any moves. Foundations start empty and the waste pile is empty. The solver and [solitaire-web](https://github.com/taogroves/solitaire-web) both use this format so a deal can be shared, logged, or solved without replaying a seed.

Two encodings are accepted; the solver normalizes numeric deals to a canonical 156-character form.

## Numeric format (canonical)

**Length:** `52 × 3 = 156` ASCII digits (whitespace is stripped).

Each card is three digits `RRS`:

| Part | Meaning |
|------|---------|
| `RR` | Rank, zero-padded: `01` = Ace … `13` = King |
| `S`  | Suit digit (see table below) |

### Suit digits

| Digit | Suit |
|-------|------|
| `1` | Clubs |
| `2` | Diamonds |
| `3` | Hearts |
| `4` | Spades |

This matches the web app’s `cardDealCode()` in `engine.js`.

### Card labels (alternative view)

When printed as text codes, ranks and suits map as:

| Rank | Code | Suit | Code |
|------|------|------|------|
| Ace | `A` | Hearts | `H` |
| 2–9 | `2`–`9` | Diamonds | `D` |
| 10 | `T` | Clubs | `C` |
| Jack | `J` | Spades | `S` |
| Queen | `Q` | | |
| King | `K` | | |

Example: `122` → rank `12` (Queen), suit `2` (Diamonds) → **`QD`**.

## Where cards appear on the board

The 156 characters are **not** in simple deck order. They list cards in the order they are **read** when building the tableau, then the stock.

### Tableau (28 cards)

Klondike’s triangle is filled **row by row**, left to right:

```
Row 0:  col1
Row 1:  col2  col2
Row 2:  col3  col3  col3
...
Row 6:  col7  col7  col7  col7  col7  col7  col7
```

- Positions 1–7 in the string → top card of columns 1–7 (face **down**).
- Next 6 cards → second row of columns 2–7 (face down), and so on.
- The last card placed in each column is the **bottom** card of that column and is **face up**.

So column `k` receives `k` cards; the string uses `1 + 2 + … + 7 = 28` characters for the tableau.

### Stock (24 cards)

The remaining 24 triplets describe the stock pile in **top-first** order: the next triplet is the card on **top** of the stock (the first card that will be dealt on a draw). The last triplet of the string is the **bottom** of the stock.

After parsing, the solver stores stock internally bottom → top (like a `Vec` you `pop()` from).

### Initial state summary

| Pile | Count | Face-up rule |
|------|-------|----------------|
| Tableau column `k` | `k` cards | Only the bottom card |
| Stock | 24 | All face down |
| Waste | 0 | — |
| Foundations | 0 each | — |

Rules assumed: **draw 3**, unlimited stock recycles (same as solitaire-web).

## Spaced text format (optional)

Instead of 156 digits, you may pass **52 whitespace-separated** card codes, e.g.:

```text
AH 2H 3H ... KS
```

Codes are rank + suit (`AH`, `Td`, `KS`, …). `10` or `T` for ten. This order is **not** the same as the numeric tableau order; it is only useful when you already have a list in that linear order. Prefer the numeric form for deals from the web app or seed tools.

## Example

Reference draw-three deal (from integration tests):

```text
122021053133044042092074131071132062123061011022101013064091114073063082034041014024103121094113102031033134072111084032023052012081112124043104083093051054
```

Visualize it:

```bash
cargo run --example visualize -- '<string above>'
```

## Seeds vs game strings

- A **seed** (any string) drives `seedrandom` in the web app and produces a shuffled deck, then a game string via `getGameString()`.
- A **game string** is the frozen layout; two different seeds could theoretically collide, but in practice you pass game strings to the solver for reproducible analysis.

To obtain a string from a seed in the browser: start a game with that seed and read `engine.getGameString()` from the console, or use the `visualize` example after copying a string from tooling.

## API

| Function | Role |
|----------|------|
| `parse_game_string` | Parse numeric or spaced form → 52-card deck in solver order |
| `deck_to_numeric_string` | Encode a deck back to 156 digits |
| `deal_layout_from_game_string` | Parse and describe tableau + stock for display |
| `solve_game` | Run the solver on a game string |

## Validation errors

The parser rejects:

- Wrong length (after stripping whitespace)
- Invalid rank or suit digit
- Duplicate cards (same rank+suit twice)
- Malformed text codes

Invalid strings return `SolveStatus::Invalid` from `solve_game` with an `error` message.
