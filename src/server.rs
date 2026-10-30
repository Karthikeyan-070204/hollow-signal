use crate::{game::Game, protocol::*};
use std::{
    collections::HashMap,
    io,
    net::{SocketAddr, UdpSocket},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

struct Peer {
    id: u32,
    token: u64,
    nonce: u64,
    last_seen: Instant,
    last_sequence: Option<u32>,
}
pub struct Server {
    socket: UdpSocket,
    peers: HashMap<SocketAddr, Peer>,
    game: Game,
    next_id: u32,
}
pub struct HostedServer {
    stop: Arc<AtomicBool>,
    thread: Option<thread::JoinHandle<io::Result<()>>>,
}
impl Drop for HostedServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}
impl Server {
    pub fn bind(address: &str) -> io::Result<Self> {
        let socket = UdpSocket::bind(address)?;
        socket.set_nonblocking(true)?;
        Ok(Self {
            socket,
            peers: HashMap::new(),
            game: Game::new(),
            next_id: 1,
        })
    }
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.socket.local_addr()
    }
    pub fn background(self) -> HostedServer {
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        HostedServer {
            stop,
            thread: Some(thread::spawn(move || self.run(&flag))),
        }
    }
    fn reply(&self, addr: SocketAddr, message: ServerPacket) {
        if let Ok(bytes) = encode(&message) {
            let _ = self.socket.send_to(&bytes, addr);
        }
    }
    fn receive(&mut self, addr: SocketAddr, packet: ClientPacket) {
        match packet {
            ClientPacket::Hello {
                version,
                nonce,
                name,
                color,
            } => {
                if version != VERSION {
                    self.reply(
                        addr,
                        ServerPacket::Rejected {
                            nonce,
                            reason: "Different game version; use the same build.".into(),
                        },
                    );
                    return;
                }
                if name.len() > 64 || color >= 9 {
                    return;
                }
                if let Some(peer) = self.peers.get_mut(&addr) {
                    if peer.nonce != nonce {
                        return;
                    }
                    peer.last_seen = Instant::now();
                    let (id, token) = (peer.id, peer.token);
                    self.reply(
                        addr,
                        ServerPacket::Welcome {
                            version: VERSION,
                            nonce,
                            id,
                            token,
                        },
                    );
                    return;
                }
                if self.peers.len() >= MAX_PLAYERS {
                    self.reply(
                        addr,
                        ServerPacket::Rejected {
                            nonce,
                            reason: "The arena is full (8 players).".into(),
                        },
                    );
                    return;
                }
                let id = self.next_id;
                self.next_id = self.next_id.wrapping_add(1);
                let token = rand::random();
                if self.game.add(id, &name, color) {
                    self.peers.insert(
                        addr,
                        Peer {
                            id,
                            token,
                            nonce,
                            last_seen: Instant::now(),
                            last_sequence: None,
                        },
                    );
                    self.reply(
                        addr,
                        ServerPacket::Welcome {
                            version: VERSION,
                            nonce,
                            id,
                            token,
                        },
                    );
                }
            }
            ClientPacket::Input {
                id,
                token,
                sequence,
                intent,
            } => {
                if !intent.valid() {
                    return;
                }
                if let Some(p) = self.peers.get_mut(&addr) {
                    if p.id != id
                        || p.token != token
                        || p.last_sequence.is_some_and(|s| !newer(sequence, s))
                    {
                        return;
                    }
                    p.last_seen = Instant::now();
                    p.last_sequence = Some(sequence);
                    self.game.input(id, intent);
                }
            }
            ClientPacket::Bye { id, token } => {
                if self
                    .peers
                    .get(&addr)
                    .is_some_and(|p| p.id == id && p.token == token)
                {
                    self.peers.remove(&addr);
                    self.game.remove(id);
                }
            }
        }
    }
    pub fn run(mut self, stop: &AtomicBool) -> io::Result<()> {
        let step = Duration::from_secs_f32(TICK_SECONDS);
        // Read the entire datagram before rejecting oversized protocol messages.
        // A smaller recv buffer yields WSAEMSGSIZE on Windows instead of truncation.
        let mut buffer = [0u8; 65_535];
        let mut next = Instant::now();
        while !stop.load(Ordering::Relaxed) {
            // Bound work under a noisy/malformed LAN sender.
            for _ in 0..256 {
                match self.socket.recv_from(&mut buffer) {
                    Ok((n, addr)) if n <= MAX_PACKET => {
                        if let Ok(packet) = decode(&buffer[..n]) {
                            self.receive(addr, packet);
                        }
                    }
                    Ok(_) => {}
                    Err(e) if e.kind() == io::ErrorKind::WouldBlock => break,
                    Err(e) if e.kind() == io::ErrorKind::ConnectionReset => continue,
                    Err(e) => return Err(e),
                }
            }
            let stale: Vec<_> = self
                .peers
                .iter()
                .filter(|(_, p)| p.last_seen.elapsed() > Duration::from_secs(5))
                .map(|(a, p)| (*a, p.id))
                .collect();
            for (addr, id) in stale {
                self.peers.remove(&addr);
                self.game.remove(id);
            }
            self.game.step(TICK_SECONDS);
            if !self.peers.is_empty() {
                let bytes =
                    encode(&ServerPacket::State(self.game.snapshot())).map_err(io::Error::other)?;
                for addr in self.peers.keys() {
                    let _ = self.socket.send_to(&bytes, addr);
                }
            }
            next += step;
            let now = Instant::now();
            if next > now {
                thread::sleep(next - now);
            } else if now.duration_since(next) > step * 4 {
                next = now;
            }
        }
        Ok(())
    }
}
