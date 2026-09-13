//! Procedural 3D models. Every generator works in the piece's real size and
//! the result is snapped to exactly `width × depth × height`.

use newera_core::{Furniture, OpeningKind};

use crate::Model;
use crate::mesh::{Axis, Mesh, Rgb, rgb, shade};

const METAL: [u8; 3] = [150, 154, 160];
const GLASS: [u8; 3] = [168, 206, 226];
const DARK: [u8; 3] = [40, 42, 48];
const LINEN: [u8; 3] = [245, 243, 236];

struct Ctx {
    m: Mesh,
    w: f64,
    d: f64,
    h: f64,
    c: Rgb,
}

impl Ctx {
    /// Box by explicit bounds (cm) in the local frame.
    fn cube(&mut self, x: [f64; 2], y: [f64; 2], z: [f64; 2], color: Rgb) {
        self.m.cuboid([x[0], y[0], z[0]], [x[1], y[1], z[1]], color);
    }

    fn legs(&mut self, inset: f64, top: f64, size: f64, color: Rgb) {
        let (hw, hd) = (
            self.w / 2.0 - inset - size / 2.0,
            self.d / 2.0 - inset - size / 2.0,
        );
        for (sx, sz) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
            self.m
                .block([sx * hw, sz * hd], 0.0, [size, top, size], color);
        }
    }

    fn handle(&mut self, x: f64, y: f64, z: f64, vertical: bool) {
        let (w, h) = if vertical { (1.5, 14.0) } else { (12.0, 1.5) };
        self.cube(
            [x - w / 2.0, x + w / 2.0],
            [y - h / 2.0, y + h / 2.0],
            [z, z + 2.0],
            rgb(METAL),
        );
    }
}

pub(crate) fn build(model: Model, piece: &Furniture, color: Rgb) -> Mesh {
    let mut ctx = Ctx {
        m: Mesh::default(),
        w: piece.width,
        d: piece.depth,
        h: piece.height,
        c: color,
    };
    let opening = piece.opening;
    match model {
        Model::Sofa { seats } => sofa(&mut ctx, seats),
        Model::Bed { pillows } => bed(&mut ctx, pillows),
        Model::Table { round } => table(&mut ctx, round),
        Model::Chair => chair(&mut ctx),
        Model::Stool => stool(&mut ctx),
        Model::OfficeChair => office_chair(&mut ctx),
        Model::Cabinet { doors, drawers } => cabinet(&mut ctx, doors, drawers),
        Model::Shelf { levels } => shelf(&mut ctx, levels),
        Model::TvStand => tv_stand(&mut ctx),
        Model::Tv => tv(&mut ctx),
        Model::Fridge => fridge(&mut ctx),
        Model::Stove => stove(&mut ctx),
        Model::SinkCounter => counter(&mut ctx, true),
        Model::Counter => counter(&mut ctx, false),
        Model::WallCabinet => cabinet(&mut ctx, 1, 0),
        Model::Appliance { round_door } => appliance(&mut ctx, round_door),
        Model::Microwave => microwave(&mut ctx),
        Model::Toilet => toilet(&mut ctx),
        Model::Basin => basin(&mut ctx),
        Model::Shower => shower(&mut ctx),
        Model::Bathtub => bathtub(&mut ctx),
        Model::Door => door(
            &mut ctx,
            opening.map_or(1, |o| o.leaves),
            opening.is_some_and(|o| o.sliding),
        ),
        Model::Window { panes } => window(
            &mut ctx,
            panes,
            opening.is_some_and(|o| o.kind == OpeningKind::Window),
        ),
        Model::Passage => passage(&mut ctx),
        Model::Stairs { steps } => stairs(&mut ctx, steps),
        Model::Plant => plant(&mut ctx),
        Model::Tree => tree(&mut ctx),
        Model::Lamp => lamp(&mut ctx),
        Model::Car => car(&mut ctx),
        Model::Desk => desk(&mut ctx),
        Model::Crib => crib(&mut ctx),
        Model::Column { round } => {
            let c = ctx.c;
            if round {
                ctx.m.frustum(
                    [0.0, 0.0, 0.0],
                    Axis::Y,
                    ctx.h,
                    ctx.w / 2.0,
                    ctx.w / 2.0,
                    28,
                    c,
                );
            } else {
                ctx.cube(
                    [-ctx.w / 2.0, ctx.w / 2.0],
                    [0.0, ctx.h],
                    [-ctx.d / 2.0, ctx.d / 2.0],
                    c,
                );
            }
        }
        Model::Rug | Model::Box => {
            let c = ctx.c;
            ctx.cube(
                [-ctx.w / 2.0, ctx.w / 2.0],
                [0.0, ctx.h],
                [-ctx.d / 2.0, ctx.d / 2.0],
                c,
            );
        }
    }
    ctx.m.fit_to(piece.width, piece.depth, piece.height);
    ctx.m
}

fn sofa(ctx: &mut Ctx, seats: u8) {
    let (w, d, h, c) = (ctx.w, ctx.d, ctx.h, ctx.c);
    let arm = (w * 0.1).clamp(10.0, 22.0);
    let back = (d * 0.22).clamp(12.0, 25.0);
    let feet = (h * 0.1).min(8.0);
    let seat_top = h * 0.5;
    // Base and feet.
    ctx.legs(4.0, feet, 5.0, rgb(DARK));
    ctx.cube(
        [-w / 2.0, w / 2.0],
        [feet, seat_top - 12.0],
        [-d / 2.0, d / 2.0],
        shade(c, -0.15),
    );
    // Backrest and armrests.
    ctx.cube(
        [-w / 2.0, w / 2.0],
        [feet, h],
        [-d / 2.0, -d / 2.0 + back],
        c,
    );
    ctx.cube(
        [-w / 2.0, -w / 2.0 + arm],
        [feet, h * 0.68],
        [-d / 2.0, d / 2.0],
        c,
    );
    ctx.cube(
        [w / 2.0 - arm, w / 2.0],
        [feet, h * 0.68],
        [-d / 2.0, d / 2.0],
        c,
    );
    // Seat cushions with small gaps, and back cushions.
    let n = f64::from(seats.max(1));
    let inner = w - 2.0 * arm;
    let slot = inner / n;
    for i in 0..seats.max(1) {
        let x0 = -w / 2.0 + arm + f64::from(i) * slot + 0.8;
        let x1 = x0 + slot - 1.6;
        ctx.cube(
            [x0, x1],
            [seat_top - 12.0, seat_top],
            [-d / 2.0 + back, d / 2.0 - 2.0],
            shade(c, 0.08),
        );
        ctx.cube(
            [x0, x1],
            [seat_top, h * 0.92],
            [-d / 2.0 + back, -d / 2.0 + back + 12.0],
            shade(c, 0.12),
        );
    }
}

fn bed(ctx: &mut Ctx, pillows: u8) {
    let (w, d, h, c) = (ctx.w, ctx.d, ctx.h, ctx.c);
    let head = (d * 0.04).clamp(4.0, 8.0);
    let frame_top = h * 0.45;
    ctx.cube(
        [-w / 2.0, w / 2.0],
        [0.0, frame_top],
        [-d / 2.0, d / 2.0],
        rgb([150, 110, 80]),
    );
    // Headboard rises to the full height at the back.
    ctx.cube(
        [-w / 2.0, w / 2.0],
        [0.0, h],
        [-d / 2.0, -d / 2.0 + head],
        rgb([130, 95, 68]),
    );
    let mattress_top = h * 0.82;
    ctx.cube(
        [-w / 2.0 + 2.0, w / 2.0 - 2.0],
        [frame_top, mattress_top],
        [-d / 2.0 + head, d / 2.0 - 2.0],
        rgb(LINEN),
    );
    // Duvet over the foot two thirds.
    ctx.cube(
        [-w / 2.0 + 1.0, w / 2.0 - 1.0],
        [mattress_top - 6.0, mattress_top + 3.0],
        [-d / 2.0 + d * 0.33, d / 2.0],
        c,
    );
    let n = pillows.max(1);
    let slot = (w - 20.0) / f64::from(n);
    for i in 0..n {
        let x0 = -w / 2.0 + 10.0 + f64::from(i) * slot + 3.0;
        ctx.cube(
            [x0, x0 + slot - 6.0],
            [mattress_top, mattress_top + 10.0],
            [-d / 2.0 + head + 6.0, -d / 2.0 + head + 42.0],
            rgb([252, 252, 250]),
        );
    }
}

fn table(ctx: &mut Ctx, round: bool) {
    let (w, d, h, c) = (ctx.w, ctx.d, ctx.h, ctx.c);
    let top = 3.5_f64.min(h * 0.1);
    if round {
        ctx.m
            .frustum([0.0, h - top, 0.0], Axis::Y, top, w / 2.0, w / 2.0, 40, c);
        ctx.m.cylinder(
            [0.0, 0.0, 0.0],
            Axis::Y,
            h - top,
            (w * 0.06).max(3.0),
            shade(c, -0.2),
        );
        ctx.m.frustum(
            [0.0, 0.0, 0.0],
            Axis::Y,
            3.0,
            w * 0.25,
            w * 0.22,
            28,
            shade(c, -0.25),
        );
    } else {
        ctx.cube([-w / 2.0, w / 2.0], [h - top, h], [-d / 2.0, d / 2.0], c);
        ctx.legs(5.0, h - top, 6.0, shade(c, -0.2));
    }
}

fn chair(ctx: &mut Ctx) {
    let (w, d, h, c) = (ctx.w, ctx.d, ctx.h, ctx.c);
    let seat = h * 0.5;
    ctx.legs(1.0, seat - 4.0, 4.0, shade(c, -0.2));
    ctx.cube(
        [-w / 2.0, w / 2.0],
        [seat - 4.0, seat],
        [-d / 2.0, d / 2.0],
        c,
    );
    ctx.cube(
        [-w / 2.0, w / 2.0],
        [seat, h],
        [-d / 2.0, -d / 2.0 + 4.0],
        c,
    );
    ctx.cube(
        [-w / 2.0 + 1.0, -w / 2.0 + 5.0],
        [0.0, seat],
        [-d / 2.0, -d / 2.0 + 4.0],
        shade(c, -0.2),
    );
    ctx.cube(
        [w / 2.0 - 5.0, w / 2.0 - 1.0],
        [0.0, seat],
        [-d / 2.0, -d / 2.0 + 4.0],
        shade(c, -0.2),
    );
}

fn stool(ctx: &mut Ctx) {
    let (w, h, c) = (ctx.w, ctx.h, ctx.c);
    ctx.m
        .frustum([0.0, h - 5.0, 0.0], Axis::Y, 5.0, w / 2.0, w / 2.0, 28, c);
    ctx.m
        .cylinder([0.0, 0.0, 0.0], Axis::Y, h - 5.0, 2.5, rgb(METAL));
    ctx.m.frustum(
        [0.0, 0.0, 0.0],
        Axis::Y,
        2.0,
        w / 2.0 * 0.9,
        w / 2.0 * 0.8,
        28,
        rgb(METAL),
    );
}

fn office_chair(ctx: &mut Ctx) {
    let (w, d, h, c) = (ctx.w, ctx.d, ctx.h, ctx.c);
    let seat = h * 0.45;
    for i in 0..5 {
        let a = f64::from(i) / 5.0 * std::f64::consts::TAU;
        let (x, z) = (a.cos() * w * 0.24, a.sin() * d * 0.24);
        ctx.m.block(
            [x, z],
            0.0,
            [
                w * 0.5 * a.cos().abs().max(0.15),
                4.0,
                d * 0.5 * a.sin().abs().max(0.15),
            ],
            rgb(DARK),
        );
    }
    ctx.m
        .cylinder([0.0, 4.0, 0.0], Axis::Y, seat - 10.0, 2.5, rgb(METAL));
    ctx.cube(
        [-w * 0.4, w * 0.4],
        [seat - 8.0, seat],
        [-d * 0.4, d * 0.42],
        c,
    );
    ctx.cube([-w * 0.36, w * 0.36], [seat, h], [-d * 0.46, -d * 0.36], c);
}

fn cabinet(ctx: &mut Ctx, doors: u8, drawers: u8) {
    let (w, d, h, c) = (ctx.w, ctx.d, ctx.h, ctx.c);
    let plinth = if h > 60.0 { 8.0 } else { 3.0 };
    ctx.cube(
        [-w / 2.0 + 3.0, w / 2.0 - 3.0],
        [0.0, plinth],
        [-d / 2.0, d / 2.0 - 4.0],
        shade(c, -0.35),
    );
    ctx.cube(
        [-w / 2.0, w / 2.0],
        [plinth, h],
        [-d / 2.0, d / 2.0 - 2.0],
        c,
    );
    let front = d / 2.0 - 2.0;
    let panel = shade(c, 0.05);
    if drawers > 0 {
        let rows = f64::from(drawers);
        let step = (h - plinth) / rows;
        for i in 0..drawers {
            let y0 = plinth + f64::from(i) * step + 1.0;
            ctx.cube(
                [-w / 2.0 + 1.5, w / 2.0 - 1.5],
                [y0, y0 + step - 2.0],
                [front, front + 1.5],
                panel,
            );
            ctx.handle(0.0, y0 + step / 2.0, front + 1.5, false);
        }
    } else {
        let n = f64::from(doors.max(1));
        let step = w / n;
        for i in 0..doors.max(1) {
            let x0 = -w / 2.0 + f64::from(i) * step + 1.0;
            ctx.cube(
                [x0, x0 + step - 2.0],
                [plinth + 1.0, h - 1.0],
                [front, front + 1.5],
                panel,
            );
            let hx = if i % 2 == 0 {
                x0 + step - 7.0
            } else {
                x0 + 5.0
            };
            ctx.handle(hx, plinth + (h - plinth) * 0.55, front + 1.5, true);
        }
    }
}

fn shelf(ctx: &mut Ctx, levels: u8) {
    let (w, d, h, c) = (ctx.w, ctx.d, ctx.h, ctx.c);
    let t = 2.0;
    ctx.cube([-w / 2.0, -w / 2.0 + t], [0.0, h], [-d / 2.0, d / 2.0], c);
    ctx.cube([w / 2.0 - t, w / 2.0], [0.0, h], [-d / 2.0, d / 2.0], c);
    ctx.cube(
        [-w / 2.0, w / 2.0],
        [0.0, h],
        [-d / 2.0, -d / 2.0 + 1.0],
        shade(c, -0.2),
    );
    let n = levels.max(2);
    for i in 0..=n {
        let y = f64::from(i) / f64::from(n) * (h - t);
        ctx.cube(
            [-w / 2.0 + t, w / 2.0 - t],
            [y, y + t],
            [-d / 2.0 + 1.0, d / 2.0],
            c,
        );
    }
    // A few books on the middle shelves.
    let palette = [[160, 60, 50], [50, 90, 140], [210, 180, 90], [70, 120, 80]];
    for i in 1..n.saturating_sub(1) {
        let y = f64::from(i) / f64::from(n) * (h - t) + t;
        let mut x = -w / 2.0 + t + 3.0;
        let mut k: u32 = 0;
        while x < w / 2.0 - t - 20.0 {
            let bw = 3.0 + f64::from(k % 3);
            ctx.cube(
                [x, x + bw],
                [y, y + (h / f64::from(n)) * 0.7],
                [-d / 2.0 + 3.0, d / 2.0 - 6.0],
                rgb(palette[(usize::from(i) + k as usize) % 4]),
            );
            x += bw + 0.5;
            k += 1;
        }
    }
}

fn tv_stand(ctx: &mut Ctx) {
    cabinet(ctx, 0, 1);
}

fn tv(ctx: &mut Ctx) {
    let (w, d, h) = (ctx.w, ctx.d, ctx.h);
    let foot = (h * 0.08).min(6.0);
    ctx.cube(
        [-w * 0.15, w * 0.15],
        [0.0, 1.5],
        [-d / 2.0, d / 2.0],
        rgb(DARK),
    );
    ctx.cube([-3.0, 3.0], [0.0, foot + 5.0], [-1.0, 1.0], rgb(DARK));
    ctx.cube(
        [-w / 2.0, w / 2.0],
        [foot, h],
        [-d * 0.2, d * 0.2],
        rgb(DARK),
    );
    ctx.cube(
        [-w / 2.0 + 1.5, w / 2.0 - 1.5],
        [foot + 1.5, h - 1.5],
        [d * 0.2, d * 0.2 + 0.3],
        rgb([20, 24, 32]),
    );
}

fn fridge(ctx: &mut Ctx) {
    let (w, d, h, c) = (ctx.w, ctx.d, ctx.h, ctx.c);
    ctx.cube([-w / 2.0, w / 2.0], [0.0, h], [-d / 2.0, d / 2.0 - 3.0], c);
    let front = d / 2.0 - 3.0;
    let split = h * 0.32;
    ctx.cube(
        [-w / 2.0 + 0.5, w / 2.0 - 0.5],
        [1.0, split - 0.5],
        [front, front + 3.0],
        shade(c, 0.06),
    );
    ctx.cube(
        [-w / 2.0 + 0.5, w / 2.0 - 0.5],
        [split + 0.5, h - 0.5],
        [front, front + 3.0],
        shade(c, 0.06),
    );
    ctx.handle(w / 2.0 - 6.0, split + 30.0, front + 3.0, true);
    ctx.handle(w / 2.0 - 6.0, split - 20.0, front + 3.0, true);
}

fn stove(ctx: &mut Ctx) {
    let (w, d, h, c) = (ctx.w, ctx.d, ctx.h, ctx.c);
    ctx.cube(
        [-w / 2.0, w / 2.0],
        [0.0, h - 2.0],
        [-d / 2.0, d / 2.0 - 1.0],
        c,
    );
    ctx.cube(
        [-w / 2.0, w / 2.0],
        [h - 2.0, h - 1.0],
        [-d / 2.0, d / 2.0 - 1.0],
        rgb(DARK),
    );
    for (sx, sz) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
        ctx.m.cylinder(
            [sx * w * 0.22, h - 1.0, sz * d * 0.2],
            Axis::Y,
            1.0,
            (w * 0.09).min(7.0),
            rgb([90, 92, 96]),
        );
    }
    let front = d / 2.0 - 1.0;
    ctx.cube(
        [-w / 2.0 + 4.0, w / 2.0 - 4.0],
        [10.0, h * 0.7],
        [front, front + 1.0],
        rgb([30, 32, 36]),
    );
    ctx.handle(0.0, h * 0.74, front + 1.0, false);
}

fn counter(ctx: &mut Ctx, sink: bool) {
    let (w, d, h, c) = (ctx.w, ctx.d, ctx.h, ctx.c);
    let top = 4.0;
    ctx.cube(
        [-w / 2.0 + 1.0, w / 2.0 - 1.0],
        [0.0, 10.0],
        [-d / 2.0, d / 2.0 - 6.0],
        shade(c, -0.4),
    );
    ctx.cube(
        [-w / 2.0 + 1.0, w / 2.0 - 1.0],
        [10.0, h - top],
        [-d / 2.0, d / 2.0 - 4.0],
        c,
    );
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let doors = ((w / 60.0).round() as u8).clamp(1, 6);
    let step = (w - 2.0) / f64::from(doors);
    for i in 0..doors {
        let x0 = -w / 2.0 + 1.0 + f64::from(i) * step + 0.8;
        ctx.cube(
            [x0, x0 + step - 1.6],
            [11.0, h - top - 1.0],
            [d / 2.0 - 4.0, d / 2.0 - 2.5],
            shade(c, 0.05),
        );
        ctx.handle(x0 + step / 2.0, h - top - 8.0, d / 2.0 - 2.5, false);
    }
    let stone = rgb([70, 72, 78]);
    if sink {
        let (bw, bd) = ((w * 0.45).min(60.0), (d * 0.6).min(45.0));
        ctx.cube(
            [-w / 2.0, w / 2.0],
            [h - top, h],
            [-d / 2.0, -bd / 2.0],
            stone,
        );
        ctx.cube(
            [-w / 2.0, w / 2.0],
            [h - top, h],
            [bd / 2.0, d / 2.0],
            stone,
        );
        ctx.cube(
            [-w / 2.0, -bw / 2.0],
            [h - top, h],
            [-bd / 2.0, bd / 2.0],
            stone,
        );
        ctx.cube(
            [bw / 2.0, w / 2.0],
            [h - top, h],
            [-bd / 2.0, bd / 2.0],
            stone,
        );
        ctx.cube(
            [-bw / 2.0, bw / 2.0],
            [h - 20.0, h - top],
            [-bd / 2.0, bd / 2.0],
            rgb(METAL),
        );
        ctx.m.cylinder(
            [0.0, h - top, -bd / 2.0 - 4.0],
            Axis::Y,
            top,
            1.5,
            rgb(METAL),
        );
    } else {
        ctx.cube(
            [-w / 2.0, w / 2.0],
            [h - top, h],
            [-d / 2.0, d / 2.0],
            stone,
        );
    }
}

fn appliance(ctx: &mut Ctx, round_door: bool) {
    let (w, d, h, c) = (ctx.w, ctx.d, ctx.h, ctx.c);
    ctx.cube([-w / 2.0, w / 2.0], [0.0, h], [-d / 2.0, d / 2.0 - 2.0], c);
    let front = d / 2.0 - 2.0;
    ctx.cube(
        [-w / 2.0 + 2.0, w / 2.0 - 2.0],
        [h - 12.0, h - 2.0],
        [front, front + 1.0],
        shade(c, -0.1),
    );
    if round_door {
        let r = (w.min(h) * 0.3).min(22.0);
        ctx.m
            .cylinder([0.0, h * 0.45, front], Axis::Z, 2.0, r, rgb(METAL));
        ctx.m.cylinder(
            [0.0, h * 0.45, front + 2.0],
            Axis::Z,
            0.5,
            r * 0.75,
            rgb(GLASS),
        );
    } else {
        ctx.cube(
            [-w / 2.0 + 1.0, w / 2.0 - 1.0],
            [2.0, h - 13.0],
            [front, front + 1.5],
            shade(c, 0.05),
        );
        ctx.handle(0.0, h - 17.0, front + 1.5, false);
    }
}

fn microwave(ctx: &mut Ctx) {
    let (w, d, h, c) = (ctx.w, ctx.d, ctx.h, ctx.c);
    ctx.cube([-w / 2.0, w / 2.0], [0.0, h], [-d / 2.0, d / 2.0 - 1.0], c);
    ctx.cube(
        [-w / 2.0 + 2.0, w / 2.0 - 12.0],
        [3.0, h - 3.0],
        [d / 2.0 - 1.0, d / 2.0],
        rgb([20, 22, 28]),
    );
    ctx.cube(
        [w / 2.0 - 10.0, w / 2.0 - 2.0],
        [3.0, h - 3.0],
        [d / 2.0 - 1.0, d / 2.0],
        rgb(METAL),
    );
}

fn toilet(ctx: &mut Ctx) {
    let (w, d, h, c) = (ctx.w, ctx.d, ctx.h, ctx.c);
    let tank_d = (d * 0.25).clamp(12.0, 20.0);
    ctx.cube(
        [-w / 2.0, w / 2.0],
        [h * 0.5, h],
        [-d / 2.0, -d / 2.0 + tank_d],
        c,
    );
    let bowl_r = (w / 2.0).min((d - tank_d) / 2.0);
    let center = -d / 2.0 + tank_d + (d - tank_d) / 2.0;
    ctx.m.frustum(
        [0.0, 0.0, center],
        Axis::Y,
        h * 0.3,
        bowl_r * 0.55,
        bowl_r * 0.75,
        24,
        c,
    );
    ctx.m.frustum(
        [0.0, h * 0.3, center],
        Axis::Y,
        h * 0.22,
        bowl_r * 0.9,
        bowl_r,
        24,
        c,
    );
    ctx.cube(
        [-w / 2.0 + 2.0, w / 2.0 - 2.0],
        [h * 0.5 - 2.0, h * 0.53],
        [-d / 2.0 + tank_d, d / 2.0],
        shade(c, 0.05),
    );
}

fn basin(ctx: &mut Ctx) {
    let (w, d, h, c) = (ctx.w, ctx.d, ctx.h, ctx.c);
    cabinet(ctx, 2, 0);
    ctx.cube(
        [-w / 2.0, w / 2.0],
        [h - 3.0, h],
        [-d / 2.0, d / 2.0],
        shade(c, 0.05),
    );
    ctx.m.cylinder(
        [0.0, h - 3.0, 0.0],
        Axis::Y,
        0.4,
        (w.min(d) * 0.3).min(22.0),
        rgb([225, 228, 232]),
    );
}

fn shower(ctx: &mut Ctx) {
    let (w, d, h, c) = (ctx.w, ctx.d, ctx.h, ctx.c);
    ctx.cube(
        [-w / 2.0, w / 2.0],
        [0.0, 5.0],
        [-d / 2.0, d / 2.0],
        rgb([240, 240, 238]),
    );
    let glass = shade(c, 0.3);
    ctx.cube(
        [-w / 2.0, w / 2.0],
        [5.0, h],
        [d / 2.0 - 1.0, d / 2.0],
        glass,
    );
    ctx.cube(
        [w / 2.0 - 1.0, w / 2.0],
        [5.0, h],
        [-d / 2.0, d / 2.0],
        glass,
    );
    ctx.cube(
        [-w / 2.0, -w / 2.0 + 1.0],
        [h - 3.0, h],
        [-d / 2.0, d / 2.0],
        rgb(METAL),
    );
    ctx.m.cylinder(
        [-w / 2.0 + 10.0, h - 25.0, -d / 2.0 + 10.0],
        Axis::Y,
        3.0,
        8.0,
        rgb(METAL),
    );
}

fn bathtub(ctx: &mut Ctx) {
    let (w, d, h, c) = (ctx.w, ctx.d, ctx.h, ctx.c);
    let rim = 8.0;
    ctx.cube([-w / 2.0, w / 2.0], [0.0, h - 15.0], [-d / 2.0, d / 2.0], c);
    ctx.cube(
        [-w / 2.0, w / 2.0],
        [h - 15.0, h],
        [-d / 2.0, -d / 2.0 + rim],
        c,
    );
    ctx.cube(
        [-w / 2.0, w / 2.0],
        [h - 15.0, h],
        [d / 2.0 - rim, d / 2.0],
        c,
    );
    ctx.cube(
        [-w / 2.0, -w / 2.0 + rim],
        [h - 15.0, h],
        [-d / 2.0, d / 2.0],
        c,
    );
    ctx.cube(
        [w / 2.0 - rim, w / 2.0],
        [h - 15.0, h],
        [-d / 2.0, d / 2.0],
        c,
    );
    ctx.cube(
        [-w / 2.0 + rim, w / 2.0 - rim],
        [h - 16.0, h - 15.0],
        [-d / 2.0 + rim, d / 2.0 - rim],
        rgb([180, 210, 225]),
    );
}

fn door(ctx: &mut Ctx, leaves: u8, sliding: bool) {
    let (w, d, h, c) = (ctx.w, ctx.d, ctx.h, ctx.c);
    let frame = 5.0;
    let trim = rgb([240, 238, 232]);
    ctx.cube(
        [-w / 2.0, -w / 2.0 + frame],
        [0.0, h],
        [-d / 2.0, d / 2.0],
        trim,
    );
    ctx.cube(
        [w / 2.0 - frame, w / 2.0],
        [0.0, h],
        [-d / 2.0, d / 2.0],
        trim,
    );
    ctx.cube(
        [-w / 2.0, w / 2.0],
        [h - frame, h],
        [-d / 2.0, d / 2.0],
        trim,
    );
    let inner = w - 2.0 * frame;
    let n = f64::from(leaves.max(1));
    for i in 0..leaves.max(1) {
        let x0 = -w / 2.0 + frame + f64::from(i) * inner / n;
        let z = if sliding {
            (f64::from(i) - 0.5) * 3.5
        } else {
            0.0
        };
        if sliding {
            ctx.cube(
                [x0 - 1.0, x0 + inner / n + 1.0],
                [0.0, h - frame],
                [z - 1.5, z + 1.5],
                rgb(GLASS),
            );
            ctx.cube(
                [x0 - 1.0, x0 + 3.0],
                [0.0, h - frame],
                [z - 1.8, z + 1.8],
                rgb(METAL),
            );
        } else {
            ctx.cube(
                [x0 + 0.3, x0 + inner / n - 0.3],
                [0.5, h - frame - 0.3],
                [-2.0, 2.0],
                c,
            );
            let hx = if i == 0 {
                x0 + inner / n - 8.0
            } else {
                x0 + 8.0
            };
            ctx.handle(hx, 100.0_f64.min(h * 0.5), 2.0, false);
            ctx.cube(
                [hx - 6.0, hx + 6.0],
                [100.0_f64.min(h * 0.5) - 0.75, 100.0_f64.min(h * 0.5) + 0.75],
                [-4.0, -2.0],
                rgb(METAL),
            );
        }
    }
}

fn window(ctx: &mut Ctx, panes: u8, _is_window: bool) {
    let (w, d, h) = (ctx.w, ctx.d, ctx.h);
    let frame = 5.0;
    let white = rgb([245, 245, 242]);
    ctx.cube(
        [-w / 2.0, w / 2.0],
        [0.0, frame],
        [-d / 2.0, d / 2.0],
        white,
    );
    ctx.cube(
        [-w / 2.0, w / 2.0],
        [h - frame, h],
        [-d / 2.0, d / 2.0],
        white,
    );
    ctx.cube(
        [-w / 2.0, -w / 2.0 + frame],
        [0.0, h],
        [-d / 2.0, d / 2.0],
        white,
    );
    ctx.cube(
        [w / 2.0 - frame, w / 2.0],
        [0.0, h],
        [-d / 2.0, d / 2.0],
        white,
    );
    let n = panes.max(1);
    let step = (w - 2.0 * frame) / f64::from(n);
    for i in 1..n {
        let x = -w / 2.0 + frame + f64::from(i) * step;
        ctx.cube([x - 1.5, x + 1.5], [frame, h - frame], [-2.0, 2.0], white);
    }
    ctx.cube(
        [-w / 2.0 + frame, w / 2.0 - frame],
        [frame, h - frame],
        [-0.4, 0.4],
        rgb(GLASS),
    );
}

fn passage(ctx: &mut Ctx) {
    let (w, d, h) = (ctx.w, ctx.d, ctx.h);
    let trim = rgb([240, 238, 232]);
    ctx.cube(
        [-w / 2.0, -w / 2.0 + 2.0],
        [0.0, h],
        [-d / 2.0, d / 2.0],
        trim,
    );
    ctx.cube(
        [w / 2.0 - 2.0, w / 2.0],
        [0.0, h],
        [-d / 2.0, d / 2.0],
        trim,
    );
    ctx.cube([-w / 2.0, w / 2.0], [h - 2.0, h], [-d / 2.0, d / 2.0], trim);
}

fn stairs(ctx: &mut Ctx, steps: u8) {
    let (w, d, h, c) = (ctx.w, ctx.d, ctx.h, ctx.c);
    let n = f64::from(steps.max(2));
    let rise = h / n;
    let run = d / n;
    // Climbs from the front (+z) towards the back (-z).
    for i in 0..steps.max(2) {
        let k = f64::from(i);
        let z1 = d / 2.0 - k * run;
        ctx.cube(
            [-w / 2.0, w / 2.0],
            [0.0, rise * (k + 1.0)],
            [z1 - run, z1],
            if i % 2 == 0 { c } else { shade(c, 0.05) },
        );
    }
}

fn plant(ctx: &mut Ctx) {
    let (w, h, c) = (ctx.w, ctx.h, ctx.c);
    let pot_h = h * 0.3;
    ctx.m.frustum(
        [0.0, 0.0, 0.0],
        Axis::Y,
        pot_h,
        w * 0.25,
        w * 0.32,
        20,
        rgb([180, 110, 80]),
    );
    ctx.m.frustum(
        [0.0, pot_h, 0.0],
        Axis::Y,
        (h - pot_h) * 0.55,
        w * 0.5,
        w * 0.35,
        16,
        c,
    );
    ctx.m.frustum(
        [0.0, pot_h + (h - pot_h) * 0.45, 0.0],
        Axis::Y,
        (h - pot_h) * 0.55,
        w * 0.38,
        0.0,
        16,
        shade(c, 0.12),
    );
}

fn tree(ctx: &mut Ctx) {
    let (w, h, c) = (ctx.w, ctx.h, ctx.c);
    let trunk = h * 0.35;
    ctx.m.cylinder(
        [0.0, 0.0, 0.0],
        Axis::Y,
        trunk,
        (w * 0.05).max(8.0),
        rgb([110, 80, 55]),
    );
    ctx.m.frustum(
        [0.0, trunk * 0.8, 0.0],
        Axis::Y,
        (h - trunk * 0.8) * 0.6,
        w * 0.5,
        w * 0.3,
        18,
        c,
    );
    ctx.m.frustum(
        [0.0, trunk * 0.8 + (h - trunk * 0.8) * 0.5, 0.0],
        Axis::Y,
        (h - trunk * 0.8) * 0.5,
        w * 0.34,
        0.0,
        18,
        shade(c, 0.1),
    );
}

fn lamp(ctx: &mut Ctx) {
    let (w, h, c) = (ctx.w, ctx.h, ctx.c);
    ctx.m.frustum(
        [0.0, 0.0, 0.0],
        Axis::Y,
        3.0,
        w * 0.35,
        w * 0.3,
        20,
        rgb(DARK),
    );
    ctx.m
        .cylinder([0.0, 3.0, 0.0], Axis::Y, h * 0.7, 1.2, rgb(METAL));
    ctx.m.frustum(
        [0.0, h * 0.68, 0.0],
        Axis::Y,
        h * 0.32,
        w * 0.5,
        w * 0.3,
        24,
        c,
    );
}

fn car(ctx: &mut Ctx) {
    let (w, d, h, c) = (ctx.w, ctx.d, ctx.h, ctx.c);
    // Length runs along the width axis.
    let wheel_r = (h * 0.22).min(34.0);
    for sx in [-1.0, 1.0] {
        for sz in [-1.0, 1.0] {
            let z0 = if sz > 0.0 { d / 2.0 - 12.0 } else { -d / 2.0 };
            ctx.m.cylinder(
                [sx * w * 0.3, wheel_r, z0],
                Axis::Z,
                12.0,
                wheel_r,
                rgb(DARK),
            );
        }
    }
    ctx.cube(
        [-w / 2.0, w / 2.0],
        [wheel_r * 0.6, h * 0.55],
        [-d / 2.0 + 8.0, d / 2.0 - 8.0],
        c,
    );
    ctx.cube(
        [-w * 0.22, w * 0.25],
        [h * 0.55, h],
        [-d / 2.0 + 16.0, d / 2.0 - 16.0],
        shade(c, 0.05),
    );
    ctx.cube(
        [-w * 0.2, w * 0.23],
        [h * 0.58, h * 0.95],
        [-d / 2.0 + 15.5, d / 2.0 - 15.5],
        rgb(GLASS),
    );
    ctx.cube(
        [-w / 2.0, w / 2.0],
        [0.0, 0.1],
        [-d / 2.0, d / 2.0],
        rgb(DARK),
    );
}

fn desk(ctx: &mut Ctx) {
    let (w, d, h, c) = (ctx.w, ctx.d, ctx.h, ctx.c);
    ctx.cube([-w / 2.0, w / 2.0], [h - 3.0, h], [-d / 2.0, d / 2.0], c);
    ctx.cube(
        [-w / 2.0, -w / 2.0 + 3.0],
        [0.0, h - 3.0],
        [-d / 2.0, d / 2.0],
        shade(c, -0.1),
    );
    let drawer_w = (w * 0.35).min(45.0);
    ctx.cube(
        [w / 2.0 - drawer_w, w / 2.0],
        [0.0, h - 3.0],
        [-d / 2.0, d / 2.0 - 1.5],
        shade(c, -0.05),
    );
    for i in 0..3 {
        let y0 = 4.0 + f64::from(i) * (h - 8.0) / 3.0;
        ctx.cube(
            [w / 2.0 - drawer_w + 1.0, w / 2.0 - 1.0],
            [y0, y0 + (h - 8.0) / 3.0 - 1.5],
            [d / 2.0 - 1.5, d / 2.0],
            shade(c, 0.05),
        );
        ctx.handle(
            w / 2.0 - drawer_w / 2.0,
            y0 + (h - 8.0) / 6.0,
            d / 2.0,
            false,
        );
    }
    ctx.cube(
        [-w / 2.0 + 3.0, w / 2.0 - drawer_w],
        [h - 25.0, h - 3.0],
        [-d / 2.0, -d / 2.0 + 2.0],
        shade(c, -0.1),
    );
}

fn crib(ctx: &mut Ctx) {
    let (w, d, h, c) = (ctx.w, ctx.d, ctx.h, ctx.c);
    ctx.legs(0.0, h, 5.0, c);
    ctx.cube(
        [-w / 2.0 + 5.0, w / 2.0 - 5.0],
        [25.0, 35.0],
        [-d / 2.0 + 5.0, d / 2.0 - 5.0],
        rgb(LINEN),
    );
    for (z0, z1) in [(-d / 2.0, -d / 2.0 + 3.0), (d / 2.0 - 3.0, d / 2.0)] {
        ctx.cube([-w / 2.0, w / 2.0], [h - 5.0, h], [z0, z1], c);
        let mut x = -w / 2.0 + 10.0;
        while x < w / 2.0 - 8.0 {
            ctx.cube([x, x + 2.0], [35.0, h - 5.0], [z0, z1], c);
            x += 9.0;
        }
    }
    for (x0, x1) in [(-w / 2.0, -w / 2.0 + 3.0), (w / 2.0 - 3.0, w / 2.0)] {
        ctx.cube([x0, x1], [h - 5.0, h], [-d / 2.0, d / 2.0], c);
        let mut z = -d / 2.0 + 10.0;
        while z < d / 2.0 - 8.0 {
            ctx.cube([x0, x1], [35.0, h - 5.0], [z, z + 2.0], c);
            z += 9.0;
        }
    }
}
