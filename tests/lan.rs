use hollow_signal::{client::Connection, game::Game, protocol::*, server::Server, world};
use std::{
    collections::{HashSet, VecDeque},
    net::UdpSocket,
    thread,
    time::{Duration, Instant},
};

fn wait_for(client: &mut Connection, predicate: impl Fn(&Snapshot) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(3);
    while Instant::now() < deadline {
        client.send(Intent::default()).unwrap();
        client.poll().unwrap();
        if client.snapshot.as_ref().is_some_and(&predicate) {
            return;
        }
        thread::sleep(Duration::from_millis(12));
    }
    panic!("Snapshot condition timed out: {:?}", client.snapshot);
}
#[test]
fn two_udp_clients_join_move_share_scores_and_leave() {
    let server = Server::bind("127.0.0.1:0").unwrap();
    let addr = server.local_addr().unwrap();
    let _server = server.background();
    let mut one = Connection::connect(addr, "Karthik", 2).unwrap();
    let mut two = Connection::connect(addr, "Friend", 4).unwrap();
    wait_for(&mut one, |s| s.players.len() == 2);
    wait_for(&mut two, |s| s.players.len() == 2);
    let start = one
        .snapshot
        .as_ref()
        .unwrap()
        .players
        .iter()
        .find(|p| p.id == one.id)
        .unwrap()
        .clone();
    let until = Instant::now() + Duration::from_millis(220);
    while Instant::now() < until {
        one.send(Intent {
            forward: 1,
            turn: 1,
            ..Default::default()
        })
        .unwrap();
        two.send(Intent::default()).unwrap();
        one.poll().unwrap();
        two.poll().unwrap();
        thread::sleep(Duration::from_millis(15));
    }
    let id = one.id;
    wait_for(&mut two, |s| {
        s.players
            .iter()
            .any(|p| p.id == id && (p.angle - start.angle).abs() > 0.2)
    });
    let seen = two
        .snapshot
        .as_ref()
        .unwrap()
        .players
        .iter()
        .find(|p| p.id == id)
        .unwrap();
    assert!((seen.x - start.x).hypot(seen.y - start.y) > 0.1);
    assert_eq!(seen.name, "Karthik");
    assert_eq!(seen.color, 2);
    assert_eq!(seen.kills, 0);
    assert!(two.snapshot.as_ref().unwrap().remaining_ms <= 300_000);
    drop(two);
    wait_for(&mut one, |s| s.players.len() == 1);
}

#[test]
fn malformed_packets_and_wrong_tokens_do_not_change_the_arena() {
    let server = Server::bind("127.0.0.1:0").unwrap();
    let addr = server.local_addr().unwrap();
    let _server = server.background();
    let mut one = Connection::connect(addr, "One", 0).unwrap();
    wait_for(&mut one, |s| s.players.len() == 1);
    let bad = UdpSocket::bind("127.0.0.1:0").unwrap();
    bad.connect(addr).unwrap();
    for bytes in [
        vec![255; 32],
        vec![0; MAX_PACKET + 20],
        encode(&ClientPacket::Bye {
            id: one.id,
            token: 0,
        })
        .unwrap(),
        encode(&ClientPacket::Input {
            id: one.id,
            token: 0,
            sequence: 900,
            intent: Intent {
                forward: 127,
                ..Default::default()
            },
        })
        .unwrap(),
    ] {
        bad.send(&bytes).unwrap();
    }
    let tick = one.snapshot.as_ref().unwrap().tick;
    wait_for(&mut one, |s| newer(s.tick, tick) && s.players.len() == 1);
}

#[test]
fn full_snapshot_fits_one_normal_lan_datagram() {
    let mut game = Game::new();
    for id in 1..=8 {
        assert!(game.add(id, "SixteenCharNameXX", id as u8 - 1));
    }
    assert!(!game.add(9, "TooMany", 0));
    let mut s = game.snapshot();
    s.feed = vec!["1234567890123456 > 1234567890123456".into(); 3];
    s.result = Some("Round 999: SixteenCharNameXX wins / 65535 kills".into());
    let bytes = encode(&ServerPacket::State(s)).unwrap();
    assert!(bytes.len() < 1200, "{} byte snapshot", bytes.len());
    assert!(decode::<ServerPacket>(&bytes).is_ok());
}

#[test]
fn map_is_enclosed_and_every_spawn_is_connected() {
    let width = world::MAP[0].len();
    assert!(world::MAP.iter().all(|r| r.len() == width));
    let mut visited = HashSet::new();
    let mut queue = VecDeque::from([(1, 1)]);
    while let Some((x, y)) = queue.pop_front() {
        if world::tile(x, y) != 0 || !visited.insert((x, y)) {
            continue;
        }
        for (dx, dy) in [(0, 1), (0, -1), (1, 0), (-1, 0)] {
            queue.push_back((x + dx, y + dy));
        }
    }
    for (x, y, _) in world::SPAWNS {
        assert!(world::walkable(x, y));
        assert!(visited.contains(&(x as i32, y as i32)));
    }
    let open = world::MAP
        .iter()
        .flat_map(|r| r.bytes())
        .filter(|c| *c == b'.')
        .count();
    assert_eq!(visited.len(), open);
}

#[test]
fn wall_collision_prevents_tunnelling_and_nonfinite_positions() {
    let (mut x, mut y) = (2.5, 8.5);
    world::move_body(&mut x, &mut y, 2.0, 0.0);
    assert!(x < 3.0 - world::RADIUS);
    assert_eq!(y, 8.5);
    assert!(!world::walkable(f32::NAN, 0.0));
}

#[test]
fn names_and_packet_order_are_safe_for_the_terminal() {
    assert!(!clean_name("\x1b[31mName\n").contains('\x1b'));
    assert_eq!(clean_name(""), "Wanderer");
    assert_eq!(clean_name(&"x".repeat(100)).len(), 16);
    assert!(newer(0, u32::MAX));
    assert!(!newer(u32::MAX, 0));
    assert!(!newer(5, 5));
}
