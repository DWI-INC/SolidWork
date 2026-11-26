//! Z-offset (first-layer height) assistant: a starting adjustment for the filament and the
//! build plate, the firmware commands to change and save the offset, what a first layer that
//! is too high or too low looks like, and a test print of first-layer patches at different
//! offsets to find the right one by eye.
//!
//! Sign convention everywhere: a negative adjustment moves the nozzle closer to the bed
//! (more squish), a positive one moves it away.

use super::config::*;
use super::gcode;
use super::infill::{self, Line};
use super::plan::{Layer, Path, Plan, Role};
use super::poly::{self, Pt};

#[derive(Clone, Debug, Default)]
pub struct Advice {
    /// starting adjustment (mm) relative to an offset that is right for PLA on a smooth plate
    pub adjust: f64,
    /// why that adjustment, each with its share (mm)
    pub reasons: Vec<(String, f64)>,
    /// further hints for this printer, plate and part
    pub tips: Vec<String>,
    /// how to find the base offset on this firmware, step by step
    pub calibrate: Vec<String>,
    /// how to keep a changed offset after a restart
    pub save: Vec<String>,
    /// first-layer squish: line width ÷ layer height
    pub squish: f64,
}

/// What a first layer looks like and which way to turn the offset.
pub const SYMPTOMS: [(&str, &str, i8); 8] = [
    ("Runde, einzelne Linien mit Lücken dazwischen", "Düse zu hoch – Offset verringern (näher ans Bett)", -1),
    ("Linien lösen sich, Ecken heben ab, Teil bewegt sich", "Düse zu hoch – Offset verringern", -1),
    ("Linien liegen nebeneinander, aber man sieht Rillen", "leicht zu hoch – 0,02 mm näher", -1),
    ("Glatte, geschlossene Fläche, Linien gerade noch erkennbar", "Richtig – so lassen", 0),
    ("Raue, wellige Oberfläche, Kämme zwischen den Linien", "Düse zu tief – Offset erhöhen", 1),
    ("Durchsichtig dünne Schicht, Düse kratzt über das Bett", "viel zu tief – Offset deutlich erhöhen", 1),
    ("Filament staut sich, Extruder klickt in der ersten Schicht", "zu tief – Offset erhöhen", 1),
    ("Elefantenfuß: unterste Kante steht seitlich über", "etwas zu tief – 0,02–0,04 mm höher", 1),
];

/// Firmware command that moves the nozzle by `delta` mm while printing (baby-stepping).
pub fn live_command(flavor: Flavor, delta: f64) -> String {
    let d = format!("{:+.3}", delta);
    match flavor {
        Flavor::Klipper => format!("SET_GCODE_OFFSET Z_ADJUST={} MOVE=1", d),
        Flavor::Marlin => format!("M290 Z{}", d),
        Flavor::RepRapFirmware => format!("M290 R1 S{}", d),
        Flavor::Bambu => "(automatisch – kein manueller Z-Offset)".into(),
    }
}

fn material_adjust(m: Material) -> (f64, &'static str) {
    match m {
        Material::Pla | Material::PlaPlus | Material::PlaSilk | Material::PlaMatte | Material::Pva => (0.0, ""),
        Material::PlaCf => (0.01, "Kohlefaser-Filament quetscht sich schlecht und verstopft bei zu viel Druck – etwas höher."),
        Material::Petg | Material::PetgCf => (0.03, "PETG klebt an der Düse und reißt bei zu wenig Abstand Fäden – 0,02–0,04 mm höher als PLA."),
        Material::Tpu => (0.04, "TPU ist weich und staut sich vor einer zu tiefen Düse – deutlich höher als PLA."),
        Material::Abs | Material::Asa | Material::Hips => (-0.01, "ABS/ASA verziehen sich: etwas mehr Anpressung gibt mehr Haftung."),
        Material::Pa | Material::PaCf | Material::Pc => (-0.01, "PA/PC schrumpfen stark: etwas mehr Anpressung gibt mehr Haftung."),
        Material::Pp => (-0.02, "PP haftet kaum: mehr Anpressung und PP-Klebeband/Bettplatte aus PP verwenden."),
    }
}

fn plate_adjust(p: Plate, m: Material) -> (f64, Option<&'static str>) {
    match p {
        Plate::TexturedPei => (-0.02, Some("Texturierte PEI-Platte: die Linien müssen in die Struktur gedrückt werden – 0,02 mm tiefer.")),
        Plate::SmoothPei if matches!(m, Material::Petg | Material::PetgCf | Material::Tpu) => {
            (0.01, Some("PETG/TPU auf glattem PEI haftet zu stark und kann die Beschichtung herausreißen – Klebestift als Trennschicht und etwas höher."))
        }
        Plate::Glass => (0.0, Some("Glas: sehr empfindlich auf den Abstand, eben aber kalt am Rand – Bett vor dem Kalibrieren 10 Minuten aufheizen.")),
        Plate::Buildtak => (0.01, Some("BuildTak hält sehr fest – nicht zu tief drucken, sonst lässt sich das Teil kaum lösen.")),
        _ => (0.0, None),
    }
}

/// Advice for the settings; with a slice, also for the first layer of that part.
pub fn advise(s: &Settings, plan: Option<&Plan>) -> Advice {
    let (p, f, q) = (&s.printer, &s.filament, &s.process);
    let mut a = Advice::default();
    let (dm, why) = material_adjust(f.material);
    if !why.is_empty() {
        a.reasons.push((why.to_string(), dm));
    }
    let (dp, why) = plate_adjust(p.plate, f.material);
    if let Some(w) = why {
        a.reasons.push((w.to_string(), dp));
    }
    a.adjust = ((dm + dp) * 1000.0).round() / 1000.0;
    if a.reasons.is_empty() {
        a.reasons.push((format!("{} auf {}: kein Ausgleich nötig – das für PLA eingestellte Offset passt.", f.material.code(), p.plate.name()), 0.0));
    }
    // squish of the first layer
    a.squish = q.first_layer_width / q.first_layer_height.max(1e-3);
    if a.squish < 1.6 {
        a.tips.push(format!(
            "Die erste Schicht ist mit {:.2} mm hoch im Verhältnis zur Breite {:.2} mm (nur {:.1} : 1). Linienbreite erste Schicht auf ≥ {:.2} mm erhöhen, dann haftet sie besser.",
            q.first_layer_height,
            q.first_layer_width,
            a.squish,
            q.first_layer_height * 2.0
        ));
    }
    if q.first_layer_height < p.nozzle * 0.5 {
        a.tips.push(format!(
            "Eine erste Schicht von {:.2} mm verzeiht kaum Unebenheiten. {:.2} mm (die Hälfte der Düse) ist robuster.",
            q.first_layer_height,
            p.nozzle * 0.5
        ));
    }
    // the part's first layer on the bed
    let start = p.start_gcode.to_ascii_uppercase();
    let levels = start.contains("BED_MESH") || start.contains("G29") || start.contains("AUTO_LEVEL") || start.contains("KAMP");
    if let Some(pl) = plan {
        let (bx, by) = p.bed_size();
        let sx = pl.first_max[0] - pl.first_min[0];
        let sy = pl.first_max[1] - pl.first_min[1];
        if sx > 0.0 && sy > 0.0 {
            let big = sx > bx * 0.5 || sy > by * 0.5;
            let cx = (pl.first_min[0] + pl.first_max[0]) / 2.0;
            let cy = (pl.first_min[1] + pl.first_max[1]) / 2.0;
            if big {
                a.tips.push(format!(
                    "Die erste Schicht ist {:.0} × {:.0} mm groß: ein Höhenunterschied des Betts von 0,05 mm macht schon eine Seite zu dünn. {}",
                    sx,
                    sy,
                    if levels { "Der Start-G-Code misst das Bett bereits ab." } else { "Vor dem Druck ein Bettnetz messen (siehe unten)." }
                ));
            }
            let edge = |c: f64, lo: f64, hi: f64, len: f64| (c - len / 2.0 - lo).min(hi - c - len / 2.0);
            if edge(cx, 0.0, bx, sx) < 15.0 || edge(cy, 0.0, by, sy) < 15.0 {
                a.tips.push("Das Teil liegt nah am Bettrand, wo das Bett oft kühler und weniger eben ist. Wenn möglich in die Mitte schieben.".into());
            }
        }
    }
    if !levels && !matches!(p.flavor, Flavor::Bambu) {
        a.tips.push(match p.flavor {
            Flavor::Klipper => "Bettnetz vor jedem Druck: „BED_MESH_CALIBRATE ADAPTIVE=1“ nach G28 in den Start-G-Code.".into(),
            Flavor::Marlin => "Bettnetz vor jedem Druck: „G29“ (oder „M420 S1“ für ein gespeichertes Netz) nach G28 in den Start-G-Code.".into(),
            _ => "Bettnetz vor jedem Druck: „G29“ nach G28 in den Start-G-Code.".into(),
        });
    }
    if f.bed_temp_first > 0.0 {
        a.tips.push(format!("Offset immer mit heißem Bett ({:.0} °C) und heißer Düse einstellen – das Bett dehnt sich beim Heizen um einige Hundertstel.", f.bed_temp_first));
    }
    a.tips.push("Vor dem Feinjustieren die Düse sauber machen: ein Filamentrest an der Spitze verfälscht jede Messung.".into());
    match p.flavor {
        Flavor::Klipper => {
            a.calibrate = vec![
                "G28 – Achsen referenzieren (Bett und Düse vorher aufheizen).".into(),
                "PROBE_CALIBRATE – mit Sensor; ohne Sensor: Z_ENDSTOP_CALIBRATE.".into(),
                "TESTZ Z=-0.1 / TESTZ Z=+0.025 … bis ein Blatt Papier (0,1 mm) mit leichtem Widerstand unter der Düse gleitet.".into(),
                "ACCEPT und SAVE_CONFIG.".into(),
            ];
            a.save = vec![
                "Während des Drucks justiert (SET_GCODE_OFFSET): danach Z_OFFSET_APPLY_PROBE (mit Sensor) bzw. Z_OFFSET_APPLY_ENDSTOP (ohne).".into(),
                "Dann SAVE_CONFIG – Klipper startet neu und behält das Offset.".into(),
            ];
        }
        Flavor::Marlin => {
            a.calibrate = vec![
                "G28 – Achsen referenzieren (Bett und Düse aufheizen).".into(),
                "Mit Sensor: im Druckermenü „Probe Z Offset“ / „Z-Offset-Assistent“, oder M851 Z-1.50 als Startwert.".into(),
                "Düse auf Z0 fahren (G1 Z0) und mit M290 Z-0.02 / M290 Z+0.02 nachstellen, bis ein Blatt Papier leicht streift.".into(),
                "M851 – zeigt das neue Offset.".into(),
            ];
            a.save = vec!["M500 – Offset im EEPROM speichern (sonst ist es nach dem Ausschalten weg).".into()];
        }
        Flavor::RepRapFirmware => {
            a.calibrate = vec![
                "G28 – referenzieren, dann G1 Z0.".into(),
                "Mit M290 R1 S-0.02 / S+0.02 nachstellen, bis ein Blatt Papier leicht streift.".into(),
                "G30 S-1 misst die Auslösehöhe des Sensors an dieser Stelle.".into(),
            ];
            a.save = vec![
                "Den Wert in config.g bei G31 Z… eintragen. Achtung: bei RRF heißt ein GRÖSSERES G31 Z die Düse NÄHER am Bett.".into(),
                "Alternativ M500 mit M501 in config.g (speichert in config-override.g).".into(),
            ];
        }
        Flavor::Bambu => {
            a.calibrate = vec!["Bambu-Drucker messen das Bett selbst ab: in den Druckoptionen „Bettnivellierung“ eingeschaltet lassen.".into()];
            a.save = vec![
                "Ein manuelles Z-Offset gibt es nicht. Ist die erste Schicht trotzdem zu dünn/dick: Bett und Düse reinigen, die Platte richtig herum einlegen und die „Flussratenkalibrierung“ laufen lassen.".into(),
            ];
            a.tips.retain(|t| !t.starts_with("Bettnetz"));
        }
    }
    a
}

/// Offsets the test print uses around the suggested adjustment.
pub fn test_offsets(center: f64) -> Vec<f64> {
    (-2..=2).map(|k| ((center + k as f64 * 0.04) * 1000.0).round() / 1000.0).collect()
}

/// A one-layer test print: one square patch per offset, left to right in rising order (the
/// nozzle highest on the right). The first patch has a tab at its front. Returns the plan and
/// its G-code.
pub fn test_print(s: &Settings, offsets: &[f64]) -> (Plan, String) {
    let mut s = s.clone();
    // one temperature, no fan, no slow-down: every patch prints the same way
    s.filament.nozzle_temp = s.filament.nozzle_temp_first;
    s.filament.bed_temp = s.filament.bed_temp_first;
    s.filament.fan_off_layers = 1000;
    s.filament.fan_always_on = false;
    s.filament.slow_down_for_layer_cooling = false;
    s.process.skirt_loops = 0;
    let (p, q) = (&s.printer, &s.process);
    let mut offs: Vec<f64> = offsets.to_vec();
    offs.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let n = offs.len().max(1) as f64;
    let (bx, by) = p.bed_size();
    let usable = match p.shape {
        BedShape::Rect => bx * 0.8,
        BedShape::Round => bx * 0.65,
    };
    let gap = 4.0;
    let size = ((usable - gap * (n - 1.0)) / n).clamp(8.0, 22.0);
    let total = size * n + gap * (n - 1.0);
    let (x0, y0) = (bx / 2.0 - total / 2.0, by / 2.0 - size / 2.0);
    let h = q.first_layer_height;
    let w = q.first_layer_width;
    let sp = q.spacing(w, h);
    let accel = q.first_layer_accel;
    let mut layers = vec![];
    for (k, &d) in offs.iter().enumerate() {
        let x = x0 + k as f64 * (size + gap);
        let mut sq = poly::rect(x, y0, x + size, y0 + size);
        if k == 0 {
            sq = poly::union(&sq, &poly::rect(x + size / 2.0 - 2.0, y0 - 5.0, x + size / 2.0 + 2.0, y0 + 1.0));
        }
        let mut paths = vec![];
        let outer = poly::offset(&sq, -w / 2.0);
        let inner = poly::offset(&outer, -sp);
        for (role, reg) in [(Role::OuterWall, &outer), (Role::InnerWall, &inner)] {
            for sh in reg {
                for c in sh {
                    paths.push(Path { role, pts: c.clone(), closed: true, width: w, height: h, flow: 1.0, speed: q.first_layer_speed, accel, spiral: false });
                }
            }
        }
        let fill = poly::offset(&inner, -sp);
        let lines: Vec<Line> = infill::monotonic_order(infill::parallel_centred(&fill, 45.0, sp), 45.0).into_iter().map(Line::open).collect();
        for l in lines {
            if l.pts.len() >= 2 {
                paths.push(Path { role: Role::BottomSurface, pts: l.pts, closed: false, width: w, height: h, flow: 1.0, speed: q.first_layer_infill_speed, accel, spiral: false });
            }
        }
        layers.push(Layer { z: (h + d).max(0.02), h, paths, ..Default::default() });
    }
    let first_min: Pt = [x0, y0 - 5.0];
    let first_max: Pt = [x0 + total, y0 + size];
    let plan = Plan { layers, warnings: vec![], first_min, first_max, max_z: h + offs.last().copied().unwrap_or(0.0), support: Default::default() };
    let mut out = gcode::write(&plan, &s, None, &super::now_text());
    let mut head = String::from("; SolidWork Z-Offset-Test\n");
    for (k, d) in offs.iter().enumerate() {
        head.push_str(&format!("; Feld {} von links: Offset {:+.3} mm{}\n", k + 1, d, if k == 0 { " (mit Lasche vorne)" } else { "" }));
    }
    out.gcode.insert_str(0, &head);
    (plan, out.gcode)
}
