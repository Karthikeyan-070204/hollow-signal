//! Small bounded datagrams. Server, client and tests share these wire types.
use bincode::Options;
use serde::{Deserialize, Serialize};
pub const VERSION: u16 = 1;
pub const PORT: u16 = 35353;
pub const MAX_PLAYERS: usize = 8;
pub const MAX_PACKET: usize = 4096;
pub const ROUND_SECONDS: f64 = 300.0;
pub const TICK_SECONDS: f32 = 1.0 / 30.0;
pub const COLOR_NAMES: [&str; 8] = [
    "cyan", "amber", "violet", "red", "lime", "blue", "pink", "white",
];
pub const COLORS: [(u8, u8, u8); 8] = [
    (69, 219, 207),
    (230, 171, 68),
    (166, 121, 224),
    (220, 83, 78),
    (152, 201, 91),
    (97, 153, 236),
    (223, 127, 170),
    (210, 217, 201),
];
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct Intent {
    pub forward: i8,
    pub strafe: i8,
    pub turn: i8,
    // Cumulative counters survive packet loss. They are not timestamps.
    pub shot: u32,
    pub reload: u32,
}
impl Intent {
    pub fn valid(self) -> bool {
        (-1..=1).contains(&self.forward)
            && (-1..=1).contains(&self.strafe)
            && (-1..=1).contains(&self.turn)
    }
}
#[derive(Debug, Serialize, Deserialize)]
pub enum ClientPacket {
    Hello {
        version: u16,
        nonce: u64,
        name: String,
        color: u8,
    },
    Input {
        id: u32,
        token: u64,
        sequence: u32,
        intent: Intent,
    },
    Bye {
        id: u32,
        token: u64,
    },
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerView {
    pub id: u32,
    pub name: String,
    pub color: u8,
    pub x: f32,
    pub y: f32,
    pub angle: f32,
    pub hp: u8,
    pub kills: u16,
    pub deaths: u16,
    pub ammo: u8,
    pub respawn_ms: u16,
    pub protection_ms: u16,
    pub reload_ms: u16,
    pub flash_ms: u16,
    pub hit_ms: u16,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    pub tick: u32,
    pub round: u32,
    pub remaining_ms: u32,
    pub players: Vec<PlayerView>,
    pub feed: Vec<String>,
    pub result: Option<String>,
}
#[derive(Debug, Serialize, Deserialize)]
pub enum ServerPacket {
    Welcome {
        version: u16,
        nonce: u64,
        id: u32,
        token: u64,
    },
    Rejected {
        nonce: u64,
        reason: String,
    },
    State(Snapshot),
}
pub fn encode<T: Serialize>(value: &T) -> Result<Vec<u8>, bincode::Error> {
    bincode::DefaultOptions::new()
        .with_fixint_encoding()
        .with_limit(MAX_PACKET as u64)
        .serialize(value)
}
pub fn decode<'a, T: Deserialize<'a>>(bytes: &'a [u8]) -> Result<T, bincode::Error> {
    bincode::DefaultOptions::new()
        .with_fixint_encoding()
        .with_limit(MAX_PACKET as u64)
        .reject_trailing_bytes()
        .deserialize(bytes)
}
pub fn newer(a: u32, b: u32) -> bool {
    a != b && a.wrapping_sub(b) < (1 << 31)
}
pub fn clean_name(value: &str) -> String {
    // No terminal escape sequences, bidi controls or ambiguous-width glyphs.
    let name: String = value
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || " _-".contains(*c))
        .take(16)
        .collect();
    let name = name.trim();
    if name.is_empty() {
        "Wanderer".into()
    } else {
        name.into()
    }
}
