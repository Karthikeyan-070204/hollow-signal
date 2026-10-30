use crate::{
    input::Input,
    protocol::*,
    render::{self, Ui},
};
use crossterm::{
    event::{
        self, DisableFocusChange, DisableMouseCapture, EnableFocusChange, EnableMouseCapture,
        Event, KeyCode, KeyEventKind, KeyModifiers, KeyboardEnhancementFlags, MouseButton,
        MouseEventKind, PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
    },
    execute, terminal,
};
use std::{
    io,
    net::{SocketAddr, UdpSocket},
    time::{Duration, Instant},
};

pub struct Connection {
    socket: UdpSocket,
    pub id: u32,
    token: u64,
    sequence: u32,
    pub snapshot: Option<Snapshot>,
    previous: Option<Snapshot>,
    pub received: Instant,
}
impl Connection {
    pub fn connect(address: SocketAddr, name: &str, color: u8) -> io::Result<Self> {
        let socket = UdpSocket::bind(if address.is_ipv4() {
            "0.0.0.0:0"
        } else {
            "[::]:0"
        })?;
        socket.connect(address)?;
        socket.set_read_timeout(Some(Duration::from_millis(150)))?;
        let nonce = rand::random();
        let hello = encode(&ClientPacket::Hello {
            version: VERSION,
            nonce,
            name: clean_name(name),
            color: color % 9,
        })
        .map_err(io::Error::other)?;
        let started = Instant::now();
        let mut last_send = started - Duration::from_secs(1);
        let mut buffer = [0; 65_535];
        while started.elapsed() < Duration::from_secs(6) {
            if last_send.elapsed() >= Duration::from_millis(250) {
                socket.send(&hello)?;
                last_send = Instant::now();
            }
            match socket.recv(&mut buffer) {
                Ok(n) if n <= MAX_PACKET => match decode::<ServerPacket>(&buffer[..n]) {
                    Ok(ServerPacket::Welcome {
                        version,
                        nonce: reply,
                        id,
                        token,
                    }) if reply == nonce && version == VERSION => {
                        socket.set_read_timeout(None)?;
                        socket.set_nonblocking(true)?;
                        return Ok(Self {
                            socket,
                            id,
                            token,
                            sequence: 0,
                            snapshot: None,
                            previous: None,
                            received: Instant::now(),
                        });
                    }
                    Ok(ServerPacket::Rejected {
                        nonce: reply,
                        reason,
                    }) if reply == nonce => return Err(io::Error::other(reason)),
                    _ => {}
                },
                Ok(_) => {}
                Err(e)
                    if matches!(
                        e.kind(),
                        io::ErrorKind::WouldBlock
                            | io::ErrorKind::TimedOut
                            | io::ErrorKind::ConnectionRefused
                            | io::ErrorKind::ConnectionReset
                    ) => {}
                Err(e) => return Err(e),
            }
        }
        Err(io::Error::new(
            io::ErrorKind::TimedOut,
            "No host response. Check the host IP, same LAN, and UDP port 35353 in the host firewall.",
        ))
    }
    pub fn send(&mut self, intent: Intent) -> io::Result<()> {
        self.sequence = self.sequence.wrapping_add(1);
        let bytes = encode(&ClientPacket::Input {
            id: self.id,
            token: self.token,
            sequence: self.sequence,
            intent,
        })
        .map_err(io::Error::other)?;
        match self.socket.send(&bytes) {
            Ok(_) => Ok(()),
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => Ok(()),
            Err(e) => Err(e),
        }
    }
    pub fn poll(&mut self) -> io::Result<()> {
        let mut buffer = [0; 65_535];
        for _ in 0..128 {
            match self.socket.recv(&mut buffer) {
                Ok(n) if n <= MAX_PACKET => {
                    if let Ok(ServerPacket::State(s)) = decode(&buffer[..n]) {
                        let valid = s.players.len() <= MAX_PLAYERS
                            && s.players.iter().all(|p| {
                                p.x.is_finite()
                                    && p.y.is_finite()
                                    && p.angle.is_finite()
                                    && p.color < 9
                            });
                        if valid
                            && self
                                .snapshot
                                .as_ref()
                                .is_none_or(|old| newer(s.tick, old.tick))
                        {
                            self.previous = self.snapshot.replace(s);
                            self.received = Instant::now();
                        }
                    }
                }
                Ok(_) => {}
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => break,
                Err(e)
                    if matches!(
                        e.kind(),
                        io::ErrorKind::ConnectionReset | io::ErrorKind::ConnectionRefused
                    ) =>
                {
                    break;
                }
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }
    pub fn interpolated(&self) -> Option<Snapshot> {
        let mut now = self.snapshot.clone()?;
        if let Some(old) = &self.previous {
            if old.round != now.round {
                return Some(now);
            }
            let ticks = now.tick.wrapping_sub(old.tick).clamp(1, 6);
            let blend = (self.received.elapsed().as_secs_f32() / (TICK_SECONDS * ticks as f32))
                .clamp(0.0, 1.0);
            for p in &mut now.players {
                if let Some(o) = old.players.iter().find(|o| o.id == p.id) {
                    if (p.x - o.x).hypot(p.y - o.y) > 1.0 || p.hp == 0 || o.hp == 0 {
                        continue;
                    }
                    p.x = o.x + (p.x - o.x) * blend;
                    p.y = o.y + (p.y - o.y) * blend;
                    let angle = (p.angle - o.angle + std::f32::consts::PI)
                        .rem_euclid(std::f32::consts::TAU)
                        - std::f32::consts::PI;
                    p.angle = o.angle + angle * blend;
                }
            }
        }
        Some(now)
    }
}
impl Drop for Connection {
    fn drop(&mut self) {
        if let Ok(bytes) = encode(&ClientPacket::Bye {
            id: self.id,
            token: self.token,
        }) {
            // Redundant goodbye; timeout also cleans up a lost datagram.
            for _ in 0..3 {
                let _ = self.socket.send(&bytes);
            }
        }
    }
}
struct TerminalGuard {
    enhanced: bool,
}
impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let mut out = io::stdout();
        if self.enhanced {
            let _ = execute!(out, PopKeyboardEnhancementFlags);
        }
        let _ = execute!(out, DisableMouseCapture, DisableFocusChange);
        ratatui::restore();
    }
}
pub fn play(mut connection: Connection, address: &str, calm: bool, fps: u16) -> io::Result<()> {
    // Colour carries the scene geometry, not just optional text decoration.
    crossterm::style::force_color_output(true);
    let mut terminal = ratatui::try_init()?;
    let mut guard = TerminalGuard { enhanced: false };
    // Ratatui's panic hook restores raw/alternate mode. Also restore our extras.
    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = execute!(
            io::stdout(),
            DisableMouseCapture,
            DisableFocusChange,
            PopKeyboardEnhancementFlags
        );
        hook(info);
    }));
    if terminal::supports_keyboard_enhancement().unwrap_or(false) {
        execute!(
            io::stdout(),
            PushKeyboardEnhancementFlags(
                KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
                    | KeyboardEnhancementFlags::REPORT_EVENT_TYPES
                    | KeyboardEnhancementFlags::REPORT_ALL_KEYS_AS_ESCAPE_CODES
            )
        )?;
        guard.enhanced = true;
    }
    execute!(io::stdout(), EnableMouseCapture, EnableFocusChange)?;
    let precise = cfg!(windows) || guard.enhanced;
    let mut input = Input::new(precise);
    let started = Instant::now();
    let minimap = true;
    let mut last_send = started - Duration::from_secs(1);
    let mut last_draw = started - Duration::from_secs(1);
    let frame_time = Duration::from_secs_f32(1.0 / fps.clamp(15, 60) as f32);
    let mut scores = false;
    let mut help = false;
    let mut calm = calm;
    loop {
        connection.poll()?;
        if connection.received.elapsed() > Duration::from_secs(5) {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "The host disconnected or the network stopped responding.",
            ));
        }
        // Drain a bounded batch so input cannot starve network/render work.
        for _ in 0..64 {
            if !event::poll(Duration::ZERO)? {
                break;
            }
            match event::read()? {
                Event::Key(key) => {
                    let down = key.kind == KeyEventKind::Press;
                    if down
                        && (key.code == KeyCode::Esc
                            || (key.code == KeyCode::Char('c')
                                && key.modifiers.contains(KeyModifiers::CONTROL)))
                    {
                        return Ok(());
                    }
                    if down && key.code == KeyCode::Tab {
                        scores = !scores;
                        input.clear();
                    } else if down && matches!(key.code, KeyCode::Char('h') | KeyCode::Char('?')) {
                        help = !help;
                        input.clear();
                    } else if down && key.code == KeyCode::Char('f') {
                        calm = !calm;
                    } else if !help && !scores {
                        input.key(key);
                    }
                }
                Event::FocusLost => input.clear(),
                Event::Mouse(m)
                    if m.kind == MouseEventKind::Down(MouseButton::Left) && !help && !scores =>
                {
                    input.fire()
                }
                _ => {}
            }
        }
        if last_send.elapsed() >= Duration::from_secs_f32(TICK_SECONDS) {
            connection.send(input.sample())?;
            last_send = Instant::now();
        }
        if last_draw.elapsed() >= frame_time {
            let snap = connection.interpolated();
            terminal.draw(|frame| {
                render::draw(
                    frame,
                    snap.as_ref(),
                    connection.id,
                    &Ui {
                        time: started.elapsed().as_secs_f32(),
                        calm,
                        scores,
                        help,
                        net_age: connection.received.elapsed().as_secs_f32(),
                        server: address,
                        precise,
                        minimap,
                    },
                )
            })?;
            last_draw = Instant::now();
        }
        std::thread::sleep(Duration::from_millis(2));
    }
}
