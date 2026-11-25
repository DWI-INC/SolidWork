//! Move / rotate gizmo for the component being placed, as in Blender: three axis arrows
//! (X red, Y green, Z blue), three rotation rings in the same colours and a centre handle.
//! Dragging an arrow moves the component along that world axis only, dragging a ring turns
//! it about that world axis through the gizmo centre, and dragging the centre moves it in the
//! screen plane. Placement constraints still apply after each step, so the gizmo only moves
//! the component in directions its constraints leave free.

use super::*;
use crate::model::assembly::Placement;
use eframe::egui::{self, Color32, Pos2, Stroke};

/// Arrow length on screen in pixels.
pub const ARROW_PX: f32 = 90.0;
/// Rotation ring radius on screen in pixels.
pub const RING_PX: f32 = 64.0;
/// Pick distance around an arrow, a ring or the centre, in pixels.
const HIT_PX: f32 = 8.0;
const RING_SEGMENTS: usize = 72;
pub const AXES: [V3; 3] = [V3::X, V3::Y, V3::Z];
const COLORS: [Color32; 3] = [Color32::from_rgb(226, 62, 72), Color32::from_rgb(112, 196, 40), Color32::from_rgb(52, 120, 238)];
const HOT: Color32 = Color32::from_rgb(255, 214, 40);

/// A gizmo handle: an axis arrow, a rotation ring or the free centre.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Handle {
    Axis(usize),
    Ring(usize),
    Centre,
}

/// The gizmo laid out in viewport pixels (relative to the viewport's top-left corner).
#[derive(Clone, Debug)]
pub struct GizmoLayout {
    pub centre: Pos2,
    /// arrow tip per axis; None when the axis points at the viewer (too short to grab)
    pub tips: [Option<Pos2>; 3],
    /// screen pixels per millimetre along each axis
    pub px_per_mm: [f32; 3],
    /// closed polyline of each rotation ring; None when the ring is seen exactly edge-on
    pub rings: [Option<Vec<Pos2>>; 3],
    /// +1 when the axis points towards the viewer, -1 away from it
    pub facing: [f32; 3],
    /// world point the gizmo sits on (rotation pivot)
    pub origin: V3,
}

fn seg_dist(p: Pos2, a: Pos2, b: Pos2) -> f32 {
    let ab = b - a;
    let t = ((p - a).dot(ab) / ab.length_sq().max(1e-6)).clamp(0.0, 1.0);
    p.distance(a + ab * t)
}

fn poly_dist(p: Pos2, pts: &[Pos2]) -> f32 {
    pts.windows(2).map(|w| seg_dist(p, w[0], w[1])).fold(f32::INFINITY, f32::min)
}

impl GizmoLayout {
    pub fn hit(&self, p: Pos2) -> Option<Handle> {
        if p.distance(self.centre) <= HIT_PX + 3.0 {
            return Some(Handle::Centre);
        }
        let arrows = (0..3).filter_map(|i| self.tips[i].map(|t| (Handle::Axis(i), seg_dist(p, self.centre, t))));
        let rings = (0..3).filter_map(|i| self.rings[i].as_ref().map(|r| (Handle::Ring(i), poly_dist(p, r) + 0.5)));
        arrows.chain(rings).filter(|(_, d)| *d <= HIT_PX).min_by(|a, b| a.1.total_cmp(&b.1)).map(|(h, _)| h)
    }

    /// A point on ring `i` (for tests and hints): the sample furthest from the arrows and
    /// the other rings.
    pub fn ring_point(&self, i: usize) -> Option<Pos2> {
        let r = self.rings[i].as_ref()?;
        let clear = |p: Pos2| {
            let arrows = (0..3).filter_map(|k| self.tips[k].map(|t| seg_dist(p, self.centre, t)));
            let rings = (0..3).filter(|k| *k != i).filter_map(|k| self.rings[k].as_ref().map(|o| poly_dist(p, o)));
            arrows.chain(rings).fold(f32::INFINITY, f32::min)
        };
        r.iter().copied().max_by(|a, b| clear(*a).total_cmp(&clear(*b)))
    }
}

impl App {
    /// True while the component being placed may be moved by hand.
    pub fn gizmo_enabled(&self) -> bool {
        self.placing() && self.placement_guide().is_none_or(|g| g.free())
    }

    /// World point the gizmo sits on: the centre of the component's bounding box.
    fn gizmo_origin(&self) -> Option<V3> {
        let ed = self.place_editor()?;
        let part = self.asm_view()?.parts.get(&ed.comp.part)?;
        let b = part.topo.bbox;
        Some(ed.comp.placement.point(if b.is_empty() { V3::ZERO } else { b.center() }))
    }

    pub fn gizmo_layout(&self) -> Option<GizmoLayout> {
        if !self.gizmo_enabled() {
            return None;
        }
        let size = self.viewport_size;
        let o = self.gizmo_origin()?;
        let (cx, cy, _) = self.camera.project(o, size)?;
        let centre = Pos2::new(cx, cy);
        // world length that appears as one pixel at the origin
        let mm = self.camera.pixel_size(size.1).max(1e-9);
        let screen = |p: V3| self.camera.project(p, size).map(|(x, y, _)| Pos2::new(x, y));
        let mut tips = [None; 3];
        let mut px_per_mm = [0.0; 3];
        let mut rings: [Option<Vec<Pos2>>; 3] = Default::default();
        let mut facing = [1.0; 3];
        let back = self.camera.back();
        for (i, axis) in AXES.iter().enumerate() {
            let dot = axis.dot(back);
            facing[i] = if dot >= 0.0 { 1.0 } else { -1.0 };
            // ring in the plane normal to the axis, RING_PX in radius on screen
            let (u, v) = (axis.any_orthonormal_vector(), axis.cross(axis.any_orthonormal_vector()));
            let r = mm * RING_PX as f64;
            let pts: Option<Vec<Pos2>> = (0..=RING_SEGMENTS)
                .map(|k| {
                    let a = k as f64 / RING_SEGMENTS as f64 * std::f64::consts::TAU;
                    screen(o + (u * a.cos() + v * a.sin()) * r)
                })
                .collect();
            // seen exactly edge-on the ring is a line through the centre and can't be turned
            rings[i] = pts.filter(|_| dot.abs() > 0.06);
            let Some(tip) = screen(o + *axis * (mm * 100.0)) else { continue };
            let d = tip - centre;
            let len = d.length();
            px_per_mm[i] = len / (mm * 100.0) as f32;
            // pointing (almost) at the viewer: no usable arrow
            if len < 18.0 {
                continue;
            }
            tips[i] = Some(centre + d / len * ARROW_PX * (len / 100.0).min(1.0));
        }
        Some(GizmoLayout { centre, tips, px_per_mm, rings, facing, origin: o })
    }

    /// Drags a gizmo handle: the pointer moved by `delta` to `pointer` (viewport pixels).
    pub fn gizmo_drag(&mut self, handle: Handle, pointer: Pos2, delta: egui::Vec2) {
        let Some(g) = self.gizmo_layout() else { return };
        let moved = match handle {
            Handle::Centre => return self.place_drag(delta.x, delta.y, false),
            Handle::Axis(i) => {
                let Some(tip) = g.tips[i] else { return };
                let along = delta.dot((tip - g.centre).normalized());
                if along == 0.0 || g.px_per_mm[i] <= 0.0 {
                    return;
                }
                let d = AXES[i] * (along / g.px_per_mm[i]) as f64;
                Box::new(move |p: Placement| p.moved(d)) as Box<dyn Fn(Placement) -> Placement>
            }
            Handle::Ring(i) => {
                // the turn of the pointer about the gizmo centre on screen; screen y points
                // down, so a counter-clockwise turn on screen is a negative angle there
                let (a, b) = (pointer - delta - g.centre, pointer - g.centre);
                if a.length() < 4.0 || b.length() < 4.0 {
                    return;
                }
                let screen_turn = (a.x * b.y - a.y * b.x).atan2(a.dot(b)) as f64;
                let angle = -screen_turn * g.facing[i] as f64;
                if angle == 0.0 {
                    return;
                }
                let o = g.origin;
                Box::new(move |p: Placement| p.rotated(o, AXES[i], angle))
            }
        };
        let Some(ed) = self.asm.as_mut().and_then(|a| a.place.as_mut()) else { return };
        ed.comp.placement = moved(ed.comp.placement);
        self.place_update_moved();
    }

    /// Paints the gizmo; `hot` is the handle under the pointer or being dragged.
    pub fn paint_gizmo(&self, painter: &egui::Painter, offset: egui::Vec2, hot: Option<Handle>) {
        let Some(g) = self.gizmo_layout() else { return };
        let c = g.centre + offset;
        let outline = Color32::from_rgba_unmultiplied(0, 0, 0, 110);
        for i in 0..3 {
            let Some(ring) = &g.rings[i] else { continue };
            let pts: Vec<Pos2> = ring.iter().map(|p| *p + offset).collect();
            let col = if hot == Some(Handle::Ring(i)) { HOT } else { COLORS[i] };
            painter.add(egui::Shape::line(pts.clone(), Stroke::new(4.5_f32, outline)));
            painter.add(egui::Shape::line(pts, Stroke::new(2.5_f32, col)));
        }
        for i in 0..3 {
            let Some(tip) = g.tips[i] else { continue };
            let tip = tip + offset;
            let col = if hot == Some(Handle::Axis(i)) { HOT } else { COLORS[i] };
            let dir = (tip - c).normalized();
            let side = egui::vec2(-dir.y, dir.x);
            let head = tip - dir * 14.0;
            painter.line_segment([c, head], Stroke::new(5.0_f32, outline));
            painter.line_segment([c, head], Stroke::new(3.0_f32, col));
            let tri = vec![tip + dir * 2.0, head + side * 7.0, head - side * 7.0];
            painter.add(egui::Shape::convex_polygon(tri, col, Stroke::new(1.0_f32, outline)));
            let label = tip + dir * 11.0;
            painter.text(label, egui::Align2::CENTER_CENTER, ["X", "Y", "Z"][i], egui::FontId::proportional(12.0), col);
        }
        let col = if hot == Some(Handle::Centre) { HOT } else { Color32::WHITE };
        painter.circle_filled(c, 7.0, Color32::from_rgba_unmultiplied(255, 255, 255, 60));
        painter.circle_stroke(c, 7.0, Stroke::new(2.0_f32, col));
        painter.circle_filled(c, 2.0, col);
    }
}
