//! The Assembler tool uses real placement, library, routing and file entry points.
use freo::app::{App, Tab};
use freo::kernel::math::{V2, V3};
use freo::model::{
    fastener::{screw, Head},
    regen,
    threads::ISO,
};
use freo::pcb::{
    bench::{Bench, Drive, LedState, Level},
    board::{FpInst, Track, F_CU},
    body3d, footprints,
    modules::{Module, MODULES},
    Board, Schematic,
};

#[test]
fn screws_have_a_thread_bearing_face_and_recess() {
    for spec in [&ISO[0], &ISO[2], ISO.last().unwrap()] {
        for head in Head::ALL {
            let m = screw(spec, 10, head).unwrap();
            let r = regen::run(&m);
            assert!(r.errors.is_empty(), "{head:?}: {:?}", r.errors);
            assert_eq!(r.cosmetics.len(), 1);
            assert!(!r.cosmetics[0].internal);
            assert_eq!(r.cosmetics[0].pitch, spec.pitch);
            assert!((r.topo.bbox.min.y + 10.0).abs() < 1e-6);
            let top = r.topo.bbox.max.y;
            let depth = r.topo.ray_first_hit(V3::new(0.01, top + 1.0, 0.01), -V3::Y, 1e-5).unwrap();
            if head == Head::Hex {
                assert!((depth - 1.0).abs() < 1e-5);
            } else {
                assert!(depth > top * 0.5 + 1.0, "missing drive {head:?}");
            }
        }
    }
}

#[test]
fn modules_transfer_from_the_schematic_to_numbered_lands_and_solids() {
    let mut sch = Schematic::default();
    for (i, m) in MODULES.iter().enumerate() {
        sch.add_symbol(m.id(), V2::new(i as f64 * 100.0, 0.0)).unwrap();
    }
    sch.annotate(false);
    let mut b = Board::default();
    assert_eq!(b.update_from(&sch).added.len(), 2);
    for (i, m) in MODULES.iter().enumerate() {
        let f = &b.footprints[i];
        let pins = m.pins();
        assert_eq!(pins.len(), if *m == Module::Pi4 { 40 } else { 38 });
        for pin in pins {
            assert!(sch.def(m.id()).unwrap().pin(&pin.number).is_some());
            assert_eq!(f.fp.pads.iter().find(|q| q.num == pin.number).unwrap().pos, pin.pos);
        }
        let (part, _) = body3d::module_part(*m).unwrap();
        let r = regen::run(&part);
        assert!(r.errors.is_empty(), "{:?}", r.errors);
        for &(x, z) in m.mounts() {
            assert!(r.topo.ray_first_hit(V3::new(x, 40.0, z), -V3::Y, 0.01).is_none());
        }
    }
}

fn chip_pad(board: &Board, name: &str) -> V2 {
    Module::Esp32.pins().iter().find(|p| p.name == name).map(|p| board.footprints[0].xform(p.pos)).unwrap()
}

#[test]
fn bench_follows_real_copper_and_measures_led_current() {
    let mut b = Module::Esp32.board();
    let src = chip_pad(&b, "GPIO25");
    let sink = chip_pad(&b, "GPIO26");
    fn pad(b: &Bench, name: &str) -> usize {
        b.contacts.iter().position(|p| p.name == name).unwrap()
    }
    b.footprints[0].nets = vec!["SAME".into(); 38];
    let mut bench = Bench::new(&b);
    let (out, other) = (pad(&bench, "GPIO25"), pad(&bench, "GPIO26"));
    bench.contacts[out].drive = Drive::High;
    assert_eq!(bench.read().pins[out], Level::Floating);
    bench.chips[0].usb = true;
    assert_eq!(bench.read().pins[out], Level::High);
    assert_eq!(bench.read().pins[other], Level::Floating);
    let track = |a, b| Track { a, b, width: 0.25, layer: F_CU, net: "different label".into() };
    b.tracks.push(track(src, sink));
    let mut bench = Bench::new(&b);
    bench.chips[0].usb = true;
    bench.contacts[out].drive = Drive::High;
    assert_eq!(bench.read().pins[other], Level::High);
    bench.contacts[other].drive = Drive::Low;
    assert_eq!(bench.read().pins[out], Level::Conflict);
    assert!(!bench.contacts[pad(&bench, "GPIO34")].output);

    fn add(b: &mut Board, id: &str, reference: &str, value: &str, pos: V2) -> (V2, V2) {
        let fp = footprints::find(id).unwrap().clone();
        let f = FpInst { fp, reference: reference.into(), value: value.into(), pos, rot: 0.0, bottom: false, nets: vec![String::new(); 2], locked: false };
        let pads = (f.pad_pos(0), f.pad_pos(1));
        b.footprints.push(f);
        pads
    }
    b.tracks.clear();
    let (r1, r2) = add(&mut b, "Resistor_SMD:R_0805_2012Metric", "R1", "330", src - V2::new(11.0, 0.0));
    let (k, a) = add(&mut b, "LED_SMD:LED_0805_2012Metric", "D1", "red", src - V2::new(18.0, 0.0));
    let gnd = chip_pad(&b, "GND");
    let corner = V2::new(k.x, gnd.y);
    b.tracks.extend([track(src, r2), track(r1, a), track(k, corner), track(corner, gnd)]);
    let mut bench = Bench::new(&b);
    bench.chips[0].usb = true;
    bench.contacts[out].drive = Drive::High;
    let LedState::Current(ma) = bench.read().leds[0].1 else { panic!("unresolved LED") };
    assert!((ma - 1.3 / 330.0 * 1000.0).abs() < 0.01);
    bench.contacts[out].drive = Drive::Low;
    assert_eq!(bench.read().leds[0].1, LedState::Current(0.0));
}

#[test]
fn assemble_board_and_hardware_cancel_undo_save_and_reopen() {
    let mut app = App::new(None);
    app.data = freo::app::demo::bracket();
    app.dirty = true;
    app.open_assembler();
    assert_eq!(app.tab, Tab::Tools);
    assert_eq!(app.asm.as_ref().unwrap().data.components[0].part, "BRACKET");
    app.assembler_place_screw(2, 10, Head::Socket);
    app.place_cancel();
    assert_eq!(app.asm.as_ref().unwrap().data.parts.len(), 1);
    app.assembler_place_screw(2, 10, Head::Socket);
    app.assembler_place_freely();
    app.place_finish();
    app.assembler_place_screw(2, 10, Head::Socket);
    app.assembler_place_freely();
    app.place_finish();
    assert_eq!(app.asm.as_ref().unwrap().data.parts.len(), 2);
    app.pcb.project.name = "Controller".into();
    let mut board = Module::Esp32.board();
    board.rect_outline(-10.0, -10.0, 50.0, 75.0);
    app.pcb.project.board = board.clone();
    app.pcb_open();
    app.pcb_make_3d();
    assert!(app.placing());
    app.assembler_place_freely();
    app.place_finish();
    assert_eq!(app.tab, Tab::Tools);
    let a = app.asm.as_ref().unwrap();
    assert!(a.res.messages.is_empty(), "{:?}", a.res.messages);
    assert_eq!(a.data.components.len(), 4);
    assert!(a.data.part("Controller").unwrap().board.is_some());
    app.do_undo();
    assert!(app.asm.as_ref().unwrap().data.part("Controller").is_none());
    app.do_redo();
    let path = std::env::temp_dir().join(format!("freo-assembler-{}.pfasm", std::process::id()));
    app.asm_save_to(path.clone());
    let mut other = App::new(None);
    other.load(path.clone());
    let a = other.asm.as_ref().unwrap();
    assert_eq!(a.data.components.len(), 4);
    let saved = a.data.part("Controller").unwrap().board.as_ref().unwrap();
    assert_eq!(Bench::new(saved).contacts.len(), 38);
    assert_eq!(saved.footprints[0].fp.pads, board.footprints[0].fp.pads);
    std::fs::remove_file(path).unwrap();
}

// Exercise the actual viewport event path, including camera projection, filtering and hit targets.
fn click_model(app: &mut App, point: V3) {
    use eframe::egui::{self, Event, PointerButton, Pos2, Rect};
    let ctx = egui::Context::default();
    let screen = Rect::from_min_size(Pos2::ZERO, egui::vec2(1000.0, 700.0));
    let frame = |app: &mut App, events| {
        let input = egui::RawInput { screen_rect: Some(screen), events, ..Default::default() };
        let _ = ctx.run(input, |ctx| {
            egui::CentralPanel::default().frame(egui::Frame::NONE).show(ctx, |ui| app.viewport_ui(ui));
        });
    };
    frame(app, vec![]);
    let (x, y, _) = app.camera.project(point, app.viewport_size).unwrap();
    let pos = egui::pos2(x, y);
    assert!(screen.contains(pos));
    for pressed in [true, false] {
        frame(app, vec![Event::PointerMoved(pos), Event::PointerButton { pos, button: PointerButton::Primary, pressed, modifiers: Default::default() }]);
    }
}

#[test]
fn screw_seats_in_a_pi_mounting_hole() {
    let mut app = App::new(None);
    app.go_home();
    app.open_assembler();
    app.assembler_place_module(Module::Pi4);
    assert!(!app.placing(), "the first part is the base");
    app.assembler_place_screw(1, 8, Head::Phillips);
    app.camera.rot = freo::app::camera::StdView::Top.rotation();
    let (x, z) = Module::Pi4.mounts()[0];
    click_model(&mut app, V3::new(x, 1.6, z));
    let ed = app.place_editor().unwrap();
    assert!(ed.status.is_full(), "{:?}", ed.status);
    assert!((ed.comp.placement.pos - V3::new(x, 1.6, z)).length() < 1e-4, "{:?}", ed.comp.placement.pos);
    app.place_finish();
    app.run_interference();
    let hits = app.asm.as_ref().unwrap().interference.as_ref().unwrap();
    assert!(hits.is_empty(), "{hits:?}");
}

#[test]
fn bench_supply_runs_a_module_through_header_pads() {
    for module in MODULES {
        let b = module.board();
        let mut bench = Bench::new(&b);
        let out = bench.contacts.iter().position(|pin| pin.name == "GPIO17").unwrap();
        bench.contacts[out].drive = Drive::High;
        bench.supply.on = true;
        assert_eq!(bench.read().pins[out], Level::High);
        bench.supply.five_volts = false;
        assert_eq!(bench.read().pins[out], Level::Floating);
        bench.supply.positive = bench.contacts.iter().position(|pin| pin.name == "3V3").unwrap();
        assert_eq!(bench.read().running[0], module == Module::Esp32);
        bench.supply.ground = bench.supply.positive;
        assert_eq!(bench.read().pins[bench.supply.positive], Level::Conflict);
        assert_eq!(bench.read().pins[out], Level::Floating);
    }
}

#[test]
fn plated_drill_remains_open_through_the_trace_and_copper_pad() {
    let mut b = Board::default();
    b.rect_outline(-10.0, -10.0, 20.0, 20.0);
    b.vias.push(freo::pcb::board::Via { pos: V2::ZERO, dia: 4.5, drill: 2.7, net: "GND".into() });
    b.tracks.push(Track { a: V2::ZERO, b: V2::new(7.0, 0.0), width: 0.5, layer: F_CU, net: "GND".into() });
    let populated = body3d::populated_part(&b, "CONTROL").unwrap();
    let mesh = regen::run(&populated);
    assert!(mesh.errors.is_empty());
    assert!(mesh.topo.ray_first_hit(V3::Y * 10.0, -V3::Y, 0.01).is_none(), "the screw bore is open through both foils");
    let top = mesh.topo.ray_first_hit(V3::new(1.8, 10.0, 0.0), -V3::Y, 0.01).unwrap();
    assert!((10.0 - top - b.thickness - body3d::COPPER_T).abs() < 1e-5);
}

fn click_controls(app: &mut App, labels: &[&str]) {
    use eframe::egui::{self, Event, PointerButton, Pos2, Rect, Shape};
    let ctx = egui::Context::default();
    let mut frame = |events: Vec<Event>| {
        let input = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(1280.0, 800.0))), events, ..Default::default() };
        ctx.run(input, |ctx| eframe::App::update(app, ctx, &mut eframe::Frame::_new_kittest())).shapes
    };
    for &target in labels {
        frame(vec![]);
        let shapes = frame(vec![]);
        let pos = shapes
            .iter()
            .find_map(|s| match &s.shape {
                Shape::Text(t) if t.galley.text() == target => Some(t.visual_bounding_rect().center()),
                _ => None,
            })
            .unwrap_or_else(|| panic!("missing button {target}"));
        for pressed in [true, false] {
            frame(vec![Event::PointerMoved(pos), Event::PointerButton { pos, button: PointerButton::Primary, pressed, modifiers: Default::default() }]);
        }
    }
}
#[test]
fn tools_ribbon_opens_assembler_and_starts_screw_placement() {
    let mut app = App::new(None);
    app.data = freo::app::demo::bracket();
    app.tab = Tab::Tools;
    click_controls(&mut app, &["Assembler", "Schraube", "Platzieren"]);
    assert!(app.placing());
    assert!(app.place_editor().unwrap().comp.part.contains("Phillips"));
}

#[test]
fn batch_import_snap_and_restore_contacts() {
    use freo::model::assembly::{AsmCType, AsmConstraint, Placement};
    let dir = std::env::temp_dir().join(format!("freo-guided-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let sample = freo::app::demo::sample_assembly();
    let paths: Vec<_> = sample
        .parts
        .iter()
        .take(3)
        .map(|p| {
            let path = dir.join(format!("{}.pfprt", p.data.name));
            std::fs::write(&path, serde_json::to_vec(&p.data).unwrap()).unwrap();
            path
        })
        .collect();
    let mut app = App::new(None);
    app.go_home();
    click_controls(&mut app, &["Assembler"]);
    app.queue_assembler_files(paths.clone());
    assert_eq!(app.asm.as_ref().unwrap().data.components.len(), 1);
    assert_eq!(app.place_editor().unwrap().comp.part, "PILLOW_BLOCK");
    app.place_finish();
    assert!(app.placing(), "must pick a location or choose free placement");
    app.camera.rot = freo::app::camera::StdView::Top.rotation();
    let start = app.place_editor().unwrap().comp.placement;
    app.camera.rot = freo::app::camera::StdView::Bottom.rotation();
    click_model(&mut app, start.point(V3::ZERO));
    assert_eq!(app.place_ref_picks().len(), 1);
    app.camera.rot = freo::app::camera::StdView::Top.rotation();
    let point = V3::new(12.0, 15.0, 6.0);
    click_model(&mut app, point);
    click_controls(&mut app, &["90° drehen", "Einbauen"]);
    assert_eq!(app.place_editor().unwrap().comp.part, "SHAFT");
    click_controls(&mut app, &["Unterseite verwenden"]);
    click_model(&mut app, V3::new(12.0, 75.0, 6.0));
    app.place_finish();
    assert_eq!(app.asm.as_ref().unwrap().data.components.len(), 3);
    let path = dir.join("test.pfasm");
    app.asm_save_to(path.clone());
    app.asm_load(path);
    let a = app.asm.as_ref().unwrap();
    assert!((a.res.placements[&101].pos - point).length() < 0.01);
    let original = a.res.placements[&102];
    let block = a.res.placements[&101];
    assert!((block.rot.angle_between(glam::DQuat::IDENTITY) - std::f64::consts::FRAC_PI_2).abs() < 1e-4);
    let shift = V3::new(-8.0, 10.0, 4.0);
    app.asm_commit("", |data| {
        let base = &mut data.components[0];
        base.placement = Placement { pos: shift, rot: glam::DQuat::from_rotation_y(0.6) };
        base.constraints = vec![AsmConstraint::new(AsmCType::Fix)];
    });
    let moved = &app.asm.as_ref().unwrap().res.placements;
    assert!((moved[&102].pos - moved[&100].point(original.pos)).length() < 0.01);
    assert!(moved[&102].rot.angle_between(moved[&100].rot * original.rot) < 1e-4);
    app.open_assembler();
    app.queue_assembler_files(paths);
    app.place_cancel();
    app.queue_assembler_files(vec![]);
    assert!(!app.placing());
    assert_eq!(app.asm.as_ref().unwrap().data.components.len(), 3);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn picked_side_mates_to_top_and_axis_controls_preserve_the_pivot() {
    use freo::app::camera::StdView;
    use glam::DQuat;
    let sample = freo::app::demo::sample_assembly();
    let mut app = App::new(None);
    app.data = sample.parts[0].data.clone();
    app.open_assembler();
    let name = app.asm.as_mut().unwrap().data.add_part(sample.parts[1].data.clone(), None);
    app.assemble_part(&name);
    let start = app.place_editor().unwrap().comp.placement;
    let center = V3::new(0.0, 30.0, 0.0);
    let mut expected = start.rot;
    for (label, axis) in [("X", V3::X), ("Y", V3::Y), ("Z", V3::Z)] {
        click_controls(&mut app, &[label, "+"]);
        expected = DQuat::from_axis_angle(axis, 15f64.to_radians()) * expected;
        let pose = app.place_editor().unwrap().comp.placement;
        assert!(pose.rot.angle_between(expected) < 1e-5, "{label}");
        assert!(pose.point(center).distance(start.point(center)) < 1e-5);
    }
    // Pick an off-centre point on the block's rotated front face, then the base's top.
    let pose = app.place_editor().unwrap().comp.placement;
    app.camera.rot = pose.rot * StdView::Front.rotation();
    let local = V3::new(11.0, 14.0, 25.0);
    click_model(&mut app, pose.point(local));
    app.camera.rot = StdView::Top.rotation();
    let target = V3::new(23.0, 15.0, 8.0);
    click_model(&mut app, target);
    let snapped = app.place_editor().unwrap().comp.placement;
    assert!(snapped.vector(V3::Z).distance(-V3::Y) < 1e-5);
    assert!(snapped.point(local).distance(target) < 0.01);
    click_controls(&mut app, &["Y", "−"]);
    let turned = app.place_editor().unwrap().comp.placement;
    assert!(turned.rot.angle_between(DQuat::from_rotation_y(-15f64.to_radians()) * snapped.rot) < 1e-5);
    assert!(turned.point(local).distance(target) < 0.01);
    click_controls(&mut app, &["Einbauen"]);
    assert!(!app.placing());
    app.do_undo();
    assert_eq!(app.asm.as_ref().unwrap().data.components.len(), 1);
}
