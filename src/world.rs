//! The same immutable arena is used by simulation and renderer.
pub const MAP: [&str; 20] = [
    "111111111111111111111111",
    "1......................1",
    "1......1........1......1",
    "1..22..1..3333..1..22..1",
    "1......1........1......1",
    "1......1........1......1",
    "111.1111..11..1111.11111",
    "1......................1",
    "1..11..............11..1",
    "1..11..33......33..11..1",
    "1......................1",
    "1......................1",
    "1..11..33......33..11..1",
    "1..11..............11..1",
    "1......................1",
    "111.1111..11..1111.11111",
    "1......1........1......1",
    "1..22......33......22..1",
    "1......................1",
    "111111111111111111111111",
];
pub const SPAWNS: [(f32, f32, f32); 8] = [
    (3.5, 2.5, 0.4),
    (20.5, 2.5, 2.7),
    (3.5, 18.5, -0.4),
    (20.5, 18.5, -2.7),
    (11.5, 7.5, 1.57),
    (12.5, 14.5, -1.57),
    (1.5, 10.5, 0.0),
    (22.5, 10.5, std::f32::consts::PI),
];
pub const RADIUS: f32 = 0.22;
pub const FAR: f32 = 15.0;

pub fn tile(x: i32, y: i32) -> u8 {
    if x < 0 || y < 0 {
        return 1;
    }
    MAP.get(y as usize)
        .and_then(|row| row.as_bytes().get(x as usize))
        .map_or(1, |c| if *c == b'.' { 0 } else { c - b'0' })
}

pub fn walkable(x: f32, y: f32) -> bool {
    if !x.is_finite() || !y.is_finite() {
        return false;
    }
    for dx in [-RADIUS, RADIUS] {
        for dy in [-RADIUS, RADIUS] {
            if tile((x + dx).floor() as i32, (y + dy).floor() as i32) != 0 {
                return false;
            }
        }
    }
    true
}

pub fn move_body(x: &mut f32, y: &mut f32, dx: f32, dy: f32) {
    // Small substeps prevent tunnelling, including callers with a larger dt.
    let steps = ((dx.abs().max(dy.abs()) / 0.1).ceil() as usize).clamp(1, 64);
    for _ in 0..steps {
        let nx = *x + dx / steps as f32;
        if walkable(nx, *y) {
            *x = nx;
        }
        let ny = *y + dy / steps as f32;
        if walkable(*x, ny) {
            *y = ny;
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct RayHit {
    pub distance: f32,
    pub material: u8,
    pub side: bool,
    pub u: f32,
}

/// DDA. Direction need not be normalized: distance is the ray parameter.
/// Camera-plane rays therefore return perpendicular depth without fisheye.
pub fn raycast(x: f32, y: f32, dx: f32, dy: f32) -> RayHit {
    let mut mx = x.floor() as i32;
    let mut my = y.floor() as i32;
    let ddx = if dx.abs() < 0.00001 {
        1e8
    } else {
        (1.0 / dx).abs()
    };
    let ddy = if dy.abs() < 0.00001 {
        1e8
    } else {
        (1.0 / dy).abs()
    };
    let sx = if dx < 0.0 { -1 } else { 1 };
    let sy = if dy < 0.0 { -1 } else { 1 };
    let mut tx = if dx < 0.0 {
        x - mx as f32
    } else {
        mx as f32 + 1.0 - x
    } * ddx;
    let mut ty = if dy < 0.0 {
        y - my as f32
    } else {
        my as f32 + 1.0 - y
    } * ddy;
    for _ in 0..96 {
        let (distance, side) = if tx < ty {
            let d = tx;
            tx += ddx;
            mx += sx;
            (d, false)
        } else {
            let d = ty;
            ty += ddy;
            my += sy;
            (d, true)
        };
        let material = tile(mx, my);
        if material != 0 || distance >= FAR {
            let u = if side {
                x + distance * dx
            } else {
                y + distance * dy
            };
            return RayHit {
                distance: distance.max(0.001),
                material,
                side,
                u: u.rem_euclid(1.0),
            };
        }
    }
    RayHit {
        distance: FAR,
        material: 0,
        side: false,
        u: 0.0,
    }
}
