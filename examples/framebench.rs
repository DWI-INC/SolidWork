//! Headless frame-time benchmark: runs the whole UI (`eframe::App::update` plus tessellation)
//! with synthetic input for orbit, zoom, hover and assembly drag on a heavy part and on the
//! sample assembly.  `cargo run --release --example framebench`
//!
//! GPU work is not included (there is no GL context); everything the UI thread does per frame is.

use eframe::egui::{self, Event, Modifiers, PointerButton, Pos2, Rect};
use freo::app::App;
use freo::model::features::*;
use std::time::Instant;

const W: f32 = 1440.0;
const H: f32 = 900.0;

struct Bench {
    ctx: egui::Context,
    t: f64,
}

impl Bench {
    fn new() -> Bench {
        Bench { ctx: egui::Context::default(), t: 0.0 }
    }

    /// One frame; returns its UI-thread time in milliseconds.
    fn frame(&mut self, app: &mut App, events: Vec<Event>, mods: Modifiers) -> f64 {
        self.t += 1.0 / 60.0;
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(W, H))),
            events,
            modifiers: mods,
            time: Some(self.t),
            predicted_dt: 1.0 / 60.0,
            ..Default::default()
        };
        let t0 = Instant::now();
        let out = self.ctx.run(input, |ctx| eframe::App::update(app, ctx, &mut eframe::Frame::_new_kittest()));
        let n = out.shapes.len();
        let prims = self.ctx.tessellate(out.shapes, out.pixels_per_point);
        if std::env::var_os("BENCH_SHAPES").is_some() {
            eprintln!("shapes {n} prims {}", prims.len());
        }
        t0.elapsed().as_secs_f64() * 1000.0
    }

    fn idle(&mut self, app: &mut App, n: usize) {
        for _ in 0..n {
            self.frame(app, vec![], Modifiers::NONE);
        }
    }

    /// Viewport centre in screen coordinates (the central panel).
    fn centre(&self) -> Pos2 {
        Pos2::new(W * 0.62, H * 0.55)
    }

    fn hover(&mut self, app: &mut App, n: usize) -> Vec<f64> {
        let c = self.centre();
        (0..n)
            .map(|k| {
                let a = k as f32 * 0.21;
                let p = c + egui::vec2(a.cos() * 180.0, a.sin() * 120.0) * (0.3 + 0.7 * (k as f32 / n as f32));
                self.frame(app, vec![Event::PointerMoved(p)], Modifiers::NONE)
            })
            .collect()
    }

    fn drag(&mut self, app: &mut App, button: PointerButton, mods: Modifiers, n: usize, step: egui::Vec2) -> Vec<f64> {
        let mut p = self.centre();
        self.frame(app, vec![Event::PointerMoved(p)], mods);
        self.frame(app, vec![Event::PointerButton { pos: p, button, pressed: true, modifiers: mods }], mods);
        let mut times = vec![];
        for k in 0..n {
            let s = if k % 40 < 20 { step } else { -step };
            p += s;
            times.push(self.frame(app, vec![Event::PointerMoved(p)], mods));
        }
        self.frame(app, vec![Event::PointerButton { pos: p, button, pressed: false, modifiers: mods }], mods);
        times
    }

    fn zoom(&mut self, app: &mut App, n: usize) -> Vec<f64> {
        let c = self.centre();
        self.frame(app, vec![Event::PointerMoved(c)], Modifiers::NONE);
        (0..n)
            .map(|k| {
                let d = if k % 20 < 10 { 30.0 } else { -30.0 };
                self.frame(app, vec![Event::MouseWheel { unit: egui::MouseWheelUnit::Point, delta: egui::vec2(0.0, d), modifiers: Modifiers::NONE }], Modifiers::NONE)
            })
            .collect()
    }
}

fn report(name: &str, mut t: Vec<f64>) {
    t.sort_by(|a, b| a.total_cmp(b));
    let n = t.len();
    let mean = t.iter().sum::<f64>() / n as f64;
    println!("{name:<34} mean {mean:7.2} ms   p50 {:7.2}   p95 {:7.2}   max {:7.2}", t[n / 2], t[(n * 95 / 100).min(n - 1)], t[n - 1]);
}

fn heavy_part() -> ModelData {
    freo::app::demo::heavy_part()
}

fn main() {
    let rep: usize = std::env::var("BENCH_N").ok().and_then(|v| v.parse().ok()).unwrap_or(1);
    let only = std::env::args().nth(1).unwrap_or_default();
    if only.is_empty() || only == "part" {
        let mut app = App::new(None);
        app.data = heavy_part();
        app.regenerate(false);
        app.fit_pending = true;
        let r = app.displayed();
        println!("heavy part: {} polys, {} chains, {} faces, regen {:.0} ms", r.topo.polys.len(), r.topo.chains.len(), r.topo.faces.len(), r.time * 1000.0);
        let mut b = Bench::new();
        b.idle(&mut app, 5);
        report("part  idle", (0..60 * rep).map(|_| b.frame(&mut app, vec![], Modifiers::NONE)).collect());
        report("part  hover", b.hover(&mut app, 120 * rep));
        report("part  orbit (middle drag)", b.drag(&mut app, PointerButton::Middle, Modifiers::NONE, 120, egui::vec2(6.0, 2.0)));
        report("part  pan (shift middle)", b.drag(&mut app, PointerButton::Middle, Modifiers::SHIFT, 120, egui::vec2(5.0, 3.0)));
        report("part  zoom (wheel)", b.zoom(&mut app, 120));
    }
    if only == "edit" {
        let mut app = App::new(None);
        app.async_preview = std::env::var_os("BENCH_SYNC").is_none();
        app.data = heavy_part();
        app.regenerate(false);
        for id in [14u64, 13, 11] {
            let t0 = Instant::now();
            app.edit_feature(id);
            println!("open editor of feature {id}: {:.0} ms", t0.elapsed().as_secs_f64() * 1000.0);
            let t0 = Instant::now();
            app.update_editor(|f| match &mut f.kind {
                FeatureKind::Hole(h) => h.diameter = Dim::v(15.0),
                FeatureKind::Pattern(p) => p.spacing1 = Dim::v(10.5),
                FeatureKind::Extrude(e) => e.depth = Dim::v(31.0),
                _ => {}
            });
            println!("edit preview of feature {id}: {:.0} ms", t0.elapsed().as_secs_f64() * 1000.0);
            let t0 = Instant::now();
            app.update_editor(|f| match &mut f.kind {
                FeatureKind::Hole(h) => h.diameter = Dim::v(15.5),
                FeatureKind::Pattern(p) => p.spacing1 = Dim::v(10.6),
                FeatureKind::Extrude(e) => e.depth = Dim::v(31.5),
                _ => {}
            });
            println!("  second change: {:.0} ms", t0.elapsed().as_secs_f64() * 1000.0);
            let t0 = Instant::now();
            app.settle_preview();
            println!("  preview ready after another {:.0} ms", t0.elapsed().as_secs_f64() * 1000.0);
            app.cancel_feature();
        }
    }
    if only.is_empty() || only == "asm" {
        let mut app = App::new(None);
        app.demo_setup("assembly");
        let r = app.displayed();
        println!("sample assembly: {} polys, {} chains, home {} drawing {}", r.topo.polys.len(), r.topo.chains.len(), app.creo.home, app.drawing_active());
        let mut b = Bench::new();
        b.idle(&mut app, 5);
        println!("viewport {:?}", app.viewport_size);
        report("asm   idle", (0..60).map(|_| b.frame(&mut app, vec![], Modifiers::NONE)).collect());
        report("asm   hover", b.hover(&mut app, 120));
        report("asm   orbit (middle drag)", b.drag(&mut app, PointerButton::Middle, Modifiers::NONE, 120, egui::vec2(6.0, 2.0)));
        report("asm   zoom (wheel)", b.zoom(&mut app, 120));
    }
    if only.is_empty() || only == "heavyasm" {
        let data = freo::app::demo::heavy_assembly();
        let mut app = App::new(None);
        app.data = data.skeleton.clone();
        app.asm = Some(freo::app::assembly::AsmSession::new(data));
        app.asm_regenerate(false);
        app.fit_pending = true;
        let r = app.displayed();
        println!("heavy assembly: {} polys, {} chains", r.topo.polys.len(), r.topo.chains.len());
        let mut b = Bench::new();
        b.idle(&mut app, 5);
        report("hasm  idle", (0..60).map(|_| b.frame(&mut app, vec![], Modifiers::NONE)).collect());
        report("hasm  hover", b.hover(&mut app, 120));
        report("hasm  orbit (middle drag)", b.drag(&mut app, PointerButton::Middle, Modifiers::NONE, 120, egui::vec2(6.0, 2.0)));
        app.assemble_part("BOLT_M10");
        assert!(app.placing());
        b.idle(&mut app, 3);
        report("hasm  drag component", b.drag(&mut app, PointerButton::Primary, Modifiers::NONE, 60 * rep, egui::vec2(5.0, 2.0)));
        report("hasm  rotate component (shift)", b.drag(&mut app, PointerButton::Primary, Modifiers::SHIFT, 60, egui::vec2(5.0, 2.0)));
        report("hasm  hover while placing", b.hover(&mut app, 60));
        app.place_cancel();
        b.idle(&mut app, 3);
        app.edit_placement(110);
        b.idle(&mut app, 3);
        report("hasm  drag heavy component", b.drag(&mut app, PointerButton::Primary, Modifiers::NONE, 30 * rep, egui::vec2(5.0, 2.0)));
    }
    for scene in ["sketch", "round", "extrude", "editdims"] {
        if !(only.is_empty() || only == scene) {
            continue;
        }
        let mut app = App::new(None);
        app.demo_setup(scene);
        let mut b = Bench::new();
        b.idle(&mut app, 5);
        report(&format!("{scene:<5} idle"), (0..60).map(|_| b.frame(&mut app, vec![], Modifiers::NONE)).collect());
        report(&format!("{scene:<5} hover"), b.hover(&mut app, 120));
        report(&format!("{scene:<5} orbit"), b.drag(&mut app, PointerButton::Middle, Modifiers::NONE, 120, egui::vec2(6.0, 2.0)));
        report(&format!("{scene:<5} zoom"), b.zoom(&mut app, 60));
    }
    if only.is_empty() || only == "drag" {
        let mut app = App::new(None);
        app.demo_setup("assemble");
        // a free component (no constraints yet): drag it around and rotate it
        if let Some(e) = app.asm.as_mut().and_then(|a| a.place.as_mut()) {
            for c in &mut e.comp.constraints {
                c.a = None;
                c.b = None;
            }
        }
        let mut b = Bench::new();
        b.idle(&mut app, 5);
        assert!(app.placing(), "assemble scene places a bolt");
        report("asm   drag component", b.drag(&mut app, PointerButton::Primary, Modifiers::NONE, 120, egui::vec2(5.0, 2.0)));
        report("asm   rotate component (shift)", b.drag(&mut app, PointerButton::Primary, Modifiers::SHIFT, 120, egui::vec2(5.0, 2.0)));
        report("asm   hover while placing", b.hover(&mut app, 120));
    }
}
