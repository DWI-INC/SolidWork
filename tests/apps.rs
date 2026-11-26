//! The tools of the Anwendungen tab: engines checked against hand calculations, and the
//! dialogs driven headless through the same paths the ribbon buttons use.

use freo::app::{App, Pick, Tab};
use freo::apps::{gdt, mold, moldflow, raytrace, stackup, topopt, weld, worksheet};
use freo::kernel::math::*;
use freo::model::features::*;
use freo::model::regen;
use freo::model::sim::*;
use freo::sim::run::Shared;
use freo::sketch::sketch::Sketch;
use std::sync::atomic::{AtomicBool, AtomicU32};
use std::sync::Mutex;

/// Plate along X: 0..len, w (Z) × t (Y), bottom on the TOP plane.
fn plate_data(len: f64, w: f64, t: f64) -> ModelData {
    let mut m = ModelData::new_part("PLATTE");
    let FeatureKind::Extrude(mut e) = FeatureKind::new_extrude() else { unreachable!() };
    e.plane = Some(PlaneRef::Datum(TOP_ID));
    e.sketch = Sketch::rect(len, w, len / 2.0, 0.0);
    e.depth = Dim::v(t);
    m.features.push(Feature { id: 10, name: "Profil 1".into(), suppressed: false, hidden: false, kind: FeatureKind::Extrude(e) });
    m
}

fn plate(len: f64, w: f64, t: f64) -> regen::RegenResult {
    let r = regen::run(&plate_data(len, w, t));
    assert!(r.errors.is_empty(), "{:?}", r.errors);
    r
}

fn shared() -> Shared {
    Shared { progress: AtomicU32::new(0), cancel: AtomicBool::new(false), stage: Mutex::new(String::new()), partial: Mutex::new(None), done: Mutex::new(None) }
}

fn face(r: &regen::RegenResult, n: V3) -> usize {
    let t = &r.topo;
    (0..t.faces.len())
        .filter(|&f| t.faces[f].planar && t.faces[f].normal.dot(n) > 0.999)
        .max_by(|&a, &b| t.faces[a].center.dot(n).partial_cmp(&t.faces[b].center.dot(n)).unwrap())
        .unwrap()
}

#[test]
fn draft_check_projected_area_and_parting_plane() {
    let r = plate(100.0, 40.0, 3.0);
    let c = mold::check(&r.topo, V3::Y, 1.0);
    // the plate's shadow from above: 100 × 40
    assert!((c.projected - 4000.0).abs() < 4000.0 * 0.02, "projected {}", c.projected);
    // top releases to the fixed half, bottom to the moving half, the four sides have no draft
    assert!((c.area[0] - 4000.0).abs() < 1.0 && (c.area[1] - 4000.0).abs() < 1.0, "{:?}", c.area);
    assert!((c.area[3] - 2.0 * (100.0 + 40.0) * 3.0).abs() < 1.0, "{:?}", c.area);
    assert_eq!(c.area[4], 0.0);
    assert_eq!(c.bad_faces.len(), 4);
    assert!((c.extent.z - 3.0).abs() < 1e-6);
    // pulled sideways, the large faces have no draft
    let side = mold::check(&r.topo, V3::X, 1.0);
    assert!((side.projected - 120.0).abs() < 6.0, "{}", side.projected);
}

#[test]
fn undercut_found_under_an_overhang() {
    // I-profile in the FRONT plane, extruded along Z: the flanges hide each other
    let mut m = ModelData::new_part("I");
    for (i, (w, h, cx, cy)) in [(60.0, 10.0, 30.0, 5.0), (10.0, 30.0, 30.0, 20.0), (60.0, 5.0, 30.0, 32.5)].iter().enumerate() {
        let FeatureKind::Extrude(mut e) = FeatureKind::new_extrude() else { unreachable!() };
        e.plane = Some(PlaneRef::Datum(FRONT_ID));
        e.sketch = Sketch::rect(*w, *h, *cx, *cy);
        e.depth = Dim::v(20.0);
        m.features.push(Feature { id: 10 + i as u64, name: format!("Profil {}", i + 1), suppressed: false, hidden: false, kind: FeatureKind::Extrude(e) });
    }
    let r = regen::run(&m);
    assert!(r.errors.is_empty(), "{:?}", r.errors);
    // pulled along +Y the lower flange's top lies under the upper flange: undercut
    let up = mold::check(&r.topo, V3::Y, 1.0);
    assert!(up.area[4] > 400.0, "{:?}", up.area);
    assert!(!up.undercut_faces.is_empty());
    // pulled along Z (the extrusion direction) the profile comes out freely
    let z = mold::check(&r.topo, V3::Z, 1.0);
    assert_eq!(z.area[4], 0.0, "{:?}", z.area);
}

#[test]
fn mould_base_from_standard_sizes() {
    let b = mold::mold_base(V3::new(100.0, 40.0, 3.0), 1.5, 1.5, 2, 0.5, 170.0);
    assert_eq!(b.layout, (2, 1));
    let a = &b.plates[1];
    assert!([156.0, 196.0, 246.0, 296.0, 346.0, 396.0, 446.0].contains(&a.size.0) && [156.0, 196.0, 246.0, 296.0].contains(&a.size.1), "{:?}", a.size);
    assert!(b.height > 100.0 && b.height < 400.0, "{}", b.height);
    let m = b.machine.expect("a machine of the standard series");
    assert!(m.kn >= 340.0 && m.tie_bar >= a.size.1);
    assert!(b.steel_kg > 10.0);
    // cooling of a 2 mm ABS wall: about 8 s
    let t = mold::cooling_time(&mold::MATERIALS[0], 2.0);
    assert!(t > 5.0 && t < 12.0, "{}", t);
}

#[test]
fn filling_a_plate_from_its_end() {
    let r = plate(120.0, 30.0, 3.0);
    let inp = moldflow::FillInput { topo: r.topo.clone(), material: mold::MATERIALS[0], gate: Some(V3::new(0.0, 1.5, 0.0)), fill_time: 1.5, cells: 15_000, cavities: 1 };
    let res = moldflow::run(&inp, &shared()).expect("fill");
    let o = &res.out;
    let fill = &o.fields[0];
    assert_eq!(fill.name, "Füllzeit");
    assert!((fill.max as f64 - 1.5).abs() < 0.05, "{}", fill.max);
    // fill time grows along the flow: the far end fills last
    let s = &o.surface;
    let (mut near, mut far) = (vec![], vec![]);
    for (i, p) in s.pos.iter().enumerate() {
        if p[0] < 15.0 {
            near.push(fill.vals[i]);
        } else if p[0] > 105.0 {
            far.push(fill.vals[i]);
        }
    }
    let avg = |v: &Vec<f32>| v.iter().filter(|x| x.is_finite()).sum::<f32>() / v.len().max(1) as f32;
    assert!(avg(&near) < 0.3 && avg(&far) > 1.2, "near {} far {}", avg(&near), avg(&far));
    // wall thickness reads the plate's 3 mm in the middle of the faces
    let th = &o.fields[2];
    assert_eq!(th.name, "Wanddicke");
    let mid: Vec<f32> = s.pos.iter().enumerate().filter(|(_, p)| p[1] > 2.9 && (p[0] - 60.0).abs() < 20.0 && p[2].abs() < 8.0).map(|(i, _)| th.vals[i]).collect();
    let m = mid.iter().sum::<f32>() / mid.len().max(1) as f32;
    assert!((m - 3.0).abs() < 0.9, "wall {}", m);
    assert!(res.pressure_mpa > 0.5 && res.pressure_mpa < 300.0, "{}", res.pressure_mpa);
    // the animation hides what is not filled yet
    let half = moldflow::progress_output(o, &res.fill_at_vertex, 0.75);
    let unfilled = half.fields[0].vals.iter().filter(|v| v.is_nan()).count();
    assert!(unfilled > s.pos.len() / 4 && unfilled < s.pos.len() * 3 / 4, "{} of {}", unfilled, s.pos.len());
}

#[test]
fn generative_design_removes_material_and_keeps_the_load_path() {
    let r = plate(80.0, 10.0, 30.0);
    let mut s = Study::new(1, "S".into(), StudyKind::Structural);
    let mut fix = SimItem::new(1, "Fest".into(), ItemKind::Fixed);
    fix.faces = vec![SimFace::of(&r.topo, face(&r, V3::NEG_X))];
    s.items.push(fix);
    let mut f = SimItem::new(2, "Kraft".into(), ItemKind::Force);
    f.faces = vec![SimFace::of(&r.topo, face(&r, V3::X))];
    f.vec = [0.0, -500.0, 0.0];
    s.items.push(f);
    let inp = topopt::OptInput { topo: r.topo.clone(), study: s, volume: 0.4, iterations: 25, elements: 3000, filter: 1.5, symmetry: None, keep_skin: false };
    let res = topopt::run(&inp, &shared()).expect("optimise");
    let pct = res.out.summary.iter().find(|x| x.0 == "Ergebnis").unwrap().1.clone();
    let share: f64 = pct.split('(').nth(1).unwrap().split('%').next().unwrap().trim().parse().unwrap();
    assert!(share > 25.0 && share < 60.0, "kept {}", pct);
    // compliance falls over the iterations once the material is redistributed
    let h = &res.history;
    assert!(h.len() >= 10);
    let (first, last) = (h[1].0, h.last().unwrap().0);
    assert!(last < first, "compliance {} → {}", first, last);
    // STL: header, count, 50 bytes per triangle
    let n = u32::from_le_bytes(res.stl[80..84].try_into().unwrap()) as usize;
    assert_eq!(res.stl.len(), 84 + 50 * n);
    assert!(n > 100);
    assert!(res.out.fields[0].max > 0.0);
}

#[test]
fn stackup_worst_case_rss_and_monte_carlo() {
    let mk = |n: &str, nom: f64, dir: i8| stackup::Contributor { name: n.into(), nominal: nom, upper: 0.1, lower: -0.1, dir, dist: stackup::Dist::Normal };
    let s = stackup::Stackup { name: "K".into(), items: vec![mk("a", 50.0, 1), mk("b", 20.0, -1), mk("c", 29.0, -1)], lsl: Some(0.8), usl: Some(1.2) };
    let r = stackup::analyze(&s, 200_000, 1);
    assert!((r.nominal - 1.0).abs() < 1e-12);
    assert!((r.wc.0 - 0.7).abs() < 1e-9 && (r.wc.1 - 1.3).abs() < 1e-9, "{:?}", r.wc);
    let sigma = (3.0f64).sqrt() * 0.1 / 3.0;
    assert!((r.sigma - sigma).abs() < 1e-9);
    assert!((r.mc_sigma - sigma).abs() < sigma * 0.02, "{} vs {}", r.mc_sigma, sigma);
    let cp = r.cp.unwrap();
    assert!((cp - 0.4 / (6.0 * sigma)).abs() < 1e-9);
    // ±0.2 against σ = 0.0577: z = 3.46 on both sides → about 530 ppm
    let ppm = r.ppm.unwrap();
    assert!(ppm > 400.0 && ppm < 700.0, "{}", ppm);
    assert!((r.share.iter().sum::<f64>() - 1.0).abs() < 1e-9);
    assert_eq!(stackup::iso2768(25.0, 1), 0.2);
}

#[test]
fn worksheet_evaluates_definitions_results_and_templates() {
    let mut vars = freo::model::expr::Vars::new();
    vars.insert("W".into(), 120.0);
    let (lines, defined) = worksheet::evaluate("// Kommentar\nF := 1200 [N]\nA := W*10 = [mm^2]\nsigma := F/A =\nF/A*2 =\nfalsch\n", &vars);
    assert_eq!(lines.len(), 6);
    assert_eq!(lines[2].value, Some(1200.0));
    assert_eq!(lines[2].unit, "mm^2");
    assert_eq!(lines[3].value, Some(1.0));
    assert_eq!(lines[4].value, Some(2.0));
    assert!(lines[5].error.is_some());
    assert_eq!(defined.get("sigma"), Some(&1.0));
    let mut all = vars.clone();
    for k in ["MASSE", "VOLUMEN", "OBERFLAECHE"] {
        all.insert(k.into(), 1.0);
    }
    for (n, t) in worksheet::TEMPLATES {
        let (l, _) = worksheet::evaluate(t, &all);
        let errs: Vec<_> = l.iter().filter_map(|x| x.error.clone()).collect();
        assert!(errs.is_empty(), "{}: {:?}", n, errs);
    }
}

#[test]
fn weld_seam_volume_and_strength() {
    let r = plate(100.0, 40.0, 10.0);
    let t = &r.topo;
    // the long top edge on the +Z side
    let e = (0..t.chains.len()).find(|&c| {
        let ch = &t.chains[c];
        (ch.length() - 100.0).abs() < 1e-6 && ch.points.iter().all(|p| p.y > 9.9 && p.z > 19.9)
    });
    let e = e.expect("edge");
    let seam = weld::WeldSeam { id: 1, name: "N".into(), kind: weld::WeldKind::Fillet, a: 4.0, both_sides: false, intermittent: None, edges: vec![t.edge_ref(e)] };
    let m = weld::MATERIALS[0];
    let c = weld::calc(&seam, t, &m, &weld::PROCESSES[0], 10.0);
    assert!((c.weld_length - 100.0).abs() < 1e-6);
    assert!((c.volume - 4.0 * 4.0 * 1.1 * 100.0).abs() < 1e-6);
    // τ = 10 kN / (4 · 100) = 25 MPa against 360 / (√3 · 0.8 · 1.25) = 207.8 MPa
    let (tau, f, eta) = weld::strength(c.throat_area, 10_000.0, &m);
    assert!((tau - 25.0).abs() < 1e-9 && (f - 207.846).abs() < 0.01 && eta < 0.13);
    let mut im = seam.clone();
    im.intermittent = Some((20.0, 50.0));
    assert!((im.weld_length(t) - 40.0).abs() < 1e-9);
    assert!(!weld::bead_lines(&seam, t).is_empty());
}

#[test]
fn gdt_advisor_proposes_a_datum_frame_and_finds_holes() {
    let r = regen::run(&freo::app::demo::bracket());
    assert!(r.errors.is_empty());
    let items = gdt::advise(&r.topo, 1, &[]);
    let datums: Vec<String> = items.iter().filter_map(|i| if let gdt::Proposal::Datum { letter } = &i.what { Some(letter.clone()) } else { None }).collect();
    assert_eq!(datums, vec!["A", "B", "C"]);
    assert!(items.iter().any(|i| matches!(&i.what, gdt::Proposal::Gtol { ch, .. } if *ch == freo::model::drawing::GChar::Flatness)));
    // the bracket's bolt holes are located with position at MMC
    let pos: Vec<&gdt::Item> = items
        .iter()
        .filter(|i| matches!(&i.what, gdt::Proposal::Gtol { ch, mat, .. } if *ch == freo::model::drawing::GChar::Position && *mat == freo::model::drawing::MatCond::Mmc))
        .collect();
    assert!(!pos.is_empty(), "{:?}", items.iter().map(|i| i.label.clone()).collect::<Vec<_>>());
    // checks: a frame pointing to an undefined datum
    let bad = Annot3 {
        id: 1,
        kind: AnnotKind::Gtol { ch: freo::model::drawing::GChar::Parallelism, diameter: false, value: 0.1, mat: Default::default(), datums: vec!["X".into()] },
        refs: vec![],
        pos: V3::ZERO,
        plane: String::new(),
        group: None,
    };
    let msgs = gdt::check(&[bad]);
    assert!(msgs.iter().any(|m| m.contains("Bezug X")), "{:?}", msgs);
}

#[test]
fn render_studio_draws_the_part() {
    let r = plate(60.0, 40.0, 20.0);
    let mut cam = freo::app::camera::Camera::default();
    cam.target = r.topo.bbox.center();
    cam.scale = 50.0;
    let s = raytrace::Settings {
        width: 48,
        height: 32,
        scene: raytrace::Scene::StudioLight,
        material: raytrace::MATERIALS[0],
        color: [0.8, 0.1, 0.1],
        ground: true,
        ground_mirror: false,
        exposure: 1.0,
        max_samples: 3,
        bounces: 4,
        light_az: 30.0,
        light_el: 50.0,
    };
    let job = raytrace::start(r.topo.clone(), cam, s);
    let t0 = std::time::Instant::now();
    while job.samples.load(std::sync::atomic::Ordering::Relaxed) < 3 {
        std::thread::sleep(std::time::Duration::from_millis(10));
        assert!(t0.elapsed().as_secs() < 60);
    }
    let img = job.image();
    // the centre shows the red part, a corner the light backdrop
    let px = |x: usize, y: usize| {
        let i = (y * 48 + x) * 4;
        (img[i] as i32, img[i + 1] as i32, img[i + 2] as i32)
    };
    let c = px(24, 16);
    assert!(c.0 > c.1 + 30 && c.0 > c.2 + 30, "centre {:?}", c);
    let k = px(1, 1);
    assert!(k.0 > 150 && (k.0 - k.1).abs() < 20, "corner {:?}", k);
}

/// Runs one UI frame of the Anwendungen windows and the ribbon.
fn frame(app: &mut App, ctx: &eframe::egui::Context) {
    let _ = ctx.run(Default::default(), |ctx| {
        eframe::egui::CentralPanel::default().show(ctx, |ui| app.apps_ribbon_ui(ui));
        app.apps_windows(ctx);
    });
}

#[test]
fn every_dialog_opens_and_works_through_the_app() {
    let mut app = App::new(None);
    app.data = freo::app::demo::bracket();
    app.regenerate(false);
    assert!(!app.result.topo.polys.is_empty());
    let ctx = eframe::egui::Context::default();
    // open everything at once and draw a few frames
    app.apps.weld_open = true;
    app.apps.mold_open = true;
    app.mold_analyze();
    app.open_moldbase();
    app.apps.gen_open = true;
    app.apps.conv_open = true;
    app.apps.results_open = true;
    app.apps.fill_open = true;
    app.apps.math_open = true;
    app.apps.gdt_open = true;
    app.gdt_analyze();
    app.apps.tol_open = true;
    app.apps.render_open = true;
    for _ in 0..3 {
        frame(&mut app, &ctx);
    }
    // the draft colours are painted on the model
    assert!(app.creo.tools.overlay.is_some());
    assert!(app.apps.mold_res.is_some());
    // weld seam from a selected edge, undoable
    app.selection = vec![Pick::Edge(0)];
    app.apps.weld_open = true;
    let n0 = app.data.apps.welds.len();
    // (the dialog's button calls the same function)
    app.selection = vec![Pick::Edge(0), Pick::Edge(1)];
    frame(&mut app, &ctx);
    app.data.apps.welds.push(weld::WeldSeam {
        id: 1,
        name: "N".into(),
        kind: weld::WeldKind::Fillet,
        a: 4.0,
        both_sides: true,
        intermittent: None,
        edges: vec![app.result.topo.edge_ref(0)],
    });
    frame(&mut app, &ctx);
    assert_eq!(app.data.apps.welds.len(), n0 + 1);
    let mut lines = vec![];
    app.tab = Tab::Apps;
    app.apps_lines(&mut lines);
    assert!(!lines.is_empty(), "weld beads are drawn");
    // worksheet values become model parameters
    app.data.apps.worksheet = "Breite := W*2 =\n".into();
    app.apps.math_export.insert("Breite".into());
    frame(&mut app, &ctx);
    // GD&T proposals become 3D annotations
    let before = app.data.annots.len();
    assert!(!app.apps.gdt_items.is_empty());
    // the filling simulation runs beside the UI and lands in the Live-Simulation display
    app.apps.fill_res = 0;
    app.apps.fill_open = true;
    let t0 = std::time::Instant::now();
    app.apps.fill_job = None;
    // start through the same path as the button
    app.selection.clear();
    app.apps.fill_gate_sel = false;
    app.apps.fill_time = 1.0;
    // a frame with the job running
    app_fill(&mut app);
    while app.apps.fill_job.is_some() {
        frame(&mut app, &ctx);
        std::thread::sleep(std::time::Duration::from_millis(30));
        assert!(t0.elapsed().as_secs() < 120, "fill did not finish");
    }
    assert!(app.apps.fill_shown.is_some());
    assert_eq!(app.tab, Tab::LiveSim);
    assert!(app.sim_showing());
    assert!(app.sim.saved.iter().any(|s| s.name == "Spritzgussanalyse"));
    // animation frames
    app.apps.fill_play = true;
    for _ in 0..5 {
        frame(&mut app, &ctx);
    }
    assert!(app.sim.out.as_ref().unwrap().fields[0].name.starts_with("Füllfortschritt"));
    let _ = before;
}

fn app_fill(app: &mut App) {
    // the button's code path: fill_start is private to the dialog, so press it through the UI state
    let topo = app.result.topo.clone();
    let inp = moldflow::FillInput { topo, material: mold::MATERIALS[0], gate: None, fill_time: 1.0, cells: 8000, cavities: 1 };
    app.apps.fill_job = Some(freo::app::apps::Job::spawn(move |sh| moldflow::run(&inp, sh)));
}
