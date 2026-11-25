//! Built-in Assembler library: every part regenerates at real scale with its pins, and the
//! assembly circuit lights LEDs, runs boards from batteries and reads buttons.
use freo::model::{library, regen};
use freo::pcb::circuit::{simulate, Mode, Part, PinRef, Stimulus, Wiring};
use freo::pcb::devices::{build, BoardKind, Kind, LedColor};

#[test]
fn every_library_part_regenerates_with_its_pins() {
    for item in library::catalog() {
        if item == library::Item::Screw {
            continue;
        }
        let lp = item.build().unwrap();
        let r = regen::run(&lp.data);
        assert!(r.errors.is_empty(), "{}: {:?}", item.name(), r.errors);
        assert!(!r.topo.polys.is_empty(), "{}", item.name());
        if let Some(dev) = &lp.device {
            assert!(!dev.pins.is_empty(), "{}", item.name());
            let b = r.topo.bbox;
            for pin in &dev.pins {
                let q = freo::kernel::math::V3::new(pin.pos[0], pin.pos[1], pin.pos[2]);
                assert!(q.x > b.min.x - 1.0 && q.x < b.max.x + 1.0 && q.y > b.min.y - 1.0 && q.y < b.max.y + 1.0 && q.z > b.min.z - 1.0 && q.z < b.max.z + 1.0, "{} pin {} outside {:?}", item.name(), pin.name, b);
            }
            for lamp in &dev.lamps {
                assert!(lp.data.feature(lamp.feature).is_some(), "{} lamp {}", item.name(), lamp.name);
            }
        }
    }
}

#[test]
fn boards_have_real_sizes_and_pin_counts() {
    let cases = [
        (BoardKind::ArduinoUno, 68.58, 53.34, 32),
        (BoardKind::ArduinoNano, 18.0, 45.0, 30),
        (BoardKind::PiPico, 21.0, 51.0, 40),
        (BoardKind::BluePill, 53.34, 22.86, 40),
        (BoardKind::Nucleo64, 70.0, 82.5, 32),
        (BoardKind::Esp32DevKit, 27.94, 48.26, 38),
        (BoardKind::Pi4, 85.0, 56.0, 40),
    ];
    for (kind, w, d, pins) in cases {
        let p = build(Kind::Board(kind));
        assert_eq!(p.device.pins.len(), pins, "{kind:?}");
        // the laminate (first solid) has the board's outline
        let mut laminate = p.data.clone();
        laminate.features.retain(|f| !matches!(f.kind, freo::model::features::FeatureKind::Import(_) | freo::model::features::FeatureKind::Body(_)));
        let b = regen::run(&laminate).topo.bbox;
        assert!(((b.max.x - b.min.x) - w).abs() < 0.05, "{kind:?} width {}", b.max.x - b.min.x);
        assert!(((b.max.z - b.min.z) - d).abs() < 0.05, "{kind:?} depth {}", b.max.z - b.min.z);
        let s = kind.spec();
        for (rail, _) in s.usb {
            assert!(p.device.pin(rail).is_some() || s.internal.contains(rail), "{kind:?} {rail}");
        }
    }
}

struct Bench {
    devs: Vec<(u64, freo::pcb::devices::Device)>,
}

impl Bench {
    fn new(kinds: &[Kind]) -> Bench {
        Bench { devs: kinds.iter().enumerate().map(|(i, k)| (i as u64 + 1, build(*k).device)).collect() }
    }
    fn parts(&self) -> Vec<Part<'_>> {
        self.devs.iter().map(|(id, d)| Part { id: *id, label: d.kind.name(), device: d }).collect()
    }
}

fn wire(w: &mut Wiring, a: (u64, &str), b: (u64, &str)) {
    assert!(w.connect(PinRef::new(a.0, a.1), PinRef::new(b.0, b.1)));
}

#[test]
fn uno_lights_an_led_through_a_resistor() {
    let bench = Bench::new(&[Kind::Board(BoardKind::ArduinoUno), Kind::Resistor(220), Kind::Led(LedColor::Red)]);
    let mut w = Wiring::default();
    wire(&mut w, (1, "D9"), (2, "1"));
    wire(&mut w, (2, "2"), (3, "A"));
    wire(&mut w, (3, "K"), (1, "GND"));
    w.set_mode(1, "D9", Mode::High);
    let mut stim = Stimulus::default();
    let off = simulate(&bench.parts(), &w, &stim);
    assert!(off.running.is_empty());
    assert_eq!(off.led_ma[&3], 0.0);
    stim.usb.insert(1);
    let on = simulate(&bench.parts(), &w, &stim);
    assert!(on.running.contains(&1));
    // (5 V − 2 V) / (220 + 25 + 15) Ω ≈ 11,5 mA
    let ma = on.led_ma[&3];
    assert!((ma - 3000.0 / 260.0).abs() < 0.2, "{ma}");
    assert!(on.lamps.contains_key(&(3, "LED".into())));
    assert!(on.lamps.contains_key(&(1, "ON".into())));
    assert!(on.warnings.is_empty(), "{:?}", on.warnings);
    // without the resistor the GPIO is overloaded
    let mut direct = Wiring::default();
    wire(&mut direct, (1, "D9"), (3, "A"));
    wire(&mut direct, (3, "K"), (1, "GND"));
    direct.set_mode(1, "D9", Mode::High);
    let bad = simulate(&bench.parts(), &direct, &stim);
    assert!(bad.led_ma[&3] > 30.0);
    assert!(bad.warnings.iter().any(|w| w.contains("Vorwiderstand")), "{:?}", bad.warnings);
}

#[test]
fn battery_runs_a_board_and_a_button_switches_an_led() {
    let bench = Bench::new(&[Kind::Board(BoardKind::ArduinoNano), Kind::Battery9V, Kind::Tact6, Kind::Resistor(330), Kind::Led(LedColor::Green)]);
    let mut w = Wiring::default();
    wire(&mut w, (2, "+"), (1, "VIN"));
    wire(&mut w, (2, "-"), (1, "GND"));
    wire(&mut w, (1, "D2"), (3, "1"));
    wire(&mut w, (3, "3"), (1, "GND"));
    wire(&mut w, (1, "D13"), (4, "1"));
    wire(&mut w, (4, "2"), (5, "A"));
    wire(&mut w, (5, "K"), (1, "GND"));
    w.set_mode(1, "D2", Mode::Pullup);
    w.set_mode(1, "D13", Mode::Follow { input: "D2".into(), invert: true });
    let mut stim = Stimulus::default();
    let idle = simulate(&bench.parts(), &w, &stim);
    assert!(idle.running.contains(&1), "{:?}", idle.status);
    assert!(idle.level(1, "5V").volts().is_some_and(|v| (v - 5.0).abs() < 0.1), "{:?}", idle.level(1, "5V"));
    assert!(idle.led_ma[&5] < 0.01);
    assert!(!idle.lamps.contains_key(&(1, "L".into())));
    stim.pressed.insert((3, String::new()));
    let pressed = simulate(&bench.parts(), &w, &stim);
    assert!(pressed.led_ma[&5] > 5.0, "{:?}", pressed.status);
    assert!(pressed.lamps.contains_key(&(1, "L".into())), "on-board L follows D13");
    // battery out: everything stops
    stim.removed.insert(2);
    let dead = simulate(&bench.parts(), &w, &stim);
    assert!(dead.running.is_empty());
    assert!(dead.led_ma[&5] < 0.01);
}

#[test]
fn blinking_and_board_leds_follow_the_program() {
    let bench = Bench::new(&[Kind::Board(BoardKind::PiPico), Kind::Board(BoardKind::BluePill), Kind::Board(BoardKind::Nucleo64)]);
    let mut w = Wiring::default();
    w.set_mode(1, "GP25", Mode::Blink(500));
    w.set_mode(2, "PC13", Mode::Low);
    w.set_mode(3, "D13", Mode::Follow { input: "PC13".into(), invert: true });
    let mut stim = Stimulus { usb: [1, 2, 3].into_iter().collect(), ..Default::default() };
    let a = simulate(&bench.parts(), &w, &stim);
    stim.time = 0.6;
    let b = simulate(&bench.parts(), &w, &stim);
    assert_ne!(a.lamps.contains_key(&(1, "LED".into())), b.lamps.contains_key(&(1, "LED".into())));
    assert!(a.lamps.contains_key(&(2, "PC13".into())), "Blue Pill LED is active LOW");
    assert!(!a.lamps.contains_key(&(3, "LD2".into())));
    stim.pressed.insert((3, "B1".into()));
    let c = simulate(&bench.parts(), &w, &stim);
    assert!(c.lamps.contains_key(&(3, "LD2".into())), "B1 pulls PC13 low");
}

#[test]
fn five_volts_on_a_three_volt_gpio_is_reported() {
    let bench = Bench::new(&[Kind::Board(BoardKind::Esp32DevKit), Kind::HcSr04]);
    let mut w = Wiring::default();
    wire(&mut w, (1, "5V"), (2, "VCC"));
    wire(&mut w, (1, "GND"), (2, "GND"));
    wire(&mut w, (1, "GPIO5"), (2, "TRIG"));
    wire(&mut w, (1, "GPIO18"), (2, "ECHO"));
    wire(&mut w, (1, "GPIO19"), (1, "5V"));
    let stim = Stimulus { usb: [1].into_iter().collect(), ..Default::default() };
    let s = simulate(&bench.parts(), &w, &stim);
    assert!(s.status.iter().any(|(id, t)| *id == 2 && t.starts_with("misst")), "{:?}", s.status);
    assert!(s.warnings.iter().any(|t| t.contains("GPIO19")), "{:?}", s.warnings);
}
