//! The Blender-style move gizmo on the component being placed: dragging an arrow moves the
//! component along that world axis only, the centre moves it freely, and constraints still win.

use eframe::egui::{self, Event, PointerButton, Pos2, Rect};
use freo::app::gizmo::Handle;
use freo::app::{App, Pick};
use freo::kernel::math::*;
use freo::model::features::*;
use freo::sketch::sketch::Sketch;

fn part(name: &str, sk: Sketch, depth: f64) -> ModelData {
    let mut m = ModelData::new_part(name);
    let FeatureKind::Extrude(mut e) = FeatureKind::new_extrude() else { unreachable!() };
    e.plane = Some(PlaneRef::Datum(TOP_ID));
    e.sketch = sk;
    e.depth = Dim::v(depth);
    m.features.push(Feature { id: 10, name: "Extrude 1".into(), suppressed: false, hidden: false, kind: FeatureKind::Extrude(e) });
    m
}

/// Assembly with a base plate and a block being placed (no constraints yet).
fn placing_block() -> App {
    let mut app = App::new(None);
    app.new_assembly("ASM0001");
    for m in [part("BASE", Sketch::rect(100.0, 60.0, 0.0, 0.0), 20.0), part("BLK", Sketch::rect(30.0, 20.0, 0.0, 0.0), 10.0)] {
        let name = app.asm.as_mut().unwrap().data.add_part(m, None);
        app.assemble_part(&name);
        if app.asm.as_ref().unwrap().data.components.is_empty() {
            app.place_finish();
        }
    }
    assert!(app.placing());
    app
}

/// Runs the viewport for a press at `from`, a drag through `steps` moves to `to`, and a release.
fn drag(app: &mut App, from: Pos2, to: Pos2, steps: usize) {
    let ctx = egui::Context::default();
    let screen = Rect::from_min_size(Pos2::ZERO, egui::vec2(1000.0, 700.0));
    let frame = |app: &mut App, events| {
        let input = egui::RawInput { screen_rect: Some(screen), events, ..Default::default() };
        let _ = ctx.run(input, |ctx| {
            egui::CentralPanel::default().frame(egui::Frame::NONE).show(ctx, |ui| app.viewport_ui(ui));
        });
    };
    frame(app, vec![Event::PointerMoved(from)]);
    frame(app, vec![Event::PointerButton { pos: from, button: PointerButton::Primary, pressed: true, modifiers: Default::default() }]);
    for k in 1..=steps {
        let p = from + (to - from) * (k as f32 / steps as f32);
        frame(app, vec![Event::PointerMoved(p)]);
    }
    frame(app, vec![Event::PointerButton { pos: to, button: PointerButton::Primary, pressed: false, modifiers: Default::default() }]);
}

fn settle(app: &mut App) {
    // one frame lays out the viewport so the gizmo knows the viewport size
    drag(app, Pos2::new(5.0, 5.0), Pos2::new(5.0, 5.0), 0);
}

fn pos(app: &App) -> V3 {
    app.place_editor().unwrap().comp.placement.pos
}

#[test]
fn arrows_move_along_one_axis_and_centre_moves_freely() {
    let mut app = placing_block();
    settle(&mut app);
    for axis in 0..3 {
        let g = app.gizmo_layout().expect("gizmo while placing");
        let tip = g.tips[axis].expect("arrow visible in the standard view");
        assert_eq!(g.hit(tip), Some(Handle::Axis(axis)));
        let start = pos(&app);
        // grab the arrow halfway and pull it 60 px outwards (and a bit sideways, which is ignored)
        let grab = g.centre + (tip - g.centre) * 0.6;
        let dir = (tip - g.centre).normalized();
        let side = egui::vec2(-dir.y, dir.x);
        drag(&mut app, grab, grab + dir * 60.0 + side * 25.0, 6);
        let moved = pos(&app) - start;
        let along = moved.dot(freo::app::gizmo::AXES[axis]);
        assert!(along > 5.0, "axis {axis}: moved {:?}", moved);
        assert!((moved - freo::app::gizmo::AXES[axis] * along).length() < 1e-6, "axis {axis}: only along the axis, got {:?}", moved);
        // 60 px along the arrow is 60 px on screen
        let expect = 60.0 / g.px_per_mm[axis] as f64;
        assert!((along - expect).abs() < 0.05 * expect, "axis {axis}: {along} vs {expect}");
    }
    // the centre handle moves in the screen plane (two axes at once)
    let g = app.gizmo_layout().unwrap();
    assert_eq!(g.hit(g.centre), Some(Handle::Centre));
    let start = pos(&app);
    drag(&mut app, g.centre, g.centre + egui::vec2(40.0, 30.0), 4);
    let moved = pos(&app) - start;
    assert!(moved.length() > 5.0 && moved.dot(app.camera.back()).abs() < 1e-6, "{:?}", moved);
    // a click on the gizmo does not pick the geometry behind it as a reference
    let n = app.place_editor().unwrap().comp.constraints.iter().filter(|c| c.a.is_some() || c.b.is_some()).count();
    let g = app.gizmo_layout().unwrap();
    drag(&mut app, g.centre, g.centre, 0);
    assert_eq!(app.place_editor().unwrap().comp.constraints.iter().filter(|c| c.a.is_some() || c.b.is_some()).count(), n);
}

#[test]
fn constraints_still_hold_while_dragging_an_arrow() {
    let mut app = placing_block();
    settle(&mut app);
    let id = app.place_editor().unwrap().comp.id;
    // mate the block's bottom onto the base's top: Y is no longer free, X and Z are
    let r = app.displayed().clone();
    let t = &r.topo;
    let face = |comp: u64, up: bool| {
        (0..t.faces.len())
            .filter(|&f| r.tag_owner.get(&t.face_tag(f)) == Some(&comp) && t.faces[f].planar)
            .max_by(|&a, &b| {
                let s = |f: usize| if up { t.faces[f].normal.y * 1000.0 + t.faces[f].center.y } else { -t.faces[f].normal.y * 1000.0 - t.faces[f].center.y };
                s(a).total_cmp(&s(b))
            })
            .unwrap()
    };
    let (bottom, top) = (face(id, false), face(100, true));
    app.handle_pick(Some(Pick::Face(bottom)), false);
    app.handle_pick(Some(Pick::Face(top)), false);
    let y0 = pos(&app).y;
    for axis in [1, 0] {
        let g = app.gizmo_layout().unwrap();
        let tip = g.tips[axis].unwrap();
        let grab = g.centre + (tip - g.centre) * 0.6;
        let start = pos(&app);
        drag(&mut app, grab, grab + (tip - g.centre).normalized() * 50.0, 5);
        let moved = pos(&app) - start;
        if axis == 1 {
            assert!(moved.length() < 1e-6, "Y is held by the mate: {:?}", moved);
        } else {
            assert!(moved.x > 5.0 && moved.y.abs() < 1e-6, "slides along X on the face: {:?}", moved);
        }
    }
    assert!((pos(&app).y - y0).abs() < 1e-6);
}

#[test]
fn no_gizmo_outside_placement() {
    let mut app = placing_block();
    app.place_cancel();
    assert!(app.gizmo_layout().is_none());
}

#[test]
fn rings_turn_the_part_about_one_axis_through_the_centre() {
    let mut app = placing_block();
    settle(&mut app);
    for axis in 0..3 {
        let g = app.gizmo_layout().unwrap();
        let grab = g.ring_point(axis).expect("ring visible in the standard view");
        assert_eq!(g.hit(grab), Some(Handle::Ring(axis)), "axis {axis}");
        let before = app.place_editor().unwrap().comp.placement;
        let origin = g.origin;
        // drag 30° counter-clockwise on screen around the gizmo centre (screen y points down)
        let r = grab - g.centre;
        let arc = |deg: f32| {
            let (s, c) = (-deg.to_radians()).sin_cos();
            g.centre + egui::vec2(r.x * c - r.y * s, r.x * s + r.y * c)
        };
        let ctx = egui::Context::default();
        let screen = Rect::from_min_size(Pos2::ZERO, egui::vec2(1000.0, 700.0));
        let frame = |app: &mut App, events| {
            let input = egui::RawInput { screen_rect: Some(screen), events, ..Default::default() };
            let _ = ctx.run(input, |ctx| {
                egui::CentralPanel::default().frame(egui::Frame::NONE).show(ctx, |ui| app.viewport_ui(ui));
            });
        };
        frame(&mut app, vec![Event::PointerMoved(grab)]);
        frame(&mut app, vec![Event::PointerButton { pos: grab, button: PointerButton::Primary, pressed: true, modifiers: Default::default() }]);
        for k in 1..=15 {
            frame(&mut app, vec![Event::PointerMoved(arc(2.0 * k as f32))]);
        }
        frame(&mut app, vec![Event::PointerButton { pos: arc(30.0), button: PointerButton::Primary, pressed: false, modifiers: Default::default() }]);
        let after = app.place_editor().unwrap().comp.placement;
        let (ax, angle) = (after.rot * before.rot.inverse()).to_axis_angle();
        let signed = if ax.dot(freo::app::gizmo::AXES[axis]) >= 0.0 { angle } else { -angle };
        assert!(ax.cross(freo::app::gizmo::AXES[axis]).length() < 1e-6, "axis {axis}: turned about {:?}", ax);
        // a counter-clockwise screen drag turns positively about an axis that faces the viewer
        let expect = 30f64.to_radians() * g.facing[axis] as f64;
        assert!((signed - expect).abs() < 3f64.to_radians(), "axis {axis}: {} ° vs {} °", signed.to_degrees(), expect.to_degrees());
        // the pivot is the gizmo centre: it stays where it was
        let local = before.inverse().point(origin);
        assert!((after.point(local) - origin).length() < 1e-6, "axis {axis}: pivot moved");
    }
}
