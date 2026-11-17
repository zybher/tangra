//! Gradient fills drawn on the GPU, worked out for every pixel: the shape's own mesh (so its
//! smooth edge stays) carries where each corner sits in the gradient's space, and a small
//! shader turns that into the colour right at each pixel. Nothing is stretched from a small
//! picture, so no stair-steps at any size or zoom, and a faint dither hides 8-bit banding in
//! long, soft gradients. If the GPU can't build the shader, `canvas/gradient.rs` falls back to
//! its baked textures.

use crate::*;
use eframe::egui_glow;
use eframe::glow::{self, HasContext};
use std::sync::Mutex;

/// Colours sampled along the gradient (its stops, smoothing applied) for the shader
pub(crate) const RAMP: usize = 1024;

struct Gl {
    ok: Option<bool>,
    prog: Option<glow::Program>,
    vao: Option<glow::VertexArray>,
    vbo: Option<glow::Buffer>,
    ibo: Option<glow::Buffer>,
    ramp: Option<glow::Texture>,
}

static GL: Mutex<Gl> = Mutex::new(Gl { ok: None, prog: None, vao: None, vbo: None, ibo: None, ramp: None });

const VS: &str = r#"
uniform vec2 u_screen;
in vec2 a_pos;
in vec2 a_uv;
in float a_cov;
out vec2 v_uv;
out float v_cov;
void main() {
    v_uv = a_uv;
    v_cov = a_cov;
    gl_Position = vec4(a_pos.x / u_screen.x * 2.0 - 1.0, 1.0 - a_pos.y / u_screen.y * 2.0, 0.0, 1.0);
}
"#;

const FS: &str = r#"
uniform sampler2D u_ramp;
uniform int u_kind;
uniform float u_alpha;
in vec2 v_uv;
in float v_cov;
out vec4 o_color;
void main() {
    float t;
    if (u_kind == 0) t = v_uv.x;
    else if (u_kind == 1) t = length(v_uv);
    else if (u_kind == 2) t = fract(atan(v_uv.y, v_uv.x) / 6.28318530718);
    else t = abs(v_uv.x) + abs(v_uv.y);
    float n = float(textureSize(u_ramp, 0).x);
    vec4 c = texture(u_ramp, vec2((clamp(t, 0.0, 1.0) * (n - 1.0) + 0.5) / n, 0.5));
    float a = c.a * u_alpha * v_cov;
    // dither the final (premultiplied) colour by about one 8-bit step, so the steps between
    // neighbouring shades break up into fine grain instead of showing as lines
    float n1 = fract(52.9829189 * fract(dot(gl_FragCoord.xy, vec2(0.06711056, 0.00583715))));
    float n2 = fract(52.9829189 * fract(dot(gl_FragCoord.yx + 17.0, vec2(0.06711056, 0.00583715))));
    float d = (n1 + n2 - 1.0) / 255.0;
    o_color = clamp(vec4(c.rgb * a + d, a + d), 0.0, 1.0);
}
"#;

fn log(msg: &str) {
    let _ = std::fs::write(std::env::temp_dir().join("tangra_gradient_gl.log"), msg);
}

unsafe fn shader(gl: &glow::Context, kind: u32, src: &str) -> Option<glow::Shader> {
    let s = gl.create_shader(kind).ok()?;
    gl.shader_source(s, src);
    gl.compile_shader(s);
    if gl.get_shader_compile_status(s) {
        Some(s)
    } else {
        log(&format!("shader: {}", gl.get_shader_info_log(s)));
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
        log("old shader interface");
        return false;
    }
    let head = format!("{}\nprecision highp float;\nprecision highp int;\n", ver.version_declaration());
    let (Some(vs), Some(fs)) = (shader(gl, glow::VERTEX_SHADER, &format!("{head}{VS}")), shader(gl, glow::FRAGMENT_SHADER, &format!("{head}{FS}"))) else {
        return false;
    };
    let Ok(prog) = gl.create_program() else { return false };
    gl.attach_shader(prog, vs);
    gl.attach_shader(prog, fs);
    gl.bind_attrib_location(prog, 0, "a_pos");
    gl.bind_attrib_location(prog, 1, "a_uv");
    gl.bind_attrib_location(prog, 2, "a_cov");
    gl.link_program(prog);
    gl.detach_shader(prog, vs);
    gl.detach_shader(prog, fs);
    gl.delete_shader(vs);
    gl.delete_shader(fs);
    if !gl.get_program_link_status(prog) {
        log(&format!("link: {}", gl.get_program_info_log(prog)));
        return false;
    }
    let (Ok(vao), Ok(vbo), Ok(ibo), Ok(tex)) = (gl.create_vertex_array(), gl.create_buffer(), gl.create_buffer(), gl.create_texture()) else { return false };
    gl.bind_vertex_array(Some(vao));
    gl.bind_buffer(glow::ARRAY_BUFFER, Some(vbo));
    gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, Some(ibo));
    let stride = 5 * 4;
    gl.enable_vertex_attrib_array(0);
    gl.vertex_attrib_pointer_f32(0, 2, glow::FLOAT, false, stride, 0);
    gl.enable_vertex_attrib_array(1);
    gl.vertex_attrib_pointer_f32(1, 2, glow::FLOAT, false, stride, 8);
    gl.enable_vertex_attrib_array(2);
    gl.vertex_attrib_pointer_f32(2, 1, glow::FLOAT, false, stride, 16);
    gl.bind_vertex_array(None);
    gl.bind_buffer(glow::ARRAY_BUFFER, None);
    gl.bind_texture(glow::TEXTURE_2D, Some(tex));
    for (k, v) in [
        (glow::TEXTURE_MIN_FILTER, glow::LINEAR),
        (glow::TEXTURE_MAG_FILTER, glow::LINEAR),
        (glow::TEXTURE_WRAP_S, glow::CLAMP_TO_EDGE),
        (glow::TEXTURE_WRAP_T, glow::CLAMP_TO_EDGE),
    ] {
        gl.tex_parameter_i32(glow::TEXTURE_2D, k, v as i32);
    }
    (g.prog, g.vao, g.vbo, g.ibo, g.ramp) = (Some(prog), Some(vao), Some(vbo), Some(ibo), Some(tex));
    g.ok = Some(true);
    true
}

/// Draw one gradient-filled mesh. `data`: per corner x, y (pixels), u, v (gradient space),
/// edge coverage 0..1; `ramp`: RAMP straight RGBA colours along the gradient
pub(crate) fn draw(p: &egui::Painter, data: Vec<f32>, idx: Vec<u32>, ramp: Vec<u8>, kind: u8, alpha: f32) {
    if idx.is_empty() {
        return;
    }
    let cb = egui_glow::CallbackFn::new(move |info, painter| unsafe {
        let gl = painter.gl();
        let Ok(mut g) = GL.lock() else { return };
        if !setup(&mut g, gl) {
            return;
        }
        let prog = g.prog.unwrap();
        let [sw, sh] = info.screen_size_px;
        gl.viewport(0, 0, sw as i32, sh as i32);
        gl.enable(glow::BLEND);
        gl.blend_equation(glow::FUNC_ADD);
        gl.blend_func_separate(glow::ONE, glow::ONE_MINUS_SRC_ALPHA, glow::ONE_MINUS_DST_ALPHA, glow::ONE);
        gl.use_program(Some(prog));
        gl.uniform_2_f32(gl.get_uniform_location(prog, "u_screen").as_ref(), sw as f32, sh as f32);
        gl.uniform_1_i32(gl.get_uniform_location(prog, "u_kind").as_ref(), kind as i32);
        gl.uniform_1_f32(gl.get_uniform_location(prog, "u_alpha").as_ref(), alpha);
        gl.uniform_1_i32(gl.get_uniform_location(prog, "u_ramp").as_ref(), 0);
        gl.active_texture(glow::TEXTURE0);
        gl.bind_texture(glow::TEXTURE_2D, g.ramp);
        gl.pixel_store_i32(glow::UNPACK_ALIGNMENT, 1);
        gl.tex_image_2d(glow::TEXTURE_2D, 0, glow::RGBA8 as i32, RAMP as i32, 1, 0, glow::RGBA, glow::UNSIGNED_BYTE, Some(&ramp));
        gl.bind_vertex_array(g.vao);
        gl.bind_buffer(glow::ARRAY_BUFFER, g.vbo);
        gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, bytes_f32(&data), glow::STREAM_DRAW);
        gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, g.ibo);
        gl.buffer_data_u8_slice(glow::ELEMENT_ARRAY_BUFFER, bytes_u32(&idx), glow::STREAM_DRAW);
        gl.draw_elements(glow::TRIANGLES, idx.len() as i32, glow::UNSIGNED_INT, 0);
        gl.bind_vertex_array(None);
        gl.bind_buffer(glow::ARRAY_BUFFER, None);
    });
    p.add(egui::PaintCallback { rect: p.clip_rect(), callback: std::sync::Arc::new(cb) });
}

fn bytes_f32(v: &[f32]) -> &[u8] {
    // SAFETY: f32 has no padding; the slice covers exactly the same memory
    unsafe { std::slice::from_raw_parts(v.as_ptr() as *const u8, std::mem::size_of_val(v)) }
}

fn bytes_u32(v: &[u32]) -> &[u8] {
    // SAFETY: as above
    unsafe { std::slice::from_raw_parts(v.as_ptr() as *const u8, std::mem::size_of_val(v)) }
}

/// False once this GPU turned out unable to build the shader (baked textures are used then)
pub(crate) fn available() -> bool {
    GL.lock().map_or(true, |g| g.ok != Some(false))
}
