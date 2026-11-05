//! C3.5: `cutting.job` makes the document's cut job, in cutting order, without changing it.

use serde_json::{Value, json};
use vectorcraft_engine::Session;

const PT: f64 = 72.0 / 25.4;

/// A square of `side` mm at (`x`, `y`) mm.
fn square(s: &mut Session, x: f64, y: f64, side: f64) -> u64 {
    s.execute("shape.rectangle", &json!({"x": x * PT, "y": y * PT, "width": side * PT, "height": side * PT})).unwrap()["id"].as_u64().unwrap()
}

fn job(s: &mut Session) -> Value {
    s.execute("cutting.job", &json!({})).unwrap()
}

fn sources(j: &Value) -> Vec<u64> {
    j["job"]["paths"].as_array().unwrap().iter().map(|p| p["source"]["node"].as_u64().unwrap()).collect()
}

fn new_doc() -> Session {
    let mut s = Session::new();
    s.execute("file.new", &json!({"width": 200.0 * PT, "height": 200.0 * PT})).unwrap();
    s
}

#[test]
fn the_job_cuts_inner_shapes_first_and_scores_before_cutting() {
    let mut s = new_doc();
    let outer = square(&mut s, 10.0, 10.0, 50.0);
    let inner = square(&mut s, 25.0, 25.0, 20.0);
    let fold = square(&mut s, 100.0, 10.0, 30.0);
    s.execute("cutting.op.set", &json!({"ids": [fold], "op": "score"})).unwrap();
    let before = s.active().unwrap().doc.clone();
    let j = job(&mut s);
    assert!(std::sync::Arc::ptr_eq(&before, &s.active().unwrap().doc), "the document isn't changed");
    assert_eq!(sources(&j), [fold, inner, outer]);
    let ops = &j["job"]["operations"];
    assert_eq!((ops[0]["kind"].clone(), ops[1]["kind"].clone()), (json!("score"), json!("cut")));
    assert_eq!(j["job"]["paths"][0]["op"], 0);
    // In millimetres on the mat: the inner square's corners at 25 and 45 mm.
    let pts = j["job"]["paths"][1]["points"].as_array().unwrap();
    let xs: Vec<f64> = pts.iter().map(|p| p[0].as_f64().unwrap()).collect();
    assert!(xs.iter().all(|x| (x - 25.0).abs() < 1e-6 || (x - 45.0).abs() < 1e-6), "{xs:?}");
    assert_eq!(j["stats"]["pathCount"], 3);
    assert!((j["stats"]["pathMm"].as_f64().unwrap() - (200.0 + 80.0 + 120.0)).abs() < 1e-6);
}

#[test]
fn art_off_the_mat_is_an_error_unless_clipped_to_it() {
    let mut s = new_doc();
    square(&mut s, 10.0, 10.0, 10.0);
    s.execute("cutting.settings.set", &json!({"offsetMm": [290, 0]})).unwrap();
    let e = s.execute("cutting.job", &json!({})).unwrap_err().to_string();
    assert!(e.contains("cut area"), "{e}");
    s.execute("cutting.settings.set", &json!({"clipToMat": true})).unwrap();
    let j = job(&mut s);
    assert!(j["job"]["warnings"].as_array().unwrap().iter().any(|w| w["message"].as_str().unwrap().contains("edge")));
    assert!(j["stats"]["pathCount"].as_u64().unwrap() >= 1);
    assert!(s.execute("cutting.settings.set", &json!({"clipToMat": "yes"})).is_err());
}

/// C3.7: the blade's run through the job, from the cut area's origin.
#[test]
fn the_simulation_follows_the_job() {
    let mut s = new_doc();
    let a = square(&mut s, 10.0, 10.0, 10.0);
    square(&mut s, 50.0, 10.0, 10.0);
    let start = s.execute("cutting.simulate", &json!({"t": 0})).unwrap();
    assert_eq!((start["position"].clone(), start["down"].clone()), (json!([0.0, 0.0]), json!(false)));
    let total = start["totalMm"].as_f64().unwrap();
    let stats = job(&mut s)["stats"].clone();
    assert!((total - stats["bladeMm"].as_f64().unwrap() - stats["travelMm"].as_f64().unwrap()).abs() < 1e-6);
    let starts = start["pathStartsMm"].as_array().unwrap().clone();
    assert_eq!(starts.len(), 2);
    // 1 mm after the blade comes down on the first square: down, on it, cutting.
    let on = s.execute("cutting.simulate", &json!({"mm": starts[0].as_f64().unwrap() + 1.0})).unwrap();
    assert_eq!((on["down"].clone(), on["path"].clone(), on["op"].clone()), (json!(true), json!(0), json!("cut")));
    assert_eq!(job(&mut s)["job"]["paths"][0]["source"]["node"], json!(a));
    let end = s.execute("cutting.simulate", &json!({})).unwrap();
    assert_eq!(end["doneMm"], end["totalMm"]);
    assert!(end["totalSeconds"].as_f64().unwrap() > 0.0);
    assert!(s.execute("cutting.simulate", &json!({"t": "half"})).is_err());
}

#[test]
fn an_empty_document_makes_an_empty_job() {
    let mut s = new_doc();
    let j = job(&mut s);
    assert_eq!(j["job"]["paths"], json!([]));
    assert_eq!(j["stats"]["pathCount"], 0);
}

/// C3.8: the preview as an SVG opens at the mat's size; as JSON it is cutting.job.
#[test]
fn the_preview_exports_as_svg_and_json() {
    let mut s = new_doc();
    square(&mut s, 10.0, 10.0, 10.0);
    let svg = s.execute("cutting.preview.export", &json!({})).unwrap();
    assert_eq!(svg["name"], json!("Untitled-1-cut.svg"));
    let text = svg["text"].as_str().unwrap().to_string();
    assert!(text.contains(r#"data-order="1" data-op="cut""#));
    s.execute("document.open", &json!({"name": "preview.svg", "dataBase64": vectorcraft_format::base64_encode(text.as_bytes())})).unwrap();
    let ab = s.active().unwrap().doc.artboards[0].rect;
    assert!((ab.width() / PT - 304.8).abs() < 1e-3 && (ab.height() / PT - 304.8).abs() < 1e-3, "{ab:?}");
    s.execute("document.activate", &json!({"index": 0})).unwrap();
    let as_json = s.execute("cutting.preview.export", &json!({"format": "json"})).unwrap();
    let v: Value = serde_json::from_str(as_json["text"].as_str().unwrap()).unwrap();
    // The export is the job and its numbers; cutting.job also answers with the quote (C18.3).
    let mut want = job(&mut s);
    want.as_object_mut().unwrap().remove("quote");
    assert_eq!(v, want);
    assert!(s.execute("cutting.preview.export", &json!({"format": "pdf"})).is_err());
}

/// C3.10: two overlapping squares set to Cut Outline are cut as one outline.
#[test]
fn cut_outline_unites_overlapping_shapes() {
    let mut s = new_doc();
    let a = square(&mut s, 10.0, 10.0, 40.0);
    let b = square(&mut s, 30.0, 30.0, 40.0);
    assert_eq!(job(&mut s)["stats"]["pathCount"], 2);
    s.execute("cutting.op.set", &json!({"ids": [a, b], "op": "outline"})).unwrap();
    assert_eq!(s.execute("cutting.op.get", &json!({"ids": [a]})).unwrap()["objects"][0]["op"], json!("outline"));
    let j = job(&mut s);
    assert_eq!(j["stats"]["pathCount"], 1);
    // Round both: 40 + 20 + 20 + 40 on each of two sides, less nothing inside.
    assert!((j["stats"]["pathMm"].as_f64().unwrap() - 240.0).abs() < 0.1, "{}", j["stats"]["pathMm"]);
    assert_eq!(j["job"]["operations"][0]["kind"], json!("cut"));
}

/// C5.2: blade depth, acceleration, corner overcut and track enhancing through the settings.
#[test]
fn machine_settings_reach_the_job() {
    let mut s = new_doc();
    square(&mut s, 10.0, 10.0, 20.0);
    s.execute("cutting.settings.set", &json!({"trackEnhancing": true, "operations": {"cut": {"depth": 3, "accel": 2, "cornerOvercutMm": 0.5}}}))
        .unwrap();
    let j = job(&mut s);
    assert_eq!(j["job"]["trackEnhancing"], json!(true));
    let op = &j["job"]["operations"][0];
    assert_eq!((op["depth"].clone(), op["accel"].clone(), op["cornerOvercutMm"].clone()), (json!(3), json!(2), json!(0.5)));
    // The square's three corners after its start each run past and back: 4 + 3 × 2 points.
    assert_eq!(j["job"]["paths"][0]["points"].as_array().unwrap().len(), 10);
    s.execute("cutting.settings.set", &json!({"operations": {"cut": {"depth": null}}})).unwrap();
    assert_eq!(job(&mut s)["job"]["operations"][0]["depth"], Value::Null);
    // C5.4: the blade offset, null for the usual blade's.
    assert_eq!(job(&mut s)["job"]["operations"][0]["bladeOffsetMm"], Value::Null);
    s.execute("cutting.settings.set", &json!({"operations": {"cut": {"bladeOffsetMm": 0.25}}})).unwrap();
    assert_eq!(job(&mut s)["job"]["operations"][0]["bladeOffsetMm"], json!(0.25));
    for bad in [
        json!({"operations": {"cut": {"depth": 12}}}),
        json!({"operations": {"cut": {"accel": 0}}}),
        json!({"trackEnhancing": "yes"}),
        json!({"operations": {"cut": {"bladeOffsetMm": 3}}}),
    ] {
        assert!(s.execute("cutting.settings.set", &bad).is_err(), "{bad}");
    }
}

/// C14.6: fewest roller moves: two columns of squares, cut row by row down the mat rather than
/// down one column and back up the other; an unknown order refused.
#[test]
fn rollers_cut_down_the_mat_once() {
    let mut s = new_doc();
    let mut rows = vec![];
    for k in 0..4 {
        let y = 10.0 + k as f64 * 45.0;
        rows.push([square(&mut s, 10.0, y, 10.0), square(&mut s, 150.0, y, 10.0)]);
    }
    assert!(s.execute("cutting.settings.set", &json!({"ordering": "sideways"})).is_err());
    s.execute("cutting.settings.set", &json!({"ordering": "rollers"})).unwrap();
    let order = sources(&job(&mut s));
    let row_of = |id: u64| rows.iter().position(|r| r.contains(&id)).unwrap();
    let seq: Vec<usize> = order.iter().map(|id| row_of(*id)).collect();
    assert!(seq.windows(2).all(|w| w[0] <= w[1]), "row by row: {seq:?}");
}

/// C18.2: the checks before cutting reach the job's warnings, naming the object.
#[test]
fn the_checks_name_what_spoils_a_cut() {
    let mut s = Session::new();
    s.execute("file.new", &json!({"width": 600, "height": 600})).unwrap();
    let clean = s.execute("cutting.job", &json!({})).unwrap()["job"]["warnings"].as_array().map_or(0, Vec::len);
    let tiny = s.execute("shape.rectangle", &json!({"x": 100, "y": 100, "width": 4, "height": 4})).unwrap()["id"].clone();
    let j = s.execute("cutting.job", &json!({})).unwrap();
    let w = j["job"]["warnings"].as_array().unwrap();
    assert_eq!(w.len(), clean + 1, "{w:?}");
    assert_eq!(w.last().unwrap()["source"]["node"], tiny);
    assert!(w.last().unwrap()["message"].as_str().unwrap().contains("smaller than 2 mm"));
}

/// C18.9: the weeding map is an SVG of the cut sheet, filled even-odd.
#[test]
fn the_weeding_map_is_an_svg() {
    let mut s = new_doc();
    square(&mut s, 10.0, 10.0, 10.0);
    let r = s.execute("cutting.weedingMap", &json!({})).unwrap();
    assert!(r["data"].as_str().unwrap().contains("fill-rule=\"evenodd\""));
    assert!(r["name"].as_str().unwrap().ends_with("-weeding.svg"));
}

/// C4.14: a design's curves go in the job beside its points, in step with them whatever order and
/// direction the paths are cut in: each curve starts at its path's first point, an open one ends
/// at its last, a closed one comes back to its start; curved pieces stay cubics. Off with the
/// setting; none for a perforated path.
#[test]
fn curves_go_in_the_job_in_step_with_the_points() {
    use vectorcraft_cutjob::{Native, Piece};
    let mut s = Session::new();
    s.execute("file.new", &json!({"width": 600, "height": 600})).unwrap();
    let first = s.execute("shape.ellipse", &json!({"x": 300, "y": 40, "width": 80, "height": 50})).unwrap()["id"].as_u64().unwrap();
    s.execute("shape.ellipse", &json!({"x": 40, "y": 300, "width": 60, "height": 60})).unwrap();
    s.execute("shape.star", &json!({"cx": 200, "cy": 200, "radius1": 50, "radius2": 20, "points": 5})).unwrap();
    s.execute("shape.spiral", &json!({"cx": 450, "cy": 400, "radius": 60})).unwrap();
    square(&mut s, 150.0, 150.0, 20.0);
    let doc = s.active().unwrap().doc.clone();
    let job = vectorcraft_cutting::build_job(&doc).unwrap();
    assert_eq!(job.curves.len(), job.paths.len());
    let near = |a: [f64; 2], b: [f64; 2]| (a[0] - b[0]).hypot(a[1] - b[1]) < 1e-6;
    let mut cubics = 0;
    for (p, c) in job.paths.iter().zip(&job.curves) {
        let Some(Native::Path { start, pieces }) = c else { panic!("every path here has its curve: {p:?}") };
        assert!(near(*start, p.points[0]), "starts where the points do: {start:?} {:?}", p.points[0]);
        let end = pieces.last().unwrap().end();
        if p.closed {
            assert!(near(end, *start), "a closed curve comes back to its start");
        } else {
            assert!(near(end, *p.points.last().unwrap()), "an open curve ends where the points do");
        }
        cubics += pieces.iter().filter(|q| matches!(q, Piece::Cubic(..))).count();
    }
    assert!(cubics >= 8, "the ellipses and the spiral keep their curves: {cubics}");
    // The setting off: no curves.
    s.execute("cutting.settings.set", &json!({"sendCurves": false})).unwrap();
    assert!(vectorcraft_cutting::build_job(&s.active().unwrap().doc).unwrap().curves.is_empty());
    s.execute("cutting.settings.set", &json!({"sendCurves": true})).unwrap();
    // A perforated path is cut in pieces: none of them has a curve.
    s.execute("select.set", &json!({"ids": [first]})).unwrap();
    s.execute("cutting.op.set", &json!({"op": "perforate"})).unwrap();
    let job = vectorcraft_cutting::build_job(&s.active().unwrap().doc).unwrap();
    let perforated: Vec<_> =
        job.paths.iter().zip(job.curves.iter().chain(std::iter::repeat(&None))).filter(|(p, _)| p.source.node == first).collect();
    assert!(perforated.len() > 1 && perforated.iter().all(|(_, c)| c.is_none()), "{}", perforated.len());
}

/// C4.15: a perforated line whose cut share is near one of the machine's own (the settings'
/// 3 mm and 1 mm: 25/32 of 4 mm is 3.125) is kept as a whole line beside its dashes, made to
/// the machine's share, with its curve; a share the machine hasn't, or Send Curves off, only
/// dashes, of the lengths set.
#[test]
fn a_perforated_line_the_machine_can_dash_is_kept_whole() {
    let mut s = Session::new();
    s.execute("file.new", &json!({"width": 600, "height": 600})).unwrap();
    let ring = s.execute("shape.ellipse", &json!({"x": 100, "y": 100, "width": 100, "height": 100})).unwrap()["id"].as_u64().unwrap();
    square(&mut s, 300.0, 300.0, 20.0);
    s.execute("cutting.op.set", &json!({"ids": [ring], "op": "perforate"})).unwrap();
    let job = vectorcraft_cutting::build_job(&s.active().unwrap().doc).unwrap();
    assert_eq!(job.dashes.len(), 1, "{:?}", job.dashes);
    let d = &job.dashes[0];
    assert!((d.pitch_mm - 4.0).abs() < 1e-9 && (d.share - 25.0 / 32.0).abs() < 1e-9, "{d:?}");
    assert!(d.closed && d.curve.is_some());
    let dashes = &job.paths[d.start..d.start + d.count];
    assert!(dashes.len() > 10 && dashes.iter().all(|p| p.source.node == ring && !p.closed && p.op == dashes[0].op));
    assert!(
        job.paths.iter().enumerate().all(|(i, p)| (p.source.node == ring) == (d.start..d.start + d.count).contains(&i)),
        "the ring's dashes together"
    );
    let length = |p: &vectorcraft_cutjob::Toolpath| p.points.windows(2).map(|w| (w[1][0] - w[0][0]).hypot(w[1][1] - w[0][1])).sum::<f64>();
    let first = length(&dashes[0]);
    assert!((first - 3.125).abs() < 0.05, "the machine's share of the pitch: {first}");
    assert!(dashes[0].points[0] == d.points[0], "the dashes start where the line does");
    // A share the machine hasn't: dashes only, as set.
    s.execute("cutting.settings.set", &json!({"perforateCutMm": 2.0, "perforateGapMm": 1.0})).unwrap();
    let job = vectorcraft_cutting::build_job(&s.active().unwrap().doc).unwrap();
    assert!(job.dashes.is_empty());
    let first = job.paths.iter().find(|p| p.source.node == ring).map(length).unwrap();
    assert!((first - 2.0).abs() < 0.05, "{first}");
    // Send Curves off: the same, at the lengths set.
    s.execute("cutting.settings.set", &json!({"perforateCutMm": 3.0, "sendCurves": false})).unwrap();
    let job = vectorcraft_cutting::build_job(&s.active().unwrap().doc).unwrap();
    assert!(job.dashes.is_empty());
    let first = job.paths.iter().find(|p| p.source.node == ring).map(length).unwrap();
    assert!((first - 3.0).abs() < 0.05, "{first}");
}

/// C4.10: a job made from a design, its curves and a ring the machine dashes (C4.15) included,
/// reads back from the Silhouette bytes as its own paths: as many runs, each starting where its
/// path does (taking the machine's dashes to start with a cut at the line's start).
#[test]
fn a_designs_job_reads_back_from_the_bytes() {
    use vectorcraft_cutter::Driver;
    use vectorcraft_cutter_silhouette::decode::{blade_paths, decode};
    let mut s = Session::new();
    s.execute("file.new", &json!({"width": 600, "height": 600})).unwrap();
    let ring = s.execute("shape.ellipse", &json!({"x": 100, "y": 100, "width": 100, "height": 100})).unwrap()["id"].as_u64().unwrap();
    s.execute("shape.ellipse", &json!({"x": 300, "y": 40, "width": 80, "height": 50})).unwrap();
    s.execute("shape.star", &json!({"cx": 200, "cy": 400, "radius1": 50, "radius2": 20, "points": 5})).unwrap();
    square(&mut s, 400.0, 400.0, 20.0);
    s.execute("cutting.op.set", &json!({"ids": [ring], "op": "perforate"})).unwrap();
    let job = vectorcraft_cutting::build_job(&s.active().unwrap().doc).unwrap();
    assert!(!job.dashes.is_empty() && !job.curves.is_empty());
    let cameo = vectorcraft_cutter::DeviceInfo { id: "usb".into(), vendor_id: 0x0B4D, product_id: 0x1137, name: "Cameo".into() };
    let bytes = vectorcraft_cutter_silhouette::Silhouette.encode(&job, &cameo).unwrap().chunks.concat();
    let runs = blade_paths(&decode(&bytes).unwrap()).unwrap();
    assert_eq!(runs.len(), job.paths.len(), "one run a path, the ring's dashes made by the machine");
    for (i, (run, p)) in runs.iter().zip(&job.paths).enumerate() {
        // A dash measured along the machine's curve drifts a little from one along the points.
        let dash = job.dashes.iter().any(|d| (d.start..d.start + d.count).contains(&i));
        let reach = if dash { 0.3 } else { 0.03 };
        let (a, b) = (run[0], p.points[0]);
        assert!((a[0] - b[0]).abs() < reach && (a[1] - b[1]).abs() < reach, "path {i}: {a:?} {b:?}");
    }
}

/// C4.12: the calibration sheet comes in as one group centred on the artboard, one undo step;
/// its job cuts the eight shapes, the 100 mm lines 100 mm on the mat; with Print & Cut, four
/// rings more and the crosses left out.
#[test]
fn the_calibration_sheet_cuts_what_it_measures() {
    let mut s = Session::new();
    s.execute("file.new", &json!({"width": 600, "height": 600})).unwrap();
    let r = s.execute("cutting.calibration", &json!({"printAndCut": false})).unwrap();
    assert_eq!((r["parts"].clone(), r["printAndCut"].clone()), (json!(8), json!(false)));
    let job = vectorcraft_cutting::build_job(&s.active().unwrap().doc).unwrap();
    assert_eq!(job.paths.len(), 8, "{:?}", job.paths.len());
    let length = |p: &vectorcraft_cutjob::Toolpath| p.points.windows(2).map(|w| (w[1][0] - w[0][0]).hypot(w[1][1] - w[0][1])).sum::<f64>();
    let open: Vec<f64> = job.paths.iter().filter(|p| !p.closed).map(length).collect();
    assert_eq!(open.len(), 2);
    assert!(open.iter().all(|l| (l - 100.0).abs() < 1e-6), "{open:?}");
    s.execute("edit.undo", &json!({})).unwrap();
    assert!(vectorcraft_cutting::build_job(&s.active().unwrap().doc).map_or(true, |j| j.paths.is_empty()));
    let r = s.execute("cutting.calibration", &json!({"printAndCut": true})).unwrap();
    assert_eq!(r["parts"], json!(16));
    let job = vectorcraft_cutting::build_job(&s.active().unwrap().doc).unwrap();
    assert_eq!(job.paths.len(), 12, "the rings cut, the crosses not");
    assert!(s.execute("cutting.calibration", &json!({"printAndCut": 3})).is_err());
}

/// C18.4: a name tag (a rectangle and a name bound to a text variable) and three data sets make
/// three groups named after them, each with its record's name, nested on the mat; the template
/// hidden and not cut; one undo step. A record too long for the mat is refused by name, with
/// nothing changed.
#[test]
fn a_batch_makes_a_piece_for_each_record() {
    let mut s = Session::new();
    s.execute("file.new", &json!({"width": 600, "height": 600})).unwrap();
    let tag = s.execute("shape.rectangle", &json!({"x": 20, "y": 20, "width": 150, "height": 60})).unwrap()["id"].as_u64().unwrap();
    let name = s.execute("text.create", &json!({"x": 30, "y": 60, "text": "Name", "size": 24})).unwrap()["id"].as_u64().unwrap();
    s.execute("variable.define", &json!({"name": "who", "kind": "text"})).unwrap();
    s.execute("variable.bind", &json!({"variable": "who", "ids": [name]})).unwrap();
    for who in ["Anna", "Bo", "Christopher"] {
        s.execute("dataset.new", &json!({"name": who, "values": {"who": who}})).unwrap();
    }
    s.execute("select.set", &json!({"ids": [tag, name]})).unwrap();
    let before = vectorcraft_cutting::build_job(&s.active().unwrap().doc).unwrap().paths.len();
    let r = s.execute("cutting.batch", &json!({})).unwrap();
    let records = r["records"].as_array().unwrap();
    assert_eq!(records.iter().map(|x| x["name"].as_str().unwrap()).collect::<Vec<_>>(), ["Anna", "Bo", "Christopher"]);
    let doc = s.active().unwrap().doc.clone();
    for x in records {
        let g = doc.node(vectorcraft_doc::NodeId(x["id"].as_u64().unwrap())).unwrap();
        assert_eq!(g.name.as_deref(), x["name"].as_str());
        let texts: Vec<String> = g
            .children()
            .unwrap()
            .iter()
            .filter_map(|c| match &c.kind {
                vectorcraft_doc::NodeKind::Text(t) => Some(t.plain_text()),
                _ => None,
            })
            .collect();
        assert_eq!(texts, [x["name"].as_str().unwrap()]);
    }
    assert!(!doc.node(vectorcraft_doc::NodeId(tag)).unwrap().visible && !doc.node(vectorcraft_doc::NodeId(name)).unwrap().visible);
    let job = vectorcraft_cutting::build_job(&doc).unwrap();
    assert!(job.paths.len() > before * 2, "three tags cut, the template not: {} {before}", job.paths.len());
    s.execute("edit.undo", &json!({})).unwrap();
    assert!(s.active().unwrap().doc.node(vectorcraft_doc::NodeId(name)).unwrap().visible, "one undo step");
    // A name that makes the tag wider than the mat: refused by name, nothing added.
    s.execute("dataset.new", &json!({"name": "Long", "values": {"who": "W".repeat(120)}})).unwrap();
    s.execute("select.set", &json!({"ids": [tag, name]})).unwrap();
    let e = s.execute("cutting.batch", &json!({"datasets": ["Bo", "Long"]})).unwrap_err().to_string();
    assert!(e.contains("\"Long\""), "{e}");
    assert!(s.active().unwrap().doc.node(vectorcraft_doc::NodeId(name)).unwrap().visible);
    // Nothing bound in the design: refused.
    s.execute("select.set", &json!({"ids": [tag]})).unwrap();
    assert!(s.execute("cutting.batch", &json!({})).is_err());
}

proptest::proptest! {
    #![proptest_config(proptest::prelude::ProptestConfig { cases: 24, ..proptest::prelude::ProptestConfig::default() })]

    /// C18.4: random records (names of letters, digits and punctuation, empty, long) make a batch or a refusal,
    /// never a crash; a batch has one piece a record, with its text.
    #[test]
    fn random_data_sets_batch_or_refuse(values in proptest::collection::vec("[A-Za-z0-9 ÅÄÖåäöéüß.,'&-]{0,40}", 1..6)) {
        let mut s = Session::new();
        s.execute("file.new", &json!({"width": 600, "height": 600})).unwrap();
        let name = s.execute("text.create", &json!({"x": 30, "y": 60, "text": "Name", "size": 18})).unwrap()["id"].as_u64().unwrap();
        s.execute("variable.define", &json!({"name": "who", "kind": "text"})).unwrap();
        s.execute("variable.bind", &json!({"variable": "who", "ids": [name]})).unwrap();
        let mut made = 0;
        for (i, v) in values.iter().enumerate() {
            if s.execute("dataset.new", &json!({"name": format!("r{i}"), "values": {"who": v}})).is_ok() {
                made += 1;
            }
        }
        s.execute("select.set", &json!({"ids": [name]})).unwrap();
        match s.execute("cutting.batch", &json!({})) {
            Ok(r) => proptest::prop_assert_eq!(r["records"].as_array().unwrap().len(), made),
            Err(e) => proptest::prop_assert!(!e.to_string().is_empty()),
        }
    }
}
