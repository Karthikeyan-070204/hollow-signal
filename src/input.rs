use crate::protocol::Intent;
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};
use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

pub struct Input {
    held: HashMap<KeyCode, Instant>,
    pub intent: Intent,
    precise: bool,
}
impl Input {
    pub fn new(precise: bool) -> Self {
        Self {
            held: HashMap::new(),
            intent: Intent::default(),
            precise,
        }
    }
    pub fn clear(&mut self) {
        self.held.clear();
    }
    pub fn key(&mut self, event: KeyEvent) {
        let code = match event.code {
            KeyCode::Char(c) => KeyCode::Char(c.to_ascii_lowercase()),
            c => c,
        };
        if event.kind == KeyEventKind::Release {
            self.held.remove(&code);
            return;
        }
        if event.kind == KeyEventKind::Press {
            if matches!(code, KeyCode::Char(' ') | KeyCode::Enter) {
                self.fire();
            }
            if code == KeyCode::Char('r') {
                self.intent.reload = self.intent.reload.wrapping_add(1);
            }
        }
        self.held.insert(code, Instant::now());
    }
    pub fn fire(&mut self) {
        self.intent.shot = self.intent.shot.wrapping_add(1);
    }
    pub fn sample(&mut self) -> Intent {
        if !self.precise {
            self.held
                .retain(|_, t| t.elapsed() < Duration::from_millis(180));
        }
        let on = |keys: &[KeyCode]| -> i8 { keys.iter().any(|k| self.held.contains_key(k)) as i8 };
        self.intent.forward =
            on(&[KeyCode::Char('w'), KeyCode::Up]) - on(&[KeyCode::Char('s'), KeyCode::Down]);
        self.intent.strafe = on(&[KeyCode::Char('d')]) - on(&[KeyCode::Char('a')]);
        self.intent.turn =
            on(&[KeyCode::Char('e'), KeyCode::Right]) - on(&[KeyCode::Char('q'), KeyCode::Left]);
        self.intent
    }
}
