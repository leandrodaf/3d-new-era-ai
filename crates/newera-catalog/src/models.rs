//! Procedural 3D models. Every generator works in the piece's real size and
//! the result is snapped to exactly `width × depth × height`.

use newera_core::{Furniture, OpeningKind};

use crate::Model;
use crate::mesh::{Axis, Mesh, Rgb, rgb, shade};

const METAL: [u8; 3] = [150, 154, 160];
use crate::mesh::GLASS;
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
    let opening = piece.opening.as_ref();
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
        Model::Cooktop => cooktop(&mut ctx),
        Model::SinkBowl => sink_bowl(&mut ctx),
        Model::Oven => oven(&mut ctx),
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
        Model::Footing => footing(&mut ctx),
        Model::Railing => railing(&mut ctx),
        Model::Fence => fence(&mut ctx),
        Model::Corrugated => corrugated(&mut ctx),
        Model::GlassPanel => glass_panel(&mut ctx),
        Model::GarageDoor => garage_door(&mut ctx),
        Model::Pool => pool(&mut ctx),
        Model::Lounger => lounger(&mut ctx),
        Model::Grill => grill(&mut ctx),
        Model::SofaL => sofa_l(&mut ctx),
        Model::Bench => bench(&mut ctx),
        Model::DiningSet { chairs } => dining_set(&mut ctx, chairs),
        Model::Planter => planter(&mut ctx),
        Model::Rug | Model::Box | Model::Point(_) => {
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

fn cooktop(ctx: &mut Ctx) {
    let (w, d, h) = (ctx.w, ctx.d, ctx.h);
    // The burner box hangs under the counter; the glass shows on top.
    let glass = 0.6_f64.min(h);
    ctx.cube(
        [-w / 2.0 + 3.0, w / 2.0 - 3.0],
        [0.0, h - glass],
        [-d / 2.0 + 3.0, d / 2.0 - 3.0],
        rgb([60, 62, 66]),
    );
    ctx.cube(
        [-w / 2.0, w / 2.0],
        [h - glass, h],
        [-d / 2.0, d / 2.0],
        rgb([22, 22, 26]),
    );
    for (sx, sz, r) in [
        (-0.22, -0.2, 7.0),
        (0.22, -0.2, 5.5),
        (-0.22, 0.2, 5.5),
        (0.22, 0.2, 8.5),
    ] {
        ctx.m.cylinder(
            [sx * w, h, sz * d],
            Axis::Y,
            0.15,
            f64::min(r, w * 0.12),
            rgb([70, 70, 76]),
        );
    }
}

fn sink_bowl(ctx: &mut Ctx) {
    let (w, d, h, c) = (ctx.w, ctx.d, ctx.h, ctx.c);
    let wall = 0.8;
    // Rim on the counter, bowl below it.
    ctx.cube(
        [-w / 2.0, w / 2.0],
        [h - 0.5, h],
        [-d / 2.0, -d / 2.0 + 2.5],
        c,
    );
    ctx.cube(
        [-w / 2.0, w / 2.0],
        [h - 0.5, h],
        [d / 2.0 - 2.5, d / 2.0],
        c,
    );
    ctx.cube(
        [-w / 2.0, -w / 2.0 + 2.5],
        [h - 0.5, h],
        [-d / 2.0, d / 2.0],
        c,
    );
    ctx.cube(
        [w / 2.0 - 2.5, w / 2.0],
        [h - 0.5, h],
        [-d / 2.0, d / 2.0],
        c,
    );
    let (iw, id) = (w / 2.0 - 2.5, d / 2.0 - 2.5);
    ctx.cube([-iw, iw], [0.0, wall], [-id, id], shade(c, -0.15));
    ctx.cube([-iw, iw], [0.0, h - 0.5], [-id, -id + wall], c);
    ctx.cube([-iw, iw], [0.0, h - 0.5], [id - wall, id], c);
    ctx.cube([-iw, -iw + wall], [0.0, h - 0.5], [-id, id], c);
    ctx.cube([iw - wall, iw], [0.0, h - 0.5], [-id, id], c);
    ctx.m
        .cylinder([0.0, wall, 0.0], Axis::Y, 0.2, 2.5, rgb([120, 122, 126]));
}

fn oven(ctx: &mut Ctx) {
    let (w, d, h, c) = (ctx.w, ctx.d, ctx.h, ctx.c);
    ctx.cube([-w / 2.0, w / 2.0], [0.0, h], [-d / 2.0, d / 2.0 - 2.0], c);
    let front = d / 2.0 - 2.0;
    ctx.cube(
        [-w / 2.0 + 1.0, w / 2.0 - 1.0],
        [2.0, h - 10.0],
        [front, front + 2.0],
        rgb([18, 18, 22]),
    );
    ctx.cube(
        [-w / 2.0 + 1.0, w / 2.0 - 1.0],
        [h - 9.0, h - 1.0],
        [front, front + 1.5],
        rgb(METAL),
    );
    ctx.handle(0.0, h - 13.0, front + 2.0, false);
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

fn footing(ctx: &mut Ctx) {
    let (w, d, h, c) = (ctx.w, ctx.d, ctx.h, ctx.c);
    let base = h * 0.4;
    ctx.cube(
        [-w / 2.0, w / 2.0],
        [0.0, base],
        [-d / 2.0, d / 2.0],
        shade(c, -0.1),
    );
    let (pw, pd) = (w * 0.45, d * 0.45);
    ctx.cube([-pw / 2.0, pw / 2.0], [base, h], [-pd / 2.0, pd / 2.0], c);
}

fn railing(ctx: &mut Ctx) {
    let (w, d, h, c) = (ctx.w, ctx.d, ctx.h, ctx.c);
    let post = d.clamp(3.0, 6.0);
    let spans = (w / 120.0).ceil().max(1.0);
    for i in 0..=crate::count(spans) {
        let x = -w / 2.0 + post / 2.0 + (w - post) * f64::from(i) / spans;
        ctx.cube(
            [x - post / 2.0, x + post / 2.0],
            [0.0, h],
            [-d / 2.0, d / 2.0],
            c,
        );
    }
    for y in [h - post, h * 0.55, 10.0] {
        ctx.cube(
            [-w / 2.0, w / 2.0],
            [y, y + post * 0.6],
            [-d / 2.0, d / 2.0],
            c,
        );
    }
}

fn fence(ctx: &mut Ctx) {
    let (w, d, h, c) = (ctx.w, ctx.d, ctx.h, ctx.c);
    let post = d.clamp(6.0, 12.0);
    let spans = (w / 200.0).ceil().max(1.0);
    for i in 0..=crate::count(spans) {
        let x = -w / 2.0 + post / 2.0 + (w - post) * f64::from(i) / spans;
        ctx.cube(
            [x - post / 2.0, x + post / 2.0],
            [0.0, h],
            [-d / 2.0, d / 2.0],
            shade(c, -0.15),
        );
    }
    let boards = (h / 15.0).floor().max(2.0);
    let pitch = h / boards;
    for i in 0..crate::count(boards) {
        let y = f64::from(i) * pitch + 1.0;
        ctx.cube(
            [-w / 2.0, w / 2.0],
            [y, y + pitch - 2.0],
            [-d / 4.0, d / 4.0],
            c,
        );
    }
}

fn corrugated(ctx: &mut Ctx) {
    let (w, d, h, c) = (ctx.w, ctx.d, ctx.h, ctx.c);
    // Ridges every ~18 cm across the width, running along the depth.
    let waves = (w / 18.0).round().max(2.0);
    let step = w / waves;
    for i in 0..crate::count(waves) {
        let x = -w / 2.0 + f64::from(i) * step;
        let top = [x + step * 0.25, x + step * 0.75];
        ctx.cube(
            [x, x + step],
            [0.0, h * 0.35],
            [-d / 2.0, d / 2.0],
            shade(c, -0.08),
        );
        ctx.cube([top[0], top[1]], [h * 0.35, h], [-d / 2.0, d / 2.0], c);
    }
}

fn glass_panel(ctx: &mut Ctx) {
    let (w, d, h, c) = (ctx.w, ctx.d, ctx.h, ctx.c);
    let frame = 2.0_f64.min(w / 10.0);
    ctx.cube(
        [-w / 2.0 + frame, w / 2.0 - frame],
        [frame, h - frame],
        [-d / 2.0, d / 2.0],
        c,
    );
    let metal = rgb(METAL);
    ctx.cube(
        [-w / 2.0, w / 2.0],
        [0.0, frame],
        [-d / 2.0, d / 2.0],
        metal,
    );
    ctx.cube(
        [-w / 2.0, w / 2.0],
        [h - frame, h],
        [-d / 2.0, d / 2.0],
        metal,
    );
    ctx.cube(
        [-w / 2.0, -w / 2.0 + frame],
        [0.0, h],
        [-d / 2.0, d / 2.0],
        metal,
    );
    ctx.cube(
        [w / 2.0 - frame, w / 2.0],
        [0.0, h],
        [-d / 2.0, d / 2.0],
        metal,
    );
}

fn garage_door(ctx: &mut Ctx) {
    let (w, d, h, c) = (ctx.w, ctx.d, ctx.h, ctx.c);
    let panels = (h / 50.0).round().max(3.0);
    let step = h / panels;
    for i in 0..crate::count(panels) {
        let y = f64::from(i) * step;
        ctx.cube(
            [-w / 2.0, w / 2.0],
            [y + 0.6, y + step - 0.6],
            [-2.0, 2.0],
            if i % 2 == 0 { c } else { shade(c, -0.05) },
        );
    }
    ctx.cube(
        [-w / 2.0, -w / 2.0 + 5.0],
        [0.0, h],
        [-d / 2.0, d / 2.0],
        rgb(DARK),
    );
    ctx.cube(
        [w / 2.0 - 5.0, w / 2.0],
        [0.0, h],
        [-d / 2.0, d / 2.0],
        rgb(DARK),
    );
    ctx.handle(0.0, h * 0.2, 2.0, false);
}

fn pool(ctx: &mut Ctx) {
    let (w, d, h, c) = (ctx.w, ctx.d, ctx.h, ctx.c);
    let coping = 30.0_f64.min(w / 6.0).min(d / 6.0);
    let stone = rgb([225, 220, 205]);
    ctx.cube(
        [-w / 2.0, w / 2.0],
        [0.0, h],
        [-d / 2.0, -d / 2.0 + coping],
        stone,
    );
    ctx.cube(
        [-w / 2.0, w / 2.0],
        [0.0, h],
        [d / 2.0 - coping, d / 2.0],
        stone,
    );
    ctx.cube(
        [-w / 2.0, -w / 2.0 + coping],
        [0.0, h],
        [-d / 2.0 + coping, d / 2.0 - coping],
        stone,
    );
    ctx.cube(
        [w / 2.0 - coping, w / 2.0],
        [0.0, h],
        [-d / 2.0 + coping, d / 2.0 - coping],
        stone,
    );
    ctx.cube(
        [-w / 2.0 + coping, w / 2.0 - coping],
        [0.0, h * 0.7],
        [-d / 2.0 + coping, d / 2.0 - coping],
        c,
    );
}

fn lounger(ctx: &mut Ctx) {
    let (w, d, h, c) = (ctx.w, ctx.d, ctx.h, ctx.c);
    let seat = h * 0.4;
    ctx.legs(3.0, seat - 5.0, 4.0, rgb(METAL));
    ctx.cube(
        [-w / 2.0, w / 2.0],
        [seat - 5.0, seat],
        [-d / 2.0 + d * 0.3, d / 2.0],
        c,
    );
    // Raised back over the first third, as a slanted slab.
    let (z0, z1) = (-d / 2.0, -d / 2.0 + d * 0.3);
    let (x0, x1) = (-w / 2.0, w / 2.0);
    #[allow(clippy::cast_possible_truncation)]
    let v = |x: f64, y: f64, z: f64| [x as f32, y as f32, z as f32];
    let slab = [v(x0, h, z0), v(x1, h, z0), v(x1, seat, z1), v(x0, seat, z1)];
    ctx.m.polygon(&[slab[3], slab[2], slab[1], slab[0]], c);
    ctx.m
        .polygon(&[slab[0], slab[1], slab[2], slab[3]], shade(c, -0.1));
}

fn grill(ctx: &mut Ctx) {
    let (w, d, h, c) = (ctx.w, ctx.d, ctx.h, ctx.c);
    let counter = h * 0.4;
    ctx.cube([-w / 2.0, w / 2.0], [0.0, counter], [-d / 2.0, d / 2.0], c);
    ctx.cube(
        [-w / 2.0, w / 2.0],
        [counter, counter + 4.0],
        [-d / 2.0, d / 2.0],
        rgb([60, 60, 62]),
    );
    // Firebox opening and chimney hood.
    ctx.cube(
        [-w * 0.35, w * 0.35],
        [counter + 4.0, counter + 40.0],
        [-d / 2.0, -d / 2.0 + 10.0],
        c,
    );
    ctx.cube(
        [-w / 2.0, w / 2.0],
        [counter + 40.0, counter + 70.0],
        [-d / 2.0, d / 2.0],
        shade(c, -0.1),
    );
    ctx.cube(
        [-w * 0.2, w * 0.2],
        [counter + 70.0, h],
        [-d * 0.3, d * 0.2],
        c,
    );
}

fn sofa_l(ctx: &mut Ctx) {
    let (w, d, h, c) = (ctx.w, ctx.d, ctx.h, ctx.c);
    let seat_d = (d * 0.55).min(95.0);
    let back = 22.0_f64.min(seat_d * 0.25);
    let arm = 18.0_f64.min(w * 0.08);
    let seat = h * 0.5;
    let chaise = (w * 0.3).max(70.0).min(w * 0.5);
    // Main seat along the back.
    ctx.cube(
        [-w / 2.0, w / 2.0],
        [0.0, seat],
        [-d / 2.0, -d / 2.0 + seat_d],
        shade(c, -0.1),
    );
    ctx.cube(
        [-w / 2.0, w / 2.0],
        [0.0, h],
        [-d / 2.0, -d / 2.0 + back],
        c,
    );
    ctx.cube(
        [-w / 2.0, -w / 2.0 + arm],
        [0.0, h * 0.68],
        [-d / 2.0, -d / 2.0 + seat_d],
        c,
    );
    // Chaise running forward on the right.
    ctx.cube(
        [w / 2.0 - chaise, w / 2.0],
        [0.0, seat],
        [-d / 2.0 + seat_d, d / 2.0],
        shade(c, -0.1),
    );
    ctx.cube(
        [w / 2.0 - arm, w / 2.0],
        [0.0, h * 0.68],
        [-d / 2.0, d / 2.0],
        c,
    );
}

fn bench(ctx: &mut Ctx) {
    let (w, d, h, c) = (ctx.w, ctx.d, ctx.h, ctx.c);
    ctx.cube([-w / 2.0, w / 2.0], [h - 5.0, h], [-d / 2.0, d / 2.0], c);
    for x in [-w / 2.0 + 8.0, w / 2.0 - 14.0] {
        ctx.cube(
            [x, x + 6.0],
            [0.0, h - 5.0],
            [-d / 2.0 + 4.0, d / 2.0 - 4.0],
            shade(c, -0.2),
        );
    }
}

fn dining_set(ctx: &mut Ctx, chairs: u8) {
    let (w, d, h, c) = (ctx.w, ctx.d, ctx.h, ctx.c);
    let chair = 45.0_f64.min(d * 0.3);
    let (tw, td) = (w - 2.0 * chair * 0.6, d - 2.0 * chair);
    let top = 75.0_f64.min(h * 0.85);
    // Table in the middle.
    ctx.cube(
        [-tw / 2.0, tw / 2.0],
        [top - 4.0, top],
        [-td / 2.0, td / 2.0],
        c,
    );
    ctx.legs(chair + 5.0, top - 4.0, 6.0, shade(c, -0.2));
    let per_side = u32::from(chairs.max(2)) / 2;
    let seat = 45.0_f64.min(top * 0.6);
    let fabric = rgb([150, 140, 125]);
    for side in [-1.0, 1.0] {
        for i in 0..per_side {
            let x = -tw / 2.0 + tw * (f64::from(i) + 0.5) / f64::from(per_side);
            let (z0, z1) = if side < 0.0 {
                (-d / 2.0, -d / 2.0 + chair)
            } else {
                (d / 2.0 - chair, d / 2.0)
            };
            ctx.cube(
                [x - chair / 2.2, x + chair / 2.2],
                [seat - 4.0, seat],
                [z0 + 3.0, z1 - 3.0],
                fabric,
            );
            let back = if side < 0.0 {
                [z0, z0 + 4.0]
            } else {
                [z1 - 4.0, z1]
            };
            ctx.cube(
                [x - chair / 2.2, x + chair / 2.2],
                [seat - 4.0, h],
                back,
                fabric,
            );
            ctx.cube(
                [x - 2.0, x + 2.0],
                [0.0, seat - 4.0],
                [f64::midpoint(z0, z1) - 2.0, f64::midpoint(z0, z1) + 2.0],
                shade(c, -0.2),
            );
        }
    }
}

fn planter(ctx: &mut Ctx) {
    let (w, d, h, c) = (ctx.w, ctx.d, ctx.h, ctx.c);
    let soil = h * 0.55;
    ctx.cube([-w / 2.0, w / 2.0], [0.0, soil], [-d / 2.0, d / 2.0], c);
    let green = rgb([70, 140, 80]);
    let clumps = (w / 30.0).round().max(1.0);
    for i in 0..crate::count(clumps) {
        let x = -w / 2.0 + w * (f64::from(i) + 0.5) / clumps;
        ctx.m.frustum(
            [x, soil, 0.0],
            Axis::Y,
            h - soil,
            (w / clumps / 2.0).min(d / 2.0),
            2.0,
            10,
            shade(green, if i % 2 == 0 { 0.0 } else { 0.1 }),
        );
    }
}

/// Places a 2D point of a solid's ring at sweep position 0 or 1.
type Lift<'a> = Box<dyn Fn([f64; 2], f64) -> [f32; 3] + 'a>;

/// A polygon swept into a closed solid: plan outlines go up by the height,
/// profiles run along the depth. Points are centered on the piece.
pub(crate) fn solid(shape: &newera_core::SolidShape, piece: &Furniture, color: Rgb) -> Mesh {
    use newera_core::{Point2, SolidShape};
    let mut m = Mesh::default();
    // `ring` in its own 2D plane, `lift(p, t)` puts point p at sweep t ∈ {0, 1}.
    let (ring, lift): (Vec<[f64; 2]>, Lift<'_>) = match shape {
        SolidShape::Outline(points) => (
            points.clone(),
            #[allow(clippy::cast_possible_truncation)]
            Box::new(move |p, t| [p[0] as f32, (t * piece.height) as f32, p[1] as f32]),
        ),
        SolidShape::Profile(points) => (
            points.clone(),
            #[allow(clippy::cast_possible_truncation)]
            Box::new(move |p, t| [p[0] as f32, p[1] as f32, ((t - 0.5) * piece.depth) as f32]),
        ),
    };
    if ring.len() < 3 {
        return m;
    }
    let as_points: Vec<Point2> = ring.iter().map(|p| Point2::new(p[0], p[1])).collect();
    // Counter-clockwise in its plane so caps and sides face out.
    let ccw = newera_core::signed_area(&as_points) > 0.0;
    let flip_outline = matches!(shape, SolidShape::Outline(_));
    let tris = newera_core::triangulate(&as_points);
    let cap = |m: &mut Mesh, t: f64, up: bool| {
        for [a, b, c] in &tris {
            let (a, b, c) = (lift(ring[*a], t), lift(ring[*b], t), lift(ring[*c], t));
            // Outlines map plan y to z, which mirrors their winding.
            if up == (ccw != flip_outline) {
                m.polygon(&[a, b, c], color);
            } else {
                m.polygon(&[a, c, b], color);
            }
        }
    };
    cap(&mut m, 0.0, false);
    cap(&mut m, 1.0, true);
    let n = ring.len();
    for i in 0..n {
        let (p, q) = (ring[i], ring[(i + 1) % n]);
        let quad = [lift(p, 0.0), lift(q, 0.0), lift(q, 1.0), lift(p, 1.0)];
        if ccw == flip_outline {
            m.polygon(&[quad[3], quad[2], quad[1], quad[0]], shade(color, -0.05));
        } else {
            m.polygon(&quad, shade(color, -0.05));
        }
    }
    m.fit_to(piece.width, piece.depth, piece.height);
    m
}

#[cfg(test)]
mod solid_tests {
    use newera_core::{Furniture, SolidShape};

    use crate::mesh::Rgb;

    fn faces_out(mesh: &crate::Mesh) {
        #[allow(clippy::cast_precision_loss)]
        let n = mesh.positions.len() as f32;
        let center = mesh.positions.iter().fold([0.0f32; 3], |c, p| {
            [c[0] + p[0] / n, c[1] + p[1] / n, c[2] + p[2] / n]
        });
        for tri in mesh.indices.chunks(3) {
            let [a, b, c] = [0, 1, 2].map(|k| mesh.positions[tri[k] as usize]);
            let normal = mesh.normals[tri[0] as usize];
            let mid = [
                (a[0] + b[0] + c[0]) / 3.0,
                (a[1] + b[1] + c[1]) / 3.0,
                (a[2] + b[2] + c[2]) / 3.0,
            ];
            let out = [mid[0] - center[0], mid[1] - center[1], mid[2] - center[2]];
            let dot = out[0] * normal[0] + out[1] * normal[1] + out[2] * normal[2];
            assert!(dot > -1e-3, "face points inward: {tri:?}");
        }
    }

    #[test]
    fn outlines_and_profiles_are_closed_outward_solids() {
        let grey: Rgb = [0.5; 3];
        for points in [
            vec![
                [-100.0, -50.0],
                [100.0, -50.0],
                [100.0, 50.0],
                [-100.0, 50.0],
            ],
            vec![
                [-100.0, -50.0],
                [-100.0, 50.0],
                [100.0, 50.0],
                [100.0, -50.0],
            ],
        ] {
            let piece = Furniture {
                width: 200.0,
                depth: 100.0,
                height: 20.0,
                ..Furniture::default()
            };
            let mesh = super::solid(&SolidShape::Outline(points), &piece, grey);
            assert_eq!(mesh.indices.len() / 3, 2 + 2 + 8);
            faces_out(&mesh);
        }
        // A 600 cm wide, 675 cm high gable swept 20 cm.
        for points in [
            vec![[-300.0, 0.0], [300.0, 0.0], [0.0, 675.0]],
            vec![[-300.0, 0.0], [0.0, 675.0], [300.0, 0.0]],
        ] {
            let piece = Furniture {
                width: 600.0,
                depth: 20.0,
                height: 675.0,
                ..Furniture::default()
            };
            let mesh = super::solid(&SolidShape::Profile(points), &piece, grey);
            faces_out(&mesh);
            let top = mesh.positions.iter().map(|p| p[1]).fold(f32::MIN, f32::max);
            assert!((top - 675.0).abs() < 1e-3);
        }
    }
}
