//! The Assembler's library, circuit test and strength test through the app.
use freo::app::{App, Tab};
use freo::kernel::math::V3;
use freo::model::library::{demo_product, Item};
use freo::pcb::circuit::PinRef;
use freo::pcb::devices::{BoardKind, Kind, LedColor};

fn frames(app: &mut App, ctx: &eframe::egui::Context, events: Vec<eframe::egui::Event>) {
    use eframe::egui::{self, Pos2, Rect};
    let screen = Rect::from_min_size(Pos2::ZERO, egui::vec2(1000.0, 700.0));
    let input = egui::RawInput { screen_rect: Some(screen), events, ..Default::default() };
    let _ = ctx.run(input, |ctx| {
        egui::CentralPanel::default().frame(egui::Frame::NONE).show(ctx, |ui| app.viewport_ui(ui));
    });
}

fn click_at(app: &mut App, ctx: &eframe::egui::Context, point: V3) {
    use eframe::egui::{Event, PointerButton};
    frames(app, ctx, vec![]);
    let (x, y, _) = app.camera.project(point, app.viewport_size).unwrap();
    let pos = eframe::egui::pos2(x, y);
    for pressed in [true, false] {
        frames(app, ctx, vec![Event::PointerMoved(pos), Event::PointerButton { pos, button: PointerButton::Primary, pressed, modifiers: Default::default() }]);
    }
}

fn product_app() -> App {
    let mut app = App::new(None);
    app.go_home();
    app.open_assembler();
    let data = demo_product();
    let a = app.asm.as_mut().unwrap();
    a.data = data;
    app.asm_regenerate(false);
    app
}

#[test]
fn library_parts_are_placed_like_other_parts_and_keep_their_device() {
    let mut app = App::new(None);
    app.go_home();
    app.open_assembler();
    app.assembler_insert(Item::Device(Kind::Board(BoardKind::PiPico)));
    assert!(!app.placing(), "the first part becomes the base");
    app.assembler_insert(Item::Device(Kind::Led(LedColor::Green)));
    assert!(app.placing());
    app.assembler_place_freely();
    app.place_finish();
    let a = app.asm.as_ref().unwrap();
    assert_eq!(a.data.components.len(), 2);
    assert!(a.data.parts.iter().all(|p| p.device.is_some()));
    assert_eq!(app.circuit_parts().len(), 2);
    // the device survives saving and loading the assembly
    let json = serde_json::to_string(&a.data).unwrap();
    let back: freo::model::assembly::AssemblyData = serde_json::from_str(&json).unwrap();
    for (x, y) in back.parts.iter().zip(&a.data.parts) {
        let (dx, dy) = (x.device.as_ref().unwrap(), y.device.as_ref().unwrap());
        assert_eq!((dx.kind, &dx.lamps), (dy.kind, &dy.lamps));
        for (p, q) in dx.pins.iter().zip(&dy.pins) {
            assert!(p.name == q.name && (0..3).all(|k| (p.pos[k] - q.pos[k]).abs() < 1e-9));
        }
    }
    assert_eq!(back.components.len(), a.data.components.len());
    // enclosures and fasteners too
    for item in [Item::Box { w: 60.0, d: 40.0, h: 20.0, wall: 1.6 }, Item::Standoff(10)] {
        app.assembler_insert(item);
        app.assembler_place_freely();
        app.place_finish();
    }
    assert_eq!(app.asm.as_ref().unwrap().data.components.len(), 4);
}

#[test]
fn demo_product_lights_its_led_when_the_button_is_pressed() {
    let mut app = product_app();
    app.open_circuit();
    let ids: Vec<u64> = app.asm.as_ref().unwrap().data.components.iter().map(|c| c.id).collect();
    let (led, button) = (ids[4], ids[5]);
    let sol = app.asm.as_ref().unwrap().assembler.circuit.as_ref().unwrap().sol.as_ref().unwrap();
    assert!(sol.running.contains(&ids[1]), "Uno runs from the battery: {:?}", sol.status);
    assert!(sol.led_ma[&led] < 0.01);
    let unlit = app.asm.as_ref().unwrap().glow.len();
    // press the button by clicking it in the model (lid hidden so it can be seen)
    let a = app.asm.as_mut().unwrap();
    a.data.component_mut(ids[3]).unwrap().hidden = true;
    app.asm_regenerate(false);
    app.camera.rot = freo::app::camera::StdView::Top.rotation();
    let ctx = eframe::egui::Context::default();
    frames(&mut app, &ctx, vec![]);
    let b = app.displayed().topo.bbox;
    app.camera.fit(&b, app.viewport_size.0 / app.viewport_size.1);
    let top = app.asm.as_ref().unwrap().res.shown[&button].point(V3::new(0.0, 6.9, 0.0));
    click_at(&mut app, &ctx, top);
    app.circuit_update();
    let a = app.asm.as_ref().unwrap();
    let ui = a.assembler.circuit.as_ref().unwrap();
    assert!(ui.stim.pressed.contains(&(button, String::new())), "the click presses the button");
    let ma = ui.sol.as_ref().unwrap().led_ma[&led];
    assert!(ma > 8.0 && ma < 15.0, "{ma}");
    assert!(a.glow.len() > unlit, "the LED lens glows");
    assert!(a.glow.values().all(|c| c.iter().all(|&k| k >= 2.0)));
}

#[test]
fn wires_are_drawn_by_clicking_two_pins() {
    let mut app = product_app();
    app.open_circuit();
    let ids: Vec<u64> = app.asm.as_ref().unwrap().data.components.iter().map(|c| c.id).collect();
    app.asm.as_mut().unwrap().data.component_mut(ids[3]).unwrap().hidden = true;
    app.asm_regenerate(false);
    app.asm.as_mut().unwrap().assembler.circuit.as_mut().unwrap().picking = true;
    app.camera.rot = freo::app::camera::StdView::Top.rotation();
    let ctx = eframe::egui::Context::default();
    frames(&mut app, &ctx, vec![]);
    let b = app.displayed().topo.bbox;
    app.camera.fit(&b, app.viewport_size.0 / app.viewport_size.1);
    let before = app.asm.as_ref().unwrap().data.wiring.wires.len();
    let pin = |app: &App, comp: u64, name: &str| {
        let a = app.asm.as_ref().unwrap();
        let c = a.data.component(comp).unwrap();
        let q = a.data.part(&c.part).unwrap().device.as_ref().unwrap().pin(name).unwrap().pos;
        a.res.shown[&comp].point(V3::new(q[0], q[1], q[2]))
    };
    let (a, b) = (pin(&app, ids[1], "A0"), pin(&app, ids[1], "D7"));
    click_at(&mut app, &ctx, a);
    click_at(&mut app, &ctx, b);
    let w = &app.asm.as_ref().unwrap().data.wiring.wires;
    assert_eq!(w.len(), before + 1);
    let last = w.last().unwrap();
    assert_eq!((last.a.clone(), last.b.clone()), (PinRef::new(ids[1], "A0"), PinRef::new(ids[1], "D7")));
    app.asm_undo(false);
    assert_eq!(app.asm.as_ref().unwrap().data.wiring.wires.len(), before);
}

#[test]
fn strength_test_takes_supports_and_loads_from_the_assembly() {
    let mut app = product_app();
    let ids: Vec<u64> = app.asm.as_ref().unwrap().data.components.iter().map(|c| c.id).collect();
    // the enclosure stands on the table and carries the board, the battery and the lid
    let s = app.strength_setup(ids[0]).unwrap();
    assert!(s.supports.is_empty());
    assert_eq!(s.fixed.len(), 1);
    let carried: Vec<u64> = s.loads.iter().map(|l| l.0).collect();
    assert!(carried.contains(&ids[1]) && carried.contains(&ids[2]) && carried.contains(&ids[3]), "{carried:?}");
    let lid = s.loads.iter().find(|l| l.0 == ids[3]).unwrap();
    // the lid carries the LED and the button hanging from it
    let lid_alone = {
        let a = app.asm.as_ref().unwrap();
        let c = a.data.component(ids[3]).unwrap();
        freo::app::export::mass_properties(&a.res.parts[&c.part], a.data.part(&c.part).unwrap().data.density).mass
    };
    assert!(lid.2 > lid_alone, "{} vs {}", lid.2, lid_alone);
    assert_eq!(s.material, "PLA");
    // the lid rests on the enclosure
    let l = app.strength_setup(ids[3]).unwrap();
    assert_eq!(l.supports, vec![ids[0]]);
    app.run_strength(ids[0], 20.0, 20.0).unwrap();
    assert!(app.editing_from_asm().is_some());
    assert_eq!(app.tab, Tab::LiveSim);
    let st = app.data.sims.last().unwrap();
    assert!(st.items.iter().all(|it| it.faces.iter().all(|f| f.resolve(&app.result.topo).is_some())), "every face is found on the part");
    app.sim_solve_blocking();
    assert!(app.sim.out.as_ref().is_some_and(|o| !o.fields.is_empty()));
    app.asm_return();
    assert!(app.asm.as_ref().unwrap().data.parts[0].data.sims.len() == 1, "the study is kept with the part");
}
