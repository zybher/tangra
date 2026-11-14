//! Windows glue: window show/hide, tray, global hotkeys, clipboard, PNG io, annotation baking.

use crate::model::{self, ImageNode, Pt};
use eframe::egui;
use global_hotkey::{GlobalHotKeyEvent, HotKeyState};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};

pub const TITLE: &str = "Tangra";

#[link(name = "user32")]
extern "system" {
    fn FindWindowW(class: *const u16, title: *const u16) -> isize;
    fn ShowWindow(hwnd: isize, cmd: i32) -> i32;
    fn GetWindowLongPtrW(hwnd: isize, index: i32) -> isize;
    fn SetWindowLongPtrW(hwnd: isize, index: i32, value: isize) -> isize;
    fn SetForegroundWindow(hwnd: isize) -> i32;
    fn GetForegroundWindow() -> isize;
    fn IsWindowVisible(hwnd: isize) -> i32;
    fn IsIconic(hwnd: isize) -> i32;
    fn GetAsyncKeyState(vk: i32) -> i16;
    fn GetCursorPos(p: *mut [i32; 2]) -> i32;
    fn ScreenToClient(hwnd: isize, p: *mut [i32; 2]) -> i32;
}

#[link(name = "shell32")]
extern "system" {
    fn ShellExecuteW(
        hwnd: isize,
        op: *const u16,
        file: *const u16,
        params: *const u16,
        dir: *const u16,
        show: i32,
    ) -> isize;
}

#[link(name = "kernel32")]
extern "system" {
    fn GetCurrentProcess() -> isize;
    fn GetProcessTimes(h: isize, created: *mut u64, exited: *mut u64, kernel: *mut u64, user: *mut u64) -> i32;
    fn K32GetProcessMemoryInfo(process: isize, pmc: *mut MemCounters, cb: u32) -> i32;
    fn SetProcessWorkingSetSize(process: isize, min: usize, max: usize) -> i32;
}

#[repr(C)]
#[derive(Default)]
struct MemCounters {
    cb: u32,
    page_fault_count: u32,
    peak_working_set_size: usize,
    working_set_size: usize,
    quota_peak_paged_pool_usage: usize,
    quota_paged_pool_usage: usize,
    quota_peak_non_paged_pool_usage: usize,
    quota_non_paged_pool_usage: usize,
    pagefile_usage: usize,
    peak_pagefile_usage: usize,
    private_usage: usize,
    private_working_set_size: usize,
    shared_commit_usage: u64,
}

#[link(name = "dwmapi")]
extern "system" {
    fn DwmSetWindowAttribute(hwnd: isize, attr: u32, value: *const u32, size: u32) -> i32;
}

/// Windows 11 rounds or squares the window's corners on the spot; no restart needed.
pub fn set_round_corners(on: bool) {
    let h = hwnd();
    if h == 0 {
        return;
    }
    // DWMWA_WINDOW_CORNER_PREFERENCE = 33; 2 = round, 1 = square
    let pref: u32 = if on { 2 } else { 1 };
    unsafe {
        DwmSetWindowAttribute(h, 33, &pref, 4);
    }
}

const GWL_EXSTYLE: i32 = -20;
const WS_EX_TOOLWINDOW: isize = 0x80;

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Cursor in window-client physical pixels. Used for OS file drops, where the
/// window gets no mouse-move events while Explorer owns the drag.
pub fn cursor_client() -> Option<(f32, f32)> {
    let mut p = [0i32; 2];
    unsafe {
        let h = hwnd();
        if h == 0 || GetCursorPos(&mut p) == 0 || ScreenToClient(h, &mut p) == 0 {
            return None;
        }
    }
    Some((p[0] as f32, p[1] as f32))
}

fn hwnd() -> isize {
    unsafe { FindWindowW(std::ptr::null(), wide(TITLE).as_ptr()) }
}

/// True if a Tangra window already exists (used for single-instance).
#[allow(dead_code)]
pub fn window_exists() -> bool {
    hwnd() != 0
}

/// Window is on screen (not hidden to tray and not minimized), so the UI loop is running.
pub fn window_live() -> bool {
    let h = hwnd();
    h != 0 && unsafe { IsWindowVisible(h) != 0 && IsIconic(h) == 0 }
}

pub fn window_is_foreground() -> bool {
    let h = hwnd();
    h != 0 && unsafe { GetForegroundWindow() == h }
}

pub fn show_window() {
    let h = hwnd();
    if h == 0 {
        return;
    }
    unsafe {
        // Coming back from the tray: give the window its taskbar button again
        let ex = GetWindowLongPtrW(h, GWL_EXSTYLE);
        if ex & WS_EX_TOOLWINDOW != 0 {
            ShowWindow(h, 0);
            SetWindowLongPtrW(h, GWL_EXSTYLE, ex & !WS_EX_TOOLWINDOW);
        }
        ShowWindow(h, if IsIconic(h) != 0 { 9 } else { 5 }); // SW_RESTORE / SW_SHOW
        SetForegroundWindow(h);
    }
}

pub fn hide_window() {
    let h = hwnd();
    if h != 0 {
        // A truly hidden window never receives the redraw the UI loop is waiting
        // for, so the loop spins a CPU core. Minimised without a taskbar button
        // looks the same to the user and lets the loop sleep.
        unsafe {
            ShowWindow(h, 0); // SW_HIDE
            let ex = GetWindowLongPtrW(h, GWL_EXSTYLE);
            SetWindowLongPtrW(h, GWL_EXSTYLE, ex | WS_EX_TOOLWINDOW);
            ShowWindow(h, 7); // SW_SHOWMINNOACTIVE
        }
    }
}

/// Hands every page we are not actively using back to the OS (used when hidden to tray).
pub fn trim_memory() {
    unsafe { SetProcessWorkingSetSize(GetCurrentProcess(), usize::MAX, usize::MAX) };
}

/// Private working set in MB: the same number Task Manager shows.
pub fn memory_mb() -> f32 {
    let mut c = MemCounters::default();
    let size = std::mem::size_of::<MemCounters>() as u32;
    unsafe {
        let h = GetCurrentProcess();
        if K32GetProcessMemoryInfo(h, &mut c, size) != 0 {
            return c.private_working_set_size as f32 / 1_048_576.0;
        }
        if K32GetProcessMemoryInfo(h, &mut c, size - 16) != 0 {
            return c.working_set_size as f32 / 1_048_576.0;
        }
    }
    0.0
}

/// CPU time this process has used so far, in milliseconds.
pub fn cpu_ms() -> f64 {
    let (mut c, mut e, mut k, mut u) = (0u64, 0u64, 0u64, 0u64);
    unsafe { GetProcessTimes(GetCurrentProcess(), &mut c, &mut e, &mut k, &mut u) };
    (k + u) as f64 / 10_000.0
}

pub fn ctrl_v_down() -> bool {
    unsafe { GetAsyncKeyState(0x11) < 0 && GetAsyncKeyState(0x56) < 0 }
}

pub fn open_url(url: &str) {
    if !(url.starts_with("http://") || url.starts_with("https://")) {
        return;
    }
    unsafe {
        ShellExecuteW(
            0,
            wide("open").as_ptr(),
            wide(url).as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            1,
        );
    }
}

// ---------- tray + global hotkeys ----------

pub enum Msg {
    Tangra,
    Settings,
    Quit,
    Hide,
}

#[derive(Clone)]
pub struct Bridge {
    pub tx: Sender<Msg>,
    pub ctx: egui::Context,
    /// (summon hotkey id, quick-dump hotkey id)
    pub ids: Arc<Mutex<(u32, u32)>>,
    /// Set when the board file was changed on disk while the window was hidden.
    pub reload: Arc<AtomicBool>,
}

impl Bridge {
    fn send(&self, m: Msg) {
        let _ = self.tx.send(m);
        self.ctx.request_repaint();
    }
    fn show(&self) {
        show_window();
        self.ctx.request_repaint();
    }
}

/// These handlers run on the main thread's message pump, which keeps working while the
/// window is hidden, so the tray and hotkeys stay responsive with the UI fully asleep.
pub fn install_handlers(bridge: Bridge) {
    let b = bridge.clone();
    GlobalHotKeyEvent::set_event_handler(Some(move |e: GlobalHotKeyEvent| {
        if e.state() != HotKeyState::Pressed {
            return;
        }
        let (summon, dump) = *b.ids.lock().unwrap();
        if e.id() == summon {
            if window_live() && window_is_foreground() {
                b.send(Msg::Hide);
            } else {
                b.show();
            }
        } else if e.id() == dump {
            if window_live() {
                b.send(Msg::Tangra);
            } else {
                let _ = quick_dump_to_disk();
                b.reload.store(true, Ordering::SeqCst);
            }
        }
    }));

    let b = bridge.clone();
    TrayIconEvent::set_event_handler(Some(move |e: TrayIconEvent| {
        if let TrayIconEvent::Click {
            button: MouseButton::Left,
            button_state: MouseButtonState::Up,
            ..
        } = e
        {
            b.show();
        }
    }));

    let b = bridge;
    MenuEvent::set_event_handler(Some(move |e: MenuEvent| match e.id.0.as_str() {
        "open" => b.show(),
        "settings" => {
            b.show();
            b.send(Msg::Settings);
        }
        "quit" => {
            if window_live() {
                b.send(Msg::Quit);
            } else {
                // Everything is saved before the window hides, so this is safe.
                std::process::exit(0);
            }
        }
        _ => {}
    }));
}

pub fn build_tray(rgba: Vec<u8>, w: u32, h: u32) -> Option<TrayIcon> {
    let menu = Menu::new();
    let open = MenuItem::with_id("open", "Open Tangra", true, None);
    let settings = MenuItem::with_id("settings", "Settings", true, None);
    let quit = MenuItem::with_id("quit", "Quit Tangra", true, None);
    let _ = menu.append_items(&[&open, &settings, &PredefinedMenuItem::separator(), &quit]);
    TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_menu_on_left_click(false)
        .with_tooltip("Tangra")
        .with_icon(Icon::from_rgba(rgba, w, h).ok()?)
        .build()
        .ok()
}

// ---------- clipboard ----------

pub enum Clip {
    Image { w: u32, h: u32, rgba: Vec<u8> },
    Url(String),
    Text(String),
    Empty,
}

pub fn read_clipboard() -> Clip {
    let Ok(mut cb) = arboard::Clipboard::new() else {
        return Clip::Empty;
    };
    if let Ok(img) = cb.get_image() {
        if img.width > 0 && img.height > 0 {
            return Clip::Image {
                w: img.width as u32,
                h: img.height as u32,
                rgba: img.bytes.into_owned(),
            };
        }
    }
    if let Ok(text) = cb.get_text() {
        let t = text.trim();
        if !t.is_empty() {
            if (t.starts_with("http://") || t.starts_with("https://")) && !t.contains(char::is_whitespace) {
                return Clip::Url(t.to_string());
            }
            return Clip::Text(t.to_string());
        }
    }
    Clip::Empty
}

pub fn copy_image(w: usize, h: usize, rgba: &[u8]) -> Result<(), String> {
    let mut cb = arboard::Clipboard::new().map_err(|e| e.to_string())?;
    cb.set_image(arboard::ImageData {
        width: w,
        height: h,
        bytes: std::borrow::Cow::Borrowed(rgba),
    })
    .map_err(|e| e.to_string())
}

// ---------- image io ----------

pub fn save_png(path: &Path, rgba: &[u8], w: u32, h: u32) -> Result<(), String> {
    use image::codecs::png::{CompressionType, FilterType, PngEncoder};
    use image::ImageEncoder;
    let file = std::fs::File::create(path).map_err(|e| e.to_string())?;
    PngEncoder::new_with_quality(
        std::io::BufWriter::new(file),
        CompressionType::Fast,
        FilterType::Adaptive,
    )
    .write_image(rgba, w, h, image::ExtendedColorType::Rgba8)
    .map_err(|e| e.to_string())
}

pub fn load_rgba(path: &Path) -> Result<(u32, u32, Vec<u8>), String> {
    let img = image::open(path).map_err(|e| e.to_string())?.into_rgba8();
    Ok((img.width(), img.height(), img.into_raw()))
}

/// Silent quick-dump used while the window is hidden: appends straight to the board file.
pub fn quick_dump_to_disk() -> Result<(), String> {
    let settings = model::load_settings();
    let mut board = model::load_board(&settings.active_board_id);
    let n = (board.images.len() + board.sticky_notes.len() + board.link_cards.len()) as f32;
    let center = Pt {
        x: board.camera.x + 480.0 + (n % 12.0) * 24.0,
        y: board.camera.y + 340.0 + (n % 12.0) * 24.0,
    };
    match read_clipboard() {
        Clip::Image { w, h, rgba } => {
            let file = format!("{}.png", model::new_id());
            save_png(&model::asset_path(&board.id, &file), &rgba, w, h)?;
            board.add_image(file, w, h, center);
        }
        Clip::Url(u) => {
            board.add_link(&u, center);
        }
        Clip::Text(t) => {
            board.add_note(&t, center);
        }
        Clip::Empty => return Ok(()),
    }
    board.updated_at = model::now_ms();
    model::save_board(&board)
}

// ---------- baking annotations into pixels ----------

pub fn hex_rgb(s: &str) -> [u8; 3] {
    let h = s.trim_start_matches('#');
    let p = |i: usize| u8::from_str_radix(h.get(i..i + 2).unwrap_or("ff"), 16).unwrap_or(255);
    if h.len() >= 6 {
        [p(0), p(2), p(4)]
    } else {
        [255, 255, 255]
    }
}

fn blend(buf: &mut [u8], w: usize, h: usize, x: i32, y: i32, rgb: [u8; 3], a: f32) {
    if x < 0 || y < 0 || x as usize >= w || y as usize >= h || a <= 0.0 {
        return;
    }
    let i = (y as usize * w + x as usize) * 4;
    let a = a.min(1.0);
    for c in 0..3 {
        buf[i + c] = (buf[i + c] as f32 * (1.0 - a) + rgb[c] as f32 * a).round() as u8;
    }
    let da = buf[i + 3] as f32 / 255.0;
    buf[i + 3] = ((a + da * (1.0 - a)) * 255.0).round() as u8;
}

/// Even-odd scanline fill.
fn fill_poly(buf: &mut [u8], w: usize, h: usize, pts: &[(f32, f32)], rgb: [u8; 3], alpha: f32) {
    if pts.len() < 3 || w == 0 || h == 0 {
        return;
    }
    let y0 = pts.iter().fold(f32::MAX, |m, p| m.min(p.1)).floor().max(0.0) as usize;
    let y1 = (pts.iter().fold(f32::MIN, |m, p| m.max(p.1)).ceil().max(0.0) as usize).min(h);
    let mut xs: Vec<f32> = Vec::new();
    for y in y0..y1 {
        let yc = y as f32 + 0.5;
        xs.clear();
        for i in 0..pts.len() {
            let (a, b) = (pts[i], pts[(i + 1) % pts.len()]);
            if (a.1 <= yc) != (b.1 <= yc) {
                xs.push(a.0 + (yc - a.1) / (b.1 - a.1) * (b.0 - a.0));
            }
        }
        xs.sort_by(|a, b| a.total_cmp(b));
        for pair in xs.chunks_exact(2) {
            let xa = pair[0].round().max(0.0) as i32;
            let xb = (pair[1].round() as i32).min(w as i32);
            for x in xa..xb {
                blend(buf, w, h, x, y as i32, rgb, alpha);
            }
        }
    }
}

fn draw_stroke(buf: &mut [u8], w: usize, h: usize, pts: &[(f32, f32)], radius: f32, rgb: [u8; 3], alpha: f32) {
    if pts.is_empty() || w == 0 || h == 0 {
        return;
    }
    let pad = radius + 1.5;
    let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for &(x, y) in pts {
        x0 = x0.min(x);
        y0 = y0.min(y);
        x1 = x1.max(x);
        y1 = y1.max(y);
    }
    let bx0 = (x0 - pad).floor().max(0.0) as usize;
    let by0 = (y0 - pad).floor().max(0.0) as usize;
    let bx1 = ((x1 + pad).ceil().max(0.0) as usize).min(w);
    let by1 = ((y1 + pad).ceil().max(0.0) as usize).min(h);
    if bx1 <= bx0 || by1 <= by0 {
        return;
    }
    let (bw, bh) = (bx1 - bx0, by1 - by0);
    // Coverage mask first, so a stroke that crosses itself keeps a uniform opacity
    let mut mask = vec![0u8; bw * bh];
    let segs: Vec<((f32, f32), (f32, f32))> = if pts.len() == 1 {
        vec![(pts[0], pts[0])]
    } else {
        pts.windows(2).map(|s| (s[0], s[1])).collect()
    };
    for (a, b) in segs {
        let sx0 = ((a.0.min(b.0) - pad).floor().max(bx0 as f32) as usize).min(bx1);
        let sy0 = ((a.1.min(b.1) - pad).floor().max(by0 as f32) as usize).min(by1);
        let sx1 = ((a.0.max(b.0) + pad).ceil().max(0.0) as usize).min(bx1);
        let sy1 = ((a.1.max(b.1) + pad).ceil().max(0.0) as usize).min(by1);
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        let len2 = dx * dx + dy * dy;
        for y in sy0..sy1 {
            for x in sx0..sx1 {
                let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
                let t = if len2 > 0.0 {
                    (((px - a.0) * dx + (py - a.1) * dy) / len2).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                let (cx, cy) = (a.0 + dx * t - px, a.1 + dy * t - py);
                let cov = (radius - (cx * cx + cy * cy).sqrt() + 0.5).clamp(0.0, 1.0);
                let m = &mut mask[(y - by0) * bw + (x - bx0)];
                *m = (*m).max((cov * 255.0) as u8);
            }
        }
    }
    for y in 0..bh {
        for x in 0..bw {
            let m = mask[y * bw + x];
            if m > 0 {
                blend(buf, w, h, (x + bx0) as i32, (y + by0) as i32, rgb, m as f32 / 255.0 * alpha);
            }
        }
    }
}

fn draw_text(buf: &mut [u8], w: usize, h: usize, text: &str, x: f32, y: f32, px: f32, rgb: [u8; 3]) {
    use ab_glyph::{point, Font, FontVec, PxScale, ScaleFont};
    let font = ["seguisb.ttf", "segoeui.ttf", "arial.ttf"]
        .iter()
        .find_map(|f| std::fs::read(format!("C:\\Windows\\Fonts\\{f}")).ok())
        .and_then(|b| FontVec::try_from_vec(b).ok());
    let Some(font) = font else { return };
    let scale = PxScale::from(px * 1.2);
    let scaled = font.as_scaled(scale);
    let width: f32 = text.chars().map(|c| scaled.h_advance(scaled.glyph_id(c))).sum();
    // Dark backing plate, same as on the canvas
    let pad = px * 0.25;
    for by in (y - pad * 0.5) as i32..(y + px * 1.2 + pad * 0.5) as i32 {
        for bx in (x - pad) as i32..(x + width + pad) as i32 {
            blend(buf, w, h, bx, by, [0, 0, 0], 0.7);
        }
    }
    let mut cx = x;
    for ch in text.chars() {
        let id = scaled.glyph_id(ch);
        let glyph = id.with_scale_and_position(scale, point(cx, y + scaled.ascent()));
        cx += scaled.h_advance(id);
        if let Some(outline) = font.outline_glyph(glyph) {
            let b = outline.px_bounds();
            outline.draw(|gx, gy, c| {
                blend(buf, w, h, b.min.x as i32 + gx as i32, b.min.y as i32 + gy as i32, rgb, c);
            });
        }
    }
}

/// Draws an image's annotations into its pixels (for "copy with annotations").
/// Size of the coarse grid an image is reduced to for pixelation.
pub fn mosaic_dims(w: u32, h: u32) -> (u32, u32) {
    let block = (w.min(h) as f32 / 45.0).clamp(10.0, 32.0);
    (((w as f32 / block) as u32).max(1), ((h as f32 / block) as u32).max(1))
}

fn near_path(pts: &[(f32, f32)], p: (f32, f32), r: f32) -> bool {
    let d2 = |a: (f32, f32), b: (f32, f32)| {
        let (vx, vy) = (b.0 - a.0, b.1 - a.1);
        let len = vx * vx + vy * vy;
        let t = if len > 0.0 { (((p.0 - a.0) * vx + (p.1 - a.1) * vy) / len).clamp(0.0, 1.0) } else { 0.0 };
        let (dx, dy) = (p.0 - a.0 - vx * t, p.1 - a.1 - vy * t);
        dx * dx + dy * dy
    };
    match pts.len() {
        0 => false,
        1 => d2(pts[0], pts[0]) <= r * r,
        _ => pts.windows(2).any(|s| d2(s[0], s[1]) <= r * r),
    }
}

/// Replaces the pixels under every pixelate swipe with coarse blocks of the image.
fn pixelate(img: &ImageNode, buf: &mut [u8], w: usize, h: usize, k: f32, scale: f32) {
    if w == 0 || h == 0 || buf.len() < w * h * 4 || !img.annotations.strokes.iter().any(|s| s.pixelate) {
        return;
    }
    let Some(src) = image::RgbaImage::from_raw(w as u32, h as u32, buf[..w * h * 4].to_vec()) else { return };
    let (nx, ny) = mosaic_dims(w as u32, h as u32);
    let small = image::imageops::thumbnail(&src, nx, ny);
    for s in img.annotations.strokes.iter().filter(|s| s.pixelate) {
        let pts: Vec<(f32, f32)> = s.points.iter().map(|p| (p.x * k, p.y * k)).collect();
        if pts.is_empty() {
            continue;
        }
        let r = ((s.width / scale) * k / 2.0).max(1.0);
        let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
        for p in &pts {
            x0 = x0.min(p.0);
            y0 = y0.min(p.1);
            x1 = x1.max(p.0);
            y1 = y1.max(p.1);
        }
        let (x0, x1) = ((x0 - r).floor().max(0.0) as usize, ((x1 + r).ceil().max(0.0) as usize).min(w));
        let (y0, y1) = ((y0 - r).floor().max(0.0) as usize, ((y1 + r).ceil().max(0.0) as usize).min(h));
        for y in y0..y1 {
            for x in x0..x1 {
                if near_path(&pts, (x as f32 + 0.5, y as f32 + 0.5), r) {
                    let c = small.get_pixel(((x * nx as usize) / w) as u32, ((y * ny as usize) / h) as u32);
                    let i = (y * w + x) * 4;
                    buf[i..i + 4].copy_from_slice(&c.0);
                }
            }
        }
    }
}

pub fn bake(img: &ImageNode, buf: &mut [u8], w: usize, h: usize) {
    let (sx, sy) = img.scale();
    // Annotation coords are in original-image pixels; k covers a decoded size mismatch
    let k = if img.original_width > 0.0 { w as f32 / img.original_width } else { 1.0 };
    // Pixelation goes first so ink drawn on top of it stays readable
    pixelate(img, buf, w, h, k, sx.min(sy));
    for s in img.annotations.strokes.iter().filter(|s| !s.pixelate) {
        let pts: Vec<(f32, f32)> = s.points.iter().map(|p| (p.x * k, p.y * k)).collect();
        let radius = (s.width / sx.min(sy)) * k / 2.0;
        let closed = s.closed_len();
        if let (Some(n), true) = (closed, s.fill) {
            fill_poly(buf, w, h, &pts[..n], hex_rgb(&s.color), s.fill_alpha());
            if s.no_stroke {
                continue;
            }
        }
        draw_stroke(buf, w, h, &pts, radius.max(0.5), hex_rgb(&s.color), s.opacity.unwrap_or(1.0));
    }
    for t in &img.annotations.texts {
        if !t.text.is_empty() {
            let fs = if t.font_size > 0.0 { t.font_size } else { 16.0 };
            draw_text(buf, w, h, &t.text, t.x * k, t.y * k, fs * k, hex_rgb(&t.color));
        }
    }
}
