//! Regressions for the assembly placement solver and the Component Placement workflow:
//! Flip, picks after a full constraint set, Angle Offset at 0°/180°, zero distances,
//! stored placements after a parent goes away, and save / reload.

use freo::app::{App, Pick};
use freo::kernel::math::*;
use freo::model::assembly::*;
use freo::model::features::*;
use freo::model::regen;
use freo::sketch::sketch::Sketch;

fn feat(id: u64, kind: FeatureKind) -> Feature {
    Feature { id, name: format!("Extrude {}", id - 9), suppressed: false, hidden: false, kind }
}

fn extrude(sk: Sketch, depth: f64) -> FeatureKind {
    let FeatureKind::Extrude(mut e) = FeatureKind::new_extrude() else { unreachable!() };
    e.plane = Some(PlaneRef::Datum(TOP_ID));
    e.sketch = sk;
    e.depth = Dim::v(depth);
    FeatureKind::Extrude(e)
}

fn part(name: &str, sk: Sketch, depth: f64) -> ModelData {
    let mut m = ModelData::new_part(name);
    m.features.push(feat(10, extrude(sk, depth)));
    m
}

/// 100 x 60 x 20 base with a Ø10 through hole at the origin.
fn base() -> ModelData {
    let mut m = part("BASE", Sketch::rect(100.0, 60.0, 0.0, 0.0), 20.0);
    let FeatureKind::Hole(mut h) = FeatureKind::new_hole() else { unreachable!() };
    h.plane = Some(PlaneRef::Face(FaceRef { point: V3::new(0.0, 20.0, 0.0), normal: V3::Y }));
    h.diameter = Dim::v(10.0);
    h.through_all = true;
    m.features.push(Feature { id: 11, name: "Hole 1".into(), suppressed: false, hidden: false, kind: FeatureKind::Hole(h) });
    m
}

fn setup(tag: &str) -> (App, std::path::PathBuf) {
    let dir = std::env::temp_dir().join(format!("freo_asmfix_{}_{}", tag, std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    for (file, m) in [
        ("base", base()),
        ("peg", part("PEG", Sketch::circle(5.0, 0.0, 0.0), 30.0)),
        ("blk", part("BLK", Sketch::rect(30.0, 20.0, 0.0, 0.0), 10.0)),
    ] {
        std::fs::write(dir.join(format!("{file}.pfprt")), serde_json::to_string(&m).unwrap()).unwrap();
    }
    let mut app = App::new(None);
    app.new_assembly("ASM0001");
    app.assemble_path(dir.join("base.pfprt"));
    app.place_finish();
    (app, dir)
}

/// A face of component `comp` on screen, chosen by `score` (highest wins).
fn face_of(app: &App, comp: u64, planar: bool, score: impl Fn(V3, V3) -> f64) -> usize {
    let r = app.displayed();
    let t = &r.topo;
    (0..t.faces.len())
        .filter(|&f| r.tag_owner.get(&t.face_tag(f)) == Some(&comp) && t.faces[f].planar == planar)
        .max_by(|&a, &b| score(t.faces[a].center, t.faces[a].normal).partial_cmp(&score(t.faces[b].center, t.faces[b].normal)).unwrap())
        .expect("face")
}

fn pick(app: &mut App, p: Pick) {
    app.handle_pick(Some(p), false);
}

fn top_of(app: &App, comp: u64) -> usize {
    face_of(app, comp, true, |c, n| n.y * 1000.0 + c.y)
}

fn bottom_of(app: &App, comp: u64) -> usize {
    face_of(app, comp, true, |c, n| -n.y * 1000.0 - c.y)
}

/// Mates `id`'s bottom to `on`'s top and aligns its RIGHT / FRONT datums with the assembly's.
fn place_on(app: &mut App, id: u64, on: u64) {
    let (b, t) = (bottom_of(app, id), top_of(app, on));
    pick(app, Pick::Face(b));
    pick(app, Pick::Face(t));
    for d in [RIGHT_ID, FRONT_ID] {
        pick(app, Pick::Plane(comp_datum_id(id, d)));
        pick(app, Pick::Plane(d));
    }
}

#[test]
fn flip_of_a_free_coaxial_survives_later_updates_and_drags() {
    let (mut app, dir) = setup("flip");
    app.assemble_path(dir.join("peg.pfprt"));
    let id = app.place_editor().unwrap().comp.id;
    let (s, h) = (face_of(&app, id, false, |_, _| 0.0), face_of(&app, 100, false, |_, _| 0.0));
    pick(&mut app, Pick::Face(s));
    pick(&mut app, Pick::Face(h));
    let up = |app: &App| app.place_editor().unwrap().comp.placement.vector(V3::Y);
    assert!(up(&app).dot(V3::Y) > 0.999);
    {
        let ed = app.asm.as_mut().unwrap().place.as_mut().unwrap();
        ed.active = 0;
        ed.comp.constraints[0].flip();
    }
    app.place_update();
    assert!(up(&app).dot(V3::Y) < -0.999, "flipped: {:?}", up(&app));
    // any later update (offset edit, drag, ⌘G) used to turn the peg back
    app.place_update();
    assert!(up(&app).dot(V3::Y) < -0.999, "still flipped after the next update: {:?}", up(&app));
    app.place_drag(0.0, -10.0, false);
    assert!(up(&app).dot(V3::Y) < -0.999, "still flipped after dragging: {:?}", up(&app));
    app.place_finish();
    let a = app.asm.as_ref().unwrap();
    assert!(a.res.placements[&id].vector(V3::Y).dot(V3::Y) < -0.999, "kept after OK");
    // and flipping back works
    app.edit_placement(id);
    {
        let ed = app.asm.as_mut().unwrap().place.as_mut().unwrap();
        ed.comp.constraints[0].flip();
    }
    app.place_update();
    app.place_update();
    assert!(up(&app).dot(V3::Y) > 0.999);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn flipping_a_mate_before_its_first_solve_aligns_the_surfaces() {
    let mut asm = AssemblyData::new("A");
    asm.add_part(base(), None);
    let blk = part("BLK", Sketch::rect(30.0, 20.0, 0.0, 0.0), 10.0);
    asm.add_part(blk.clone(), None);
    let mut c = Component::new(100, "BASE", Placement::default());
    c.constraints.push(AsmConstraint::new(AsmCType::Default));
    asm.components.push(c);
    let (rb, rk) = (regen::run(&base()), regen::run(&blk));
    let face = |r: &regen::RegenResult, n: V3| {
        let t = &r.topo;
        t.face_ref((0..t.faces.len()).filter(|&f| t.faces[f].planar && t.faces[f].normal.dot(n) > 0.999).max_by(|&a, &b| (t.faces[a].center.dot(n)).total_cmp(&t.faces[b].center.dot(n))).unwrap())
    };
    let mut k = AsmConstraint::new(AsmCType::Coincident);
    k.a = Some(AsmRef::Surface { comp: 101, face: face(&rk, -V3::Y), planar: true });
    k.b = Some(AsmRef::Surface { comp: 100, face: face(&rb, V3::Y), planar: true });
    k.flip();
    let mut c = Component::new(101, "BLK", Placement::translation(V3::new(0.0, 40.0, 0.0)));
    c.constraints.push(k);
    asm.components.push(c);
    let res = regen(&asm, &mut PartCache::default(), &RegenOpts::default());
    assert!(res.placements[&101].vector(V3::Y).dot(V3::Y) < -0.999, "Flip turns the mate into an align: {:?}", res.placements[&101]);
}

#[test]
fn picking_after_a_full_set_starts_a_new_constraint() {
    let (mut app, dir) = setup("full");
    app.assemble_path(dir.join("blk.pfprt"));
    let id = app.place_editor().unwrap().comp.id;
    for d in [RIGHT_ID, TOP_ID, FRONT_ID] {
        pick(&mut app, Pick::Plane(comp_datum_id(id, d)));
        pick(&mut app, Pick::Plane(d));
    }
    let ed = app.place_editor().unwrap();
    assert!(ed.status.is_full(), "{:?}", ed.status);
    let before = ed.comp.constraints.clone();
    // a further pick used to overwrite the FRONT/ASM_FRONT constraint's component reference
    let b = bottom_of(&app, id);
    pick(&mut app, Pick::Face(b));
    let ed = app.place_editor().unwrap();
    assert_eq!(&ed.comp.constraints[..3], &before[..], "finished constraints are untouched");
    assert_eq!(ed.comp.constraints.len(), 4);
    assert_eq!(ed.active, 3);
    assert!(ed.comp.constraints[3].a.is_some() && ed.comp.constraints[3].b.is_none());
    // re-picking through a collector still replaces that reference
    {
        let ed = app.asm.as_mut().unwrap().place.as_mut().unwrap();
        ed.active = 1;
        ed.side = Some(freo::app::assembly::Side::Comp);
    }
    let b = bottom_of(&app, id);
    pick(&mut app, Pick::Face(b));
    let ed = app.place_editor().unwrap();
    assert_eq!(ed.comp.constraints.len(), 4);
    assert!(matches!(ed.comp.constraints[1].a, Some(AsmRef::Surface { .. })));
    let _ = std::fs::remove_dir_all(dir);
}

fn plate_with(angle: f64) -> AsmResult {
    let mut asm = AssemblyData::new("A");
    asm.add_part(part("BLK", Sketch::rect(30.0, 20.0, 0.0, 0.0), 10.0), None);
    let pc = |ty, a: u64, b: u64, off: f64| AsmConstraint { ty, a: Some(AsmRef::Plane { comp: 100, id: a }), b: Some(AsmRef::AsmPlane(b)), offset: off, sign: 0.0, enabled: true };
    let mut c = Component::new(100, "BLK", Placement::translation(V3::new(3.0, 4.0, 5.0)));
    c.constraints = vec![pc(AsmCType::Coincident, TOP_ID, TOP_ID, 0.0), pc(AsmCType::Angle, RIGHT_ID, RIGHT_ID, angle)];
    asm.components.push(c);
    regen(&asm, &mut PartCache::default(), &RegenOpts::default())
}

#[test]
fn angle_offset_fixes_the_rotation_at_every_angle() {
    // TOP on ASM_TOP leaves X, Z and the spin about Y; the angle takes the spin at 0° and 180° too
    for (deg, x) in [(0.0, V3::X), (30.0, V3::new(30f64.to_radians().cos(), 0.0, -30f64.to_radians().sin())), (90.0, -V3::Z), (180.0, -V3::X)] {
        let res = plate_with(deg);
        assert_eq!(res.status[&100], PlaceStatus::Partial(2), "{deg}°");
        let got = res.placements[&100].vector(V3::X);
        assert!(got.dot(x).abs() > 1.0 - 1e-6 && (deg != 0.0 && deg != 180.0 || got.dot(x) > 1.0 - 1e-6), "{deg}°: {:?}", got);
    }
}

#[test]
fn zero_distance_between_axes_acts_as_coaxial() {
    let mut asm = AssemblyData::new("A");
    asm.add_part(base(), None);
    let peg = part("PEG", Sketch::circle(5.0, 0.0, 0.0), 30.0);
    asm.add_part(peg.clone(), None);
    let mut c = Component::new(100, "BASE", Placement::default());
    c.constraints.push(AsmConstraint::new(AsmCType::Default));
    asm.components.push(c);
    let cyl = |r: &regen::RegenResult| {
        let t = &r.topo;
        t.surface_ref((0..t.faces.len()).find(|&f| !t.faces[f].planar).unwrap())
    };
    let (rb, rp) = (regen::run(&base()), regen::run(&peg));
    let mut k = AsmConstraint::new(AsmCType::Distance);
    k.a = Some(AsmRef::Surface { comp: 101, face: cyl(&rp), planar: false });
    k.b = Some(AsmRef::Surface { comp: 100, face: cyl(&rb), planar: false });
    let mut c = Component::new(101, "PEG", Placement::translation(V3::new(20.0, 40.0, 7.0)));
    c.constraints.push(k);
    asm.components.push(c);
    let res = regen(&asm, &mut PartCache::default(), &RegenOpts::default());
    // |d| − 0 has no gradient on the axis: the status used to report extra freedom
    assert_eq!(res.status[&101], PlaceStatus::Partial(2), "spin and slide remain");
    let p = res.placements[&101].point(V3::ZERO);
    assert!(V3::new(p.x, 0.0, p.z).length() < 1e-5, "on the hole axis: {:?}", p);
}

#[test]
fn children_follow_edits_and_stay_put_when_their_parent_goes() {
    let (mut app, dir) = setup("parent");
    app.assemble_path(dir.join("blk.pfprt"));
    let blk = app.place_editor().unwrap().comp.id;
    place_on(&mut app, blk, 100);
    assert!(app.place_editor().unwrap().status.is_full());
    app.place_finish();
    app.assemble_path(dir.join("peg.pfprt"));
    let peg = app.place_editor().unwrap().comp.id;
    place_on(&mut app, peg, blk);
    assert!(app.place_editor().unwrap().status.is_full());
    app.place_finish();
    let y = |app: &App, id: u64| app.asm.as_ref().unwrap().res.placements[&id].pos.y;
    assert!((y(&app, peg) - 30.0).abs() < 1e-5);

    // lift the block by 25 with a distance: the peg follows
    app.edit_placement(blk);
    {
        let ed = app.asm.as_mut().unwrap().place.as_mut().unwrap();
        ed.comp.constraints[0].ty = AsmCType::Distance;
        ed.comp.constraints[0].offset = 25.0;
    }
    app.place_update();
    assert!(app.place_editor().unwrap().status.is_full());
    app.place_finish();
    assert!((y(&app, blk) - 45.0).abs() < 1e-5, "{}", y(&app, blk));
    assert!((y(&app, peg) - 55.0).abs() < 1e-5, "{}", y(&app, peg));

    // save and reopen: same placements and status
    let path = dir.join("a.pfasm");
    app.asm_save_to(path.clone());
    let mut app2 = App::new(None);
    app2.load(path);
    let a2 = app2.asm.as_ref().unwrap();
    assert!((a2.res.placements[&peg].pos.y - 55.0).abs() < 1e-5);
    assert!(a2.res.status[&peg].is_full() && a2.res.status[&blk].is_full());

    // suppressing the block leaves the peg where it was (its mate reference is missing)
    app.asm_set_flag(&[blk], Some(true), None);
    assert!((y(&app, peg) - 55.0).abs() < 1e-5, "peg jumped to {}", y(&app, peg));
    app.asm_set_flag(&[blk], Some(false), None);
    // deleting it drops the mate; the peg keeps its last position instead of the first one
    app.asm_delete(&[blk]);
    assert!((y(&app, peg) - 55.0).abs() < 1e-5, "peg jumped to {}", y(&app, peg));
    app.do_undo();
    assert_eq!(app.asm.as_ref().unwrap().data.components.len(), 3);
    assert!((y(&app, peg) - 55.0).abs() < 1e-5);
    let _ = std::fs::remove_dir_all(dir);
}
