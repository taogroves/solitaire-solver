use solitaire_solver::{deal_layout_from_game_string, DealLayout};
use std::env;
use std::io::{self, Read};

fn read_input() -> Result<String, io::Error> {
    let mut buf = String::new();
    io::stdin().read_to_string(&mut buf)?;
    Ok(buf)
}

fn card_token(label: &str, face_up: bool) -> String {
    if face_up {
        format!("[{label:>2}]")
    } else {
        " ## ".to_string()
    }
}

fn print_ascii(layout: &DealLayout) {
    println!("Klondike deal (draw 3, unlimited recycles)\n");

    let max_height = layout
        .tableau
        .iter()
        .map(|col| col.cards.len())
        .max()
        .unwrap_or(0);

    print!("        ");
    for col in &layout.tableau {
        print!(" {:>4} ", col.index);
    }
    println!();

    for row in 0..max_height {
        print!("        ");
        for col in &layout.tableau {
            if row < col.cards.len() {
                let card = &col.cards[row];
                print!("{} ", card_token(&card.label, card.face_up));
            } else {
                print!("      ");
            }
        }
        println!();
    }

    println!("\nTableau (bottom → top per column; ↑ = face up):");
    for col in &layout.tableau {
        let parts: Vec<String> = col
            .cards
            .iter()
            .rev()
            .map(|c| {
                if c.face_up {
                    format!("{}↑", c.label)
                } else {
                    "??".to_string()
                }
            })
            .collect();
        println!("  col {}: {}", col.index, parts.join(" → "));
    }

    let stock = &layout.stock;
    println!("\nStock ({} cards, bottom → top):", stock.len());
    if stock.is_empty() {
        println!("  (empty)");
    } else if stock.len() <= 12 {
        println!("  {}", stock.join(" → "));
    } else {
        let head: Vec<_> = stock.iter().take(6).map(String::as_str).collect();
        let tail: Vec<_> = stock
            .iter()
            .skip(stock.len() - 6)
            .map(String::as_str)
            .collect();
        println!(
            "  {} → … ({} hidden) … → {}",
            head.join(" → "),
            stock.len() - 12,
            tail.join(" → ")
        );
    }
    if let Some(top) = stock.last() {
        println!("  (top of stock / first draw: {top})");
    }

    println!("\nCanonical numeric string ({} chars):", layout.numeric_string.len());
    println!("{}", layout.numeric_string);
}

fn main() {
    let mut json_mode = false;
    let mut positional = Vec::new();
    for arg in env::args().skip(1) {
        if arg == "--json" {
            json_mode = true;
        } else {
            positional.push(arg);
        }
    }

    let game_string = match positional.len() {
        0 => {
            eprintln!("Reading game string from stdin…");
            match read_input() {
                Ok(s) => s,
                Err(err) => {
                    eprintln!("failed to read stdin: {err}");
                    std::process::exit(2);
                }
            }
        }
        1 if positional[0] == "-" => match read_input() {
            Ok(s) => s,
            Err(err) => {
                eprintln!("failed to read stdin: {err}");
                std::process::exit(2);
            }
        },
        1 => positional[0].clone(),
        _ => {
            eprintln!(
                "usage: cargo run --example visualize -- [--json] <game-string|->\n\
                 \n\
                 Print the initial tableau and stock for a 156-digit numeric deal\n\
                 (or 52 spaced card codes). Use - or no argument to read from stdin.\n\
                 \n\
                 See docs/GAME_STRING.md for format details."
            );
            std::process::exit(2);
        }
    };

    let layout = match deal_layout_from_game_string(&game_string) {
        Ok(layout) => layout,
        Err(err) => {
            eprintln!("invalid game string: {err}");
            std::process::exit(1);
        }
    };

    if json_mode {
        println!("{}", serde_json::to_string_pretty(&layout).unwrap());
    } else {
        print_ascii(&layout);
    }
}
