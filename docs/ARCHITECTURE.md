# Build it, then understand it

## The shortest useful tour

1. Read `src/main.rs`: it chooses **host**, **join** or **server**. With no arguments, it asks for your name and character colour.
2. Read `src/protocol.rs`: these enums and structs are the messages computers exchange. An input message says “move forward and turn left”, never “my position is X” or “I killed someone”.
3. Read `src/world.rs`: the map is a grid. A dot means walkable floor, a number means a wall material. `move_body` keeps a player's small collision body out of walls. `raycast` finds the first wall along a direction.
4. Read `src/game.rs`: the host's game rules. It applies movement at a fixed speed, validates shot cadence, checks the first target in front of a weapon, awards points and resets each round.
5. Read `src/server.rs`: receives UDP messages, checks the sender's address/token/sequence, advances the game at 30 Hz and sends snapshots to every connected player.
6. Read `src/client.rs`: joins a server, transmits input, receives snapshots, interpolates movement and runs the terminal loop.
7. Read `src/input.rs`: tracks movement keys and increments shot/reload counters. On terminals without release events, held keys expire after 180 ms without a repeat.
8. Read `src/render.rs`: converts the latest world snapshot into the pixels, weapon, HUD, scoreboard and help overlay you see.

## One shot, from keyboard to score

You press Space. Your client increments its shot counter and includes that counter in subsequent input packets. Repeating the counter means a dropped packet need not lose the shot completely. The server accepts only a newer input sequence from the player's known socket and token. It compares the shot counter with the last one it consumed.

If the character is alive, the input is recent, the magazine has ammunition, and the cooldown/reload permits firing, the server spends one round. It casts a ray to the nearest wall and intersects that ray with other players' collision circles. Only the nearest character before the wall can be hit. Protection may block the damage. Three normal hits reduce 100 health to zero.

The server increments the shooter's kill count and the victim's death count, then schedules a respawn. The next snapshot carries health, scores, ammo, hit marker timing and death/respawn state to all clients. No client can award itself a point through the normal input protocol.

## Why this renderer looks three-dimensional

The map itself is two-dimensional. For each screen column, the renderer casts one ray into the map. A near wall occupies a taller vertical strip; a distant wall occupies a shorter strip. Each wall strip samples a small procedural brick/concrete texture.

The rays are constructed from a forward direction and a perpendicular camera plane. Their returned DDA parameter is perpendicular depth, so wall strips avoid the common fisheye distortion. Floors and ceilings sample world coordinates from their row's perspective distance. An exponential mix with a grey-green fog colour fades distant surfaces.

Other players are camera-facing pixel sprites. They are drawn far-to-near and checked against the wall-depth buffer for every column. One terminal cell uses a foreground and background colour with the upper-half-block glyph, providing two vertical colour samples.

This approach has no pitch, vertical aiming, stairs, different floor heights or real 3D models. The small weapon image is an overlay, not a 3D object.

## Networking choices and their consequences

| Choice | Reason / consequence |
|---|---|
| UDP | Small real-time inputs and snapshots; lost snapshots are replaced by newer ones. |
| Fixed 30 Hz host simulation | Movement and firing do not depend on a client's frame rate or packet spam. |
| 30 Hz input transmission | Independent of rendering rate. |
| Cumulative shot/reload counters | Retry actions through repeated input; duplicate packets do not duplicate a shot. |
| Bincode with a size limit | Compact binary packets; complete eight-player snapshots are tested below 1,200 bytes. |
| Versioned handshake | Clients with an incompatible wire version are rejected. |
| Random token + socket address | Input/leave messages must match the accepted session. This is not encrypted authentication. |
| Sequence numbers | Old/reordered inputs and snapshots are discarded, including wraparound handling. |
| 250 ms input expiry | A temporary loss of input stops movement instead of walking forever. |
| 5 s disconnect timeout | Abandoned sessions leave the arena. |
| Client interpolation | Smooths authoritative snapshots, at the cost of about one extra snapshot interval of visual delay. |

The client does not predict movement and the server does not rewind hits for latency. Aim is tested against the current server state, so this version is intended for low-latency LAN use. A single malformed/noisy sender cannot make one tick process an unlimited number of packets, but this is not a DoS-resistant server.

## Change the game deliberately

- **Map and spawns:** edit `MAP` and `SPAWNS` in `world.rs`, preserving enclosed walls and walkable, connected spawn points. Run the connectivity/collision tests.
- **Health/damage/magazine/cooldowns:** edit `game.rs`. Update HUD/manual text and combat tests together; some labels intentionally match the fixed v0.1 rules.
- **Five-minute duration:** `ROUND_SECONDS` in `protocol.rs` is the authority. For the requested game it stays `300.0`; all clients display server time.
- **Character sprites, wall textures, fog palette:** `render.rs`. Fog currently has no server-side visibility restrictions; it is a visual effect.
- **Character colours:** `COLORS` and `COLOR_NAMES` in `protocol.rs`, currently exactly eight. Update bounds and tests if you add colours.
- **Multiplayer capacity:** `MAX_PLAYERS`; review packet sizes and spawn selection before increasing it.

## Useful next development steps

1. Play a two-computer LAN match and tune turn speed, fog visibility and terminal font size from actual feedback.
2. Add configurable key bindings and a full-screen setup lobby if desired.
3. Add a second arena selected by the server and include a map identifier in the handshake.
4. Add sound as an optional client feature, keeping it separate from authoritative combat.
5. Implement movement prediction/reconciliation and lag-compensated hit testing before targeting high-latency networking.
6. If switching to Bevy, move `world`, `game` and `protocol` into a shared crate and build a second client. Define new 3D mechanics explicitly rather than assuming the old 2.5D simulation supports them.

