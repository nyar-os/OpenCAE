//! Deterministic PDF rendering of the SVG sheets produced by [`crate::export`].
//!
//! The exporter writes one fixed visual language — white page rectangles,
//! axis and member lines, dashed outlines, polylines, support triangles,
//! node dots, arrow markers and plain text — and this module converts exactly
//! that subset into a single-page PDF 1.4 document of the same 960 × 720
//! point size. Unknown elements are skipped, so the output is a faithful
//! preview of the sheet rather than a general SVG renderer.
//!
//! The writer is byte-deterministic: no document information dictionary, no
//! timestamps and fixed object numbering, so the same SVG always produces
//! the same PDF bytes, matching the repository-wide determinism rule.

use std::fmt::Write as _;

/// Page height of the exported sheets, in PostScript points; the page is
/// 960 points wide, matching the SVG viewBox.
const PAGE_HEIGHT: f64 = 720.0;

/// Converts an SVG sheet written by [`crate::export`] into PDF bytes.
///
/// Coordinates are flipped from the SVG top-left origin to the PDF
/// bottom-left origin; strokes, fills, dashes, arrowheads and text are
/// carried over. Text is set in Helvetica with WinAnsi encoding, and glyphs
/// outside that encoding (`≡`, `∝`, `→`, `…`) are mapped to their nearest
/// ASCII stand-in.
pub fn pdf_from_svg(svg: &str) -> Vec<u8> {
    let mut content = String::new();
    // Group defaults for `font-size` and `fill`, mirroring the `<g>`
    // attributes the exporter uses for legend and label blocks.
    let mut font_stack: Vec<f64> = Vec::new();
    let mut fill_stack: Vec<Option<String>> = Vec::new();
    let mut skip_tag: Option<String> = None;
    let mut rest = svg;

    while let Some(start) = rest.find('<') {
        rest = &rest[start + 1..];
        let Some(end) = rest.find('>')
        else {
            break;
        };
        let tag = rest[..end].to_string();
        rest = &rest[end + 1..];

        if let Some(open) = &skip_tag {
            if tag == format!("/{open}") {
                skip_tag = None;
            }
            continue;
        }
        if tag.starts_with('?') || tag.starts_with('!') {
            continue;
        }
        if let Some(closing) = tag.strip_prefix('/') {
            if closing == "g" {
                font_stack.pop();
                fill_stack.pop();
            }
            continue;
        }
        let self_closing = tag.ends_with('/');
        let name = tag.split([' ', '\t', '\n', '/']).next().unwrap_or("");
        match name {
            "rect" => {
                let x = number(&tag, "x");
                let y = number(&tag, "y");
                let width = number(&tag, "width");
                let height = number(&tag, "height");
                if let Some(color) = attribute(&tag, "fill").and_then(parse_color) {
                    let _ = writeln!(content, "{color} rg");
                    let _ =
                        writeln!(content, "{} {} {} {} re f", num(x), num(PAGE_HEIGHT - y - height), num(width), num(height));
                }
            }
            "line" => {
                let (x1, y1) = (number(&tag, "x1"), number(&tag, "y1"));
                let (x2, y2) = (number(&tag, "x2"), number(&tag, "y2"));
                let color = attribute(&tag, "stroke").and_then(parse_color).unwrap_or_else(|| "0 0 0".to_string());
                let width = attribute(&tag, "stroke-width").and_then(|v| v.parse::<f64>().ok()).unwrap_or(1.0);
                let _ = write!(content, "{color} RG\n{} w\n", num(width));
                if attribute(&tag, "stroke-dasharray").is_some() {
                    content.push_str("[4 3] 0 d\n");
                }
                let _ =
                    write!(content, "{} {} m\n{} {} l\nS\n", num(x1), num(PAGE_HEIGHT - y1), num(x2), num(PAGE_HEIGHT - y2),);
                if attribute(&tag, "stroke-dasharray").is_some() {
                    content.push_str("[] 0 d\n");
                }
                if attribute(&tag, "marker-end").is_some() {
                    write_arrowhead(&mut content, x1, y1, x2, y2, &color);
                }
            }
            "polyline" | "polygon" => {
                let points = attribute(&tag, "points").unwrap_or_default();
                let filled = name == "polygon" && attribute(&tag, "fill").is_some_and(|v| v != "none");
                let mut first = true;
                let mut path = String::new();
                for pair in points.split_whitespace() {
                    let (x, y) = pair.split_once(',').unwrap_or(("0", "0"));
                    let (x, y) = (x.parse::<f64>().unwrap_or(0.0), y.parse::<f64>().unwrap_or(0.0));
                    let _ = writeln!(path, "{} {} {}", num(x), num(PAGE_HEIGHT - y), if first { "m" } else { "l" });
                    first = false;
                }
                if first {
                    continue;
                }
                if filled {
                    let color = attribute(&tag, "fill").and_then(parse_color).unwrap_or_else(|| "0 0 0".to_string());
                    let _ = write!(content, "{color} rg\n{path}h\nf\n");
                }
                else {
                    let color = attribute(&tag, "stroke").and_then(parse_color).unwrap_or_else(|| "0 0 0".to_string());
                    let width = attribute(&tag, "stroke-width").and_then(|v| v.parse::<f64>().ok()).unwrap_or(1.0);
                    let _ = write!(content, "{color} RG\n{} w\n{path}S\n", num(width));
                }
            }
            "circle" => {
                let (cx, cy, r) = (number(&tag, "cx"), number(&tag, "cy"), number(&tag, "r"));
                let color = attribute(&tag, "fill").and_then(parse_color).unwrap_or_else(|| "0 0 0".to_string());
                let _ = writeln!(content, "{color} rg");
                write_circle(&mut content, cx, PAGE_HEIGHT - cy, r);
            }
            "text" => {
                let close = rest.find("</text>").unwrap_or(0);
                let raw = &rest[..close];
                rest = &rest[close + "</text>".len()..];
                let size = attribute(&tag, "font-size")
                    .and_then(|v| v.parse::<f64>().ok())
                    .or_else(|| font_stack.last().copied())
                    .unwrap_or(16.0);
                let color = attribute(&tag, "fill")
                    .and_then(parse_color)
                    .or_else(|| fill_stack.last().and_then(|v| v.as_ref().and_then(|v| parse_color(v))))
                    .unwrap_or_else(|| "0 0 0".to_string());
                let anchor = attribute(&tag, "text-anchor").unwrap_or("start");
                let text = unescape(raw);
                let estimated = text.chars().count() as f64 * size * 0.55;
                let x = number(&tag, "x")
                    - match anchor {
                        "middle" => estimated / 2.0,
                        "end" => estimated,
                        _ => 0.0,
                    };
                let _ = write!(
                    content,
                    "BT\n/F1 {} Tf\n{color} rg\n1 0 0 1 {} {} Tm\n({}) Tj\nET\n",
                    num(size),
                    num(x),
                    num(PAGE_HEIGHT - number(&tag, "y")),
                    escape_text(&text),
                );
            }
            "g" => {
                font_stack.push(attribute(&tag, "font-size").and_then(|v| v.parse::<f64>().ok()).unwrap_or(16.0));
                fill_stack.push(attribute(&tag, "fill").map(str::to_string));
            }
            "marker" | "defs" | "title" | "linearGradient" | "radialGradient" | "style" if !self_closing => {
                skip_tag = Some(name.to_string());
            }
            _ => {}
        }
    }

    // The content stream is pure ASCII (see `escape_text`), so character
    // counts equal byte counts and the whole document can be assembled as
    // text before the final conversion.
    let mut pdf = String::new();
    pdf.push_str("%PDF-1.4\n");
    let mut offsets = Vec::with_capacity(5);
    offsets.push(pdf.len());
    pdf.push_str("1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n");
    offsets.push(pdf.len());
    pdf.push_str("2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n");
    offsets.push(pdf.len());
    pdf.push_str(
        "3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 960 720] /Resources << /Font << /F1 5 0 R >> >> \
         /Contents 4 0 R >>\nendobj\n",
    );
    offsets.push(pdf.len());
    let _ = write!(pdf, "4 0 obj\n<< /Length {} >>\nstream\n{content}endstream\nendobj\n", content.len());
    offsets.push(pdf.len());
    pdf.push_str("5 0 obj\n<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>\nendobj\n");
    let xref = pdf.len();
    let _ = write!(pdf, "xref\n0 6\n0000000000 65535 f \n");
    for offset in &offsets {
        let _ = writeln!(pdf, "{offset:010} 00000 n ");
    }
    let _ = write!(pdf, "trailer\n<< /Size 6 /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n");
    pdf.into_bytes()
}

/// Extracts a double-quoted attribute value from a tag body.
fn attribute<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let key = format!("{name}=\"");
    let mut search = tag;
    while let Some(pos) = search.find(&key) {
        let value_start = pos + key.len();
        if let Some(value_end) = search[value_start..].find('"') {
            return Some(&search[value_start..value_start + value_end]);
        }
        search = &search[value_start..];
    }
    None
}

/// Parses a numeric attribute, defaulting to zero.
fn number(tag: &str, name: &str) -> f64 {
    attribute(tag, name).and_then(|value| value.parse::<f64>().ok()).unwrap_or(0.0)
}

/// Formats a PDF real with at most three decimals and no trailing zeros.
fn num(value: f64) -> String {
    if !value.is_finite() {
        return "0".to_string();
    }
    let text = format!("{value:.3}");
    let text = text.trim_end_matches('0').trim_end_matches('.');
    if text == "-0" { "0".to_string() } else { text.to_string() }
}

/// Converts `#rrggbb` into the `"r g b"` operand string PDF colour operators
/// expect; anything else becomes black.
fn parse_color(value: &str) -> Option<String> {
    let hex = value.strip_prefix('#').unwrap_or(value);
    if hex.len() != 6 || !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    let channel = |range: std::ops::Range<usize>| u8::from_str_radix(&hex[range], 16).unwrap_or(0) as f64 / 255.0;
    Some(format!("{} {} {}", num3(channel(0..2)), num3(channel(2..4)), num3(channel(4..6))))
}

fn num3(value: f64) -> String {
    format!("{value:.4}")
}

/// Fills the arrowhead triangle at the end of a marker line.
fn write_arrowhead(content: &mut String, x1: f64, y1: f64, x2: f64, y2: f64, color: &str) {
    let (dx, dy) = (x2 - x1, y2 - y1);
    let length = (dx * dx + dy * dy).sqrt();
    if length <= 0.0 || !length.is_finite() {
        return;
    }
    let (ux, uy) = (dx / length, dy / length);
    let (nx, ny) = (-uy, ux);
    let back = 10.0;
    let half = 4.5;
    let corners = [
        (x2, y2),
        (x2 - ux * back + nx * half, y2 - uy * back + ny * half),
        (x2 - ux * back - nx * half, y2 - uy * back - ny * half),
    ];
    let _ = writeln!(content, "{color} rg");
    for (index, (x, y)) in corners.iter().enumerate() {
        let _ = writeln!(content, "{} {} {}", num(*x), num(PAGE_HEIGHT - *y), if index == 0 { "m" } else { "l" });
    }
    content.push_str("h\nf\n");
}

/// Approximates a circle with four cubic Bézier segments.
fn write_circle(content: &mut String, cx: f64, cy: f64, r: f64) {
    if r <= 0.0 || !r.is_finite() {
        return;
    }
    const K: f64 = 0.5522847498;
    let _ = writeln!(content, "{} {} m", num(cx + r), num(cy));
    let arcs = [(1.0, 0.0), (0.0, 1.0), (-1.0, 0.0), (0.0, -1.0)];
    for (index, (sx, sy)) in arcs.iter().enumerate() {
        let (px, py) = (cx + r * sx, cy + r * sy);
        let (tx, ty) = arcs[(index + 1) % arcs.len()];
        let (qx, qy) = (cx + r * tx, cy + r * ty);
        let (c1x, c1y) = (px - r * K * sy, py + r * K * sx);
        let (c2x, c2y) = (qx + r * K * ty, qy - r * K * tx);
        let _ = writeln!(content, "{} {} {} {} {} {} c", num(c1x), num(c1y), num(c2x), num(c2y), num(qx), num(qy));
    }
    content.push_str("f\n");
}

/// Resolves the XML entities the exporter emits inside text nodes.
fn unescape(text: &str) -> String {
    text.replace("&amp;", "&").replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&apos;", "'")
}

/// Maps the text into an ASCII-only PDF string literal.
///
/// Latin-1 glyphs (`·`, `×`, `°`) become WinAnsi octal escapes, mathematical
/// glyphs outside the encoding fall back to their nearest ASCII stand-in and
/// the string delimiters are backslash-escaped, so the emitted literal is
/// valid for every PDF reader.
fn escape_text(text: &str) -> String {
    let mut out = String::new();
    for ch in text.chars() {
        match ch {
            '(' | ')' | '\\' => {
                out.push('\\');
                out.push(ch);
            }
            '\u{2261}' => out.push('='),
            '\u{221d}' => out.push('~'),
            '\u{2192}' => out.push_str("->"),
            '\u{2026}' => out.push_str("..."),
            '\u{2013}' | '\u{2014}' => out.push('-'),
            '\n' | '\r' | '\t' => out.push(' '),
            ch if (ch as u32) < 32 => out.push('?'),
            ch if (ch as u32) < 128 => out.push(ch),
            ch if (ch as u32) < 256 => {
                let _ = write!(out, "\\{:03o}", ch as u32);
            }
            _ => out.push('?'),
        }
    }
    out
}
