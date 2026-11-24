//! Metric assembly screws. The bearing face is at Y=0 and the shank points towards -Y.
//! Head and drive profiles are nominal clearance models; pitch follows the ISO coarse table.

use super::assembly::AsmCType;
use super::features::*;
use super::threads::ThreadSpec;
use crate::kernel::math::{V2, V3};
use crate::sketch::sketch::Sketch;
use std::f64::consts::TAU;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Head {
    Phillips,
    Socket,
    Hex,
    Slotted,
    SixLobe,
}

impl Head {
    pub const ALL: [Self; 5] = [Self::Phillips, Self::Socket, Self::Hex, Self::Slotted, Self::SixLobe];

    pub fn label(self) -> &'static str {
        match self {
            Self::Phillips => "Kreuzschlitz (Phillips)",
            Self::Socket => "Innensechskant",
            Self::Hex => "Außensechskant",
            Self::Slotted => "Schlitz",
            Self::SixLobe => "Innensechsrund",
        }
    }
}

fn polygon(points: impl IntoIterator<Item = V2>) -> Sketch {
    let mut sk = Sketch::default();
    let ids: Vec<_> = points.into_iter().map(|p| sk.add_point(p)).collect();
    for i in 0..ids.len() {
        sk.add_line(ids[i], ids[(i + 1) % ids.len()]);
    }
    sk
}

fn radial(n: usize, radius: impl Fn(f64) -> f64) -> Sketch {
    polygon((0..n).map(|i| {
        let a = i as f64 * TAU / n as f64;
        radius(a) * V2::new(a.cos(), a.sin())
    }))
}

fn extrude(plane: PlaneRef, sketch: Sketch, depth: f64, flip: bool, remove: bool) -> FeatureKind {
    let FeatureKind::Extrude(mut e) = FeatureKind::new_extrude() else { unreachable!() };
    e.plane = Some(plane);
    e.sketch = sketch;
    e.depth = Dim::v(depth);
    e.flip = flip;
    e.remove = remove;
    FeatureKind::Extrude(e)
}

pub fn screw(spec: &ThreadSpec, length: u16, head: Head) -> Result<ModelData, String> {
    if length == 0 {
        return Err("Die Schraubenlänge muss positiv sein.".into());
    }
    let d = spec.major;
    let len = f64::from(length);
    let h = match head {
        Head::Socket => d,
        Head::Hex => 0.65 * d,
        _ => 0.68 * d,
    };
    let mut m = ModelData::new_part_template(&format!("{} × {} {}", spec.name, length, head.label()));
    m.appearance.color = [0.66, 0.69, 0.73];
    let mut add = |name: &str, kind| {
        m.features.push(Feature { id: m.next_id, name: name.into(), kind, suppressed: false, hidden: false });
        m.next_id += 1;
    };
    let base = PlaneRef::Datum(TOP_ID);
    add("Schaft", extrude(base, Sketch::circle(d / 2.0, 0.0, 0.0), len, true, false));
    let cap = if head == Head::Hex { radial(6, |_| d) } else { Sketch::circle(0.90 * d, 0.0, 0.0) };
    add("Kopf", extrude(base, cap, h, false, false));
    let recess = match head {
        Head::Hex => None,
        Head::Socket => Some(radial(6, |_| 0.48 * d)),
        Head::SixLobe => Some(radial(48, |a| d * (0.37 + 0.075 * (6.0 * a).cos()))),
        Head::Slotted => Some(Sketch::rect(2.0 * d, 0.22 * d, 0.0, 0.0)),
        Head::Phillips => {
            let (a, b) = (0.62 * d, 0.15 * d);
            Some(polygon([(-b, -a), (b, -a), (b, -b), (a, -b), (a, b), (b, b), (b, a), (-b, a), (-b, b), (-a, b), (-a, -b), (-b, -b)].map(|(x, y)| V2::new(x, y))))
        }
    };
    if let Some(sk) = recess {
        let top = PlaneRef::Face(FaceRef { point: V3::Y * h, normal: V3::Y });
        add("Antrieb", extrude(top, sk, 0.60 * h, true, true));
    }
    let shaft = FaceRef { point: V3::new(d / 2.0, -len / 2.0, 0.0), normal: V3::X };
    add("Außengewinde", FeatureKind::Thread(ThreadDef { face: Some(shaft), diameter: Dim::v(d), pitch: Dim::v(spec.pitch), length: Dim::v(len), flip: false }));
    m.interface = vec![
        IfaceItem { name: "Bohrungsachse".into(), ty: AsmCType::Coincident, r: IfaceRef::Axis(6) },
        IfaceItem { name: "Auflagefläche".into(), ty: AsmCType::Coincident, r: IfaceRef::Surface { face: FaceRef { point: V3::X * (0.70 * d), normal: -V3::Y }, planar: true } },
    ];
    Ok(m)
}
