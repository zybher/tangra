//! Offscreen layers on the GPU. egui never changes the framebuffer between the shapes it
//! draws, so a paint callback placed before some shapes can point the drawing at a texture of
//! our own, and one placed after them can put things back:
//! - blend modes: an object (a frame: with everything in it) is drawn on its own clear layer,
//!   then mixed with what's under it by a small shader (`begin_blend` / `end_blend`);
//! - copy as picture: the selection is drawn on a clear layer the size of the screen and read
//!   back as pixels (`begin_capture` / `end_capture`, `take_capture`).
//! Layers nest (a blended object inside a copied selection). If the shader can't be built,
//! nothing is redirected and everything simply draws as Normal.

use crate::*;
use eframe::egui_glow;
use eframe::glow::{self, HasContext};
use std::sync::Mutex;

/// Blend modes in menu order (the number stored in `board.blend`)
pub(crate) const BLENDS: [&str; 12] = [
    "Normal", "Multiply", "Screen", "Overlay", "Darken", "Lighten", "Color dodge", "Color burn", "Hard light", "Soft light", "Difference", "Exclusion",
];

#[derive(Clone, Copy)]
struct Target {
    fbo: glow::Framebuffer,
    tex: glow::Texture,
    size: [i32; 2],
}

struct Gl {
    /// None: not tried yet; Some(false): this GPU can't, draw everything as Normal
    ok: Option<bool>,
    prog: Option<glow::Program>,
    vao: Option<glow::VertexArray>,
    /// layer targets, one per nesting depth
    pool: Vec<Target>,
    /// what's under a blended layer, copied out for the shader
    back: Option<Target>,
    capture: Option<Target>,
    /// targets drawn into right now, innermost last (None entry: never used)
    stack: Vec<Target>,
}

static GL: Mutex<Gl> = Mutex::new(Gl { ok: None, prog: None, vao: None, pool: Vec::new(), back: None, capture: None, stack: Vec::new() });
/// The last capture read back: width, height, straight (not premultiplied) RGBA, top row first
static CAPTURED: Mutex<Option<(usize, usize, Vec<u8>)>> = Mutex::new(None);

const VS: &str = r#"
void main() {
    vec2 p = vec2(float((gl_VertexID & 1) << 2) - 1.0, float((gl_VertexID & 2) << 1) - 1.0);
    gl_Position = vec4(p, 0.0, 1.0);
}
"#;

// Colours are premultiplied, in the same gamma space egui draws in. W3C compositing:
// result = (1 - as) * Cb + as * ((1 - ab) * Cs + ab * B(cb, cs))
const FS: &str = r#"
uniform sampler2D u_layer;
uniform sampler2D u_back;
uniform int u_mode;
out vec4 o_color;
float scr(float b, float s) { return b + s - b * s; }
float hard(float b, float s) { return s <= 0.5 ? b * 2.0 * s : scr(b, 2.0 * s - 1.0); }
float soft(float b, float s) {
    if (s <= 0.5) return b - (1.0 - 2.0 * s) * b * (1.0 - b);
    float d = b <= 0.25 ? ((16.0 * b - 12.0) * b + 4.0) * b : sqrt(b);
    return b + (2.0 * s - 1.0) * (d - b);
}
float ch(float b, float s) {
    if (u_mode == 1) return b * s;
    if (u_mode == 2) return scr(b, s);
    if (u_mode == 3) return hard(s, b);
    if (u_mode == 4) return min(b, s);
    if (u_mode == 5) return max(b, s);
    if (u_mode == 6) return b <= 0.0 ? 0.0 : (s >= 1.0 ? 1.0 : min(1.0, b / (1.0 - s)));
    if (u_mode == 7) return b >= 1.0 ? 1.0 : (s <= 0.0 ? 0.0 : 1.0 - min(1.0, (1.0 - b) / s));
    if (u_mode == 8) return hard(b, s);
    if (u_mode == 9) return soft(b, s);
    if (u_mode == 10) return abs(b - s);
    if (u_mode == 11) return b + s - 2.0 * b * s;
    return s;
}
void main() {
    ivec2 c = ivec2(gl_FragCoord.xy);
    vec4 L = texelFetch(u_layer, c, 0);
    vec4 B = texelFetch(u_back, c, 0);
    vec3 cs = L.a > 0.0 ? L.rgb / L.a : vec3(0.0);
    vec3 cb = B.a > 0.0 ? B.rgb / B.a : vec3(0.0);
    vec3 mixed = vec3(ch(cb.r, cs.r), ch(cb.g, cs.g), ch(cb.b, cs.b));
    vec3 inner = (1.0 - B.a) * cs + B.a * clamp(mixed, 0.0, 1.0);
    o_color = vec4(L.a * inner + (1.0 - L.a) * B.rgb, L.a + B.a * (1.0 - L.a));
}
"#;

unsafe fn shader(gl: &glow::Context, kind: u32, src: &str) -> Option<glow::Shader> {
    let s = gl.create_shader(kind).ok()?;
    gl.shader_source(s, src);
    gl.compile_shader(s);
    if gl.get_shader_compile_status(s) {
        Some(s)
    } else {
        eprintln!("layers: shader: {}", gl.get_shader_info_log(s));
        gl.delete_shader(s);
        None
    }
}

unsafe fn setup(g: &mut Gl, gl: &glow::Context) -> bool {
    if let Some(ok) = g.ok {
        return ok;
    }
    g.ok = Some(false);
    let ver = egui_glow::ShaderVersion::get(gl);
    if !ver.is_new_shader_interface() {
        return false;
    }
    let head = format!("{}\nprecision highp float;\nprecision highp int;\n", ver.version_declaration());
    let (Some(vs), Some(fs)) = (shader(gl, glow::VERTEX_SHADER, &format!("{head}{VS}")), shader(gl, glow::FRAGMENT_SHADER, &format!("{head}{FS}"))) else {
        return false;
    };
    let Ok(prog) = gl.create_program() else { return false };
    gl.attach_shader(prog, vs);
    gl.attach_shader(prog, fs);
    gl.link_program(prog);
    gl.detach_shader(prog, vs);
    gl.detach_shader(prog, fs);
    gl.delete_shader(vs);
    gl.delete_shader(fs);
    if !gl.get_program_link_status(prog) {
        eprintln!("layers: link: {}", gl.get_program_info_log(prog));
        return false;
    }
    let Ok(vao) = gl.create_vertex_array() else { return false };
    g.prog = Some(prog);
    g.vao = Some(vao);
    g.ok = Some(true);
    true
}

/// A clear-able texture of `size` we can draw into (remade when the window size changes)
unsafe fn target(gl: &glow::Context, slot: &mut Option<Target>, size: [i32; 2]) -> Option<Target> {
    if let Some(t) = slot.filter(|t| t.size == size) {
        return Some(t);
    }
    if let Some(t) = slot.take() {
        gl.delete_framebuffer(t.fbo);
        gl.delete_texture(t.tex);
    }
    let tex = gl.create_texture().ok()?;
    gl.bind_texture(glow::TEXTURE_2D, Some(tex));
    gl.tex_image_2d(glow::TEXTURE_2D, 0, glow::RGBA8 as i32, size[0], size[1], 0, glow::RGBA, glow::UNSIGNED_BYTE, None);
    for (k, v) in [
        (glow::TEXTURE_MIN_FILTER, glow::NEAREST),
        (glow::TEXTURE_MAG_FILTER, glow::NEAREST),
        (glow::TEXTURE_WRAP_S, glow::CLAMP_TO_EDGE),
        (glow::TEXTURE_WRAP_T, glow::CLAMP_TO_EDGE),
    ] {
        gl.tex_parameter_i32(glow::TEXTURE_2D, k, v as i32);
    }
    let fbo = gl.create_framebuffer().ok()?;
    gl.bind_framebuffer(glow::FRAMEBUFFER, Some(fbo));
    gl.framebuffer_texture_2d(glow::FRAMEBUFFER, glow::COLOR_ATTACHMENT0, glow::TEXTURE_2D, Some(tex), 0);
    if gl.check_framebuffer_status(glow::FRAMEBUFFER) != glow::FRAMEBUFFER_COMPLETE {
        gl.delete_framebuffer(fbo);
        gl.delete_texture(tex);
        return None;
    }
    let t = Target { fbo, tex, size };
    *slot = Some(t);
    Some(t)
}

/// What drawing goes to once the innermost layer ends
fn parent(g: &Gl, painter: &egui_glow::Painter) -> Option<glow::Framebuffer> {
    g.stack.last().map(|t| t.fbo).or(painter.intermediate_fbo())
}

unsafe fn push(gl: &glow::Context, t: Target, g: &mut Gl) {
    gl.bind_framebuffer(glow::FRAMEBUFFER, Some(t.fbo));
    gl.disable(glow::SCISSOR_TEST);
    gl.clear_color(0.0, 0.0, 0.0, 0.0);
    gl.clear(glow::COLOR_BUFFER_BIT);
    g.stack.push(t);
}

fn add(p: &egui::Painter, f: impl Fn(egui::PaintCallbackInfo, &egui_glow::Painter) + Sync + Send + 'static) {
    p.add(egui::PaintCallback { rect: p.clip_rect(), callback: std::sync::Arc::new(egui_glow::CallbackFn::new(f)) });
}

/// What `p` paints from here until `end_blend` goes on its own clear layer
pub(crate) fn begin_blend(p: &egui::Painter) {
    add(p, |info, painter| unsafe {
        let gl = painter.gl();
        let Ok(mut g) = GL.lock() else { return };
        if !setup(&mut g, gl) {
            return;
        }
        let size = [info.screen_size_px[0] as i32, info.screen_size_px[1] as i32];
        let depth = g.stack.len();
        let mut slot = g.pool.get(depth).copied();
        let Some(t) = target(gl, &mut slot, size) else {
            g.ok = Some(false);
            return;
        };
        if depth < g.pool.len() {
            g.pool[depth] = t;
        } else {
            g.pool.push(t);
        }
        push(gl, t, &mut g);
    });
}

/// Mix the layer begun by `begin_blend` with what's under it (`mode`: index into BLENDS)
pub(crate) fn end_blend(p: &egui::Painter, mode: u8) {
    add(p, move |info, painter| unsafe {
        let gl = painter.gl();
        let Ok(mut g) = GL.lock() else { return };
        if g.ok != Some(true) {
            return;
        }
        let Some(layer) = g.stack.pop() else { return };
        let to = parent(&g, painter);
        gl.bind_framebuffer(glow::FRAMEBUFFER, to);
        let size = layer.size;
        let mut back = g.back;
        let Some(bk) = target(gl, &mut back, size) else { return };
        g.back = Some(bk);
        // `target` may have bound its own framebuffer: back to the parent
        gl.bind_framebuffer(glow::FRAMEBUFFER, to);
        let c = info.clip_rect_in_pixels();
        let (x, y, w, h) = (c.left_px.max(0), c.from_bottom_px.max(0), c.width_px.min(size[0]), c.height_px.min(size[1]));
        if w <= 0 || h <= 0 {
            return;
        }
        gl.bind_texture(glow::TEXTURE_2D, Some(bk.tex));
        gl.copy_tex_sub_image_2d(glow::TEXTURE_2D, 0, x, y, x, y, w, h);
        gl.viewport(0, 0, size[0], size[1]);
        gl.enable(glow::SCISSOR_TEST);
        gl.scissor(x, y, w, h);
        gl.disable(glow::BLEND);
        gl.use_program(g.prog);
        let prog = g.prog.unwrap();
        gl.uniform_1_i32(gl.get_uniform_location(prog, "u_layer").as_ref(), 0);
        gl.uniform_1_i32(gl.get_uniform_location(prog, "u_back").as_ref(), 1);
        gl.uniform_1_i32(gl.get_uniform_location(prog, "u_mode").as_ref(), mode as i32);
        gl.active_texture(glow::TEXTURE1);
        gl.bind_texture(glow::TEXTURE_2D, Some(bk.tex));
        gl.active_texture(glow::TEXTURE0);
        gl.bind_texture(glow::TEXTURE_2D, Some(layer.tex));
        gl.bind_vertex_array(g.vao);
        gl.draw_arrays(glow::TRIANGLES, 0, 3);
        gl.bind_vertex_array(None);
        gl.active_texture(glow::TEXTURE1);
        gl.bind_texture(glow::TEXTURE_2D, None);
        gl.active_texture(glow::TEXTURE0);
    });
}

/// What `p` paints from here until `end_capture` is drawn on a clear layer instead of the screen
pub(crate) fn begin_capture(p: &egui::Painter) {
    add(p, |info, painter| unsafe {
        let gl = painter.gl();
        let Ok(mut g) = GL.lock() else { return };
        if !setup(&mut g, gl) {
            return;
        }
        let size = [info.screen_size_px[0] as i32, info.screen_size_px[1] as i32];
        let mut slot = g.capture;
        let Some(t) = target(gl, &mut slot, size) else {
            g.ok = Some(false);
            return;
        };
        g.capture = Some(t);
        push(gl, t, &mut g);
    });
}

/// Read back the top-left `size` (points) of the capture layer; `take_capture` then has it
pub(crate) fn end_capture(p: &egui::Painter, size: egui::Vec2) {
    add(p, move |info, painter| unsafe {
        let gl = painter.gl();
        let Ok(mut g) = GL.lock() else { return };
        if g.ok != Some(true) {
            return;
        }
        let Some(t) = g.stack.pop() else { return };
        let ppp = info.pixels_per_point;
        let (w, h) = (((size.x * ppp).round() as i32).clamp(1, t.size[0]), ((size.y * ppp).round() as i32).clamp(1, t.size[1]));
        let mut buf = vec![0u8; (w * h * 4) as usize];
        gl.bind_framebuffer(glow::FRAMEBUFFER, Some(t.fbo));
        gl.pixel_store_i32(glow::PACK_ALIGNMENT, 1);
        gl.read_pixels(0, t.size[1] - h, w, h, glow::RGBA, glow::UNSIGNED_BYTE, glow::PixelPackData::Slice(&mut buf));
        gl.bind_framebuffer(glow::FRAMEBUFFER, parent(&g, painter));
        // bottom row first and premultiplied -> top row first, straight alpha
        let row = (w * 4) as usize;
        let mut out = vec![0u8; buf.len()];
        for y in 0..h as usize {
            let src = &buf[(h as usize - 1 - y) * row..][..row];
            let dst = &mut out[y * row..][..row];
            for (s, d) in src.chunks_exact(4).zip(dst.chunks_exact_mut(4)) {
                let a = s[3];
                if a == 0 {
                    continue;
                }
                let un = |c: u8| ((c as u32 * 255 + a as u32 / 2) / a as u32).min(255) as u8;
                d.copy_from_slice(&[un(s[0]), un(s[1]), un(s[2]), a]);
            }
        }
        if let Ok(mut c) = CAPTURED.lock() {
            *c = Some((w as usize, h as usize, out));
        }
    });
}

/// The finished capture, once (a frame after it was asked for)
pub(crate) fn take_capture() -> Option<(usize, usize, Vec<u8>)> {
    CAPTURED.lock().ok()?.take()
}

/// False once this GPU turned out unable to run the layer shader
pub(crate) fn available() -> bool {
    GL.lock().map_or(true, |g| g.ok != Some(false))
}
