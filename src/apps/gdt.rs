//! GD&T Advisor: proposes a datum reference frame and geometric tolerances for a part from its
//! geometry (largest planar face A, perpendicular faces B and C, hole patterns located with
//! position at MMC, coaxial cylinders with runout, form and orientation of the other
//! faces), with values from ISO 2768-2 for the chosen class, and checks the GD&T already in
//! the model for gaps and contradictions.

use crate::kernel::math::*;
use crate::kernel::topology::Topology;
use crate::model::drawing::{GChar, MatCond};
use crate::model::features::{Annot3, AnnotKind};

#[derive(Clone, Debug)]
pub enum Proposal {
    Datum { letter: String },
    Gtol { ch: GChar, diameter: bool, value: f64, mat: MatCond, datums: Vec<String> },
}

#[derive(Clone, Debug)]
pub struct Item {
    pub face: usize,
    /// further faces of the same feature (hole pattern)
    pub also: Vec<usize>,
    pub what: Proposal,
    pub label: String,
    pub reason: String,
    pub on: bool,
}

/// ISO 2768-2 classes H, K, L.
pub const CLASSES: [&str; 3] = ["H (fein)", "K (mittel)", "L (grob)"];

fn table(len: f64, class: usize, rows: &[(f64, [f64; 3])]) -> f64 {
    rows.iter().find(|r| len <= r.0).unwrap_or(rows.last().unwrap()).1[class.min(2)]
}

/// Geradheit und Ebenheit
pub fn flatness(len: f64, class: usize) -> f64 {
    table(len, class, &[(10.0, [0.02, 0.05, 0.1]), (30.0, [0.05, 0.1, 0.2]), (100.0, [0.1, 0.2, 0.4]), (300.0, [0.2, 0.4, 0.8]), (1000.0, [0.3, 0.6, 1.2]), (1e9, [0.4, 0.8, 1.6])])
}
/// Rechtwinkligkeit
pub fn perpendicularity(len: f64, class: usize) -> f64 {
    table(len, class, &[(100.0, [0.2, 0.4, 0.6]), (300.0, [0.3, 0.6, 1.0]), (1000.0, [0.4, 0.8, 1.5]), (1e9, [0.5, 1.0, 2.0])])
}
/// Lauf
pub fn runout(class: usize) -> f64 {
    [0.1, 0.2, 0.5][class.min(2)]
}
/// Position of holes: about half the clearance of a medium-fit bolt hole
pub fn position(d: f64, class: usize) -> f64 {
    let base = if d <= 6.0 {
        0.1
    } else if d <= 18.0 {
        0.2
    } else {
        0.3
    };
    base * [0.5, 1.0, 2.0][class.min(2)]
}

/// Size of a face: the diagonal of its bounding box.
fn face_len(t: &Topology, f: usize) -> f64 {
    let mut b = BBox::default();
    for &pi in &t.faces[f].polys {
        for &v in &t.polys[pi] {
            b.add(t.positions[v]);
        }
    }
    b.diagonal()
}

/// A point on the face (centre of its largest polygon) and the normal there.
pub fn face_anchor(t: &Topology, f: usize) -> (V3, V3) {
    let face = &t.faces[f];
    let pi = face.polys.iter().copied().max_by(|a, b| t.poly_area(*a).partial_cmp(&t.poly_area(*b)).unwrap()).unwrap_or(0);
    (t.poly_centroid(pi), t.planes[pi].n)
}

struct Cyl {
    face: usize,
    p: V3,
    d: V3,
    r: f64,
    hole: bool,
}

fn cylinders(t: &Topology) -> Vec<Cyl> {
    let mut out = vec![];
    for f in 0..t.faces.len() {
        if t.faces[f].planar {
            continue;
        }
        let Some((p, d, r)) = t.surface_axis(f) else { continue };
        if r <= 0.0 || !r.is_finite() {
            continue;
        }
        // a hole's surface faces its axis
        let (c, n) = face_anchor(t, f);
        let q = c - p;
        let radial = q - d * q.dot(d);
        let hole = n.dot(radial) < 0.0;
        out.push(Cyl { face: f, p, d, r, hole });
    }
    out
}

pub fn advise(t: &Topology, class: usize, existing: &[Annot3]) -> Vec<Item> {
    let mut items = vec![];
    let used: Vec<String> = existing.iter().filter_map(|a| if let AnnotKind::Datum { letter } = &a.kind { Some(letter.clone()) } else { None }).collect();
    let mut letters = ('A'..='Z').map(|c| c.to_string()).filter(|l| !used.contains(l));
    let total: f64 = t.faces.iter().map(|f| f.area).sum::<f64>().max(1e-9);
    let mut planar: Vec<usize> = (0..t.faces.len()).filter(|&f| t.faces[f].planar).collect();
    planar.sort_by(|a, b| t.faces[*b].area.partial_cmp(&t.faces[*a].area).unwrap());
    let cyl = cylinders(t);
    let mut datums: Vec<(String, usize, V3)> = vec![];
    // ---- rotational part: the largest outside cylinder is the datum axis
    let biggest_cyl = cyl.iter().filter(|c| !c.hole).max_by(|a, b| t.faces[a.face].area.partial_cmp(&t.faces[b.face].area).unwrap());
    let rotational = biggest_cyl.is_some_and(|c| t.faces[c.face].area > planar.first().map_or(0.0, |&f| t.faces[f].area) * 1.2);
    if rotational {
        let c = biggest_cyl.unwrap();
        let l = letters.next().unwrap_or_else(|| "A".into());
        items.push(Item {
            face: c.face,
            also: vec![],
            what: Proposal::Datum { letter: l.clone() },
            label: format!("Bezug {} – Achse der Mantelfläche ⌀{}", l, super::num(2.0 * c.r)),
            reason: "Größte Außenzylinderfläche: legt die Drehachse fest (Lagersitz / Spannfläche).".into(),
            on: true,
        });
        items.push(Item {
            face: c.face,
            also: vec![],
            what: Proposal::Gtol { ch: GChar::Cylindricity, diameter: false, value: flatness(2.0 * c.r, class) * 0.5, mat: MatCond::Rfs, datums: vec![] },
            label: format!("Zylinderform {} an Bezug {}", super::num(flatness(2.0 * c.r, class) * 0.5), l),
            reason: "Ein Bezugselement braucht eine Formtoleranz, die enger ist als die darauf bezogenen Toleranzen.".into(),
            on: true,
        });
        for o in cyl.iter().filter(|o| o.face != c.face && !o.hole) {
            let coax = o.d.cross(c.d).length() < 0.01 && {
                let q = o.p - c.p;
                (q - c.d * q.dot(c.d)).length() < 0.01 * c.r.max(1.0)
            };
            if coax {
                items.push(Item {
                    face: o.face,
                    also: vec![],
                    what: Proposal::Gtol { ch: GChar::CircularRunout, diameter: false, value: runout(class), mat: MatCond::Rfs, datums: vec![l.clone()] },
                    label: format!("Lauf {} zu {} an ⌀{}", super::num(runout(class)), l, super::num(2.0 * o.r)),
                    reason: "Koaxialer Absatz: der Rundlauf zur Bezugsachse begrenzt Unwucht und Versatz (ISO 2768-2).".into(),
                    on: true,
                });
            }
        }
        datums.push((l, c.face, c.d));
    }
    // ---- prismatic: A, B, C from the planar faces
    if !rotational && !planar.is_empty() {
        let a = planar[0];
        let na = t.faces[a].normal;
        let b = planar.iter().copied().find(|&f| t.faces[f].normal.dot(na).abs() < 0.02);
        let c = b.and_then(|b| {
            let nb = t.faces[b].normal;
            planar.iter().copied().find(|&f| t.faces[f].normal.dot(na).abs() < 0.02 && t.faces[f].normal.dot(nb).abs() < 0.02)
        });
        let la = letters.next().unwrap_or_else(|| "A".into());
        let len_a = face_len(t, a);
        items.push(Item {
            face: a,
            also: vec![],
            what: Proposal::Datum { letter: la.clone() },
            label: format!("Bezug {} – größte ebene Fläche", la),
            reason: "Primärbezug: die größte Auflagefläche nimmt drei Freiheitsgrade (Anlage mit drei Punkten).".into(),
            on: true,
        });
        items.push(Item {
            face: a,
            also: vec![],
            what: Proposal::Gtol { ch: GChar::Flatness, diameter: false, value: flatness(len_a, class), mat: MatCond::Rfs, datums: vec![] },
            label: format!("Ebenheit {} an Bezug {}", super::num(flatness(len_a, class)), la),
            reason: format!("Formtoleranz des Primärbezugs, ISO 2768-2 {} für {} mm.", CLASSES[class.min(2)], super::num(len_a)),
            on: true,
        });
        datums.push((la.clone(), a, na));
        if let Some(b) = b {
            let lb = letters.next().unwrap_or_else(|| "B".into());
            let len = face_len(t, b);
            items.push(Item {
                face: b,
                also: vec![],
                what: Proposal::Datum { letter: lb.clone() },
                label: format!("Bezug {} – größte Fläche senkrecht zu {}", lb, la),
                reason: "Sekundärbezug: nimmt zwei weitere Freiheitsgrade (Anlage mit zwei Punkten).".into(),
                on: true,
            });
            items.push(Item {
                face: b,
                also: vec![],
                what: Proposal::Gtol { ch: GChar::Perpendicularity, diameter: false, value: perpendicularity(len, class), mat: MatCond::Rfs, datums: vec![la.clone()] },
                label: format!("Rechtwinkligkeit {} zu {} an Bezug {}", super::num(perpendicularity(len, class)), la, lb),
                reason: "Der Sekundärbezug wird zum Primärbezug ausgerichtet.".into(),
                on: true,
            });
            datums.push((lb.clone(), b, t.faces[b].normal));
            if let Some(c) = c {
                let lc = letters.next().unwrap_or_else(|| "C".into());
                let len = face_len(t, c);
                items.push(Item {
                    face: c,
                    also: vec![],
                    what: Proposal::Datum { letter: lc.clone() },
                    label: format!("Bezug {} – senkrecht zu {} und {}", lc, la, lb),
                    reason: "Tertiärbezug: nimmt den letzten Freiheitsgrad (ein Punkt).".into(),
                    on: true,
                });
                items.push(Item {
                    face: c,
                    also: vec![],
                    what: Proposal::Gtol {
                        ch: GChar::Perpendicularity,
                        diameter: false,
                        value: perpendicularity(len, class),
                        mat: MatCond::Rfs,
                        datums: vec![la.clone(), lb.clone()],
                    },
                    label: format!("Rechtwinkligkeit {} zu {}|{} an Bezug {}", super::num(perpendicularity(len, class)), la, lb, lc),
                    reason: "Der Tertiärbezug wird zu den beiden ersten ausgerichtet.".into(),
                    on: true,
                });
                datums.push((lc, c, t.faces[c].normal));
            }
        }
        // other planar faces: orientation to A (largest first, small ones left out)
        let datum_faces: Vec<usize> = datums.iter().map(|d| d.1).collect();
        let mut n_other = 0;
        for &f in planar.iter().skip(1) {
            if datum_faces.contains(&f) || t.faces[f].area < total * 0.015 || n_other >= 10 {
                continue;
            }
            let n = t.faces[f].normal;
            let len = face_len(t, f);
            let (ch, why) = if n.dot(na).abs() > 0.999 {
                (GChar::Parallelism, "parallel zum Primärbezug: legt Höhe/Dicke fest")
            } else if n.dot(na).abs() < 0.02 {
                (GChar::Perpendicularity, "senkrecht zum Primärbezug")
            } else {
                (GChar::Angularity, "schräg zum Primärbezug: Neigung toleriert die Lage der Schräge")
            };
            let v = if ch == GChar::Parallelism { flatness(len, class) } else { perpendicularity(len, class) };
            items.push(Item {
                face: f,
                also: vec![],
                what: Proposal::Gtol { ch, diameter: false, value: v, mat: MatCond::Rfs, datums: vec![la.clone()] },
                label: format!("{} {} zu {} ({} mm²)", ch.name(), super::num(v), la, super::num(t.faces[f].area)),
                reason: format!("Ebene Fläche {}.", why),
                on: n_other < 6,
            });
            n_other += 1;
        }
    }
    // ---- hole patterns: position at MMC to the datum frame
    let frame: Vec<String> = datums.iter().map(|d| d.0.clone()).collect();
    let mut groups: Vec<(f64, V3, Vec<usize>)> = vec![];
    for c in cyl.iter().filter(|c| c.hole && !datums.iter().any(|d| d.1 == c.face)) {
        let d = 2.0 * c.r;
        if let Some(g) = groups.iter_mut().find(|g| (g.0 - d).abs() < 0.02 && g.1.cross(c.d).length() < 0.01) {
            // a hole cut in several faces counts once: same axis = same hole
            g.2.push(c.face);
        } else {
            groups.push((d, c.d, vec![c.face]));
        }
    }
    for (d, axis, faces) in groups {
        // distinct holes in this group (faces on different axes)
        let mut axes: Vec<V3> = vec![];
        for &f in &faces {
            if let Some(cy) = cyl.iter().find(|c| c.face == f) {
                if !axes.iter().any(|p| {
                    let q = cy.p - *p;
                    (q - cy.d * q.dot(cy.d)).length() < 0.05 * cy.r.max(0.5)
                }) {
                    axes.push(cy.p);
                }
            }
        }
        let n = axes.len().max(1);
        let datums_for: Vec<String> = if rotational {
            frame.clone()
        } else if datums.first().is_some_and(|a| a.2.cross(axis).length() < 0.02) {
            frame.clone()
        } else {
            frame.iter().take(2).cloned().collect()
        };
        let v = position(d, class);
        items.push(Item {
            face: faces[0],
            also: faces[1..].to_vec(),
            what: Proposal::Gtol { ch: GChar::Position, diameter: true, value: v, mat: MatCond::Mmc, datums: datums_for.clone() },
            label: format!("{}× ⌀{}: Position ⌀{} Ⓜ {}", n, super::num(d), super::num(v), datums_for.join("|")),
            reason: if n > 1 {
                "Lochbild: Position mit Maximum-Material-Bedingung erlaubt einen Bonus aus der Bohrungstoleranz und passt zur Lehrenprüfung.".into()
            } else {
                "Bohrung: Position statt ± Koordinatenmaßen – rundes Toleranzfeld, 57 % mehr Fläche bei gleicher Funktion.".into()
            },
            on: true,
        });
    }
    items
}

/// Problems in the GD&T already in the model.
pub fn check(existing: &[Annot3]) -> Vec<String> {
    let mut out = vec![];
    let letters: Vec<&String> = existing.iter().filter_map(|a| if let AnnotKind::Datum { letter } = &a.kind { Some(letter) } else { None }).collect();
    for (i, l) in letters.iter().enumerate() {
        if letters[..i].contains(l) {
            out.push(format!("Bezugsbuchstabe {} ist mehrfach vergeben.", l));
        }
        if ["I", "O", "Q"].contains(&l.as_str()) {
            out.push(format!("Bezugsbuchstabe {} vermeiden (ISO 5459: I, O und Q sind nicht zulässig).", l));
        }
    }
    for a in existing {
        if let AnnotKind::Gtol { ch, value, mat, datums, .. } = &a.kind {
            if *value <= 0.0 {
                out.push(format!("{}: Toleranzwert fehlt.", ch.name()));
            }
            for d in datums {
                let d = d.trim();
                if !d.is_empty() && !letters.iter().any(|l| l.as_str() == d) {
                    out.push(format!("{} bezieht sich auf Bezug {}, der im Modell nicht definiert ist.", ch.name(), d));
                }
            }
            let form = matches!(ch, GChar::Straightness | GChar::Flatness | GChar::Circularity | GChar::Cylindricity);
            if form && datums.iter().any(|d| !d.trim().is_empty()) {
                out.push(format!("{} ist eine Formtoleranz und hat keinen Bezug.", ch.name()));
            }
            let needs = matches!(
                ch,
                GChar::Angularity | GChar::Perpendicularity | GChar::Parallelism | GChar::CircularRunout | GChar::TotalRunout | GChar::Concentricity | GChar::Symmetry
            );
            if needs && datums.iter().all(|d| d.trim().is_empty()) {
                out.push(format!("{} braucht einen Bezug.", ch.name()));
            }
            if *mat != MatCond::Rfs && !matches!(ch, GChar::Position | GChar::Straightness | GChar::Perpendicularity | GChar::Parallelism | GChar::Angularity) {
                out.push(format!("{}: Material-Bedingung ist hier nicht anwendbar.", ch.name()));
            }
        }
    }
    if existing.iter().any(|a| matches!(a.kind, AnnotKind::Gtol { .. })) && letters.is_empty() {
        out.push("Es gibt geometrische Toleranzen, aber keinen Bezug: Bezugssystem festlegen.".into());
    }
    out
}
