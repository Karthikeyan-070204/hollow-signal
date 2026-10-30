//! CPU raycaster. Each terminal cell holds TWO RGB pixels using the '▀' glyph.
use crate::{protocol::*, world};
use ratatui::{
    Frame,
    buffer::Buffer,
    layout::{Alignment, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Widget},
};

type Rgb = (u8, u8, u8);
const BG: Color = Color::Rgb(9, 14, 15);
const INK: Color = Color::Rgb(192, 204, 185);
const DIM: Color = Color::Rgb(104, 125, 113);
const ACCENT: Color = Color::Rgb(191, 214, 117);
const FOG: Rgb = (30, 43, 42);
const PLANE: f32 = 0.72;
pub struct Ui<'a> {
    pub time: f32,
    pub calm: bool,
    pub scores: bool,
    pub help: bool,
    pub net_age: f32,
    pub server: &'a str,
    pub precise: bool,
}

fn rgb(c: Rgb) -> Color {
    Color::Rgb(c.0, c.1, c.2)
}
fn scale(c: Rgb, k: f32) -> Rgb {
    (
        (c.0 as f32 * k).clamp(0.0, 255.0) as u8,
        (c.1 as f32 * k).clamp(0.0, 255.0) as u8,
        (c.2 as f32 * k).clamp(0.0, 255.0) as u8,
    )
}
fn mix(a: Rgb, b: Rgb, t: f32) -> Rgb {
    let t = t.clamp(0.0, 1.0);
    (
        (a.0 as f32 + (b.0 as f32 - a.0 as f32) * t) as u8,
        (a.1 as f32 + (b.1 as f32 - a.1 as f32) * t) as u8,
        (a.2 as f32 + (b.2 as f32 - a.2 as f32) * t) as u8,
    )
}
fn fog(c: Rgb, d: f32) -> Rgb {
    mix(c, FOG, 1.0 - (-0.19 * d).exp())
}
fn noise(x: i32, y: i32) -> f32 {
    let n = (x as u32).wrapping_mul(374761393) ^ (y as u32).wrapping_mul(668265263);
    ((n ^ (n >> 13)).wrapping_mul(1274126177) & 255) as f32 / 255.0
}

pub struct Scene {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<Rgb>,
}
impl Scene {
    fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            pixels: vec![FOG; width * height],
        }
    }
    fn set(&mut self, x: i32, y: i32, c: Rgb) {
        if x >= 0 && y >= 0 && (x as usize) < self.width && (y as usize) < self.height {
            self.pixels[y as usize * self.width + x as usize] = c;
        }
    }
    pub fn cast(
        width: usize,
        height: usize,
        me: &PlayerView,
        snapshot: &Snapshot,
        ui: &Ui<'_>,
    ) -> Self {
        let mut scene = Self::new(width, height);
        if width == 0 || height == 0 {
            return scene;
        }
        let w = width as f32;
        let h = height as f32;
        let horizon = h * 0.5;
        let projection = w / (2.0 * PLANE);
        let (sin, cos) = me.angle.sin_cos();
        let mut depths = vec![world::FAR; width];
        for (x, depth) in depths.iter_mut().enumerate() {
            let camera = 2.0 * (x as f32 + 0.5) / w - 1.0;
            let dx = cos - sin * PLANE * camera;
            let dy = sin + cos * PLANE * camera;
            let hit = world::raycast(me.x, me.y, dx, dy);
            *depth = hit.distance;
            let wall_scale = 2.0;
            let wall_height = (projection / hit.distance) * wall_scale;
            let top = horizon - wall_height * 0.5;
            let bottom = horizon + wall_height * 0.5;
            for y in 0..height {
                let py = y as f32 + 0.5;
                let mut c = if py >= top
                    && py <= bottom
                    && hit.distance < world::FAR
                    && hit.material != 0
                {
                    let v = ((py - top) / wall_height).clamp(0.0, 1.0);
                    let tx = (hit.u * 32.0) as i32;
                    let ty = (v * 48.0) as i32;
                    let base = match hit.material {
                        2 => (109, 57, 49),
                        3 => (74, 107, 92),
                        _ => (106, 113, 97),
                    };
                    let mortar = ty % 12 == 0 || (tx + (ty / 12 % 2) * 16) % 32 == 0;
                    let streak = if noise(tx / 2, 3) > 0.83 { 0.76 } else { 1.0 };
                    let light = if hit.side { 0.7 } else { 1.0 };
                    let mut texture = scale(
                        base,
                        (0.78 + noise(tx, ty) * 0.28)
                            * light
                            * streak
                            * if mortar { 0.53 } else { 1.0 },
                    );
                    if hit.material == 3 && (ty == 8 || ty == 9) {
                        texture = (122, 179, 139);
                    }
                    // A dark skirting rail and vertical red stains anchor the texture.
                    if v > 0.91 {
                        texture = scale(texture, 0.36);
                    }
                    fog(texture, hit.distance)
                } else {
                    let floor = py >= horizon;
                    let distance = (0.5 * projection / (py - horizon).abs().max(0.5)).min(70.0);
                    let wx = me.x + dx * distance;
                    let wy = me.y + dy * distance;
                    let tx = (wx * 12.0).floor() as i32;
                    let ty = (wy * 12.0).floor() as i32;
                    let checker = (wx.floor() as i32 + wy.floor() as i32).rem_euclid(2) == 0;
                    let base = if floor {
                        if checker { (57, 63, 52) } else { (45, 52, 46) }
                    } else {
                        (24, 31, 30)
                    };
                    let seam = if floor && (tx.rem_euclid(12) == 0 || ty.rem_euclid(12) == 0) {
                        0.65
                    } else {
                        1.0
                    };
                    fog(scale(base, (0.83 + noise(tx, ty) * 0.2) * seam), distance)
                };
                // Slow, low-contrast haze; no full-screen flashes or shaking.
                if !ui.calm {
                    let drift =
                        ((py * 0.07 + ui.time * 0.18 + camera * 1.3).sin() * 0.5 + 0.5) * 0.025;
                    c = mix(c, (74, 94, 81), drift);
                }
                scene.set(x as i32, y as i32, c);
            }
        }
        let mut people: Vec<_> = snapshot
            .players
            .iter()
            .filter(|p| p.id != me.id && p.hp > 0)
            .collect();
        people.sort_by(|a, b| {
            let da = (a.x - me.x).hypot(a.y - me.y);
            let db = (b.x - me.x).hypot(b.y - me.y);
            db.total_cmp(&da)
        });
        for p in people {
            let rx = p.x - me.x;
            let ry = p.y - me.y;
            let z = rx * cos + ry * sin;
            let side = -rx * sin + ry * cos;
            if z <= 0.1 || z > world::FAR {
                continue;
            }
            let cx = w * 0.5 + side / z * projection;
            let sh = (projection / z * 0.88).min(h * 4.0);
            let sw = sh * 0.5;
            let top = horizon + projection / z * 0.5 - sh;
            let left = cx - sw * 0.5;
            let color = COLORS[p.color as usize % 9];
            for x in (left.floor() as i32).max(0)..((left + sw).ceil() as i32).min(width as i32) {
                if z > depths[x as usize] + 0.01 {
                    continue;
                }
                for y in (top.floor() as i32).max(0)..((top + sh).ceil() as i32).min(height as i32)
                {
                    let sx = ((x as f32 - left) / sw * 12.0).clamp(0.0, 11.0) as usize;
                    let sy = ((y as f32 - top) / sh * 24.0).clamp(0.0, 23.0) as usize;
                    let symbol = CHARACTER[sy].as_bytes()[sx];
                    let base = match symbol {
                        b'c' => color,
                        b'd' => scale(color, 0.45),
                        b'b' => (24, 29, 27),
                        b'f' => (177, 180, 147),
                        b'e' => {
                            if p.protection_ms > 0 {
                                (172, 231, 222)
                            } else {
                                (217, 190, 109)
                            }
                        }
                        b'g' => (67, 75, 70),
                        _ => continue,
                    };
                    let mut c = fog(base, z);
                    if p.protection_ms > 0 && symbol == b'c' {
                        c = mix(c, (106, 192, 186), 0.35);
                    }
                    scene.set(x, y, c);
                }
            }
        }
        // Soft peripheral darkness is fixed in place, not a camera shake.
        for y in 0..height {
            for x in 0..width {
                let nx = (x as f32 / w - 0.5) * 2.0;
                let ny = (y as f32 / h - 0.5) * 2.0;
                let edge = ((nx * nx + ny * ny) * 0.24).min(0.52);
                let i = y * width + x;
                scene.pixels[i] = scale(scene.pixels[i], 1.0 - edge);
                if me.hp > 0 && me.hp < 35 {
                    scene.pixels[i] = mix(scene.pixels[i], (74, 23, 24), edge * 0.75);
                }
            }
        }
        if me.hp > 0 {
            scene.weapon(me, ui);
            let cross = if me.hit_ms > 0 {
                (225, 154, 82)
            } else {
                (175, 194, 161)
            };
            let (cx, cy) = (width as i32 / 2, height as i32 / 2);
            if me.hit_ms > 0 {
                for (dx, dy) in [(-2, -2), (2, -2), (-2, 2), (2, 2)] {
                    scene.set(cx + dx, cy + dy, cross);
                }
            } else {
                for (dx, dy) in [(-2, 0), (2, 0), (0, -2), (0, 2)] {
                    scene.set(cx + dx, cy + dy, cross);
                }
            }
        }
        scene
    }
    fn weapon(&mut self, p: &PlayerView, ui: &Ui<'_>) {
        let scale = ((self.height as f32 / 42.0).floor() as i32).clamp(1, 3);
        let w = WEAPON[0].len() as i32;
        let x0 = self.width as i32 / 2 - w * scale / 2;
        let lower = if p.reload_ms > 0 { 4 * scale } else { 0 };
        let y0 = self.height as i32 - WEAPON.len() as i32 * scale + lower;
        for (y, row) in WEAPON.iter().enumerate() {
            for (x, ch) in row.bytes().enumerate() {
                let c = match ch {
                    b'#' => (59, 67, 63),
                    b'+' => (123, 134, 119),
                    b'=' => (33, 40, 38),
                    b'c' => COLORS[p.color as usize % 9],
                    b'h' => (94, 83, 65),
                    _ => continue,
                };
                for dy in 0..scale {
                    for dx in 0..scale {
                        self.set(x0 + x as i32 * scale + dx, y0 + y as i32 * scale + dy, c);
                    }
                }
            }
        }
        if p.flash_ms > 0 && !ui.calm {
            let cx = self.width as i32 / 2;
            for dy in -5_i32..0 {
                for dx in -4_i32..=4 {
                    if dx.abs() + dy.abs() < 7 {
                        self.set(cx + dx, y0 + dy, (215, 185, 117));
                    }
                }
            }
        }
    }
}
impl Widget for Scene {
    fn render(self, area: Rect, buf: &mut Buffer) {
        for y in 0..area.height as usize {
            for x in 0..area.width as usize {
                if x >= self.width || y * 2 + 1 >= self.height {
                    continue;
                }
                buf[(area.x + x as u16, area.y + y as u16)]
                    .set_symbol("▀")
                    .set_fg(rgb(self.pixels[y * 2 * self.width + x]))
                    .set_bg(rgb(self.pixels[(y * 2 + 1) * self.width + x]));
            }
        }
    }
}

const CHARACTER: [&str; 24] = [
    "....bbbb....",
    "...bbbbbb...",
    "...bffffb...",
    "...beffeb...",
    "...bffffb...",
    "....bffb....",
    "...cccccc...",
    "..cccccccc..",
    ".ddccccccdd.",
    ".ddccccccdd.",
    ".ddccccccgg.",
    ".ddcccggggg.",
    ".ddcccggggg.",
    ".ddccccccdd.",
    "..dccccccd..",
    "...bbbbbb...",
    "...bbbbbb...",
    "...dd..dd...",
    "...dd..dd...",
    "...dd..dd...",
    "...dd..dd...",
    "...dd..dd...",
    "..bbb..bbb..",
    "..bbb..bbb..",
];
const WEAPON: [&str; 18] = [
    "         ++         ",
    "        +##+        ",
    "        +==+        ",
    "        +==+        ",
    "        +==+        ",
    "       +####+       ",
    "       +#cc#+       ",
    "       +####+       ",
    "      +######+      ",
    "      +##==##+      ",
    "     +########+     ",
    "     +##====##+     ",
    "    hh+######+hh    ",
    "   hhh+######+hhh   ",
    "  hhhh+######+hhhh  ",
    "  hhhh+######+hhhh  ",
    " hhhhh+######+hhhhh ",
    "hhhhhh+######+hhhhhh",
];

fn center(frame: &mut Frame, text: impl Into<String>, y: u16, color: Color) {
    let a = frame.area();
    frame.render_widget(
        Paragraph::new(text.into())
            .style(Style::default().fg(color).bg(BG))
            .alignment(Alignment::Center),
        Rect::new(a.x, y, a.width, 1),
    );
}
fn modal(frame: &mut Frame, width: u16, height: u16, title: &str, lines: Vec<Line<'static>>) {
    let a = frame.area();
    let w = width.min(a.width.saturating_sub(4));
    let h = height.min(a.height.saturating_sub(4));
    let rect = Rect::new(a.x + (a.width - w) / 2, a.y + (a.height - h) / 2, w, h);
    frame.render_widget(Clear, rect);
    frame.render_widget(
        Paragraph::new(lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(title.to_owned())
                    .border_style(Style::default().fg(DIM)),
            )
            .style(Style::default().fg(INK).bg(BG)),
        rect,
    );
}
pub fn draw(frame: &mut Frame, snapshot: Option<&Snapshot>, id: u32, ui: &Ui<'_>) {
    let a = frame.area();
    frame.render_widget(Block::default().style(Style::default().bg(BG).fg(INK)), a);
    if a.width < 60 || a.height < 22 {
        frame.render_widget(
            Paragraph::new("HOLLOW SIGNAL\nResize to at least 60 columns x 22 rows.\nEsc exits.")
                .style(Style::default().fg(ACCENT)),
            a,
        );
        return;
    }
    let Some(s) = snapshot else {
        center(frame, "CONNECTING TO THE SIGNAL ...", a.height / 2, ACCENT);
        return;
    };
    let Some(me) = s.players.iter().find(|p| p.id == id) else {
        center(frame, "WAITING FOR YOUR SIGNAL ...", a.height / 2, ACCENT);
        return;
    };
    let seconds = s.remaining_ms.div_ceil(1000);
    let title = "  H O L L O W  S I G N A L     /     WARD 09";
    let status = format!(
        "ROUND {:02}   {:02}:{:02}   {}/8 SIGNALS  ",
        s.round,
        seconds / 60,
        seconds % 60,
        s.players.len()
    );
    frame.render_widget(
        Paragraph::new(title).style(Style::default().fg(ACCENT)),
        Rect::new(a.x, a.y, a.width, 1),
    );
    frame.render_widget(
        Paragraph::new(status)
            .alignment(Alignment::Right)
            .style(Style::default().fg(INK)),
        Rect::new(a.x, a.y + 1, a.width, 1),
    );
    frame.render_widget(
        Paragraph::new(format!(
            "  {}  /  {}",
            me.name,
            COLOR_NAMES[me.color as usize % 9]
        ))
        .style(Style::default().fg(rgb(COLORS[me.color as usize % 9]))),
        Rect::new(a.x, a.y + 1, a.width / 2, 1),
    );
    let vw = a.width.min(240);
    let vh = (a.height - 7).min(90);
    let view = Rect::new(a.x + (a.width - vw) / 2, a.y + 3, vw, vh);
    frame.render_widget(Scene::cast(vw as usize, vh as usize * 2, me, s, ui), view);
    let bottom = a.y + a.height - 3;
    let health_color = if me.hp < 35 {
        Color::Rgb(224, 113, 93)
    } else {
        ACCENT
    };
    let ammo = if me.reload_ms > 0 {
        format!("RELOADING {:.1}s", me.reload_ms as f32 / 1000.0)
    } else {
        format!("AMMO {:02}/08", me.ammo)
    };
    let health = format!("  VITAL {:03}  ", me.hp);
    let shield = if me.protection_ms > 0 && me.hp > 0 {
        "  SHIELDED"
    } else {
        ""
    };
    let hud = Line::from(vec![
        Span::styled(health, Style::default().fg(health_color)),
        Span::raw(format!(
            "|  {ammo}  |  KILLS {:02}  DEATHS {:02}{shield}",
            me.kills, me.deaths
        )),
    ]);
    frame.render_widget(
        Paragraph::new(hud).style(Style::default().fg(INK)),
        Rect::new(a.x, bottom, a.width, 1),
    );
    let message = if ui.net_age > 0.5 {
        "SIGNAL WEAK / waiting for host".to_owned()
    } else if let Some(r) = &s.result {
        r.clone()
    } else if let Some(f) = s.feed.last() {
        f.clone()
    } else if !ui.calm && ui.time % 18.0 > 12.0 {
        [
            "THE WALLS KEEP COUNT.",
            "YOU HAVE BEEN HERE BEFORE.",
            "SOMEONE IS STANDING WHERE YOU WERE.",
            "THE SIGNAL KNOWS YOUR NAME.",
        ][(ui.time / 18.0) as usize % 4]
            .into()
    } else {
        format!(
            "{}  /  {}",
            ui.server,
            if s.players.len() == 1 {
                "alone in the ward; invite a second player"
            } else {
                "the round continues"
            }
        )
    };
    center(frame, message, bottom + 1, DIM);
    center(
        frame,
        "WASD move  Q/E turn  SPACE fire  R reload  TAB scores  H help  ESC exit",
        bottom + 2,
        DIM,
    );
    if me.hp == 0 {
        modal(
            frame,
            46,
            6,
            " SIGNAL LOST ",
            vec![
                Line::from(""),
                Line::from(format!(
                    "  Reconstituting in {:.1} seconds",
                    me.respawn_ms as f32 / 1000.0
                )),
                Line::from("  The ward has not finished with you."),
            ],
        );
    }
    if ui.scores {
        let mut players = s.players.clone();
        players.sort_by(|a, b| {
            b.kills
                .cmp(&a.kills)
                .then(a.deaths.cmp(&b.deaths))
                .then(a.id.cmp(&b.id))
        });
        let mut lines = vec![
            Line::from(""),
            Line::from("  SIGNAL               KILLS   DEATHS   STATE"),
            Line::from(""),
        ];
        for p in players {
            lines.push(Line::styled(
                format!(
                    "  {:<18}   {:>3}     {:>3}     {}",
                    p.name,
                    p.kills,
                    p.deaths,
                    if p.hp == 0 { "lost" } else { "alive" }
                ),
                Style::default().fg(rgb(COLORS[p.color as usize % 9])),
            ));
        }
        lines.push(Line::from(""));
        lines.push(Line::from("  1 kill = 1 point. Scores reset every 05:00."));
        lines.push(Line::from("  Tab closes. The game keeps running."));
        modal(
            frame,
            60,
            lines.len() as u16 + 2,
            " SIGNAL REGISTER ",
            lines,
        );
    }
    if ui.help {
        modal(
            frame,
            72,
            18,
            " FIELD MANUAL / H TO CLOSE ",
            vec![
                Line::from(""),
                Line::from("  W/S or Up/Down   Forward / backward"),
                Line::from("  A/D              Strafe left / right"),
                Line::from("  Q/E or Left/Right Turn left / right"),
                Line::from("  Space / Enter / left click    Fire"),
                Line::from("  R                Reload (also automatic when empty)"),
                Line::from("  Tab              Toggle scoreboard"),
                Line::from("  F                Toggle ambient text / haze / muzzle effect"),
                Line::from("  Esc / Ctrl+C     Leave game"),
                Line::from(""),
                Line::from("  Three hits eliminate. Respawn: 2.5s. Shield: 2s."),
                Line::from("  Firing cancels your shield. Fog stays on in both modes."),
                Line::from("  Scores reset together after 5 minutes. Most kills wins."),
                Line::styled(
                    if ui.precise {
                        "  Input: key release supported / simultaneous movement"
                    } else {
                        "  Input: pulse fallback / use short taps if holding stutters"
                    },
                    Style::default().fg(ACCENT),
                ),
                Line::from("  Menus do not pause a multiplayer round."),
            ],
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::Game;
    use ratatui::{Terminal, backend::TestBackend};
    #[test]
    fn renders_resizes_and_overlays_without_panics() {
        let mut game = Game::new();
        game.add(1, "Tester", 2);
        let snap = game.snapshot();
        for (w, h) in [(10, 5), (60, 22), (120, 40), (240, 100)] {
            let mut t = Terminal::new(TestBackend::new(w, h)).unwrap();
            for help in [false, true] {
                t.draw(|f| {
                    draw(
                        f,
                        Some(&snap),
                        1,
                        &Ui {
                            time: 13.0,
                            calm: false,
                            scores: !help,
                            help,
                            net_age: 0.0,
                            server: "127.0.0.1:35353",
                            precise: true,
                        },
                    )
                })
                .unwrap();
            }
        }
    }
    #[test]
    fn sprites_are_rectangular() {
        assert!(CHARACTER.iter().all(|s| s.len() == 12));
        assert!(WEAPON.iter().all(|s| s.len() == 20));
    }
}
