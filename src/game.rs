//! Authoritative fixed-step simulation. No client supplies position or damage.
use crate::{protocol::*, world};
use std::collections::VecDeque;

const MAGAZINE: u8 = 8;
const SPEED: f32 = 2.7;
const TURN_SPEED: f32 = 2.25;

struct Player {
    view: PlayerView,
    intent: Intent,
    input_at: f64,
    seen_shot: u32,
    seen_reload: u32,
    next_shot: f64,
    reload_at: f64,
    respawn_at: f64,
    protected_until: f64,
    flash_until: f64,
    hit_until: f64,
}
pub struct Game {
    players: Vec<Player>,
    now: f64,
    elapsed: f64,
    round: u32,
    tick: u32,
    feed: VecDeque<(f64, String)>,
    result: Option<(f64, String)>,
}
impl Default for Game {
    fn default() -> Self {
        Self::new()
    }
}
impl Game {
    pub fn new() -> Self {
        Self {
            players: vec![],
            now: 0.0,
            elapsed: 0.0,
            round: 1,
            tick: 0,
            feed: VecDeque::new(),
            result: None,
        }
    }
    pub fn add(&mut self, id: u32, name: &str, color: u8) -> bool {
        if self.players.len() >= MAX_PLAYERS || self.players.iter().any(|p| p.view.id == id) {
            return false;
        }
        if self.players.is_empty() {
            self.elapsed = 0.0;
        }
        let view = PlayerView {
            id,
            name: clean_name(name),
            color: color % 8,
            x: 0.0,
            y: 0.0,
            angle: 0.0,
            hp: 100,
            kills: 0,
            deaths: 0,
            ammo: MAGAZINE,
            respawn_ms: 0,
            protection_ms: 0,
            reload_ms: 0,
            flash_ms: 0,
            hit_ms: 0,
        };
        self.say(format!("{} entered the signal", view.name));
        self.players.push(Player {
            view,
            intent: Intent::default(),
            input_at: self.now,
            seen_shot: 0,
            seen_reload: 0,
            next_shot: 0.0,
            reload_at: 0.0,
            respawn_at: 0.0,
            protected_until: 0.0,
            flash_until: 0.0,
            hit_until: 0.0,
        });
        self.spawn(self.players.len() - 1);
        true
    }
    pub fn remove(&mut self, id: u32) {
        if let Some(p) = self.players.iter().find(|p| p.view.id == id) {
            self.say(format!("{} lost the signal", p.view.name));
        }
        self.players.retain(|p| p.view.id != id);
    }
    pub fn input(&mut self, id: u32, intent: Intent) {
        if !intent.valid() {
            return;
        }
        if let Some(p) = self.players.iter_mut().find(|p| p.view.id == id) {
            p.intent = intent;
            p.input_at = self.now;
        }
    }
    fn say(&mut self, message: String) {
        self.feed.push_back((self.now + 6.0, message));
        while self.feed.len() > 3 {
            self.feed.pop_front();
        }
    }
    fn spawn(&mut self, index: usize) {
        let spawn = world::SPAWNS
            .iter()
            .enumerate()
            .max_by(|(ia, a), (ib, b)| {
                let distance = |point: &(f32, f32, f32)| {
                    self.players
                        .iter()
                        .enumerate()
                        .filter(|(i, p)| *i != index && p.view.hp > 0)
                        .map(|(_, p)| (p.view.x - point.0).hypot(p.view.y - point.1))
                        .fold(50.0_f32, f32::min)
                };
                distance(a).total_cmp(&distance(b)).then_with(|| {
                    let shift = self.players[index].view.id as usize + self.round as usize;
                    ((ia + shift) % 8).cmp(&((ib + shift) % 8))
                })
            })
            .unwrap()
            .1;
        let p = &mut self.players[index];
        p.view.x = spawn.0;
        p.view.y = spawn.1;
        p.view.angle = spawn.2;
        p.view.hp = 100;
        p.view.ammo = MAGAZINE;
        p.respawn_at = 0.0;
        p.reload_at = 0.0;
        p.protected_until = self.now + 2.0;
        p.next_shot = self.now + 0.25;
        p.flash_until = 0.0;
        p.hit_until = 0.0;
        p.intent.forward = 0;
        p.intent.strafe = 0;
        p.intent.turn = 0;
        p.seen_shot = p.intent.shot;
        p.seen_reload = p.intent.reload;
    }
    pub fn step(&mut self, dt: f32) {
        let dt = dt.clamp(0.0, 0.1);
        self.now += dt as f64;
        self.tick = self.tick.wrapping_add(1);
        if !self.players.is_empty() {
            self.elapsed += dt as f64;
        }
        if self.elapsed >= ROUND_SECONDS {
            self.end_round();
        }
        while self.feed.front().is_some_and(|f| f.0 <= self.now) {
            self.feed.pop_front();
        }
        if self.result.as_ref().is_some_and(|r| r.0 <= self.now) {
            self.result = None;
        }
        for i in 0..self.players.len() {
            if self.players[i].view.hp == 0 && self.players[i].respawn_at <= self.now {
                self.spawn(i);
            }
        }
        let mut shots = Vec::new();
        for (i, p) in self.players.iter_mut().enumerate() {
            let fresh = self.now - p.input_at < 0.25;
            let fire = newer(p.intent.shot, p.seen_shot);
            let reload = newer(p.intent.reload, p.seen_reload);
            if fire {
                p.seen_shot = p.intent.shot;
            }
            if reload {
                p.seen_reload = p.intent.reload;
            }
            if p.view.hp == 0 {
                continue;
            }
            if p.reload_at > 0.0 && p.reload_at <= self.now {
                p.view.ammo = MAGAZINE;
                p.reload_at = 0.0;
            }
            if fresh {
                p.view.angle = (p.view.angle + p.intent.turn as f32 * TURN_SPEED * dt)
                    .rem_euclid(std::f32::consts::TAU);
                let mut f = p.intent.forward as f32;
                let mut s = p.intent.strafe as f32;
                let length = f.hypot(s).max(1.0);
                f /= length;
                s /= length;
                let (sin, cos) = p.view.angle.sin_cos();
                world::move_body(
                    &mut p.view.x,
                    &mut p.view.y,
                    (f * cos - s * sin) * SPEED * dt,
                    (f * sin + s * cos) * SPEED * dt,
                );
            }
            if reload && p.view.ammo < MAGAZINE && p.reload_at == 0.0 {
                p.reload_at = self.now + 1.4;
            }
            if fire && fresh && self.now >= p.next_shot && p.reload_at == 0.0 && p.view.ammo > 0 {
                p.view.ammo -= 1;
                p.next_shot = self.now + 0.38;
                p.flash_until = self.now + 0.09;
                p.protected_until = self.now;
                shots.push(i);
                if p.view.ammo == 0 {
                    p.reload_at = self.now + 1.4;
                }
            }
        }
        for shooter in shots {
            self.shoot(shooter);
        }
    }
    fn shoot(&mut self, index: usize) {
        // Shots scheduled in the same tick are simultaneous, including trades.
        let s = &self.players[index].view;
        let (dy, dx) = s.angle.sin_cos();
        let wall_distance = world::raycast(s.x, s.y, dx, dy).distance.min(world::FAR); // raycast - imaginary ray into the game world to find a wall. s.x, s.y – Starting position of the ray. dx, dy – Direction in which the ray travels. distance – Gets the distance to the detected wall. min(world::FAR) – Makes sure the distance never exceeds world::FAR
        let mut closest = wall_distance;
        let mut target = None;
        for (i, p) in self.players.iter().enumerate() {
            if i == index || p.view.hp == 0 {
                continue;
            }
            let vx = p.view.x - s.x;
            let vy = p.view.y - s.y;
            let along = vx * dx + vy * dy;
            let cross = vx * dy - vy * dx;
            if along <= 0.0 || cross.abs() > world::RADIUS {
                continue;
            }
            let near = (along - (world::RADIUS.powi(2) - cross.powi(2)).sqrt()).max(0.0);
            if near < closest {
                closest = near;
                target = Some(i);
            }
        }
        if let Some(i) = target {
            if self.players[i].protected_until > self.now {
                return;
            }
            self.players[i].view.hp = self.players[i].view.hp.saturating_sub(34);
            self.players[index].hit_until = self.now + 0.18;
            if self.players[i].view.hp == 0 {
                self.players[i].view.deaths = self.players[i].view.deaths.saturating_add(1);
                self.players[i].respawn_at = self.now + 2.5;
                self.players[index].view.kills = self.players[index].view.kills.saturating_add(1);
                self.say(format!(
                    "{} > {}",
                    self.players[index].view.name, self.players[i].view.name
                ));
            }
        }
    }
    fn end_round(&mut self) {
        let high = self.players.iter().map(|p| p.view.kills).max().unwrap_or(0);
        let leaders: Vec<_> = self
            .players
            .iter()
            .filter(|p| p.view.kills == high)
            .collect();
        let result = if high == 0 {
            format!("Round {}: the silence wins", self.round)
        } else if leaders.len() > 1 {
            format!(
                "Round {}: {}-way tie / {} kills",
                self.round,
                leaders.len(),
                high
            )
        } else {
            format!(
                "Round {}: {} wins / {} kills",
                self.round, leaders[0].view.name, high
            )
        };
        self.result = Some((self.now + 7.0, result));
        self.round += 1;
        self.elapsed -= ROUND_SECONDS;
        self.feed.clear();
        for p in &mut self.players {
            p.view.kills = 0;
            p.view.deaths = 0;
            p.view.hp = 0;
        }
        for i in 0..self.players.len() {
            self.spawn(i);
        }
    }
    pub fn snapshot(&self) -> Snapshot {
        let ms = |until: f64| {
            ((until - self.now).max(0.0) * 1000.0)
                .ceil()
                .min(u16::MAX as f64) as u16
        };
        Snapshot {
            tick: self.tick,
            round: self.round,
            remaining_ms: ((ROUND_SECONDS - self.elapsed).max(0.0) * 1000.0).ceil() as u32,
            players: self
                .players
                .iter()
                .map(|p| {
                    let mut v = p.view.clone();
                    v.respawn_ms = ms(p.respawn_at);
                    v.protection_ms = ms(p.protected_until);
                    v.reload_ms = ms(p.reload_at);
                    v.flash_ms = ms(p.flash_until);
                    v.hit_ms = ms(p.hit_until);
                    v
                })
                .collect(),
            feed: self.feed.iter().map(|f| f.1.clone()).collect(),
            result: self.result.as_ref().map(|r| r.1.clone()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn duel() -> Game {
        let mut g = Game::new();
        g.add(1, "One", 0);
        g.add(2, "Two", 1);
        g.players[0].view.x = 2.5;
        g.players[0].view.y = 10.5;
        g.players[0].view.angle = 0.0;
        g.players[1].view.x = 5.5;
        g.players[1].view.y = 10.5;
        for p in &mut g.players {
            p.protected_until = 0.0;
            p.next_shot = 0.0;
        }
        g
    }
    fn fire(g: &mut Game, sequence: u32) {
        g.input(
            1,
            Intent {
                shot: sequence,
                ..Default::default()
            },
        );
        g.step(TICK_SECONDS);
    }
    fn advance(g: &mut Game, seconds: f32) {
        for _ in 0..(seconds / TICK_SECONDS).ceil() as usize {
            g.step(TICK_SECONDS);
        }
    }
    #[test]
    fn three_hits_score_once_then_respawn() {
        let mut g = duel();
        for n in 1..=3 {
            fire(&mut g, n);
            advance(&mut g, 0.4);
        }
        assert_eq!(g.players[0].view.kills, 1);
        assert_eq!(g.players[1].view.hp, 0);
        assert_eq!(g.players[1].view.deaths, 1);
        advance(&mut g, 2.6);
        assert_eq!(g.players[1].view.hp, 100);
        assert!(g.players[1].protected_until > g.now);
    }
    #[test]
    fn wall_blocks_shots() {
        let mut g = duel();
        g.players[0].view.y = 8.5;
        g.players[1].view.y = 8.5;
        fire(&mut g, 1);
        assert_eq!(g.players[1].view.hp, 100);
    }
    #[test]
    fn duplicate_shot_and_fire_spam_cannot_bypass_cooldown() {
        let mut g = duel();
        fire(&mut g, 1);
        fire(&mut g, 1);
        fire(&mut g, 2);
        assert_eq!(g.players[1].view.hp, 66);
        assert_eq!(g.players[0].view.ammo, 7);
    }
    #[test]
    fn spawn_protection_blocks_damage_and_firing_cancels_own() {
        let mut g = duel();
        g.players[1].protected_until = 2.0;
        g.players[0].protected_until = 2.0;
        fire(&mut g, 1);
        assert_eq!(g.players[1].view.hp, 100);
        assert!(g.players[0].protected_until <= g.now);
    }
    #[test]
    fn reload_finishes_and_stale_input_stops() {
        let mut g = duel();
        fire(&mut g, 1);
        g.input(
            1,
            Intent {
                forward: 1,
                shot: 1,
                reload: 1,
                ..Default::default()
            },
        );
        advance(&mut g, 1.6);
        assert_eq!(g.players[0].view.ammo, 8);
        let x = g.players[0].view.x;
        advance(&mut g, 1.0);
        assert_eq!(g.players[0].view.x, x);
    }
    #[test]
    fn round_is_exactly_five_minutes_and_resets_everyone() {
        let mut g = duel();
        g.players[0].view.kills = 4;
        g.players[1].view.deaths = 4;
        g.elapsed = 299.95;
        g.step(TICK_SECONDS);
        assert_eq!(g.round, 1);
        g.step(TICK_SECONDS);
        assert_eq!(g.round, 2);
        for p in &g.players {
            assert_eq!(
                (p.view.kills, p.view.deaths, p.view.hp, p.view.ammo),
                (0, 0, 100, 8)
            );
        }
        assert!(g.snapshot().result.unwrap().contains("One wins"));
        assert!(g.snapshot().remaining_ms > 299_900);
    }
    #[test]
    fn diagonal_speed_is_normalized() {
        let mut g = duel();
        g.input(
            1,
            Intent {
                forward: 1,
                strafe: 1,
                ..Default::default()
            },
        );
        g.step(TICK_SECONDS);
        let p = &g.players[0].view;
        assert!(((p.x - 2.5).hypot(p.y - 10.5) - SPEED * TICK_SECONDS).abs() < 0.0001);
    }
}
