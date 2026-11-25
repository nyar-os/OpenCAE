//! Layout tests for the exported drawings.
//!
//! These tests read the generated SVG back and check the properties a reader
//! sees, not the code that wrote it: every glyph and every drawn line stays
//! inside the page frame, no label overprints a member or another label, the
//! title block never spills past its cells, the model uses at least 60 % of the
//! drawing area, and the new shear/moment sheets reproduce the analytic beam
//! solutions.

use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use cae_model::{
    DrawingKind, ElementKind, ExportFormat, ExportOptions, Model, OpenCae, OpenCaeOptions, Units, solve_internal_forces,
};

/// Page frame shared by every sheet.
const FRAME: (f64, f64, f64, f64) = (12.0, 12.0, 948.0, 708.0);
/// Geometry viewport of the plain sheets.
const VIEW: (f64, f64, f64, f64) = (96.0, 78.0, 864.0, 584.0);
/// Cell dividers of the title block.
const TITLE_SPLITS: [f64; 4] = [470.0, 620.0, 726.0, 844.0];
const TITLE_TOP: f64 = 622.0;
const TITLE_NOTE_DIVIDER: f64 = 667.0;

fn temporary_dir(name: &str) -> PathBuf {
    let unique = SystemTime::now().duration_since(UNIX_EPOCH).expect("system clock").as_nanos();
    let directory = std::env::temp_dir().join(format!("ocae-layout-{name}-{}-{unique}", std::process::id()));
    fs::create_dir_all(&directory).expect("create temporary directory");
    directory
}

fn save_session(model: &Model, directory: &Path, name: &str) -> OpenCae {
    let path = directory.join(format!("{name}.ocae"));
    model.save_atomic(&path).expect("save document");
    let mut session = OpenCae::new(OpenCaeOptions::default());
    session.open(&path).expect("open document");
    session
}

fn demo_truss() -> Model {
    let mut model = Model::new(Units::Millimeter);
    model.add_node("n1", -500.0, 0.0).expect("n1");
    model.add_node("n2", 500.0, 0.0).expect("n2");
    model.add_node("n3", 0.0, 500.0).expect("n3");
    model.add_element("e1", "n1", "n3", None).expect("e1");
    model.add_element("e2", "n2", "n3", None).expect("e2");
    model.set_material_full(Some("steel".to_string()), 210_000.0, 100.0, 7.85e-9);
    model.add_support_full("n1", true, true, false, false).expect("pin");
    model.add_support_full("n2", true, true, false, false).expect("pin");
    model.add_load("n3", None, Some(-1000.0)).expect("apex load");
    model.add_combination("D", 1.0, 0.0).expect("dead combination");
    model.add_combination("D+L", 1.5, 0.0).expect("factored combination");
    model
}

/// Simply supported 1000 mm beam under `q = -2 N/mm`.
fn uniformly_loaded_beam() -> Model {
    let mut model = Model::new(Units::Millimeter);
    model.add_node("n1", 0.0, 0.0).expect("n1");
    model.add_node("n2", 1000.0, 0.0).expect("n2");
    model.add_element_kind("b1", "n1", "n2", None, ElementKind::Beam, Some(1.0e6), Some(-2.0)).expect("b1");
    model.add_support_full("n1", true, true, false, false).expect("pin");
    model.add_support_full("n2", false, true, false, false).expect("roller");
    model
}

/// Two 500 mm beam halves with a 1000 N centre load.
fn centre_loaded_beam() -> Model {
    let mut model = Model::new(Units::Millimeter);
    model.add_node("n1", 0.0, 0.0).expect("n1");
    model.add_node("n2", 500.0, 0.0).expect("n2");
    model.add_node("n3", 1000.0, 0.0).expect("n3");
    model.add_element_kind("b1", "n1", "n2", None, ElementKind::Beam, Some(1.0e6), None).expect("b1");
    model.add_element_kind("b2", "n2", "n3", None, ElementKind::Beam, Some(1.0e6), None).expect("b2");
    model.add_support_full("n1", true, true, false, false).expect("pin");
    model.add_support_full("n3", false, true, false, false).expect("roller");
    model.add_load("n2", None, Some(-1000.0)).expect("centre load");
    model
}

fn export(session: &OpenCae, directory: &Path, name: &str, kind: &str) -> String {
    let target = directory.join(name);
    let options =
        ExportOptions { drawing: Some(kind.parse::<DrawingKind>().expect("drawing kind")), ..ExportOptions::default() };
    session.export_with_options(ExportFormat::Svg, &target, options).expect("sheet export");
    fs::read_to_string(&target).expect("read exported sheet")
}

fn export_plain(session: &OpenCae, directory: &Path, name: &str) -> String {
    let target = directory.join(name);
    session.export(ExportFormat::Svg, &target).expect("plain svg export");
    fs::read_to_string(&target).expect("read plain sheet")
}

fn attribute<'a>(line: &'a str, key: &str) -> Option<&'a str> {
    let needle = format!("{key}=\"");
    let start = line.find(&needle)? + needle.len();
    let rest = &line[start..];
    let end = rest.find('"')?;
    Some(&rest[..end])
}

fn number(line: &str, key: &str) -> Option<f64> {
    attribute(line, key)?.parse().ok()
}

#[derive(Debug, Clone)]
struct TextBox {
    left: f64,
    top: f64,
    right: f64,
    bottom: f64,
    line: usize,
    content: String,
}

impl TextBox {
    fn overlaps(&self, other: &Self) -> bool {
        self.left < other.right && other.left < self.right && self.top < other.bottom && other.top < self.bottom
    }
}

fn text_boxes(svg: &str) -> Vec<TextBox> {
    svg.lines()
        .enumerate()
        .filter_map(|(index, line)| {
            let trimmed = line.trim_start();
            if !trimmed.starts_with("<text ") {
                return None;
            }
            let x = number(trimmed, "x")?;
            let y = number(trimmed, "y")?;
            let size = number(trimmed, "font-size").unwrap_or(11.0);
            let content = trimmed.split_once('>')?.1.split_once("</text>")?.0.to_string();
            if content.is_empty() {
                return None;
            }
            let width = content.chars().count() as f64 * size * 0.56;
            let left = match attribute(trimmed, "text-anchor") {
                Some("end") => x - width,
                Some("middle") => x - width / 2.0,
                _ => x,
            };
            Some(TextBox { left, top: y - size * 0.88, right: left + width, bottom: y + size * 0.25, line: index + 1, content })
        })
        .collect()
}

#[derive(Debug, Clone, Copy)]
struct Segment {
    from: (f64, f64),
    to: (f64, f64),
}

fn line_is_leader(line: &str) -> bool {
    attribute(line, "stroke") == Some("#9ca3af") && attribute(line, "stroke-width") == Some("0.9")
}

fn segments(svg: &str) -> Vec<Segment> {
    let mut result = Vec::new();
    for line in svg.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("<line ") {
            if attribute(trimmed, "data-legend").is_some() || line_is_leader(trimmed) {
                continue;
            }
            if let (Some(x1), Some(y1), Some(x2), Some(y2)) =
                (number(trimmed, "x1"), number(trimmed, "y1"), number(trimmed, "x2"), number(trimmed, "y2"))
            {
                result.push(Segment { from: (x1, y1), to: (x2, y2) });
            }
            continue;
        }
        let is_polyline = trimmed.starts_with("<polyline ");
        let is_polygon = trimmed.starts_with("<polygon ");
        if !(is_polyline || is_polygon) || attribute(trimmed, "data-legend").is_some() {
            continue;
        }
        // A fill with `stroke="none"` is a shaded region, not an outline: text
        // may sit on the pale fill, only the curve and the axes are lines.
        if is_polygon && attribute(trimmed, "stroke") == Some("none") {
            continue;
        }
        let Some(points) = attribute(trimmed, "points")
        else {
            continue;
        };
        let parsed: Vec<(f64, f64)> = points
            .split_whitespace()
            .filter_map(|pair| {
                let (x, y) = pair.split_once(',')?;
                Some((x.parse().ok()?, y.parse().ok()?))
            })
            .collect();
        for window in parsed.windows(2) {
            result.push(Segment { from: window[0], to: window[1] });
        }
        if is_polygon {
            if let (Some(first), Some(last)) = (parsed.first(), parsed.last()) {
                result.push(Segment { from: *last, to: *first });
            }
        }
    }
    result
}

fn point_in_rect(point: (f64, f64), rect: (f64, f64, f64, f64)) -> bool {
    point.0 >= rect.0 && point.0 <= rect.2 && point.1 >= rect.1 && point.1 <= rect.3
}

fn orientation(a: (f64, f64), b: (f64, f64), c: (f64, f64)) -> f64 {
    (b.0 - a.0) * (c.1 - a.1) - (b.1 - a.1) * (c.0 - a.0)
}

fn crosses(a: (f64, f64), b: (f64, f64), c: (f64, f64), d: (f64, f64)) -> bool {
    let o1 = orientation(a, b, c);
    let o2 = orientation(a, b, d);
    let o3 = orientation(c, d, a);
    let o4 = orientation(c, d, b);
    ((o1 > 0.0 && o2 < 0.0) || (o1 < 0.0 && o2 > 0.0)) && ((o3 > 0.0 && o4 < 0.0) || (o3 < 0.0 && o4 > 0.0))
}

fn segment_hits_box(segment: Segment, text: &TextBox) -> bool {
    let rect = (text.left - 0.5, text.top - 0.5, text.right + 0.5, text.bottom + 0.5);
    if point_in_rect(segment.from, rect) || point_in_rect(segment.to, rect) {
        return true;
    }
    let corners = [(rect.0, rect.1), (rect.2, rect.1), (rect.2, rect.3), (rect.0, rect.3)];
    (0..4).any(|index| crosses(segment.from, segment.to, corners[index], corners[(index + 1) % 4]))
}

fn assert_inside_frame(svg: &str, sheet: &str) {
    for text in text_boxes(svg) {
        assert!(
            text.left >= FRAME.0 - 0.6
                && text.right <= FRAME.2 + 0.6
                && text.top >= FRAME.1 - 0.6
                && text.bottom <= FRAME.3 + 0.6,
            "{sheet}: text `{}` at line {} leaves the frame: {:?}",
            text.content,
            text.line,
            text
        );
    }
    for segment in segments(svg) {
        for point in [segment.from, segment.to] {
            assert!(
                point.0 >= FRAME.0 - 0.6 && point.0 <= FRAME.2 + 0.6 && point.1 >= FRAME.1 - 0.6 && point.1 <= FRAME.3 + 0.6,
                "{sheet}: geometry point {point:?} leaves the frame"
            );
        }
    }
}

fn assert_no_label_crosses_geometry(svg: &str, sheet: &str) {
    let lines = segments(svg);
    for text in text_boxes(svg).into_iter().filter(|text| text.bottom < 600.0) {
        for segment in &lines {
            assert!(
                !segment_hits_box(*segment, &text),
                "{sheet}: label `{}` at line {} crosses {:?}",
                text.content,
                text.line,
                segment
            );
        }
    }
}

fn assert_labels_do_not_overlap(svg: &str, sheet: &str) {
    let labels: Vec<TextBox> = text_boxes(svg).into_iter().filter(|text| text.bottom < 600.0).collect();
    for (index, first) in labels.iter().enumerate() {
        for second in &labels[index + 1..] {
            assert!(
                !first.overlaps(second),
                "{sheet}: labels `{}` (line {}) and `{}` (line {}) overlap: {:?} vs {:?}",
                first.content,
                first.line,
                second.content,
                second.line,
                first,
                second
            );
        }
    }
    let ids: Vec<&TextBox> = labels.iter().filter(|text| text.content.starts_with('n')).collect();
    let unique: HashSet<String> = ids.iter().map(|text| text.content.clone()).collect();
    assert_eq!(ids.len(), unique.len(), "{sheet}: duplicate node labels");
}

fn assert_title_block_fits(svg: &str, sheet: &str) {
    let texts = text_boxes(svg);
    let mut saw_title = false;
    for text in &texts {
        if text.top < TITLE_TOP || text.bottom > FRAME.3 {
            continue;
        }
        saw_title = true;
        let right = if text.top < TITLE_NOTE_DIVIDER {
            if text.left < TITLE_SPLITS[0] {
                TITLE_SPLITS[0]
            }
            else if text.left < TITLE_SPLITS[1] {
                TITLE_SPLITS[1]
            }
            else if text.left < TITLE_SPLITS[2] {
                TITLE_SPLITS[2]
            }
            else if text.left < TITLE_SPLITS[3] {
                TITLE_SPLITS[3]
            }
            else {
                FRAME.2
            }
        }
        else {
            FRAME.2
        };
        assert!(
            text.right <= right - 1.0,
            "{sheet}: title block text `{}` at line {} overruns its cell (right {:.2} > {:.2})",
            text.content,
            text.line,
            text.right,
            right
        );
    }
    assert!(saw_title, "{sheet}: no title block text found");
}

fn assert_sheet(svg: &str, sheet: &str) {
    assert_inside_frame(svg, sheet);
    assert_no_label_crosses_geometry(svg, sheet);
    assert_labels_do_not_overlap(svg, sheet);
    assert_title_block_fits(svg, sheet);
}

#[test]
fn every_sheet_keeps_all_content_inside_the_frame() {
    let directory = temporary_dir("frames");
    let truss = demo_truss();
    let session = save_session(&truss, &directory, "truss");
    let plain = export_plain(&session, &directory, "truss.svg");
    assert_sheet(&plain, "truss.svg");
    let contour_target = directory.join("truss-axial.svg");
    session
        .export_with_options(
            ExportFormat::Svg,
            &contour_target,
            ExportOptions { contour: Some(cae_model::ContourQuantity::Axial), ..ExportOptions::default() },
        )
        .expect("contour export");
    let contour = fs::read_to_string(&contour_target).expect("read contour sheet");
    assert_sheet(&contour, "truss-axial.svg");
    for (file, kind) in [
        ("forces.svg", "forces"),
        ("shear.svg", "shear"),
        ("moment.svg", "moment"),
        ("displacements.svg", "displacement"),
        ("reactions.svg", "reactions"),
        ("overlay.svg", "overlay"),
        ("modes.svg", "modal"),
        ("envelope.svg", "envelope"),
        ("summary.svg", "summary"),
    ] {
        let sheet = export(&session, &directory, file, kind);
        assert_sheet(&sheet, file);
    }

    let directory = temporary_dir("frames-beam");
    let beam = uniformly_loaded_beam();
    let session = save_session(&beam, &directory, "beam");
    for (file, kind) in [("forces.svg", "forces"), ("shear.svg", "shear"), ("moment.svg", "moment")] {
        let sheet = export(&session, &directory, file, kind);
        assert_sheet(&sheet, file);
    }
}

#[test]
fn truss_content_meets_the_coverage_floor() {
    let directory = temporary_dir("coverage");
    let truss = demo_truss();
    let session = save_session(&truss, &directory, "truss");
    for file in ["truss.svg", "truss-axial.svg"] {
        let sheet = if file == "truss.svg" {
            export_plain(&session, &directory, "truss.svg")
        }
        else {
            let target = directory.join(file);
            session
                .export_with_options(
                    ExportFormat::Svg,
                    &target,
                    ExportOptions { contour: Some(cae_model::ContourQuantity::Axial), ..ExportOptions::default() },
                )
                .expect("contour export");
            fs::read_to_string(&target).expect("read contour sheet")
        };
        let points: Vec<(f64, f64)> = segments(&sheet).into_iter().flat_map(|segment| [segment.from, segment.to]).collect();
        let (min_x, max_x) =
            points.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(low, high), point| (low.min(point.0), high.max(point.0)));
        let (min_y, max_y) =
            points.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(low, high), point| (low.min(point.1), high.max(point.1)));
        let coverage = (max_x - min_x) * (max_y - min_y) / ((VIEW.2 - VIEW.0) * (VIEW.3 - VIEW.1));
        assert!(coverage >= 0.60, "{file}: content covers only {:.1} % of the viewport", coverage * 100.0);
    }
}

#[test]
fn shear_sheet_matches_the_uniform_load_solution() {
    let directory = temporary_dir("shear-uniform");
    let model = uniformly_loaded_beam();
    let session = save_session(&model, &directory, "uniform");
    let text = export(&session, &directory, "shear.svg", "shear");
    assert!(text.contains("OpenCAE shear force diagram"), "missing title: {text}");
    // Both member ends are labelled: V(0) = +qL/2, V(L) = -qL/2.
    assert!(text.contains("V = 1000 N"), "missing the left end value: {text}");
    assert!(text.contains("V = -1000 N"), "missing the right end value: {text}");
    assert!(text.contains("positive V on local +y"), "missing the sign legend: {text}");
    assert!(text.contains("dV/dx = q"), "missing the sign convention: {text}");
    // The filled diagram covers both a positive and a negative lobe.
    assert!(text.matches("fill=\"#dbeafe\"").count() >= 4, "missing the positive fill: {text}");
    assert!(text.matches("fill=\"#fee2e2\"").count() >= 4, "missing the negative fill: {text}");

    let diagrams = solve_internal_forces(&model, 11).expect("internal forces");
    let first = diagrams.elements[0].stations.first().expect("first station").shear;
    let last = diagrams.elements[0].stations.last().expect("last station").shear;
    assert!((first - 1000.0).abs() < 1.0, "recovered V(0) = {first}");
    assert!((last + 1000.0).abs() < 1.0, "recovered V(L) = {last}");
}

#[test]
fn moment_sheet_matches_q_l_squared_over_eight_and_p_l_over_four() {
    let directory = temporary_dir("moment-uniform");
    let session = save_session(&uniformly_loaded_beam(), &directory, "uniform");
    let text = export(&session, &directory, "moment.svg", "moment");
    assert!(text.contains("OpenCAE bending moment diagram"), "missing title: {text}");
    assert!(text.contains("max |M| = 250000 N·mm @ x=500"), "missing qL²/8: {text}");
    assert_eq!(text.matches("M = 0 N·mm").count(), 2, "both simple supports must be labelled zero: {text}");
    assert!(text.contains("sagging moment is positive"), "missing the sign convention: {text}");
    // The filled diagram must use the page: bounding box area / viewport area.
    let points: Vec<(f64, f64)> = segments(&text).into_iter().flat_map(|segment| [segment.from, segment.to]).collect();
    let (min_x, max_x) =
        points.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(low, high), point| (low.min(point.0), high.max(point.0)));
    let (min_y, max_y) =
        points.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(low, high), point| (low.min(point.1), high.max(point.1)));
    let coverage = (max_x - min_x) * (max_y - min_y) / ((VIEW.2 - VIEW.0) * (VIEW.3 - VIEW.1));
    assert!(coverage >= 0.60, "moment diagram covers only {:.1} % of the viewport", coverage * 100.0);

    let directory = temporary_dir("moment-centre");
    let session = save_session(&centre_loaded_beam(), &directory, "centre");
    let text = export(&session, &directory, "moment.svg", "moment");
    // PL/4 = 1000·1000/4 at the shared centre node; the shared node is
    // labelled once, not once per member end.
    assert_eq!(text.matches("M = 250000 N·mm").count(), 1, "PL/4 must label the centre node once: {text}");
    assert!(!text.contains("e-11"), "round-off at a zero support must be snapped: {text}");
    // The two shear panels carry P/2 end values.
    let shear = export(&session, &directory, "shear.svg", "shear");
    assert_eq!(shear.matches("V = 500 N").count(), 2, "P/2 must appear at both supports: {shear}");
    assert_eq!(shear.matches("V = -500 N").count(), 2, "P/2 must appear on both halves: {shear}");
}

#[test]
fn shear_and_moment_skip_truss_members_and_say_so() {
    let directory = temporary_dir("diagram-truss");
    let session = save_session(&demo_truss(), &directory, "truss");
    for (file, kind) in [("shear.svg", "shear"), ("moment.svg", "moment")] {
        let text = export(&session, &directory, file, kind);
        assert!(text.contains("truss members carry no shear or moment and are skipped: e1, e2"), "{file}: missing skip note");
        let filled = text.lines().filter(|line| line.starts_with("<polygon") && line.contains("fill=\"#dbeafe\"")).count();
        assert_eq!(filled, 0, "{file}: a truss must not get a filled diagram");
        assert!(text.contains("truss skipped"), "{file}: missing the truss legend entry");
    }
}

#[test]
fn shear_and_moment_sheets_are_deterministic() {
    let directory = temporary_dir("diagram-repeat");
    let session = save_session(&centre_loaded_beam(), &directory, "centre");
    for (file, kind) in [("shear.svg", "shear"), ("moment.svg", "moment")] {
        let first = export(&session, &directory, file, kind);
        let second = export(&session, &directory, file, kind);
        assert_eq!(first, second, "{file} must be byte-identical between exports");
    }
}
