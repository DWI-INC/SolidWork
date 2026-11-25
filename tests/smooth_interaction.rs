//! The fast paths that keep the graphics window smooth give exactly what the full
//! computations give: dragging a component in place, the incremental graphics buffers and
//! the accelerated ray casts.

use freo::app::App;
use freo::kernel::math::*;
use freo::model::assembly::*;
use freo::model::features::*;
use freo::model::regen::{self, RegenResult};

/// Everything of a display that is drawn or picked, exactly (Debug prints floats round-trip).
fn fingerprint(r: &RegenResult) -> String {
    let t = &r.topo;
    let faces: Vec<_> = t.faces.iter().map(|f| (&f.polys, f.planar, f.normal, f.offset, f.area, f.center)).collect();
    let chains: Vec<_> = t.chains.iter().map(|c| (&c.points, c.closed, c.face_a, c.face_b, &c.n_a, &c.n_b, &c.f_a, &c.f_b, c.convex)).collect();
    let planes: Vec<_> = t.planes.iter().map(|p| (p.n, p.w)).collect();
    let polys: Vec<_> = r.polys.iter().map(|p| (p.v.iter().map(|q| (q.p, q.n)).collect::<Vec<_>>(), p.plane.n, p.plane.w, p.tag)).collect();
    let mut owners: Vec<_> = r.tag_owner.iter().collect();
    owners.sort();
    format!(
        "{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}",
        t.positions, t.polys, t.vnormals, planes, t.tags, t.poly_face, faces, chains, t.draw_edges, t.bbox, polys, r.planes, r.axes, r.points, r.curves, r.cosmetics, owners
    )
}

fn sorted<K: Ord + Clone, V: Clone>(m: &std::collections::HashMap<K, V>) -> Vec<(K, V)> {
    let mut v: Vec<(K, V)> = m.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
    v.sort_by(|a, b| a.0.cmp(&b.0));
    v
}

fn same(a: &AsmResult, b: &AsmResult) {
    assert_eq!(fingerprint(&a.display), fingerprint(&b.display));
    assert_eq!(format!("{:?}", sorted(&a.placements)), format!("{:?}", sorted(&b.placements)));
    assert_eq!(format!("{:?}", sorted(&a.shown)), format!("{:?}", sorted(&b.shown)));
    assert_eq!(format!("{:?}", sorted(&a.status)), format!("{:?}", sorted(&b.status)));
    assert_eq!(format!("{:?}", sorted(&a.signs)), format!("{:?}", sorted(&b.signs)));
    assert_eq!(a.messages, b.messages);
    assert_eq!(a.bodies, b.bodies);
}

/// The sample assembly with a free bolt being placed (and the base plate edited in its place).
fn sample_with_free_bolt() -> (AssemblyData, Component) {
    let asm = freo::app::demo::sample_assembly();
    let mut bolt = Component::new(asm.next_id, "BOLT_M10", Placement::translation(V3::new(150.0, 20.0, 30.0)));
    bolt.constraints.push(AsmConstraint::new(AsmCType::Automatic));
    (asm, bolt)
}

#[test]
fn moving_in_place_equals_a_full_regen() {
    let (asm, mut bolt) = sample_with_free_bolt();
    let mut cache = PartCache::default();
    let mut prev = regen(&asm, &mut cache, &RegenOpts { explode: 0.0, editing: Some(&bolt) });
    for step in 0..4 {
        bolt.placement = bolt.placement.moved(V3::new(3.0, -1.5, 2.0 * step as f64)).rotated(V3::new(1.0, 2.0, 3.0), V3::new(0.3, 1.0, 0.2).normalize(), 0.21);
        let base = prev.display.generation;
        assert!(regen_moved(&asm, &mut cache, &bolt, &mut prev), "a free component is moved in place");
        assert_eq!(prev.moved, Some((base, bolt.id)));
        let full = regen(&asm, &mut cache, &RegenOpts { explode: 0.0, editing: Some(&bolt) });
        same(&prev, &full);
    }
    // the moved display picks like the rebuilt one (the ray boxes were reset)
    let full = regen(&asm, &mut cache, &RegenOpts { explode: 0.0, editing: Some(&bolt) });
    let c = bolt.placement.pos;
    for k in 0..40 {
        let o = c + V3::new(200.0, 150.0 - k as f64 * 7.0, 120.0);
        let d = (c - o).normalize();
        assert_eq!(prev.display.topo.ray_first_hit(o, d, 0.0), full.display.topo.ray_first_hit(o, d, 0.0));
    }
}

#[test]
fn constrained_components_and_dependents_take_the_full_path() {
    let asm = freo::app::demo::sample_assembly();
    let mut cache = PartCache::default();
    // the pillow block carries the shaft: moving it in place would leave the shaft behind
    let mut block = asm.component(101).unwrap().clone();
    let mut prev = regen(&asm, &mut cache, &RegenOpts { explode: 0.0, editing: Some(&block) });
    block.placement = block.placement.moved(V3::new(5.0, 0.0, 0.0));
    assert!(!regen_moved(&asm, &mut cache, &block, &mut prev));
    // a fully constrained bolt snaps back: same placement, display untouched
    let mut bolt = asm.component(103).unwrap().clone();
    let mut prev = regen(&asm, &mut cache, &RegenOpts { explode: 0.0, editing: Some(&bolt) });
    let generation = prev.display.generation;
    bolt.placement = bolt.placement.moved(V3::new(0.0, 0.0, 4.0));
    assert!(regen_moved(&asm, &mut cache, &bolt, &mut prev));
    let full = regen(&asm, &mut cache, &RegenOpts { explode: 0.0, editing: Some(&bolt) });
    same(&prev, &full);
    if prev.placements[&103] == full.placements[&103] && prev.moved.is_none() {
        assert_eq!(prev.display.generation, generation, "nothing moved: no new generation");
    }
    // a changed constraint is not a drag
    let mut changed = bolt.clone();
    changed.constraints[0].offset = 1.0;
    assert!(!regen_moved(&asm, &mut cache, &changed, &mut prev));
}

#[test]
fn dragging_in_the_app_updates_the_buffers_like_a_rebuild() {
    let mut app = App::new(None);
    app.demo_setup("assemble");
    if let Some(e) = app.asm.as_mut().and_then(|a| a.place.as_mut()) {
        e.comp.constraints = vec![AsmConstraint::new(AsmCType::Automatic)];
    }
    app.place_update();
    app.rebuild_buffers();
    let start = app.place_editor().unwrap().comp.placement;
    for _ in 0..5 {
        app.place_drag(7.0, -3.0, false);
        app.rebuild_buffers();
        assert!(app.buffers.dirty.is_some(), "only the dragged component's floats are rewritten");
        let incremental = (app.buffers.solid.clone(), app.buffers.edges.clone(), app.buffers.generation);
        app.buffers = Default::default();
        app.rebuild_buffers();
        assert_eq!(incremental.2, app.buffers.generation);
        assert!(incremental.0 == app.buffers.solid, "solid floats match a full rebuild");
        assert!(incremental.1 == app.buffers.edges, "edge floats match a full rebuild");
    }
    assert!(app.place_editor().unwrap().comp.placement.pos.distance(start.pos) > 1.0, "the bolt moved");
    // rotating (⇧ drag) as well
    app.place_drag(4.0, 6.0, true);
    app.rebuild_buffers();
    let incremental = app.buffers.solid.clone();
    app.buffers = Default::default();
    app.rebuild_buffers();
    assert!(incremental == app.buffers.solid);
    // and the preview is still what a full regeneration gives
    let a = app.asm.as_ref().unwrap();
    let full = regen(&a.data.clone(), &mut PartCache::default(), &RegenOpts { explode: 0.0, editing: Some(&a.place.as_ref().unwrap().comp) });
    same(a.preview.as_ref().unwrap(), &full);
}

/// Brute force over every triangle, as before the ray boxes.
fn brute_first_hit(r: &RegenResult, o: V3, d: V3, min_t: f64) -> Option<f64> {
    let t = &r.topo;
    let mut best: Option<f64> = None;
    for p in &t.polys {
        for k in 1..p.len() - 1 {
            let (a, b, c) = (t.positions[p[0]], t.positions[p[k]], t.positions[p[k + 1]]);
            let (e1, e2) = (b - a, c - a);
            let pv = d.cross(e2);
            let det = e1.dot(pv);
            if det.abs() < 1e-14 {
                continue;
            }
            let inv = 1.0 / det;
            let s = o - a;
            let u = s.dot(pv) * inv;
            if !(-1e-9..=1.0 + 1e-9).contains(&u) {
                continue;
            }
            let q = s.cross(e1);
            let v = d.dot(q) * inv;
            if v < -1e-9 || u + v > 1.0 + 1e-9 {
                continue;
            }
            let tt = e2.dot(q) * inv;
            if tt > min_t && best.is_none_or(|b| tt < b) {
                best = Some(tt);
            }
        }
    }
    best
}

#[test]
fn accelerated_ray_casts_match_brute_force() {
    let r = regen::run(&freo::app::demo::bracket());
    let b = r.topo.bbox;
    let mut seed = 12345u64;
    let mut rnd = || {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (seed >> 11) as f64 / (1u64 << 53) as f64
    };
    let mut hits = 0;
    for i in 0..3000 {
        let inside = V3::new(b.min.x + rnd() * (b.max.x - b.min.x), b.min.y + rnd() * (b.max.y - b.min.y), b.min.z + rnd() * (b.max.z - b.min.z));
        // rays from outside towards the part, axis-aligned ones too, and rays starting inside
        let (o, d) = match i % 3 {
            0 => {
                let d = V3::new(rnd() - 0.5, rnd() - 0.5, rnd() - 0.5).normalize();
                (inside - d * 400.0, d)
            }
            1 => {
                let d = [V3::X, V3::Y, V3::Z, -V3::X, -V3::Y, -V3::Z][(rnd() * 6.0) as usize % 6];
                (inside - d * 400.0, d)
            }
            _ => (inside, V3::new(rnd() - 0.5, rnd() - 0.5, rnd() - 0.5).normalize()),
        };
        let min_t = if i % 3 == 2 { 1e-4 } else { 0.0 };
        let (fast, slow) = (r.topo.ray_first_hit(o, d, min_t), brute_first_hit(&r, o, d, min_t));
        assert_eq!(fast, slow, "ray {i}: {o:?} {d:?}");
        hits += fast.is_some() as usize;
    }
    assert!(hits > 1000, "most rays hit ({hits})");
}

#[test]
fn part_cache_regenerates_only_changed_parts() {
    let mut cache = PartCache::default();
    let m = freo::app::demo::bracket();
    let a = cache.get(&m);
    assert!(std::sync::Arc::ptr_eq(&a, &cache.get(&m.clone())), "same content: cached");
    let mut changed = m.clone();
    if let FeatureKind::Hole(h) = &mut changed.features.iter_mut().find(|f| f.id == 14).unwrap().kind {
        h.diameter = Dim::v(12.0);
    }
    assert!(!std::sync::Arc::ptr_eq(&a, &cache.get(&changed)), "changed content: regenerated");
}

#[test]
fn background_preview_ends_where_the_synchronous_one_does() {
    let set = |d: f64| move |f: &mut Feature| {
        if let FeatureKind::Hole(h) = &mut f.kind {
            h.diameter = Dim::v(d);
        }
    };
    let mut sync = App::new(None);
    sync.data = freo::app::demo::heavy_part();
    sync.regenerate(false);
    let mut app = App::new(None);
    app.data = sync.data.clone();
    app.regenerate(false);
    app.async_preview = true;
    for a in [&mut sync, &mut app] {
        a.edit_feature(14);
    }
    // rapid changes (a dragged value): the window does not wait for each preview
    for d in [15.0, 15.5, 16.0, 16.5] {
        app.update_editor(set(d));
    }
    assert!(app.preview_job.is_some(), "a slow preview runs on the worker");
    sync.update_editor(set(16.5));
    app.settle_preview();
    assert!(app.preview_job.is_none());
    assert_eq!(fingerprint(app.preview.as_ref().unwrap()), fingerprint(sync.preview.as_ref().unwrap()), "the newest change is shown");
    app.finish_feature();
    sync.finish_feature();
    assert!(app.editor.is_none());
    assert_eq!(fingerprint(&app.result), fingerprint(&sync.result));
    // cancelling drops a running preview
    app.edit_feature(14);
    app.update_editor(set(9.0));
    app.cancel_feature();
    assert!(app.preview_job.is_none() && app.preview.is_none());
}
