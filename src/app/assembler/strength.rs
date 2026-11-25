//! Festigkeit prüfen: a structural Live-Simulation study of one component, set up from the
//! assembly. Faces resting on other components become fixed supports; faces carrying other
//! components take their weight (with everything stacked on them) times a load factor,
//! plus the part's own weight and an optional extra force (a hand pressing a button …).
use super::*;
use crate::kernel::topology::Topology;
use crate::model::sim::{ItemKind, SimFace, SimItem, Study, StudyKind, MATERIALS};
use std::collections::HashSet;

pub struct StrengthUi {
    pub comp: u64,
    /// load factor on all weights (1 = standing still, 10–50 = handling and drops)
    pub factor: f64,
    /// extra force in N on the carrying faces (or the top face)
    pub extra: f64,
}

/// Contacts and loads of a component, found in the assembly.
pub struct StrengthSetup {
    pub fixed: Vec<SimFace>,
    /// earlier components it rests on (empty: stands on a table)
    pub supports: Vec<u64>,
    /// faces carrying a component and the mass in kg resting there (with what it carries)
    pub loads: Vec<(u64, Vec<SimFace>, f64)>,
    pub top: Option<SimFace>,
    pub down: V3,
    pub mass: f64,
    pub material: &'static str,
}

const G: f64 = 9.81;

/// Planar faces of a part in assembly coordinates: (face, normal, point, bbox).
fn world_faces(topo: &Topology, pl: &Placement) -> Vec<(usize, V3, V3, BBox)> {
    topo.faces
        .iter()
        .enumerate()
        .filter(|(_, f)| f.planar && f.area > 1e-3)
        .map(|(i, f)| {
            let mut b = BBox::default();
            for &pi in &f.polys {
                for &v in &topo.polys[pi] {
                    b.add(pl.point(topo.positions[v]));
                }
            }
            (i, pl.vector(f.normal).normalize(), pl.point(f.center), b)
        })
        .collect()
}

fn overlap(a: &BBox, b: &BBox, tol: f64) -> [f64; 3] {
    let e = |lo1: f64, hi1: f64, lo2: f64, hi2: f64| hi1.min(hi2) - lo1.max(lo2) + tol;
    [e(a.min.x, a.max.x, b.min.x, b.max.x), e(a.min.y, a.max.y, b.min.y, b.max.y), e(a.min.z, a.max.z, b.min.z, b.max.z)]
}

impl App {
    pub(super) fn strength_target(&self) -> Option<u64> {
        let id = *self.tree_selection.last()?;
        self.component(id).filter(|c| !c.suppressed && !c.hidden).map(|c| c.id)
    }

    pub fn open_strength(&mut self, comp: u64) {
        if let Some(a) = &mut self.asm {
            a.assembler.strength = Some(StrengthUi { comp, factor: 1.0, extra: 0.0 });
        }
    }

    /// Contact faces of `comp` with the other components: which carry it and what it carries.
    pub fn strength_setup(&self, comp: u64) -> Result<StrengthSetup, String> {
        let a = self.asm.as_ref().ok_or("Keine Baugruppe")?;
        let comps: Vec<&Component> = a.data.components.iter().filter(|c| !c.suppressed).collect();
        let geo = |c: &Component| Some((a.res.parts.get(&c.part)?, a.res.placements.get(&c.id)?));
        let faces: Vec<(u64, Vec<(usize, V3, V3, BBox)>)> = comps.iter().filter_map(|c| geo(c).map(|(r, pl)| (c.id, world_faces(&r.topo, pl)))).collect();
        let tol = 0.05;
        // touching planar faces: (face of a, face of b, normal of a's face)
        let touching = |x: u64, y: u64| -> Vec<(usize, usize, V3)> {
            let (Some(fx), Some(fy)) = (faces.iter().find(|f| f.0 == x), faces.iter().find(|f| f.0 == y)) else { return vec![] };
            let mut v = vec![];
            for (i, n, c, b) in &fx.1 {
                for (j, m, d, e) in &fy.1 {
                    if n.dot(*m) > -0.999 || (*d - *c).dot(*n).abs() > tol {
                        continue;
                    }
                    let o = overlap(b, e, tol);
                    if o.iter().all(|&k| k > 0.0) && o.iter().filter(|&&k| k > 0.3).count() >= 2 {
                        v.push((*i, *j, *n));
                    }
                }
            }
            v
        };
        let mass_of = |id: u64| -> f64 {
            a.data.component(id).and_then(|c| Some((a.res.parts.get(&c.part)?, a.data.part(&c.part)?.data.density))).map(|(r, rho)| crate::app::export::mass_properties(r, rho).mass).unwrap_or(0.0)
        };
        // what a component carries: the parts assembled after it that touch it, transitively
        fn carried(id: u64, seen: &mut HashSet<u64>, ids: &[u64], touching: &dyn Fn(u64, u64) -> Vec<(usize, usize, V3)>, mass_of: &dyn Fn(u64) -> f64) -> f64 {
            let mut m = mass_of(id);
            let at = ids.iter().position(|&x| x == id).unwrap_or(0);
            for &o in &ids[at + 1..] {
                if !seen.contains(&o) && !touching(id, o).is_empty() {
                    seen.insert(o);
                    m += carried(o, seen, ids, touching, mass_of);
                }
            }
            m
        }
        let ids: Vec<u64> = comps.iter().map(|c| c.id).collect();
        let me = a.data.component(comp).ok_or("Komponente fehlt")?;
        let (res, pl) = geo(me).ok_or("Komponente ist nicht platziert")?;
        let topo = &res.topo;
        let order = |id: u64| ids.iter().position(|&x| x == id).unwrap_or(0);
        let mut fixed = vec![];
        let mut supports = vec![];
        let mut loads = vec![];
        let mut seen: HashSet<u64> = [comp].into_iter().collect();
        for &o in ids.iter().filter(|&&o| o != comp) {
            let t = touching(comp, o);
            if t.is_empty() {
                continue;
            }
            // parts assembled earlier hold this one; later ones are mounted on it (resting or hanging)
            let support = order(o) < order(comp);
            let mut own: Vec<usize> = t.iter().map(|x| x.0).collect();
            own.sort_unstable();
            own.dedup();
            let sf: Vec<SimFace> = own.into_iter().map(|f| SimFace::of(topo, f)).collect();
            if support {
                fixed.extend(sf);
                supports.push(o);
            } else if !seen.contains(&o) {
                seen.insert(o);
                let m = carried(o, &mut seen, &ids, &touching, &mass_of);
                loads.push((o, sf, m));
            }
        }
        let down = pl.inverse().vector(-V3::Y).normalize();
        let top = (0..topo.faces.len())
            .filter(|&f| topo.faces[f].planar && topo.faces[f].normal.dot(-down) > 0.99)
            .max_by(|&x, &y| (topo.faces[x].center.dot(-down), topo.faces[x].area).partial_cmp(&(topo.faces[y].center.dot(-down), topo.faces[y].area)).unwrap())
            .map(|f| SimFace::of(topo, f));
        fixed.dedup_by(|a, b| a == b);
        if fixed.is_empty() {
            // the base part stands on a table: its lowest downward faces
            let lowest = (0..topo.faces.len()).filter(|&f| topo.faces[f].planar && topo.faces[f].normal.dot(down) > 0.99).map(|f| topo.faces[f].center.dot(down)).fold(f64::NEG_INFINITY, f64::max);
            fixed = (0..topo.faces.len()).filter(|&f| topo.faces[f].planar && topo.faces[f].normal.dot(down) > 0.99 && topo.faces[f].center.dot(down) > lowest - tol).map(|f| SimFace::of(topo, f)).collect();
            if fixed.is_empty() {
                return Err("Keine Auflage gefunden: das Teil hat keine ebene Unterseite und berührt kein früher eingebautes Teil.".into());
            }
        }
        let rho = a.data.part(&me.part).map_or(0.0, |p| p.data.density);
        let material = MATERIALS.iter().min_by(|x, y| (x.rho * 1e-9 - rho).abs().partial_cmp(&(y.rho * 1e-9 - rho).abs()).unwrap()).map_or("Stahl", |m| m.name);
        Ok(StrengthSetup { fixed, supports, loads, top, down, mass: mass_of(comp), material })
    }

    /// Opens the component with a structural study of its assembly loads and solves it.
    pub fn run_strength(&mut self, comp: u64, factor: f64, extra: f64) -> Result<(), String> {
        let s = self.strength_setup(comp)?;
        let asm_name = self.asm.as_ref().map(|a| a.data.name.clone()).unwrap_or_default();
        let mut st = Study::new(1, format!("Festigkeit in {asm_name}"), StudyKind::Structural);
        st.material = s.material.into();
        let add = |st: &mut Study, kind: ItemKind, name: String, faces: Vec<SimFace>, vec: [f64; 3]| {
            let mut it = SimItem::new(st.next_item.max(1), name, kind);
            it.faces = faces;
            it.vec = vec;
            st.next_item = it.id + 1;
            st.items.push(it);
        };
        let d = s.down;
        add(&mut st, ItemKind::Fixed, "Auflage aus Baugruppe".into(), s.fixed.clone(), [0.0; 3]);
        add(&mut st, ItemKind::Gravity, "Eigengewicht".into(), vec![], (d * (9810.0 * factor)).to_array());
        let mut summary = vec![];
        for (o, faces, m) in &s.loads {
            let n = m * G * factor;
            summary.push(format!("{} {} N", self.comp_label(*o), format!("{n:.2}").replace('.', ",")));
            add(&mut st, ItemKind::Force, format!("Last {}", self.comp_label(*o)), faces.clone(), (d * n).to_array());
        }
        if extra > 0.0 {
            let mut faces: Vec<SimFace> = if s.loads.is_empty() { s.top.into_iter().collect() } else { s.loads.iter().flat_map(|l| l.1.clone()).collect() };
            let mut unique: Vec<SimFace> = vec![];
            for f in faces.drain(..) {
                if !unique.contains(&f) {
                    unique.push(f);
                }
            }
            let faces = unique;
            add(&mut st, ItemKind::Force, "Zusatzkraft".into(), faces, (d * extra).to_array());
        }
        let supports: Vec<String> = s.supports.iter().map(|&o| self.comp_label(o)).collect();
        self.asm_open_part(comp);
        if self.editing_from_asm().is_none() {
            return Err("Das Teil konnte nicht geöffnet werden.".into());
        }
        let label = format!("Festigkeitsstudie aus {asm_name} angelegt.");
        self.commit(&label, move |m| {
            st.id = m.sims.iter().map(|x| x.id).max().unwrap_or(0) + 1;
            m.sims.push(st);
            m.sim_active = m.sims.len() - 1;
        });
        self.sim_reset_view();
        self.sim.paused = false;
        self.tab = Tab::LiveSim;
        self.say(
            format!(
                "Festigkeit: gelagert auf {}, {}{}Werkstoff {}. Danach „Zurück zur Baugruppe“.",
                if supports.is_empty() { "dem Tisch (Unterseite)".to_string() } else { supports.join(", ") },
                if summary.is_empty() { String::new() } else { format!("Lasten {}, ", summary.join(", ")) },
                format!("{factor}-fache Gewichtskraft, ").replace('.', ","),
                s.material
            ),
            MsgKind::Info,
        );
        Ok(())
    }

    pub(super) fn strength_window(&mut self, ctx: &egui::Context) {
        let Some(ui_state) = self.asm.as_ref().and_then(|a| a.assembler.strength.as_ref()) else { return };
        let comp = ui_state.comp;
        let setup = self.strength_setup(comp);
        let label = self.comp_label(comp);
        let mut open = true;
        let mut run = false;
        let st = self.asm.as_mut().unwrap().assembler.strength.as_mut().unwrap();
        egui::Window::new("Festigkeit prüfen").id(egui::Id::new("asm_strength")).collapsible(false).resizable(false).open(&mut open).show(ctx, |ui| {
            ui.strong(&label);
            match &setup {
                Err(e) => {
                    ui.label(e);
                }
                Ok(s) => {
                    ui.label(if s.supports.is_empty() { format!("Lagerung: steht mit {} Fläche(n) auf dem Tisch", s.fixed.len()) } else { format!("Lagerung: {} Kontaktfläche(n) zu {} Teil(en)", s.fixed.len(), s.supports.len()) });
                    ui.label(format!("Getragene Teile: {} · {:.1} g", s.loads.len(), s.loads.iter().map(|l| l.2).sum::<f64>() * 1000.0).replace('.', ","));
                    ui.label(format!("Eigenmasse {:.1} g · Werkstoff {}", s.mass * 1000.0, s.material).replace('.', ","));
                    egui::Grid::new("asm_strength_grid").spacing([14.0, 8.0]).show(ui, |ui| {
                        ui.label("Lastvielfaches");
                        ui.add(egui::DragValue::new(&mut st.factor).range(1.0..=200.0).speed(0.5).suffix(" g")).on_hover_text("1 g: Ruhelage; 20–50 g: Stoß beim Absetzen oder Fallen");
                        ui.end_row();
                        ui.label("Zusatzkraft");
                        ui.add(egui::DragValue::new(&mut st.extra).range(0.0..=5000.0).suffix(" N")).on_hover_text("Auf die tragenden Flächen, sonst auf die Oberseite (z. B. Tastendruck 10–20 N)");
                        ui.end_row();
                    });
                    ui.separator();
                    run = ui.button("Studie anlegen und lösen").clicked();
                }
            }
        });
        let (factor, extra) = (st.factor, st.extra);
        if !open || run {
            self.asm.as_mut().unwrap().assembler.strength = None;
        }
        if run {
            if let Err(e) = self.run_strength(comp, factor, extra) {
                self.say(e, MsgKind::Error);
            }
        }
    }
}
