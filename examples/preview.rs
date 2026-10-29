//! Deterministic renderer preview, not a screenshot of a live multiplayer match.
//! cargo run --example preview > docs/renderer-preview.svg
use hollow_signal::{
    game::Game,
    render::{Ui, draw},
};
use ratatui::{Terminal, backend::TestBackend, style::Color};

fn css(c: Color) -> String {
    match c {
        Color::Rgb(r, g, b) => format!("#{r:02x}{g:02x}{b:02x}"),
        _ => "#0a1010".into(),
    }
}
fn main() {
    let mut g = Game::new();
    g.add(1, "Karthik", 0);
    g.add(2, "Echo", 1);
    g.add(3, "Vesper", 2);
    let mut s = g.snapshot();
    s.remaining_ms = 187_000;
    s.feed.clear();
    s.players[0].x = 8.5;
    s.players[0].y = 10.5;
    s.players[0].angle = 0.0;
    s.players[0].kills = 4;
    s.players[0].deaths = 2;
    s.players[0].protection_ms = 0;
    s.players[1].x = 12.8;
    s.players[1].y = 10.7;
    s.players[1].protection_ms = 0;
    s.players[2].x = 17.4;
    s.players[2].y = 9.8;
    s.players[2].protection_ms = 0;
    let mut t = Terminal::new(TestBackend::new(140, 44)).unwrap();
    t.draw(|f| {
        draw(
            f,
            Some(&s),
            1,
            &Ui {
                time: 14.0,
                calm: false,
                scores: false,
                help: false,
                net_age: 0.0,
                server: "192.168.1.10:35353",
                precise: true,
            },
        )
    })
    .unwrap();
    println!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"1260\" height=\"792\" viewBox=\"0 0 1260 792\"><rect width=\"1260\" height=\"792\" fill=\"#090e0f\"/>"
    );
    for y in 0..44 {
        for x in 0..140 {
            let c = &t.backend().buffer()[(x, y)];
            let px = x * 9;
            let py = y * 18;
            println!(
                "<rect x=\"{px}\" y=\"{py}\" width=\"9\" height=\"18\" fill=\"{}\"/>",
                css(c.bg)
            );
            if c.symbol() == "▀" {
                println!(
                    "<rect x=\"{px}\" y=\"{py}\" width=\"9\" height=\"9\" fill=\"{}\"/>",
                    css(c.fg)
                );
            } else if c.symbol() != " " {
                let text = c
                    .symbol()
                    .replace('&', "&amp;")
                    .replace('<', "&lt;")
                    .replace('>', "&gt;");
                println!(
                    "<text x=\"{px}\" y=\"{}\" font-family=\"monospace\" font-size=\"15\" fill=\"{}\">{text}</text>",
                    py + 14,
                    css(c.fg)
                );
            }
        }
    }
    println!("</svg>");
}
