//! The app's icon pack (24-unit SVG line icons, one consistent style) and a small drawer for
//! it: each icon's SVG (paths, rects, circles, ellipses; fill-opacity, stroke-width, dashes)
//! is parsed once into polylines and painted as vector strokes, so icons stay sharp at any
//! size and inside the bars' magnify. `draw(p, "name", centre, px, colour)`: `px` is the
//! on-screen size of the whole 24-unit box. Unknown names draw nothing.
use eframe::egui::{self, pos2, Color32, Pos2, Shape, Stroke};
use std::collections::HashMap;
use std::rc::Rc;

/// One drawable piece: polylines (closed or open), optional fill alpha, stroke width, dash
struct Item {
    subs: Vec<(Vec<[f32; 2]>, bool)>,
    fill: Option<f32>,
    stroke: Option<f32>,
    dash: Option<(f32, f32)>,
    /// fill="none": a closed shape a pointer keeps hollow (pointers fill closed shapes otherwise)
    hollow: bool,
    /// fill="#fff": the white body of a pointer; the icon's other strokes become dark creases on it
    solid: bool,
    /// A fill that isn't one convex shape (a crescent, a band with a hole): triangles, worked
    /// out once, filling it even-odd the way SVG does
    tris: Option<Vec<[f32; 2]>>,
}

/// Is this one closed outline convex (so egui can fill it directly, with smooth edges)
fn convex(v: &[[f32; 2]]) -> bool {
    let n = v.len();
    if n < 3 {
        return true;
    }
    let mut sign = 0.0_f32;
    for i in 0..n {
        let (a, b, c) = (v[i], v[(i + 1) % n], v[(i + 2) % n]);
        let cr = (b[0] - a[0]) * (c[1] - b[1]) - (b[1] - a[1]) * (c[0] - b[0]);
        if cr.abs() < 1e-6 {
            continue;
        }
        if sign == 0.0 {
            sign = cr.signum();
        } else if cr.signum() != sign {
            return false;
        }
    }
    true
}

/// Even-odd fill of any outlines (holes, dents) as triangles: the shape is cut into
/// horizontal slabs at every corner's height; in each slab the edges crossing it pair up
/// left to right, and each pair bounds one filled trapezoid
fn slabs(subs: &[(Vec<[f32; 2]>, bool)]) -> Vec<[f32; 2]> {
    let mut edges: Vec<([f32; 2], [f32; 2])> = Vec::new();
    for (v, _) in subs {
        for i in 0..v.len() {
            let (a, b) = (v[i], v[(i + 1) % v.len()]);
            if (a[1] - b[1]).abs() > 1e-5 {
                edges.push(if a[1] < b[1] { (a, b) } else { (b, a) });
            }
        }
    }
    let mut ys: Vec<f32> = edges.iter().flat_map(|e| [e.0[1], e.1[1]]).collect();
    ys.sort_by(|a, b| a.total_cmp(b));
    ys.dedup_by(|a, b| (*a - *b).abs() < 1e-4);
    let x_at = |e: &([f32; 2], [f32; 2]), y: f32| e.0[0] + (e.1[0] - e.0[0]) * (y - e.0[1]) / (e.1[1] - e.0[1]);
    let mut out = Vec::new();
    for w in ys.windows(2) {
        let (y0, y1) = (w[0], w[1]);
        let ym = (y0 + y1) / 2.0;
        let mut xs: Vec<(f32, f32, f32)> = edges.iter().filter(|e| e.0[1] <= ym && e.1[1] > ym).map(|e| (x_at(e, ym), x_at(e, y0), x_at(e, y1))).collect();
        xs.sort_by(|a, b| a.0.total_cmp(&b.0));
        for pr in xs.chunks_exact(2) {
            let (l, r) = (pr[0], pr[1]);
            out.extend([[l.1, y0], [r.1, y0], [r.2, y1], [l.1, y0], [r.2, y1], [l.2, y1]]);
        }
    }
    out
}

fn attr<'a>(el: &'a str, name: &str) -> Option<&'a str> {
    let key = format!(" {name}=\"");
    let i = el.find(&key)? + key.len();
    let j = el[i..].find('"')? + i;
    Some(&el[i..j])
}

fn num(el: &str, name: &str) -> f32 {
    attr(el, name).and_then(|v| v.parse().ok()).unwrap_or(0.0)
}

/// Numbers in a path's data (handles "1.5.5", "-1-2", exponent-free)
fn nums(s: &str) -> Vec<f32> {
    let mut out = Vec::new();
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() {
        let c = b[i] as char;
        if c.is_ascii_digit() || c == '-' || c == '.' {
            let st = i;
            i += 1;
            let mut dot = c == '.';
            while i < b.len() {
                let d = b[i] as char;
                if d.is_ascii_digit() {
                    i += 1;
                } else if d == '.' && !dot {
                    dot = true;
                    i += 1;
                } else {
                    break;
                }
            }
            out.push(s[st..i].parse().unwrap_or(0.0));
        } else {
            i += 1;
        }
    }
    out
}

const CURVE_STEPS: usize = 10;

fn arc_pts(out: &mut Vec<[f32; 2]>, p0: [f32; 2], rx: f32, ry: f32, rot: f32, large: bool, sweep: bool, p1: [f32; 2]) {
    let (mut rx, mut ry) = (rx.abs(), ry.abs());
    if rx < 1e-4 || ry < 1e-4 {
        out.push(p1);
        return;
    }
    let (s, c) = rot.to_radians().sin_cos();
    let dx = (p0[0] - p1[0]) / 2.0;
    let dy = (p0[1] - p1[1]) / 2.0;
    let x1 = c * dx + s * dy;
    let y1 = -s * dx + c * dy;
    let lam = (x1 * x1) / (rx * rx) + (y1 * y1) / (ry * ry);
    if lam > 1.0 {
        rx *= lam.sqrt();
        ry *= lam.sqrt();
    }
    let num = rx * rx * ry * ry - rx * rx * y1 * y1 - ry * ry * x1 * x1;
    let den = rx * rx * y1 * y1 + ry * ry * x1 * x1;
    let mut co = (num / den).max(0.0).sqrt();
    if large == sweep {
        co = -co;
    }
    let cx1 = co * rx * y1 / ry;
    let cy1 = -co * ry * x1 / rx;
    let cx = c * cx1 - s * cy1 + (p0[0] + p1[0]) / 2.0;
    let cy = s * cx1 + c * cy1 + (p0[1] + p1[1]) / 2.0;
    let ang = |ux: f32, uy: f32| uy.atan2(ux);
    let t0 = ang((x1 - cx1) / rx, (y1 - cy1) / ry);
    let mut dt = ang((-x1 - cx1) / rx, (-y1 - cy1) / ry) - t0;
    let tau = std::f32::consts::TAU;
    if sweep && dt < 0.0 {
        dt += tau;
    } else if !sweep && dt > 0.0 {
        dt -= tau;
    }
    let n = ((dt.abs() / tau * 32.0).ceil() as usize).max(2);
    for i in 1..=n {
        let t = t0 + dt * i as f32 / n as f32;
        let (ex, ey) = (rx * t.cos(), ry * t.sin());
        out.push([c * ex - s * ey + cx, s * ex + c * ey + cy]);
    }
}

fn parse_path(d: &str) -> Vec<(Vec<[f32; 2]>, bool)> {
    let mut subs: Vec<(Vec<[f32; 2]>, bool)> = Vec::new();
    let mut cur: Vec<[f32; 2]> = Vec::new();
    let (mut x, mut y, mut sx, mut sy) = (0.0f32, 0.0f32, 0.0f32, 0.0f32);
    let mut last_c: Option<[f32; 2]> = None;
    let mut last_q: Option<[f32; 2]> = None;
    // Split into (command, numbers)
    let mut cmds: Vec<(char, Vec<f32>)> = Vec::new();
    let mut start = None;
    for (i, ch) in d.char_indices() {
        if ch.is_ascii_alphabetic() {
            if let Some((c, s)) = start {
                cmds.push((c, nums(&d[s..i])));
            }
            start = Some((ch, i + 1));
        }
    }
    if let Some((c, s)) = start {
        cmds.push((c, nums(&d[s..])));
    }
    let flush = |cur: &mut Vec<[f32; 2]>, subs: &mut Vec<(Vec<[f32; 2]>, bool)>, closed: bool| {
        if cur.len() > 1 {
            subs.push((std::mem::take(cur), closed));
        } else {
            cur.clear();
        }
    };
    for (cmd, a) in cmds {
        let rel = cmd.is_ascii_lowercase();
        let up = cmd.to_ascii_uppercase();
        let (ox, oy) = if rel { (x, y) } else { (0.0, 0.0) };
        match up {
            'M' => {
                flush(&mut cur, &mut subs, false);
                for (k, ch) in a.chunks(2).enumerate() {
                    if ch.len() < 2 {
                        break;
                    }
                    let (ox, oy) = if rel { (x, y) } else { (0.0, 0.0) };
                    x = ox + ch[0];
                    y = oy + ch[1];
                    if k == 0 {
                        sx = x;
                        sy = y;
                    }
                    cur.push([x, y]);
                }
                last_c = None;
                last_q = None;
            }
            'L' => {
                for ch in a.chunks(2) {
                    if ch.len() < 2 {
                        break;
                    }
                    let (ox, oy) = if rel { (x, y) } else { (0.0, 0.0) };
                    x = ox + ch[0];
                    y = oy + ch[1];
                    cur.push([x, y]);
                }
                last_c = None;
                last_q = None;
            }
            'H' => {
                for v in a {
                    x = if rel { x + v } else { v };
                    cur.push([x, y]);
                }
                last_c = None;
                last_q = None;
            }
            'V' => {
                for v in a {
                    y = if rel { y + v } else { v };
                    cur.push([x, y]);
                }
                last_c = None;
                last_q = None;
            }
            'C' | 'S' => {
                let n = if up == 'C' { 6 } else { 4 };
                for ch in a.chunks(n) {
                    if ch.len() < n {
                        break;
                    }
                    let (ox, oy) = if rel { (x, y) } else { (0.0, 0.0) };
                    let (c1, c2, e) = if up == 'C' {
                        ([ox + ch[0], oy + ch[1]], [ox + ch[2], oy + ch[3]], [ox + ch[4], oy + ch[5]])
                    } else {
                        let c1 = last_c.map_or([x, y], |c| [2.0 * x - c[0], 2.0 * y - c[1]]);
                        (c1, [ox + ch[0], oy + ch[1]], [ox + ch[2], oy + ch[3]])
                    };
                    let p0 = [x, y];
                    for i in 1..=CURVE_STEPS {
                        let t = i as f32 / CURVE_STEPS as f32;
                        let u = 1.0 - t;
                        let f = |k: usize| u * u * u * p0[k] + 3.0 * u * u * t * c1[k] + 3.0 * u * t * t * c2[k] + t * t * t * e[k];
                        cur.push([f(0), f(1)]);
                    }
                    x = e[0];
                    y = e[1];
                    last_c = Some(c2);
                }
                last_q = None;
            }
            'Q' | 'T' => {
                let n = if up == 'Q' { 4 } else { 2 };
                for ch in a.chunks(n) {
                    if ch.len() < n {
                        break;
                    }
                    let (ox, oy) = if rel { (x, y) } else { (0.0, 0.0) };
                    let (c1, e) = if up == 'Q' {
                        ([ox + ch[0], oy + ch[1]], [ox + ch[2], oy + ch[3]])
                    } else {
                        (last_q.map_or([x, y], |c| [2.0 * x - c[0], 2.0 * y - c[1]]), [ox + ch[0], oy + ch[1]])
                    };
                    let p0 = [x, y];
                    for i in 1..=CURVE_STEPS {
                        let t = i as f32 / CURVE_STEPS as f32;
                        let u = 1.0 - t;
                        let f = |k: usize| u * u * p0[k] + 2.0 * u * t * c1[k] + t * t * e[k];
                        cur.push([f(0), f(1)]);
                    }
                    x = e[0];
                    y = e[1];
                    last_q = Some(c1);
                }
                last_c = None;
            }
            'A' => {
                for ch in a.chunks(7) {
                    if ch.len() < 7 {
                        break;
                    }
                    let (ox, oy) = if rel { (x, y) } else { (0.0, 0.0) };
                    let e = [ox + ch[5], oy + ch[6]];
                    arc_pts(&mut cur, [x, y], ch[0], ch[1], ch[2], ch[3] != 0.0, ch[4] != 0.0, e);
                    x = e[0];
                    y = e[1];
                }
                last_c = None;
                last_q = None;
            }
            'Z' => {
                flush(&mut cur, &mut subs, true);
                x = sx;
                y = sy;
                cur.push([x, y]);
                last_c = None;
                last_q = None;
            }
            _ => {}
        }
        let _ = (ox, oy);
    }
    flush(&mut cur, &mut subs, false);
    subs
}

fn ellipse(cx: f32, cy: f32, rx: f32, ry: f32) -> Vec<[f32; 2]> {
    (0..40).map(|i| {
        let t = i as f32 / 40.0 * std::f32::consts::TAU;
        [cx + rx * t.cos(), cy + ry * t.sin()]
    }).collect()
}

fn rect_pts(x: f32, y: f32, w: f32, h: f32, r: f32) -> Vec<[f32; 2]> {
    let r = r.min(w / 2.0).min(h / 2.0);
    if r <= 0.01 {
        return vec![[x, y], [x + w, y], [x + w, y + h], [x, y + h]];
    }
    let mut v = Vec::new();
    // Corners clockwise from top-right, each a quarter arc
    for (cx, cy, a0) in [(x + w - r, y + r, -90.0f32), (x + w - r, y + h - r, 0.0), (x + r, y + h - r, 90.0), (x + r, y + r, 180.0)] {
        for i in 0..=6 {
            let t = (a0 + 90.0 * i as f32 / 6.0).to_radians();
            v.push([cx + r * t.cos(), cy + r * t.sin()]);
        }
    }
    v
}

fn parse(svg: &str) -> Vec<Item> {
    let mut items = Vec::new();
    for el in svg.split('<').filter(|e| !e.trim().is_empty()) {
        let tag: String = el.chars().take_while(|c| c.is_ascii_alphabetic()).collect();
        let el = &format!(" {}", &el[tag.len()..]);
        let subs = match tag.as_str() {
            "path" => parse_path(attr(el, "d").unwrap_or("")),
            "rect" => vec![(rect_pts(num(el, "x"), num(el, "y"), num(el, "width"), num(el, "height"), num(el, "rx")), true)],
            "circle" => {
                let r = num(el, "r");
                vec![(ellipse(num(el, "cx"), num(el, "cy"), r, r), true)]
            }
            "ellipse" => vec![(ellipse(num(el, "cx"), num(el, "cy"), num(el, "rx"), num(el, "ry")), true)],
            _ => continue,
        };
        let fill = match attr(el, "fill") {
            Some("currentColor") => Some(attr(el, "fill-opacity").and_then(|v| v.parse().ok()).unwrap_or(1.0)),
            _ => None,
        };
        let stroke = if attr(el, "stroke") == Some("none") { None } else { Some(attr(el, "stroke-width").and_then(|v| v.parse().ok()).unwrap_or(1.75)) };
        let dash = attr(el, "stroke-dasharray").map(nums).and_then(|v| (v.len() >= 2).then(|| (v[0], v[1])));
        let hollow = attr(el, "fill") == Some("none");
        let solid = attr(el, "fill") == Some("#fff");
        let tris = fill.filter(|_| subs.len() > 1 || subs.iter().any(|(v, _)| !convex(v))).map(|_| slabs(&subs));
        items.push(Item { subs, fill, stroke, dash, hollow, solid, tris });
    }
    items
}

thread_local! {
    static CACHE: std::cell::RefCell<HashMap<&'static str, Rc<Vec<Item>>>> = std::cell::RefCell::new(HashMap::new());
}

fn items(name: &'static str) -> Option<Rc<Vec<Item>>> {
    CACHE.with(|c| {
        if let Some(v) = c.borrow().get(name) {
            return Some(v.clone());
        }
        let svg = PACK.iter().find(|(n, _)| *n == name)?.1;
        let v = Rc::new(parse(svg));
        c.borrow_mut().insert(name, v.clone());
        Some(v)
    })
}

/// The icon's subpaths in its 24-unit box, as a pointer draws them: (points, closed, filled)
pub(crate) fn subpaths(name: &'static str) -> Vec<(Vec<[f32; 2]>, bool, bool)> {
    // creases on a white body aren't part of the outline the morph blends
    items(name).map(|it| { let solid = it.iter().any(|i| i.solid); it.iter().filter(|i| !solid || i.solid).flat_map(|i| i.subs.iter().map(move |(v, c)| (v.clone(), *c, *c && !i.hollow))).collect() }).unwrap_or_default()
}

/// Paints icon `name` as a pointer: a dark edge `ink` around it, closed shapes filled with
/// `fill`, open strokes drawn in `fill`. `hot` is the point of the 24-unit box placed at `at`.
/// `rot`: radians clockwise, around the hot spot (so the point that clicks stays put)
#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_cursor(p: &egui::Painter, name: &'static str, at: Pos2, hot: [f32; 2], px: f32, rot: f32, fill: Color32, ink: Color32) {
    let Some(items) = items(name) else { return };
    let k = px / 24.0;
    let (sn, cs) = rot.sin_cos();
    let tf = |q: &[f32; 2]| {
        let (x, y) = ((q[0] - hot[0]) * k, (q[1] - hot[1]) * k);
        pos2(at.x + x * cs - y * sn, at.y + x * sn + y * cs)
    };
    let w = |it: &Item| (it.stroke.unwrap_or(1.75) * k).max(1.2);
    let pts = |v: &[[f32; 2]], closed: bool| {
        let mut v: Vec<Pos2> = v.iter().map(tf).collect();
        if closed {
            if let Some(f) = v.first().copied() {
                v.push(f);
            }
        }
        v
    };
    let has_body = items.iter().any(|i| i.solid);
    let crease = |it: &Item| has_body && !it.solid;
    // Soft shadow, then the dark edge
    for (off, col, extra) in [(egui::vec2(0.6, 1.0), Color32::from_black_alpha(50), 3.6), (egui::vec2(0.0, 0.0), ink, 2.6)] {
        for it in items.iter().filter(|i| !crease(i)) {
            for (v, closed) in &it.subs {
                let v: Vec<Pos2> = pts(v, *closed).into_iter().map(|q| q + off).collect();
                // A tiny closed shape (a dot): a stroke wider than the shape folds into junk, so
                // its edge is just a bigger disc behind it
                let bb = egui::Rect::from_points(&v);
                if *closed && !it.hollow && bb.width().max(bb.height()) < 5.0 * k {
                    p.circle_filled(bb.center(), bb.width().max(bb.height()) / 2.0 + (w(it) + extra) / 2.0, col);
                    continue;
                }
                if *closed && !it.hollow {
                    fan(p, &v, col);
                }
                safe_line(p, &v, Stroke::new(w(it) + extra, col));
            }
        }
    }
    // Body
    for it in items.iter().filter(|i| !crease(i)) {
        for (v, closed) in &it.subs {
            let v = pts(v, *closed);
            let bb = egui::Rect::from_points(&v);
            if *closed && !it.hollow && bb.width().max(bb.height()) < 5.0 * k {
                p.circle_filled(bb.center(), bb.width().max(bb.height()) / 2.0, fill);
                continue;
            }
            if *closed && !it.hollow {
                fan(p, &v, fill);
            } else {
                safe_line(p, &v, Stroke::new(w(it), fill));
            }
        }
    }
    // Finger creases, dark on the white body
    for it in items.iter().filter(|i| crease(i)) {
        for (v, closed) in &it.subs {
            safe_line(p, &pts(v, *closed), Stroke::new(w(it) * 0.75, ink));
        }
    }
}

/// A polyline with round ends, broken where it turns back on itself (a pointed join there
/// would shoot out a spike)
fn safe_line(p: &egui::Painter, v: &[Pos2], st: Stroke) {
    let mut run: Vec<Pos2> = Vec::new();
    let mut flush = |run: &mut Vec<Pos2>| {
        if run.len() >= 2 {
            p.add(Shape::line(run.clone(), st));
        }
        for q in [run.first(), run.last()].into_iter().flatten() {
            p.circle_filled(*q, st.width / 2.0, st.color);
        }
        run.clear();
    };
    for &q in v {
        if let (Some(&b), Some(&a)) = (run.last(), run.len().checked_sub(2).and_then(|i| run.get(i))) {
            let (d1, d2) = (b - a, q - b);
            if d1.length() > 1e-4 && d2.length() > 1e-4 && d1.normalized().dot(d2.normalized()) < -0.3 {
                flush(&mut run);
                run.push(b);
            }
        }
        if run.last().map_or(true, |l| (*l - q).length() > 1e-4) {
            run.push(q);
        }
    }
    flush(&mut run);
}

/// Fills a (star-shaped) polygon as a fan of triangles from its first point
fn fan(p: &egui::Painter, v: &[Pos2], col: Color32) {
    // One plain mesh, no edge smoothing: smoothed edges of thin fan triangles shoot out as
    // long spikes (the outline stroke drawn over the edge keeps it clean)
    let mut m = egui::Mesh::default();
    for q in v {
        m.colored_vertex(*q, col);
    }
    for i in 1..v.len().saturating_sub(1) {
        m.add_triangle(0, i as u32, i as u32 + 1);
    }
    p.add(Shape::mesh(m));
}

/// Paints icon `name` centred on `c`, its 24-unit box `px` pixels wide
pub(crate) fn draw(p: &egui::Painter, name: &'static str, c: Pos2, px: f32, col: Color32) {
    let Some(items) = items(name) else { return };
    draw_items(p, &items, c, px, col);
}

/// Like `draw`, for an SVG body made on the fly (pack format, not cached)
pub(crate) fn draw_svg(p: &egui::Painter, svg: &str, c: Pos2, px: f32, col: Color32) {
    draw_items(p, &parse(svg), c, px, col);
}

fn draw_items(p: &egui::Painter, items: &[Item], c: Pos2, px: f32, col: Color32) {
    let k = px / 24.0;
    let c = p.round_pos_to_pixels(c);
    let tf = |q: &[f32; 2]| pos2(c.x + (q[0] - 12.0) * k, c.y + (q[1] - 12.0) * k);
    for it in items.iter() {
        if let (Some(a), Some(t)) = (it.fill, &it.tris) {
            // no edge smoothing on these (thin slab edges would shoot out spikes)
            let fc = col.gamma_multiply(a);
            let mut m = egui::Mesh::default();
            for q in t {
                m.colored_vertex(tf(q), fc);
            }
            m.indices = (0..t.len() as u32).collect();
            p.add(Shape::mesh(m));
        } else if let Some(a) = it.fill {
            for (pts, _) in &it.subs {
                let v: Vec<Pos2> = pts.iter().map(tf).collect();
                p.add(Shape::convex_polygon(v, col.gamma_multiply(a), Stroke::NONE));
            }
        }
        let Some(w) = it.stroke else { continue };
        // Same weight ratio as the pack, kept readable when drawn small
        let st = Stroke::new((w * k).max(w / 1.75 * 1.1), col);
        for (pts, closed) in &it.subs {
            let mut v: Vec<Pos2> = pts.iter().map(tf).collect();
            if *closed {
                if let Some(f) = v.first().copied() {
                    v.push(f);
                }
            }
            match it.dash {
                // "0 gap": round dots along the line
                Some((d, g)) if d < 0.01 => {
                    let mut acc = 0.0;
                    let mut next = 0.0;
                    for s in v.windows(2) {
                        let len = (s[1] - s[0]).length();
                        while next <= acc + len {
                            let t = if len > 0.0 { (next - acc) / len } else { 0.0 };
                            p.circle_filled(s[0] + (s[1] - s[0]) * t, st.width / 2.0, col);
                            next += g * k;
                        }
                        acc += len;
                    }
                }
                Some((d, g)) => {
                    p.extend(Shape::dashed_line(&v, st, d * k, g * k));
                }
                None if *closed => {
                    v.pop();
                    p.add(Shape::closed_line(v, st));
                }
                None => {
                    p.add(Shape::line(v, st));
                }
            }
        }
    }
}

/// The pack: (name, SVG body on a 24x24 box, stroke 1.75, round caps and joins)
const PACK: &[(&str, &str)] = &[
    ("cursor", r##"<path d="M5.5 3.5L18.5 10l-5.9 2.1L10 18z"/>"##),
    ("hand", r##"<path d="M8 15.5V6.5a1.5 1.5 0 0 1 3 0V5a1.5 1.5 0 0 1 3 0v1.5a1.5 1.5 0 0 1 3 0v3a1.5 1.5 0 0 1 3 0V14a7 7 0 0 1-7 7h-1.2a6 6 0 0 1-4.7-2.3L3.9 15.9a1.5 1.5 0 0 1 2.3-1.9L8 15.5z" fill="#fff"/><path d="M11 6.5V11M14 6.5V11M17 9.5V12"/>"##),
    // Pointer shapes drawn in place of the native ones (ui/cursor.rs); (12, 12) sits on the pointer
    ("resize-h", r##"<path d="M4 12h16M7.5 8.5L4 12l3.5 3.5M16.5 8.5L20 12l-3.5 3.5"/>"##),
    ("resize-v", r##"<path d="M12 4v16M8.5 7.5L12 4l3.5 3.5M8.5 16.5L12 20l3.5-3.5"/>"##),
    ("resize-nwse", r##"<path d="M6 6l12 12M6 11V6h5M18 13v5h-5"/>"##),
    ("resize-nesw", r##"<path d="M18 6L6 18M13 6h5v5M11 18H6v-5"/>"##),
    ("eraser-ring", r##"<circle cx="12" cy="12" r="7" fill="none"/>"##),
    ("crosshair", r##"<path d="M12 3v4.5M12 16.5V21M3 12h4.5M16.5 12H21"/><circle cx="12" cy="12" r="1" fill="currentColor"/>"##),
    ("text-cursor", r##"<path d="M9 4.5c1.7 0 3 .8 3 2.5 0-1.7 1.3-2.5 3-2.5M9 19.5c1.7 0 3-.8 3-2.5 0 1.7 1.3 2.5 3 2.5M12 7v10M10.5 12h3"/>"##),
    ("grabbing", r##"<path d="M7 11V8a1.5 1.5 0 0 1 3 0V7a1.5 1.5 0 0 1 3 0v.5a1.5 1.5 0 0 1 3 0V9a1.5 1.5 0 0 1 3 0v3.5a6.5 6.5 0 0 1-6.5 6.5h-1.3a5.5 5.5 0 0 1-4.4-2.2l-1.6-2.3a2 2 0 0 1 .6-2.8z" fill="#fff"/><path d="M10 8v1.5M13 7.5V9M16 9v1M7 11v1.5c1.6 0 2.9.7 3.5 2"/>"##),
    ("frame", r##"<path d="M7.5 3v18M16.5 3v18M3 7.5h18M3 16.5h18"/>"##),
    ("rectangle", r##"<rect x="4" y="5" width="16" height="14" rx="2"/>"##),
    ("ellipse", r##"<circle cx="12" cy="12" r="8.5"/>"##),
    ("triangle", r##"<path d="M12 4l8.5 15h-17z"/>"##),
    ("polygon", r##"<path d="M12 3l7.8 4.5v9L12 21l-7.8-4.5v-9z"/>"##),
    ("star", r##"<path d="M12 3.8L14.41 9.48L20.56 10.02L15.9 14.07L17.29 20.08L12 16.9L6.71 20.08L8.1 14.07L3.44 10.02L9.59 9.48z"/>"##),
    ("shapes", r##"<rect x="3.5" y="3.5" width="7.5" height="7.5" rx="1.5"/><circle cx="16.75" cy="7.25" r="3.75"/><path d="M7.25 13.5l3.75 7h-7.5zM16.75 13.5v7M13.25 17h7"/>"##),
    ("line", r##"<path d="M5 19L19 5"/>"##),
    ("arrow", r##"<path d="M5 19L19 5M9 5h10v10"/>"##),
    ("arrow-curved", r##"<path d="M4 19.5C4 10.5 9.5 5 19 5M16 2l3 3-3 3"/>"##),
    ("connector", r##"<circle cx="4.5" cy="6" r="1.5" fill="currentColor" fill-opacity=".25"/><path d="M6 6h4a2 2 0 0 1 2 2v8a2 2 0 0 0 2 2h6M17 15l3 3-3 3"/>"##),
    ("pen", r##"<path d="M5.6 18.4L6.7 8.8L14.5 4.6L19.4 9.5L15.2 17.3zM5.6 18.4L10.2 13.8M16.2 3.5L20.5 7.8"/><circle cx="11.5" cy="12.5" r="1.75"/>"##),
    ("pencil", r##"<path d="M4 20l1-4.5L15.5 5a2.1 2.1 0 0 1 3 3L8 18.5zM13.5 7l3 3"/>"##),
    ("highlighter", r##"<rect x="3" y="19.5" width="10" height="2.5" rx="1" fill="currentColor" fill-opacity=".25" stroke="none"/><path d="M15 3.5l5 5-8 8-5-5zM7 11.5l-1.6 4.1 2.5 2.5 4.1-1.6"/>"##),
    ("brush", r##"<path d="M18.5 3.5l2 2-8 8-2-2zM10.5 11.5c-2.5 0-4 1.5-4 4 0 1.5-1 2.5-3 3.5 3 1 7 .5 8.5-2 .8-1.3.5-2.5-.5-3.5"/>"##),
    ("eraser", r##"<path d="M18.6 10.6L11 18.2a2 2 0 0 1-2.8 0l-3.4-3.4a2 2 0 0 1 0-2.8l7.6-7.6a2 2 0 0 1 2.8 0l3.4 3.4a2 2 0 0 1 0 2.8zM8.6 8.4l7 7M9.5 20.5h10.5"/>"##),
    ("text", r##"<path d="M5 6.5v-2h14v2M12 4.5v15M9.5 19.5h5"/>"##),
    ("lasso", r##"<path d="M7 16.5C4.6 15.3 3 13.4 3 11c0-3.9 4-7 9-7s9 3.1 9 7-4 7-9 7c-.9 0-1.8-.1-2.6-.3" stroke-dasharray="3 2.75"/><path d="M7.3 18.8c-.4 1.4-1.6 2.2-3.3 2.2"/><circle cx="8" cy="17.5" r="1.5"/>"##),
    ("pixelate", r##"<rect x="9.33" y="4" width="5.34" height="5.33" rx="0" fill="currentColor" fill-opacity=".25" stroke="none"/><rect x="4" y="9.33" width="5.33" height="5.34" rx="0" fill="currentColor" fill-opacity=".25" stroke="none"/><rect x="14.67" y="9.33" width="5.33" height="5.34" rx="0" fill="currentColor" fill-opacity=".25" stroke="none"/><rect x="9.33" y="14.67" width="5.34" height="5.33" rx="0" fill="currentColor" fill-opacity=".25" stroke="none"/><rect x="4" y="4" width="16" height="16" rx="2"/><path d="M9.33 4v16M14.67 4v16M4 9.33h16M4 14.67h16"/>"##),
    ("blur", r##"<circle cx="12" cy="12" r="3.5" fill="currentColor" fill-opacity=".25"/><circle cx="12" cy="12" r="8" stroke-dasharray="0 3.15"/>"##),
    ("eyedropper", r##"<path d="M19.5 4.5a2.1 2.1 0 0 0-3 0L14 7l-1-1-2 2 1 1-7 7v3h3l7-7 1 1 2-2-1-1 2.5-2.5a2.1 2.1 0 0 0 0-3z"/>"##),
    ("paint-bucket", r##"<path d="M11 3.5l8 8-6.6 6.6a2 2 0 0 1-2.8 0l-5.2-5.2a2 2 0 0 1 0-2.8z"/><path d="M4.5 11.5h14.5l-6.6 6.6a2 2 0 0 1-2.8 0z" fill="currentColor" fill-opacity=".25" stroke="none"/><path d="M7.5 7L5 4.5M20.5 15.5s1.5 2 1.5 3.2a1.5 1.5 0 0 1-3 0c0-1.2 1.5-3.2 1.5-3.2z"/>"##),
    ("crop", r##"<path d="M6 2v14a2 2 0 0 0 2 2h14M18 22V8a2 2 0 0 0-2-2H2"/>"##),
    ("magic-wand", r##"<path d="M4 20L14 10M17 3.5l.9 2.6 2.6.9-2.6.9-.9 2.6-.9-2.6-2.6-.9 2.6-.9zM7 3v3M5.5 4.5h3M19 14v3M17.5 15.5h3"/>"##),
    ("ai-sparkle", r##"<path d="M10 4.5c.6 4.2 2.3 5.9 6.5 6.5-4.2.6-5.9 2.3-6.5 6.5-.6-4.2-2.3-5.9-6.5-6.5 4.2-.6 5.9-2.3 6.5-6.5z" fill="currentColor" fill-opacity=".25"/><path d="M18 3v4M16 5h4M18 16.5v4M16 18.5h4"/>"##),
    ("heal", r##"<rect x="2.5" y="8" width="19" height="8" rx="4" transform="rotate(-45 12 12)"/><circle cx="10.6" cy="12" r="1.25" fill="currentColor" stroke="none"/><circle cx="13.4" cy="12" r="1.25" fill="currentColor" stroke="none"/><circle cx="12" cy="10.6" r="1.25" fill="currentColor" stroke="none"/><circle cx="12" cy="13.4" r="1.25" fill="currentColor" stroke="none"/>"##),
    ("clone-stamp", r##"<path d="M10 3h4a1 1 0 0 1 1 1v3.5l3 3.5H6l3-3.5V4a1 1 0 0 1 1-1z"/><rect x="5" y="11" width="14" height="4" rx="1" fill="currentColor" fill-opacity=".25"/><path d="M6 19h12"/>"##),
    ("scissors", r##"<circle cx="6" cy="6" r="2.75"/><circle cx="6" cy="18" r="2.75"/><path d="M20 4L8 16M14.5 14.5L20 20M8 8l4 4"/>"##),
    ("comment", r##"<path d="M20 15a2 2 0 0 1-2 2H8l-4 4V6a2 2 0 0 1 2-2h12a2 2 0 0 1 2 2z"/>"##),
    ("sticky-note", r##"<path d="M14 20H6a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2h12a2 2 0 0 1 2 2v8z"/><path d="M14 20v-4a2 2 0 0 1 2-2h4z" fill="currentColor" fill-opacity=".25"/>"##),
    ("ruler", r##"<path d="M3.5 15.5l12-12 5 5-12 12zM6.5 12.5l2 2M9.5 9.5L11 11M12.5 6.5l2 2"/>"##),
    ("zoom-in", r##"<circle cx="11" cy="11" r="6.5"/><path d="M16 16l4.5 4.5"/><path d="M8.5 11h5M11 8.5v5"/>"##),
    ("zoom-out", r##"<circle cx="11" cy="11" r="6.5"/><path d="M16 16l4.5 4.5"/><path d="M8.5 11h5"/>"##),
    ("cube-3d", r##"<path d="M12 3l8 4.5L12 12 4 7.5z" fill="currentColor" fill-opacity=".25" stroke="none"/><path d="M12 3l8 4.5v9L12 21l-8-4.5v-9zM12 12l8-4.5M12 12L4 7.5M12 12v9"/>"##),
    ("tilt-3d", r##"<path d="M4 5.5l10 2.5v8l-10 2.5z" fill="currentColor" fill-opacity=".25"/><path d="M17.5 7.5c1.6 1.2 2.5 2.8 2.5 4.5s-.9 3.3-2.5 4.5M17.9 13.9l-.4 2.6 2.6.4"/>"##),
    ("rotate-3d", r##"<ellipse cx="12" cy="12" rx="9" ry="3.75"/><ellipse cx="12" cy="12" rx="3.75" ry="9" stroke-dasharray="2.5 2.75"/><circle cx="12" cy="12" r="1.5" fill="currentColor" stroke="none"/>"##),
    ("depth", r##"<path d="M4 8l4-4h12l-4 4z" fill="currentColor" fill-opacity=".25" stroke="none"/><rect x="4" y="8" width="12" height="12" rx="1"/><path d="M4 8l4-4h11a1 1 0 0 1 1 1v11l-4 4M16 8l4-4"/>"##),
    ("perspective", r##"<path d="M7.5 5h9L21 19H3z" fill="currentColor" fill-opacity=".25" stroke="none"/><path d="M7.5 5h9L21 19H3zM12 5v14M5.6 11h12.8"/>"##),
    ("warp", r##"<path d="M4 5c5 2 11 2 16 0-1.5 4.5-1.5 9.5 0 14-5-2-11-2-16 0 1.5-4.5 1.5-9.5 0-14zM12 6.5v11M5.1 12h13.8"/>"##),
    ("light-source", r##"<path d="M14.5 7.5a6.5 6.5 0 0 1 0 13 3.25 6.5 0 0 0 0-13z" fill="currentColor" fill-opacity=".25" stroke="none"/><circle cx="14.5" cy="14" r="6.5"/><circle cx="5.5" cy="5.5" r="2" fill="currentColor" fill-opacity=".25"/><path d="M10 5.5h1.5M5.5 10v1.5"/>"##),
    ("drop-shadow", r##"<path d="M16 8h2a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2h-8a2 2 0 0 1-2-2v-2h6a2 2 0 0 0 2-2z" fill="currentColor" fill-opacity=".25"/><rect x="4" y="4" width="12" height="12" rx="2"/>"##),
    ("move", r##"<path d="M12 3v18M3 12h18M9.5 5.5L12 3l2.5 2.5M9.5 18.5L12 21l2.5-2.5M5.5 9.5L3 12l2.5 2.5M18.5 9.5L21 12l-2.5 2.5"/>"##),
    ("scale", r##"<rect x="5" y="5" width="14" height="14" rx="0" stroke-dasharray="2.5 2.5"/><rect x="3.25" y="3.25" width="3.5" height="3.5" rx="0.75" fill="currentColor" fill-opacity=".25"/><rect x="17.25" y="3.25" width="3.5" height="3.5" rx="0.75" fill="currentColor" fill-opacity=".25"/><rect x="3.25" y="17.25" width="3.5" height="3.5" rx="0.75" fill="currentColor" fill-opacity=".25"/><rect x="17.25" y="17.25" width="3.5" height="3.5" rx="0.75" fill="currentColor" fill-opacity=".25"/>"##),
    ("rotate-cw", r##"<path d="M20 12a8 8 0 1 1-2.34-5.66L20 8.5M20 4v4.5h-4.5"/>"##),
    ("rotate-ccw", r##"<path d="M4 12a8 8 0 1 0 2.34-5.66L4 8.5M4 4v4.5h4.5"/>"##),
    // two arrows passing each other: swap a gradient's ends
    ("swap-ends", r##"<path d="M19 8H5M9 4L5 8l4 4"/><path d="M5 16h14M15 12l4 4-4 4"/>"##),
    // Fill types (the fill dropdown)
    ("fill-solid", r##"<rect x="4" y="4" width="16" height="16" rx="2" fill="currentColor" fill-opacity=".45"/>"##),
    ("gradient-linear", r##"<rect x="8" y="4" width="4" height="16" fill="currentColor" fill-opacity=".12" stroke="none"/><rect x="12" y="4" width="4" height="16" fill="currentColor" fill-opacity=".28" stroke="none"/><path d="M16 4h2a2 2 0 0 1 2 2v12a2 2 0 0 1-2 2h-2z" fill="currentColor" fill-opacity=".5" stroke="none"/><rect x="4" y="4" width="16" height="16" rx="2"/>"##),
    ("gradient-radial", r##"<circle cx="12" cy="12" r="7.5" fill="currentColor" fill-opacity=".12" stroke="none"/><circle cx="12" cy="12" r="5" fill="currentColor" fill-opacity=".2" stroke="none"/><circle cx="12" cy="12" r="2.5" fill="currentColor" fill-opacity=".35" stroke="none"/><rect x="4" y="4" width="16" height="16" rx="2"/>"##),
    ("gradient-angular", r##"<path d="M12 12L12 4L19.6 4.4Z" fill="currentColor" fill-opacity="0.05" stroke="none"/><path d="M12 12L19.6 4.4L20 12Z" fill="currentColor" fill-opacity="0.13" stroke="none"/><path d="M12 12L20 12L19.6 19.6Z" fill="currentColor" fill-opacity="0.21" stroke="none"/><path d="M12 12L19.6 19.6L12 20Z" fill="currentColor" fill-opacity="0.29" stroke="none"/><path d="M12 12L12 20L4.4 19.6Z" fill="currentColor" fill-opacity="0.37" stroke="none"/><path d="M12 12L4.4 19.6L4 12Z" fill="currentColor" fill-opacity="0.45" stroke="none"/><path d="M12 12L4 12L4.4 4.4Z" fill="currentColor" fill-opacity="0.53" stroke="none"/><path d="M12 12L4.4 4.4L12 4Z" fill="currentColor" fill-opacity="0.61" stroke="none"/><path d="M12 12V4"/><rect x="4" y="4" width="16" height="16" rx="2"/>"##),
    ("gradient-diamond", r##"<path d="M12 4l8 8-8 8-8-8z" fill="currentColor" fill-opacity=".12" stroke="none"/><path d="M12 6.75l5.25 5.25L12 17.25 6.75 12z" fill="currentColor" fill-opacity=".2" stroke="none"/><path d="M12 9.5l2.5 2.5L12 14.5 9.5 12z" fill="currentColor" fill-opacity=".35" stroke="none"/><rect x="4" y="4" width="16" height="16" rx="2"/>"##),
    ("fill-none", r##"<rect x="4" y="4" width="16" height="16" rx="2"/><path d="M5 19L19 5"/>"##),
    // Blend modes (the blend dropdown): two circles, the overlap shows the mode
    ("blend-normal", r##"<path d="M12 6.8A6 6 0 1 0 12 17.2"/><circle cx="15" cy="12" r="6" fill="currentColor" fill-opacity=".25"/>"##),
    ("blend-multiply", r##"<circle cx="9" cy="12" r="6" fill="currentColor" fill-opacity=".25"/><circle cx="15" cy="12" r="6" fill="currentColor" fill-opacity=".25"/>"##),
    ("blend-screen", r##"<circle cx="9" cy="12" r="6"/><circle cx="15" cy="12" r="6"/>"##),
    ("blend-overlay", r##"<path d="M9 12A6 6 0 0 1 12 6.8A6 6 0 0 1 15 12Z" fill="currentColor" fill-opacity=".45" stroke="none"/><circle cx="9" cy="12" r="6"/><circle cx="15" cy="12" r="6"/><path d="M9 12h6"/>"##),
    ("blend-darken", r##"<path d="M12 6.8A6 6 0 0 1 12 17.2A6 6 0 0 1 12 6.8Z" fill="currentColor" fill-opacity=".45" stroke="none"/><circle cx="9" cy="12" r="6"/><circle cx="15" cy="12" r="6"/>"##),
    ("blend-lighten", r##"<path d="M12 6.8A6 6 0 1 0 12 17.2A6 6 0 1 0 12 6.8Z"/>"##),
    ("blend-color-dodge", r##"<circle cx="9" cy="12" r="6"/><circle cx="15" cy="12" r="6"/><path d="M12 10.25v3.5M10.25 12h3.5"/>"##),
    ("blend-color-burn", r##"<path d="M12 6.8A6 6 0 0 1 12 17.2A6 6 0 0 1 12 6.8Z" fill="currentColor" stroke="none"/><circle cx="9" cy="12" r="6"/><circle cx="15" cy="12" r="6"/>"##),
    ("blend-hard-light", r##"<path d="M9 12A6 6 0 0 1 12 6.8A6 6 0 0 1 15 12Z" fill="currentColor" stroke="none"/><circle cx="9" cy="12" r="6"/><circle cx="15" cy="12" r="6"/><path d="M9 12h6"/>"##),
    ("blend-soft-light", r##"<path d="M9 12A6 6 0 0 1 12 6.8A6 6 0 0 1 15 12C13.5 13.5 10.5 10.5 9 12Z" fill="currentColor" fill-opacity=".25" stroke="none"/><circle cx="9" cy="12" r="6"/><circle cx="15" cy="12" r="6"/><path d="M9 12c1.5-1.5 4.5 1.5 6 0"/>"##),
    ("blend-difference", r##"<path d="M12 6.8A6 6 0 1 0 12 17.2A6 6 0 0 1 12 6.8Z" fill="currentColor" fill-opacity=".45"/><path d="M12 17.2A6 6 0 1 0 12 6.8A6 6 0 0 1 12 17.2Z" fill="currentColor" fill-opacity=".45"/>"##),
    ("blend-exclusion", r##"<path d="M12 6.8A6 6 0 1 0 12 17.2M12 17.2A6 6 0 1 0 12 6.8"/><path d="M12 6.8A6 6 0 1 0 12 17.2A6 6 0 0 1 12 6.8Z" fill="currentColor" fill-opacity=".25" stroke="none"/><path d="M12 17.2A6 6 0 1 0 12 6.8A6 6 0 0 1 12 17.2Z" fill="currentColor" fill-opacity=".25" stroke="none"/><path d="M12 6.8A6 6 0 0 1 12 17.2A6 6 0 0 1 12 6.8Z" stroke-dasharray="1.5 2"/>"##),
    // Gradient stops
    ("stop-add", r##"<path d="M9.5 3.5h5A2.5 2.5 0 0 1 17 6v5a2.5 2.5 0 0 1-2.5 2.5H14l-2 2.5-2-2.5h-.5A2.5 2.5 0 0 1 7 11V6a2.5 2.5 0 0 1 2.5-2.5z"/><path d="M12 6v5M9.5 8.5h5M4 20h16"/>"##),
    ("stop-remove", r##"<path d="M9.5 3.5h5A2.5 2.5 0 0 1 17 6v5a2.5 2.5 0 0 1-2.5 2.5H14l-2 2.5-2-2.5h-.5A2.5 2.5 0 0 1 7 11V6a2.5 2.5 0 0 1 2.5-2.5z"/><path d="M9.5 8.5h5M4 20h16"/>"##),
    ("smooth", r##"<path d="M4.5 18.5C11 18.5 13 5.5 19.5 5.5"/><circle cx="4.5" cy="18.5" r="1.5" fill="currentColor" stroke="none"/><circle cx="19.5" cy="5.5" r="1.5" fill="currentColor" stroke="none"/>"##),
    ("hash", r##"<path d="M10 4L8 20M16 4l-2 16M5 9h15M4 15h15"/>"##),
    // Stroke position
    ("stroke-inside", r##"<path d="M8 5.5H16A2.5 2.5 0 0 1 18.5 8V16A2.5 2.5 0 0 1 16 18.5H8A2.5 2.5 0 0 1 5.5 16V8A2.5 2.5 0 0 1 8 5.5ZM9.5 9A0.5 0.5 0 0 0 9 9.5V14.5A0.5 0.5 0 0 0 9.5 15H14.5A0.5 0.5 0 0 0 15 14.5V9.5A0.5 0.5 0 0 0 14.5 9Z" fill="currentColor" fill-opacity=".45" stroke="none"/><rect x="5.5" y="5.5" width="13" height="13" rx="2.5" stroke-width="1"/>"##),
    ("stroke-center", r##"<path d="M7.75 3.75H16.25A4 4 0 0 1 20.25 7.75V16.25A4 4 0 0 1 16.25 20.25H7.75A4 4 0 0 1 3.75 16.25V7.75A4 4 0 0 1 7.75 3.75ZM8.25 7.25A1 1 0 0 0 7.25 8.25V15.75A1 1 0 0 0 8.25 16.75H15.75A1 1 0 0 0 16.75 15.75V8.25A1 1 0 0 0 15.75 7.25Z" fill="currentColor" fill-opacity=".45" stroke="none"/><rect x="5.5" y="5.5" width="13" height="13" rx="2.5" stroke-width="1"/>"##),
    ("stroke-outside", r##"<path d="M7.5 2H16.5A5.5 5.5 0 0 1 22 7.5V16.5A5.5 5.5 0 0 1 16.5 22H7.5A5.5 5.5 0 0 1 2 16.5V7.5A5.5 5.5 0 0 1 7.5 2ZM8 5.5A2.5 2.5 0 0 0 5.5 8V16A2.5 2.5 0 0 0 8 18.5H16A2.5 2.5 0 0 0 18.5 16V8A2.5 2.5 0 0 0 16 5.5Z" fill="currentColor" fill-opacity=".45" stroke="none"/><rect x="5.5" y="5.5" width="13" height="13" rx="2.5" stroke-width="1"/>"##),
    // Path points
    ("point-mirror-none", r##"<path d="M10.21 12.22L6.22 9.37"/><circle cx="5" cy="8.5" r="1.5"/><path d="M13.44 11.84L17.52 7.13"/><circle cx="18.5" cy="6" r="1.5"/><rect x="10.25" y="11.75" width="3.5" height="3.5" rx="0.75" fill="currentColor" stroke="none"/>"##),
    ("point-mirror-angle", r##"<path d="M10.11 13.13L5.79 15.73"/><circle cx="4.5" cy="16.5" r="1.5"/><path d="M13.89 10.87L15.42 9.94"/><circle cx="16.71" cy="9.17" r="1.5"/><rect x="10.25" y="10.25" width="3.5" height="3.5" rx="0.75" fill="currentColor" stroke="none"/>"##),
    ("point-mirror-full", r##"<path d="M10.11 13.13L5.79 15.73"/><circle cx="4.5" cy="16.5" r="1.5"/><path d="M13.89 10.87L18.21 8.27"/><circle cx="19.5" cy="7.5" r="1.5"/><rect x="10.25" y="10.25" width="3.5" height="3.5" rx="0.75" fill="currentColor" stroke="none"/>"##),
    ("point-corner", r##"<path d="M4 18.5L12 7.5l8 11"/><rect x="10.25" y="5.75" width="3.5" height="3.5" rx="0.75" fill="currentColor" stroke="none"/>"##),
    ("point-smooth", r##"<path d="M3.5 18C7 18 8 8.5 12 8.5s5 9.5 8.5 9.5"/><path d="M9.6 8.5L7.1 8.5"/><circle cx="5.5" cy="8.5" r="1.6"/><path d="M14.4 8.5L16.9 8.5"/><circle cx="18.5" cy="8.5" r="1.6"/><rect x="10.25" y="6.75" width="3.5" height="3.5" rx="0.75" fill="currentColor" stroke="none"/>"##),
    // Line ends
    ("cap-round", r##"<path d="M3 8h11a4 4 0 0 1 0 8H3" fill="currentColor" fill-opacity=".25"/><path d="M3 12h11" stroke-width="1"/><circle cx="14" cy="12" r="1.25" fill="currentColor" stroke="none"/>"##),
    ("cap-square", r##"<path d="M3 8h15v8H3" fill="currentColor" fill-opacity=".25"/><path d="M3 12h11" stroke-width="1"/><circle cx="14" cy="12" r="1.25" fill="currentColor" stroke="none"/>"##),
    ("tip-none", r##"<path d="M4 12h16"/>"##),
    ("tip-arrow", r##"<path d="M4 12h15.5M15 7.5l4.5 4.5-4.5 4.5"/>"##),
    ("tip-triangle", r##"<path d="M4 12h10"/><path d="M14 7.5l6 4.5-6 4.5z" fill="currentColor" fill-opacity=".25"/>"##),
    ("tip-circle", r##"<path d="M4 12h10"/><circle cx="17" cy="12" r="3" fill="currentColor" fill-opacity=".25"/>"##),
    ("tip-bar", r##"<path d="M4 12h15M19 7v10"/>"##),
    ("flip-horizontal", r##"<path d="M12 3v18" stroke-dasharray="2 2.75"/><path d="M9 6.5L3 17.5h6z"/><path d="M15 6.5l6 11h-6z" fill="currentColor" fill-opacity=".25"/>"##),
    ("flip-vertical", r##"<path d="M3 12h18" stroke-dasharray="2 2.75"/><path d="M6.5 9L17.5 3v6z"/><path d="M6.5 15l11 6v-6z" fill="currentColor" fill-opacity=".25"/>"##),
    ("aspect-ratio", r##"<rect x="3" y="6" width="18" height="12" rx="2"/><path d="M7 12.5V9.5h3M17 11.5v3h-3"/>"##),
    ("restore-cuts", r##"<rect x="3.5" y="3.5" width="17" height="17" rx="2.5" stroke-dasharray="3 2.6"/><path d="M8.6 12a3.6 3.6 0 1 0 1.05-2.55M9.4 7v2.7h2.7"/>"##),
    ("adjustments", r##"<path d="M4 6h9M17 6h3M4 12h3M11 12h9M4 18h11M19 18h1"/><circle cx="15" cy="6" r="2"/><circle cx="9" cy="12" r="2"/><circle cx="17" cy="18" r="2"/>"##),
    ("brightness", r##"<circle cx="12" cy="12" r="3.5"/><path d="M12 2.5v2M12 19.5v2M2.5 12h2M19.5 12h2M5.3 5.3l1.4 1.4M17.3 17.3l1.4 1.4M5.3 18.7l1.4-1.4M17.3 6.7l1.4-1.4"/>"##),
    ("contrast", r##"<circle cx="12" cy="12" r="8.5"/><path d="M12 3.5a8.5 8.5 0 0 1 0 17z" fill="currentColor" stroke="none"/>"##),
    ("saturation", r##"<path d="M12 3.5c-3 3.5-6 7-6 10.5a6 6 0 0 0 6 6z" fill="currentColor" stroke="none"/><path d="M12 3.5c-3 3.5-6 7-6 10.5a6 6 0 0 0 12 0c0-3.5-3-7-6-10.5z"/>"##),
    ("hue", r##"<circle cx="12" cy="12" r="8.5"/><circle cx="12" cy="12" r="3"/><path d="M12 9L12 3.5M14.6 10.5L19.36 7.75M14.6 13.5L19.36 16.25M12 15L12 20.5M9.4 13.5L4.64 16.25M9.4 10.5L4.64 7.75"/>"##),
    ("exposure", r##"<rect x="4" y="4" width="16" height="16" rx="2"/><path d="M5 19L19 5M7 8.5h3M8.5 7v3M14 15.5h3"/>"##),
    ("temperature", r##"<path d="M10 14.5V5a2 2 0 1 1 4 0v9.5a4 4 0 1 1-4 0zM12 16.5V9"/><circle cx="12" cy="18" r="1.6" fill="currentColor" stroke="none"/>"##),
    ("opacity", r##"<rect x="12" y="5" width="2.5" height="2.5" rx="0" fill="currentColor" stroke="none"/><rect x="14.5" y="7.5" width="2.5" height="2.5" rx="0" fill="currentColor" stroke="none"/><rect x="12" y="10" width="2.5" height="2.5" rx="0" fill="currentColor" stroke="none"/><rect x="17" y="10" width="2.5" height="2.5" rx="0" fill="currentColor" stroke="none"/><rect x="14.5" y="12.5" width="2.5" height="2.5" rx="0" fill="currentColor" stroke="none"/><rect x="12" y="15" width="2.5" height="2.5" rx="0" fill="currentColor" stroke="none"/><circle cx="12" cy="12" r="8.5"/><path d="M12 3.5v17"/>"##),
    ("vignette", r##"<rect x="3" y="5" width="18" height="14" rx="2" fill="currentColor" fill-opacity=".25"/><ellipse cx="12" cy="12" rx="6" ry="4" fill="var(--icon-bg, #fff)"/>"##),
    ("curves", r##"<rect x="4" y="4" width="16" height="16" rx="2"/><path d="M7 17c5 0 5-10 10-10"/><circle cx="7" cy="17" r="1.25" fill="currentColor" stroke="none"/><circle cx="17" cy="7" r="1.25" fill="currentColor" stroke="none"/>"##),
    ("effects", r##"<circle cx="12" cy="8.5" r="5"/><circle cx="8.5" cy="14.5" r="5"/><circle cx="15.5" cy="14.5" r="5"/>"##),
    ("noise", r##"<rect x="4" y="4" width="16" height="16" rx="2"/><circle cx="8" cy="8" r="1" fill="currentColor" stroke="none"/><circle cx="12.5" cy="7.5" r="1" fill="currentColor" stroke="none"/><circle cx="16" cy="9" r="1" fill="currentColor" stroke="none"/><circle cx="9.5" cy="12" r="1" fill="currentColor" stroke="none"/><circle cx="14" cy="12.5" r="1" fill="currentColor" stroke="none"/><circle cx="7.5" cy="16" r="1" fill="currentColor" stroke="none"/><circle cx="12" cy="16.5" r="1" fill="currentColor" stroke="none"/><circle cx="16.5" cy="15.5" r="1" fill="currentColor" stroke="none"/>"##),
    ("gradient", r##"<rect x="10" y="4.9" width="5" height="14.2" rx="0" fill="currentColor" fill-opacity=".2" stroke="none"/><rect x="15" y="4.9" width="4.1" height="14.2" rx="1.1" fill="currentColor" fill-opacity=".5" stroke="none"/><rect x="4" y="4" width="16" height="16" rx="2"/>"##),
    ("palette", r##"<path d="M12 3a9 9 0 0 0 0 18c1.1 0 1.6-.8 1.6-1.6 0-.5-.2-.8-.5-1.2-.3-.3-.5-.7-.5-1.2 0-.9.7-1.6 1.6-1.6H16a5 5 0 0 0 5-5c0-4-4-7.4-9-7.4z"/><circle cx="7.5" cy="11" r="1.25" fill="currentColor" stroke="none"/><circle cx="9.5" cy="7.5" r="1.25" fill="currentColor" stroke="none"/><circle cx="14" cy="7" r="1.25" fill="currentColor" stroke="none"/><circle cx="17" cy="10" r="1.25" fill="currentColor" stroke="none"/>"##),
    ("image", r##"<rect x="3" y="4" width="18" height="16" rx="2"/><circle cx="8.5" cy="9.5" r="1.5"/><path d="M21 15.5l-5-5L5 20"/>"##),
    ("mask", r##"<rect x="3" y="5" width="18" height="14" rx="2"/><circle cx="12" cy="12" r="4" fill="currentColor" fill-opacity=".25"/>"##),
    // Figma's: just the four rounded corners of a square
    ("corner-radius", r##"<path d="M4 9.5V8a4 4 0 0 1 4-4h1.5M14.5 4H16a4 4 0 0 1 4 4v1.5M20 14.5V16a4 4 0 0 1-4 4h-1.5M9.5 20H8a4 4 0 0 1-4-4v-1.5"/>"##),
    ("stroke-weight", r##"<path d="M4 6h16" stroke-width="1"/><path d="M4 12h16" stroke-width="2"/><path d="M4 18h16" stroke-width="3.25"/>"##),
    ("line-dashed", r##"<path d="M4 12h16" stroke-dasharray="3 4"/>"##),
    ("layers", r##"<path d="M12 3.5L20.5 8 12 12.5 3.5 8z" fill="currentColor" fill-opacity=".25"/><path d="M3.5 12l8.5 4.5 8.5-4.5M3.5 16l8.5 4.5 8.5-4.5"/>"##),
    ("bring-forward", r##"<rect x="4" y="13" width="16" height="7" rx="2"/><path d="M12 10V3M8.5 6.5L12 3l3.5 3.5"/>"##),
    ("send-backward", r##"<rect x="4" y="4" width="16" height="7" rx="2"/><path d="M12 14v7M8.5 17.5L12 21l3.5-3.5"/>"##),
    ("group", r##"<rect x="3" y="3" width="18" height="18" rx="2" stroke-dasharray="2 3"/><rect x="7" y="7" width="5.5" height="5.5" rx="1"/><rect x="11.5" y="11.5" width="5.5" height="5.5" rx="1" fill="currentColor" fill-opacity=".25"/>"##),
    ("ungroup", r##"<path d="M3 7.5V5a2 2 0 0 1 2-2h2.5M16.5 3H19a2 2 0 0 1 2 2v2.5M21 16.5V19a2 2 0 0 1-2 2h-2.5M7.5 21H5a2 2 0 0 1-2-2v-2.5"/><rect x="7" y="7" width="4.5" height="4.5" rx="1"/><rect x="12.5" y="12.5" width="4.5" height="4.5" rx="1" fill="currentColor" fill-opacity=".25"/>"##),
    ("component", r##"<path d="M12 2.5l3.25 3.25-3.25 3.25-3.25-3.25z"/><path d="M5.75 8.75l3.25 3.25-3.25 3.25-3.25-3.25z"/><path d="M18.25 8.75l3.25 3.25-3.25 3.25-3.25-3.25z"/><path d="M12 15.0l3.25 3.25-3.25 3.25-3.25-3.25z"/>"##),
    ("bool-union", r##"<path d="M4 6a2 2 0 0 1 2-2h7a2 2 0 0 1 2 2v3h3a2 2 0 0 1 2 2v7a2 2 0 0 1-2 2h-7a2 2 0 0 1-2-2v-3H6a2 2 0 0 1-2-2z" fill="currentColor" fill-opacity=".25"/>"##),
    ("bool-subtract", r##"<path d="M15 9V6a2 2 0 0 0-2-2H6a2 2 0 0 0-2 2v7a2 2 0 0 0 2 2h3v-4a2 2 0 0 1 2-2z" fill="currentColor" fill-opacity=".25"/><rect x="9" y="9" width="11" height="11" rx="2" stroke-dasharray="2 2.75"/>"##),
    ("bool-intersect", r##"<rect x="9" y="9" width="6" height="6" rx="0" fill="currentColor" fill-opacity=".25" stroke="none"/><rect x="4" y="4" width="11" height="11" rx="2" stroke-dasharray="2 2.75"/><rect x="9" y="9" width="11" height="11" rx="2" stroke-dasharray="2 2.75"/><path d="M9 15V9h6v6z"/>"##),
    ("bool-exclude", r##"<path d="M4 6a2 2 0 0 1 2-2h7a2 2 0 0 1 2 2v3h3a2 2 0 0 1 2 2v7a2 2 0 0 1-2 2h-7a2 2 0 0 1-2-2v-3H6a2 2 0 0 1-2-2zM9 9h6v6H9z" fill="currentColor" fill-opacity=".25" fill-rule="evenodd"/>"##),
    ("lock", r##"<rect x="5" y="11" width="14" height="10" rx="2"/><path d="M8 11V7.5a4 4 0 0 1 8 0V11M12 15v2"/>"##),
    ("unlock", r##"<rect x="5" y="11" width="14" height="10" rx="2"/><path d="M8 11V7.5a4 4 0 0 1 7.75-1.4M12 15v2"/>"##),
    ("eye", r##"<path d="M2.5 12S6 5 12 5s9.5 7 9.5 7-3.5 7-9.5 7-9.5-7-9.5-7z"/><circle cx="12" cy="12" r="3"/>"##),
    ("eye-off", r##"<path d="M2.5 12S6 5 12 5s9.5 7 9.5 7-3.5 7-9.5 7-9.5-7-9.5-7z"/><circle cx="12" cy="12" r="3"/><path d="M4 4l16 16"/>"##),
    ("align-left", r##"<path d="M4 3v18"/><rect x="8" y="6" width="12" height="4" rx="1.5"/><rect x="8" y="14" width="7" height="4" rx="1.5"/>"##),
    ("align-center-h", r##"<path d="M12 3v18"/><rect x="5" y="6" width="14" height="4" rx="1.5"/><rect x="7.5" y="14" width="9" height="4" rx="1.5"/>"##),
    ("align-right", r##"<path d="M20 3v18"/><rect x="4" y="6" width="12" height="4" rx="1.5"/><rect x="9" y="14" width="7" height="4" rx="1.5"/>"##),
    ("align-top", r##"<path d="M3 4h18"/><rect x="6" y="8" width="4" height="12" rx="1.5"/><rect x="14" y="8" width="4" height="7" rx="1.5"/>"##),
    ("align-center-v", r##"<path d="M3 12h18"/><rect x="6" y="5" width="4" height="14" rx="1.5"/><rect x="14" y="7.5" width="4" height="9" rx="1.5"/>"##),
    ("align-bottom", r##"<path d="M3 20h18"/><rect x="6" y="4" width="4" height="12" rx="1.5"/><rect x="14" y="9" width="4" height="7" rx="1.5"/>"##),
    ("distribute-h", r##"<path d="M4 3v18M20 3v18"/><rect x="9" y="7" width="6" height="10" rx="1.5"/>"##),
    ("distribute-v", r##"<path d="M3 4h18M3 20h18"/><rect x="7" y="9" width="10" height="6" rx="1.5"/>"##),
    ("font-size", r##"<path d="M3 7V5h10v2M8 5v14M6.5 19h3M14 12v-1.5h7V12M17.5 10.5V19M16.5 19h2"/>"##),
    ("bold", r##"<path d="M7 5h6a3.5 3.5 0 0 1 0 7H7zM7 12h7a3.5 3.5 0 0 1 0 7H7z"/>"##),
    ("italic", r##"<path d="M10 5h8M6 19h8M14 5l-4 14"/>"##),
    ("underline", r##"<path d="M7 4v7a5 5 0 0 0 10 0V4M5 20h14"/>"##),
    ("strikethrough", r##"<path d="M16.5 7.5C16 5.5 14.2 4.5 12 4.5c-2.8 0-4.5 1.5-4.5 3.5 0 1.3.7 2.3 2 3M4 12h16M15.5 15c.6.6.9 1.3.9 2 0 2-1.9 3.5-4.6 3.5-2.4 0-4.3-1.1-4.8-3"/>"##),
    ("text-align-left", r##"<path d="M4 6h16M4 10h10M4 14h16M4 18h10"/>"##),
    ("text-align-center", r##"<path d="M4 6h16M7 10h10M4 14h16M7 18h10"/>"##),
    ("text-align-right", r##"<path d="M4 6h16M10 10h10M4 14h16M10 18h10"/>"##),
    ("text-align-justify", r##"<path d="M4 6h16M4 10h16M4 14h16M4 18h16"/>"##),
    ("line-height", r##"<path d="M11 6h9M11 12h9M11 18h9M5 4v16M3 6l2-2 2 2M3 18l2 2 2-2"/>"##),
    ("letter-spacing", r##"<path d="M3 4v16M21 4v16M8.5 16L12 7l3.5 9M9.7 13h4.6"/>"##),
    ("search", r##"<circle cx="11" cy="11" r="6.5"/><path d="M16 16l4.5 4.5"/>"##),
    ("chevron-down", r##"<path d="M6 9l6 6 6-6"/>"##),
    ("chevron-up", r##"<path d="M6 15l6-6 6 6"/>"##),
    ("chevron-left", r##"<path d="M15 6l-6 6 6 6"/>"##),
    ("chevron-right", r##"<path d="M9 6l6 6-6 6"/>"##),
    ("chevrons-up-down", r##"<path d="M8 9l4-4 4 4M8 15l4 4 4-4"/>"##),
    ("caret-down", r##"<path d="M7.5 10h9L12 15z" fill="currentColor"/>"##),
    ("caret-right", r##"<path d="M10 7.5v9l5-4.5z" fill="currentColor"/>"##),
    ("undo", r##"<path d="M9 14L4 9l5-5M4 9h10.5a5.5 5.5 0 0 1 0 11H11"/>"##),
    ("redo", r##"<path d="M15 14l5-5-5-5M20 9H9.5a5.5 5.5 0 0 0 0 11H13"/>"##),
    ("history", r##"<path d="M4 12a8 8 0 1 0 2.34-5.66L4 8.5M4 4v4.5h4.5M12 8v4l3 2"/>"##),
    ("plus", r##"<path d="M12 5v14M5 12h14"/>"##),
    ("minus", r##"<path d="M5 12h14"/>"##),
    ("close", r##"<path d="M6 6l12 12M18 6L6 18"/>"##),
    ("check", r##"<path d="M5 12.5l4.5 4.5L19 7"/>"##),
    ("more-horizontal", r##"<circle cx="5.5" cy="12" r="1.25" fill="currentColor" stroke="none"/><circle cx="12" cy="12" r="1.25" fill="currentColor" stroke="none"/><circle cx="18.5" cy="12" r="1.25" fill="currentColor" stroke="none"/>"##),
    ("more-vertical", r##"<circle cx="12" cy="5.5" r="1.25" fill="currentColor" stroke="none"/><circle cx="12" cy="12" r="1.25" fill="currentColor" stroke="none"/><circle cx="12" cy="18.5" r="1.25" fill="currentColor" stroke="none"/>"##),
    ("menu", r##"<path d="M4 6h16M4 12h16M4 18h16"/>"##),
    ("settings", r##"<path d="M18.38 10.29L20.9 10.67L20.9 13.33L18.38 13.71L17.72 15.3L19.23 17.35L17.35 19.23L15.3 17.72L13.71 18.38L13.33 20.9L10.67 20.9L10.29 18.38L8.7 17.72L6.65 19.23L4.77 17.35L6.28 15.3L5.62 13.71L3.1 13.33L3.1 10.67L5.62 10.29L6.28 8.7L4.77 6.65L6.65 4.77L8.7 6.28L10.29 5.62L10.67 3.1L13.33 3.1L13.71 5.62L15.3 6.28L17.35 4.77L19.23 6.65L17.72 8.7z"/><circle cx="12" cy="12" r="3"/>"##),
    ("filter", r##"<path d="M4 5h16l-6 7.5V19l-4 2v-8.5z"/>"##),
    ("grid", r##"<rect x="4" y="4" width="16" height="16" rx="2"/><path d="M4 9.33h16M4 14.67h16M9.33 4v16M14.67 4v16"/>"##),
    ("panel-left", r##"<rect x="3" y="4" width="18" height="16" rx="2"/><path d="M9 4v16M15 10l-2 2 2 2"/>"##),
    ("panel-right", r##"<rect x="3" y="4" width="18" height="16" rx="2"/><path d="M15 4v16M9 10l2 2-2 2"/>"##),
    ("maximize", r##"<path d="M14 4h6v6M20 4l-6 6M10 20H4v-6M4 20l6-6"/>"##),
    ("minimize", r##"<path d="M4 14h6v6M10 14l-6 6M20 10h-6V4M14 10l6-6"/>"##),
    ("fit-screen", r##"<path d="M4 9V5a1 1 0 0 1 1-1h4M15 4h4a1 1 0 0 1 1 1v4M20 15v4a1 1 0 0 1-1 1h-4M9 20H5a1 1 0 0 1-1-1v-4"/><rect x="8" y="8" width="8" height="8" rx="1.5"/>"##),
    ("copy", r##"<rect x="8" y="8" width="12" height="12" rx="2"/><path d="M16 8V6a2 2 0 0 0-2-2H6a2 2 0 0 0-2 2v8a2 2 0 0 0 2 2h2"/>"##),
    ("trash", r##"<path d="M4 7h16M9 7V4.5h6V7M6 7l1 12.5A1.5 1.5 0 0 0 8.5 21h7a1.5 1.5 0 0 0 1.5-1.5L18 7M10 11v6M14 11v6"/>"##),
    ("download", r##"<path d="M12 4v11M7 10.5l5 5 5-5M4 20h16"/>"##),
    ("upload", r##"<path d="M12 15.5V4M7 9l5-5 5 5M4 20h16"/>"##),
    ("share", r##"<circle cx="18" cy="5.5" r="2.5"/><circle cx="6" cy="12" r="2.5"/><circle cx="18" cy="18.5" r="2.5"/><path d="M8.2 10.8l7.6-4.1M8.2 13.2l7.6 4.1"/>"##),
    ("link", r##"<path d="M10 14a4.5 4.5 0 0 0 6.4 0l3-3a4.5 4.5 0 0 0-6.4-6.4l-1 1M14 10a4.5 4.5 0 0 0-6.4 0l-3 3a4.5 4.5 0 0 0 6.4 6.4l1-1"/>"##),
    ("play", r##"<path d="M7 4.5v15a1 1 0 0 0 1.5.9l12-7.5a1 1 0 0 0 0-1.8l-12-7.5A1 1 0 0 0 7 4.5z"/>"##),
    ("pin", r##"<path d="M12 21s-6.5-5.8-6.5-11a6.5 6.5 0 0 1 13 0c0 5.2-6.5 11-6.5 11z"/><circle cx="12" cy="10" r="2.5"/>"##),
    ("info", r##"<circle cx="12" cy="12" r="9"/><path d="M12 11v5.5"/><circle cx="12" cy="7.75" r="1.1" fill="currentColor" stroke="none"/>"##),
    ("help", r##"<circle cx="12" cy="12" r="9"/><path d="M9.5 9.5a2.5 2.5 0 1 1 3.5 2.3c-.6.3-1 .9-1 1.6v.6"/><circle cx="12" cy="17" r="1.1" fill="currentColor" stroke="none"/>"##),
];
