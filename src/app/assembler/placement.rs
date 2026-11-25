//! Guided placement builds the same persistent assembly mates as the constraint editor.
use super::*;
use crate::model::assembly::{AsmRef, PlaceStatus};
use crate::model::regen;
use glam::DQuat;

#[derive(Clone)]
pub(super) enum Method {
    Face(Option<FaceRef>),
    Hole { openings: Vec<Hole>, axis: AsmRef, seat: AsmRef },
    Free,
}

#[derive(Clone, Copy)]
pub(super) struct Hole {
    pub center: V3,
    pub normal: V3,
    wall: AsmRef,
    seat: AsmRef,
}

#[derive(Clone)]
pub(in crate::app) struct Guide {
    pub(super) method: Method,
    pub(in crate::app) attached: bool,
    initial: Component,
    pub(super) axis: usize,
    pub(super) degrees: f64,
}

impl Guide {
    pub(in crate::app) fn free(&self) -> bool {
        matches!(self.method, Method::Free)
    }

    pub(in crate::app) fn choosing_source(&self) -> bool {
        matches!(self.method, Method::Face(None))
    }

    pub(super) fn instruction(&self) -> &'static str {
        match &self.method {
            Method::Free => "Pfeile verschieben entlang X/Y/Z, Ringe drehen um X/Y/Z, Teil ziehen verschiebt frei.",
            Method::Face(None) => "Am neuen Teil die flache Seite anklicken, die anliegen soll.",
            Method::Face(Some(_)) if self.attached => "Sitz prüfen. Zum Versetzen eine andere Stelle auf der Zielfläche anklicken.",
            Method::Face(Some(_)) => "Auf eine flache Stelle der Baugruppe klicken. Dort sitzt die Kontaktfläche des neuen Teils.",
            Method::Hole { .. } if self.attached => "Sitz prüfen und die Schraube einbauen. Ein anderer Ring ändert die Position.",
            Method::Hole { openings, .. } if openings.is_empty() => "Keine passende Durchgangsbohrung gefunden. Eine andere Schraubengröße wählen oder frei platzieren.",
            Method::Hole { .. } => "Einen markierten Ring anklicken. Der Schraubenkopf liegt auf dieser Seite der Bohrung auf.",
        }
    }
}

fn mate(a: AsmRef, b: AsmRef) -> AsmConstraint {
    AsmConstraint { a: Some(a), b: Some(b), ..AsmConstraint::new(AsmCType::Coincident) }
}

impl App {
    pub(in crate::app) fn start_placement_guide(&mut self) {
        let Some(ed) = self.place_editor() else { return };
        let initial = ed.comp.clone();
        let view = self.asm_view().unwrap();
        let Some(part) = view.parts.get(&initial.part) else { return };
        // Library screws supply an axial reference and a bearing face at the origin.
        let a = self.asm.as_ref().unwrap();
        let def = &a.data.part(&initial.part).unwrap().data;
        let mount = match def.interface.as_slice() {
            [axis, seat] if axis.ty == AsmCType::Coincident && seat.ty == AsmCType::Coincident => match (axis.r, seat.r) {
                (IfaceRef::Axis(6), IfaceRef::Surface { face, planar: true }) if face.normal.dot(-V3::Y) > 0.999 && face.point.y.abs() < 1e-6 => {
                    part.cosmetics.iter().find(|c| !c.internal).map(|c| (c.radius, face))
                }
                _ => None,
            },
            _ => None,
        };
        let targets = a.data.components.iter().take_while(|c| c.id != initial.id).any(|c| !c.suppressed && !c.hidden);
        let method = if !targets {
            Method::Free
        } else if let Some((radius, face)) = mount {
            Method::Hole { openings: self.mounting_holes(radius), axis: AsmRef::Axis { comp: initial.id, id: 6 }, seat: AsmRef::Surface { comp: initial.id, face, planar: true } }
        } else {
            Method::Face(None)
        };
        let ed = self.asm.as_mut().unwrap().place.as_mut().unwrap();
        ed.guide = Some(Guide { method, attached: false, initial, axis: 2, degrees: 15.0 });
        if self.placement_guide().unwrap().free() {
            self.assembler_place_freely();
        }
        self.say(self.placement_guide().unwrap().instruction(), MsgKind::Prompt);
    }

    pub(super) fn underside(&self) -> Option<FaceRef> {
        let view = self.asm_view()?;
        let ed = self.place_editor()?;
        let t = &view.parts.get(&ed.comp.part)?.topo;
        t.faces
            .iter()
            .enumerate()
            .filter(|(_, f)| f.planar && f.normal.dot(-V3::Y) > 0.99999 && (f.center.y - t.bbox.min.y).abs() < t.bbox.diagonal().max(1.0) * 1e-6)
            .max_by(|(_, a), (_, b)| a.area.total_cmp(&b.area))
            .map(|(_, face)| {
                let mut footprint = crate::kernel::math::BBox::default();
                for &poly in &face.polys {
                    for &v in &t.polys[poly] {
                        footprint.add(t.positions[v]);
                    }
                }
                FaceRef { point: footprint.center(), normal: face.normal }
            })
    }

    pub(super) fn choose_source(&mut self, face: FaceRef) {
        let Some(ed) = self.asm.as_mut().and_then(|a| a.place.as_mut()) else { return };
        let Some(g) = &mut ed.guide else { return };
        g.method = Method::Face(Some(face));
        g.attached = false;
        self.say("Jetzt die Zielfläche am eingebauten Teil anklicken.", MsgKind::Prompt);
    }

    pub(in crate::app) fn selected_source(&self) -> Option<AsmRef> {
        let ed = self.place_editor()?;
        let g = ed.guide.as_ref()?;
        if let Method::Face(Some(face)) = g.method {
            return Some(AsmRef::Surface { comp: ed.comp.id, face, planar: true });
        }
        None
    }

    pub(in crate::app) fn placement_guide(&self) -> Option<&Guide> {
        self.place_editor()?.guide.as_ref()
    }

    pub(in crate::app) fn can_finish_guided(&self) -> bool {
        self.placement_guide().is_none_or(|g| g.free() || (g.attached && !matches!(self.place_editor().unwrap().status, PlaceStatus::Invalid(_))))
    }

    pub fn assembler_place_freely(&mut self) {
        let Some(ed) = self.asm.as_mut().and_then(|a| a.place.as_mut()) else { return };
        let Some(g) = &mut ed.guide else { return };
        if g.axis == 3 {
            g.axis = 2;
        }
        g.method = Method::Free;
        g.attached = false;
        ed.comp.constraints.clear();
        self.place_update();
    }

    pub(super) fn restart_snap(&mut self) {
        let Some(ed) = self.asm.as_mut().and_then(|a| a.place.as_mut()) else { return };
        let Some(g) = ed.guide.take() else { return };
        let pose = ed.comp.placement;
        ed.comp = g.initial;
        ed.comp.constraints.clear();
        ed.comp.placement = pose;
        self.place_update();
        self.start_placement_guide();
    }

    pub(super) fn change_contact(&mut self) {
        let Some(ed) = self.asm.as_mut().and_then(|a| a.place.as_mut()) else { return };
        let Some(g) = &mut ed.guide else { return };
        ed.comp = g.initial.clone();
        if g.axis == 3 {
            g.axis = 2;
        }
        g.method = Method::Face(None);
        g.attached = false;
        self.place_update();
    }

    pub(super) fn advanced_placement(&mut self) {
        let Some(ed) = self.asm.as_mut().and_then(|a| a.place.as_mut()) else { return };
        let Some(g) = ed.guide.take() else { return };
        if !g.attached && !g.free() {
            ed.comp = g.initial;
        }
        if ed.comp.constraints.is_empty() {
            ed.comp.constraints.push(AsmConstraint::new(AsmCType::Automatic));
        }
        ed.active = 0;
        self.place_update();
    }

    pub(in crate::app) fn reposition_in_assembler(&mut self, id: u64) {
        if self.busy() || self.component(id).is_none_or(|c| c.suppressed || c.hidden) {
            return;
        }
        if self.asm.as_ref().is_some_and(|a| a.exploded) {
            self.toggle_exploded();
        }
        self.edit_placement(id);
        if self.placing() {
            self.start_placement_guide();
        }
    }

    pub(super) fn rotate_guided(&mut self, axis: V3, degrees: f64) {
        if !degrees.is_finite() || degrees == 0.0 {
            return;
        }
        let Some(a) = &mut self.asm else { return };
        let (Some(ed), Some(view)) = (&mut a.place, &a.preview) else { return };
        let Some(g) = &ed.guide else { return };
        let p = ed.comp.placement;
        let (pivot, normal) = match g.method {
            Method::Face(Some(face)) => (p.point(face.point), -p.vector(face.normal)),
            Method::Hole { .. } if g.attached => (p.pos, p.vector(V3::Y)),
            _ => {
                let bbox = &view.parts[&ed.comp.part].topo.bbox;
                (p.point(if bbox.is_empty() { V3::ZERO } else { bbox.center() }), V3::ZERO)
            }
        };
        let axis = if axis == V3::ZERO { normal } else { axis };
        if axis == V3::ZERO || g.attached && normal.cross(axis).length() > 1e-6 {
            return;
        }
        let rotated = p.rotated(pivot, axis, degrees.to_radians());
        if !g.attached {
            ed.comp.constraints.clear();
        }
        for c in &mut ed.comp.constraints {
            if let (Some(AsmRef::Contact { point: local, .. }), Some(AsmRef::Contact { comp, point, .. })) = (c.a, &mut c.b) {
                *point = view.shown[comp].inverse().point(rotated.point(local));
            }
        }
        ed.comp.placement = rotated;
        self.place_update();
    }

    fn mounting_holes(&self, screw_radius: f64) -> Vec<Hole> {
        let view = self.asm_view().unwrap();
        let t = &view.display.topo;
        let moving = view.bodies.iter().position(|id| *id == self.place_editor().unwrap().comp.id).unwrap();
        let mut holes = Vec::new();
        for (wall, f) in t.faces.iter().enumerate() {
            if f.planar || (t.face_tag(wall) >> crate::kernel::topology::BODY_SHIFT) as usize > moving {
                continue;
            }
            let Some((start, axis, radius, length, true)) = regen::thread_cylinder(t, wall, false) else { continue };
            if radius + 0.01 < screw_radius || radius >= 1.7 * screw_radius {
                continue;
            }
            let Some(wall_ref) = self.asm_ref_of(Pick::Face(wall)) else { continue };
            for edge in &t.chains {
                let seat = if edge.face_a == wall {
                    edge.face_b
                } else if edge.face_b == wall {
                    edge.face_a
                } else {
                    continue;
                };
                let Some(s) = t.faces.get(seat) else { continue };
                let dot = s.normal.dot(axis);
                if !s.planar || dot.abs() < 0.999 {
                    continue;
                }
                let height = (s.center - start).dot(s.normal) / dot;
                if (height - length * 0.5) * dot <= 0.0 {
                    continue;
                }
                let Some(seat_ref) = self.asm_ref_of(Pick::Face(seat)) else { continue };
                let center = start + axis * height;
                if !holes.iter().any(|h: &Hole| h.wall == wall_ref && h.seat == seat_ref) {
                    holes.push(Hole { center, normal: s.normal, wall: wall_ref, seat: seat_ref });
                }
            }
        }
        holes
    }

    /// Only openings on the visible side of an unobstructed surface get a pick target.
    pub(in crate::app) fn hole_markers(&self) -> Vec<(usize, egui::Pos2)> {
        let Some(Guide { method: Method::Hole { openings: holes, .. }, .. }) = self.placement_guide() else { return vec![] };
        let a = self.asm.as_ref().unwrap();
        let mut markers = Vec::new();
        for (i, h) in holes.iter().enumerate() {
            let Some((x, y, _)) = self.camera.project(h.center, self.viewport_size) else { continue };
            let (eye, ray) = self.camera.ray((x, y), self.viewport_size);
            if h.normal.dot(ray) > -0.02 {
                continue;
            }
            let distance = (h.center - eye).dot(ray);
            if a.res.display.topo.ray_first_hit(eye, ray, 0.0).is_some_and(|t| t < distance - 0.01) {
                continue;
            }
            markers.push((i, egui::pos2(x, y)));
        }
        markers
    }

    pub(in crate::app) fn guided_click(&mut self, px: (f32, f32), hole: Option<usize>) {
        let Some(g) = self.placement_guide() else { return };
        let id = g.initial.id;
        let constraints;
        let pose;
        match &g.method {
            Method::Free => return,
            Method::Hole { openings: holes, axis, seat } => {
                let Some(h) = hole.and_then(|i| holes.get(i)) else {
                    self.say(g.instruction(), MsgKind::Prompt);
                    return;
                };
                constraints = vec![mate(*axis, h.wall), mate(*seat, h.seat)];
                pose = Placement { pos: h.center, rot: DQuat::from_rotation_arc(V3::Y, h.normal) };
            }
            Method::Face(source) => {
                let Some(Pick::Face(f)) = self.pick_at(px) else { return };
                let Some(AsmRef::Surface { comp, face, planar: true }) = self.asm_ref_of(Pick::Face(f)) else {
                    self.say("Eine ebene Fläche anklicken.", MsgKind::Prompt);
                    return;
                };
                let view = self.asm_view().unwrap();
                let target = view.shown[&comp];
                let (eye, ray) = self.camera.ray(px, self.viewport_size);
                let normal = target.vector(face.normal);
                let denom = ray.dot(normal);
                if denom.abs() < 1e-9 {
                    return;
                }
                let hit = eye + ray * ((target.point(face.point) - eye).dot(normal) / denom);
                let Some(source) = *source else {
                    if comp != id {
                        return;
                    }
                    self.choose_source(FaceRef { point: target.inverse().point(hit), ..face });
                    return;
                };
                if comp == id {
                    return;
                }
                let current = self.place_editor().unwrap().comp.placement.rot;
                let rot = (DQuat::from_rotation_arc(current * source.normal, -normal) * current).normalize();
                pose = Placement { pos: hit - rot * source.point, rot };
                let target_face = FaceRef { point: target.inverse().point(hit), ..face };
                // Two points on the contact plane retain both the click position and the twist
                // when the parent component moves or rotates.
                let contact =
                    |point| mate(AsmRef::Contact { comp: id, face: source, point }, AsmRef::Contact { comp, face: target_face, point: target.inverse().point(pose.point(point)) });
                let lever = view.parts[&g.initial.part].topo.bbox.diagonal().max(1.0) * 0.25;
                constraints = vec![
                    mate(AsmRef::Surface { comp: id, face: source, planar: true }, AsmRef::Surface { comp, face: target_face, planar: true }),
                    contact(source.point),
                    contact(source.point + source.normal.any_orthonormal_vector() * lever),
                ];
            }
        }
        let ed = self.asm.as_mut().unwrap().place.as_mut().unwrap();
        ed.comp.placement = pose;
        ed.comp.constraints = constraints;
        let g = ed.guide.as_mut().unwrap();
        if !g.attached {
            g.axis = 3;
        }
        g.attached = true;
        self.place_update();
    }
}
