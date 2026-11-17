//! Fill for a pen path of any shape (egui only fills convex ones): the outline is cut into
//! triangles by ear clipping. A path that crosses itself still fills, as best it can.

use crate::*;

pub(crate) fn fill(pts: &[Pos2], col: Color32) -> egui::Mesh {
    let mut mesh = egui::Mesh::default();
    let cross = |a: Pos2, b: Pos2, c: Pos2| (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x);
    // Without repeats and without points lying on a straight run (a straight segment is
    // flattened into many points in a row; they'd never make an ear)
    let mut v: Vec<Pos2> = Vec::with_capacity(pts.len());
    for &p in pts {
        if v.last().map_or(true, |l: &Pos2| l.distance(p) > 1e-3) {
            v.push(p);
        }
    }
    while v.len() > 1 && v[0].distance(v[v.len() - 1]) <= 1e-3 {
        v.pop();
    }
    let mut i = 0;
    while v.len() > 3 && i < v.len() {
        let n = v.len();
        let (a, b, c) = (v[(i + n - 1) % n], v[i], v[(i + 1) % n]);
        if cross(a, b, c).abs() < 1e-4 * (a.distance(b) + b.distance(c)).max(1e-3) {
            v.remove(i);
        } else {
            i += 1;
        }
    }
    let n = v.len();
    if n < 3 {
        return mesh;
    }
    for &p in &v {
        mesh.colored_vertex(p, col);
    }
    let area: f32 = (0..n).map(|i| cross(Pos2::ZERO, v[i], v[(i + 1) % n])).sum();
    let sg = area.signum();
    let mut idx: Vec<usize> = (0..n).collect();
    let mut i = 0;
    let mut miss = 0;
    while idx.len() > 3 {
        let m = idx.len();
        let (ia, ib, ic) = (idx[(i + m - 1) % m], idx[i % m], idx[(i + 1) % m]);
        let (a, b, c) = (v[ia], v[ib], v[ic]);
        let convex = cross(a, b, c) * sg > 0.0;
        // no other corner strictly inside this one
        let ear = convex
            && !idx.iter().any(|&j| {
                if j == ia || j == ib || j == ic {
                    return false;
                }
                let q = v[j];
                let (d1, d2, d3) = (cross(a, b, q) * sg, cross(b, c, q) * sg, cross(c, a, q) * sg);
                d1 > 0.0 && d2 > 0.0 && d3 > 0.0
            });
        if ear {
            mesh.add_triangle(ia as u32, ib as u32, ic as u32);
            idx.remove(i % m);
            miss = 0;
        } else {
            i += 1;
            miss += 1;
            // crosses itself: no ear left, fan out the rest
            if miss > m {
                for k in 1..m - 1 {
                    mesh.add_triangle(idx[0] as u32, idx[k] as u32, idx[k + 1] as u32);
                }
                return mesh;
            }
        }
        if i >= idx.len() {
            i = 0;
        }
    }
    mesh.add_triangle(idx[0] as u32, idx[1] as u32, idx[2] as u32);
    mesh
}
