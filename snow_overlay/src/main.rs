use std::{io::{self, BufRead, Write}, mem, ptr, sync::mpsc::{self, Receiver}, thread, time::{Duration, Instant}};

type Handle = *mut core::ffi::c_void;
type Hwnd = Handle;
type Hdc = Handle;
type Hinstance = Handle;
type Hmonitor = Handle;
type Hcursor = Handle;
type Hbrush = Handle;
type Wparam = usize;
type Lparam = isize;
type Lresult = isize;

const WS_POPUP: u32 = 0x8000_0000;
const WS_EX_LAYERED: u32 = 0x0008_0000;
const WS_EX_TRANSPARENT: u32 = 0x0000_0020;
const WS_EX_TOOLWINDOW: u32 = 0x0000_0080;
const WS_EX_NOACTIVATE: u32 = 0x0800_0000;
const HWND_TOPMOST: Hwnd = (-1isize) as Hwnd;
const SW_SHOWNOACTIVATE: i32 = 4;
const ULW_ALPHA: u32 = 2;
const AC_SRC_OVER: u8 = 0;
const AC_SRC_ALPHA: u8 = 1;
const BI_RGB: u32 = 0;
const DIB_RGB_COLORS: u32 = 0;
const SM_XVIRTUALSCREEN: i32 = 76;
const SM_YVIRTUALSCREEN: i32 = 77;
const SM_CXVIRTUALSCREEN: i32 = 78;
const SM_CYVIRTUALSCREEN: i32 = 79;

#[repr(C)]
struct Point { x: i32, y: i32 }
#[repr(C)]
struct Size { cx: i32, cy: i32 }
#[repr(C)]
struct Blend { op: u8, flags: u8, alpha: u8, format: u8 }
#[repr(C)]
struct BitmapInfoHeader {
    size: u32, width: i32, height: i32, planes: u16, bit_count: u16,
    compression: u32, size_image: u32, xppm: i32, yppm: i32,
    used: u32, important: u32,
}
#[repr(C)]
struct BitmapInfo { header: BitmapInfoHeader, colors: [u32; 3] }
#[repr(C)]
struct WindowClassW {
    style: u32, wnd_proc: Option<unsafe extern "system" fn(Hwnd, u32, Wparam, Lparam) -> Lresult>,
    cls_extra: i32, wnd_extra: i32, instance: Hinstance, icon: Handle,
    cursor: Hcursor, background: Hbrush, menu_name: *const u16, class_name: *const u16,
}
#[repr(C)]
struct Msg { hwnd: Hwnd, message: u32, wparam: Wparam, lparam: Lparam, time: u32, pt: Point }
#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq)]
struct Rect { left: i32, top: i32, right: i32, bottom: i32 }
#[repr(C)]
struct MonitorInfo { size: u32, monitor: Rect, work: Rect, flags: u32 }

#[link(name = "user32")]
unsafe extern "system" {
    fn GetModuleHandleW(name: *const u16) -> Hinstance;
    fn RegisterClassW(class: *const WindowClassW) -> u16;
    fn CreateWindowExW(ex: u32, class: *const u16, title: *const u16, style: u32,
        x: i32, y: i32, width: i32, height: i32, parent: Hwnd, menu: Handle,
        instance: Hinstance, param: Handle) -> Hwnd;
    fn DefWindowProcW(hwnd: Hwnd, msg: u32, w: Wparam, l: Lparam) -> Lresult;
    fn ShowWindow(hwnd: Hwnd, cmd: i32) -> i32;
    fn SetWindowPos(hwnd: Hwnd, after: Hwnd, x: i32, y: i32, cx: i32, cy: i32, flags: u32) -> i32;
    fn GetSystemMetrics(index: i32) -> i32;
    fn EnumWindows(callback: Option<unsafe extern "system" fn(Hwnd, Lparam) -> i32>, data: Lparam) -> i32;
    fn IsWindowVisible(hwnd: Hwnd) -> i32;
    fn IsIconic(hwnd: Hwnd) -> i32;
    fn IsZoomed(hwnd: Hwnd) -> i32;
    fn GetWindowRect(hwnd: Hwnd, rect: *mut Rect) -> i32;
    fn MonitorFromWindow(hwnd: Hwnd, flags: u32) -> Hmonitor;
    fn GetMonitorInfoW(monitor: Hmonitor, info: *mut MonitorInfo) -> i32;
    fn GetWindowThreadProcessId(hwnd: Hwnd, process_id: *mut u32) -> u32;
    fn GetClassNameW(hwnd: Hwnd, class_name: *mut u16, max: i32) -> i32;
    fn WindowFromPoint(point: Point) -> Hwnd;
    fn GetAncestor(hwnd: Hwnd, flags: u32) -> Hwnd;
    fn TranslateMessage(msg: *const Msg) -> i32;
    fn DispatchMessageW(msg: *const Msg) -> Lresult;
    fn PeekMessageW(msg: *mut Msg, hwnd: Hwnd, min: u32, max: u32, remove: u32) -> i32;
    fn UpdateLayeredWindow(hwnd: Hwnd, dst: Hdc, dst_point: *const Point, size: *const Size,
        src: Hdc, src_point: *const Point, key: u32, blend: *const Blend, flags: u32) -> i32;
}
#[link(name = "kernel32")]
unsafe extern "system" {
    fn FreeConsole() -> i32;
}
#[link(name = "gdi32")]
unsafe extern "system" {
    fn CreateCompatibleDC(dc: Hdc) -> Hdc;
    fn CreateDIBSection(dc: Hdc, info: *const BitmapInfo, usage: u32, bits: *mut *mut core::ffi::c_void,
        section: Handle, offset: u32) -> Handle;
    fn SelectObject(dc: Hdc, object: Handle) -> Handle;
}

#[derive(Clone, Copy)]
struct Flake { x: f32, y: f32, speed: f32, drift: f32, radius: f32, alpha: f32, mass: f32, fresh: bool }
struct Tractor { x: f32, active: bool, auto_armed: bool }
struct WindowCandidate { hwnd: Hwnd, rect: Rect }
struct WindowSnow { hwnd: Hwnd, rect: Rect, snow_y: i32, snow_below: bool,
                    snowline: Vec<f32>, visible_ranges: Vec<(i32, i32)> }
struct EnumContext { excluded_hwnd: Hwnd, excluded_pid: u32, view: Rect, windows: Vec<WindowCandidate> }

#[derive(Clone, Copy)]
enum LandingTarget { Window { surface: usize, column: usize }, Floor { column: usize } }

enum Control { Enabled(bool), Rate(f32), Tractor, Quit }

fn random(seed: &mut u32) -> f32 {
    *seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
    ((*seed >> 8) as f32) / 16_777_216.0
}

fn put(buf: &mut [u32], w: usize, h: usize, x: i32, y: i32, color: u32) {
    if x >= 0 && y >= 0 && (x as usize) < w && (y as usize) < h {
        buf[y as usize * w + x as usize] = color;
    }
}

// Draws a soft circular flake at fractional coordinates. The coverage ramp at
// the edge keeps both the shape and its subpixel movement from looking jagged.
fn draw_flake(buf: &mut [u32], w: usize, h: usize, cx: f32, cy: f32, radius: f32, opacity: f32) {
    let outer = radius + 0.75;
    let min_x = (cx - outer).floor() as i32;
    let max_x = (cx + outer).ceil() as i32;
    let min_y = (cy - outer).floor() as i32;
    let max_y = (cy + outer).ceil() as i32;
    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let dx = x as f32 + 0.5 - cx;
            let dy = y as f32 + 0.5 - cy;
            let distance = (dx * dx + dy * dy).sqrt();
            let coverage = (radius + 0.65 - distance).clamp(0.0, 1.0) * opacity;
            let a = (coverage * 255.0) as u32;
            if a != 0 { put(buf, w, h, x, y, (a << 24) | (a << 16) | (a << 8) | a); }
        }
    }
}

fn draw_rect(buf: &mut [u32], w: usize, h: usize, x: i32, y: i32, rw: i32, rh: i32, color: u32) {
    for py in y.max(0)..(y + rh).min(h as i32) {
        for px in x.max(0)..(x + rw).min(w as i32) { put(buf, w, h, px, py, color); }
    }
}

fn draw_circle(buf: &mut [u32], w: usize, h: usize, cx: i32, cy: i32, r: i32, color: u32) {
    for dy in -r..=r { for dx in -r..=r {
        if dx * dx + dy * dy <= r * r { put(buf, w, h, cx + dx, cy + dy, color); }
    }}
}

fn draw_tractor(buf: &mut [u32], w: usize, h: usize, center_x: f32) {
    let x = center_x as i32;
    let ground = h as i32 - 3;
    // Tracks and wheels
    draw_rect(buf, w, h, x - 42, ground - 19, 74, 15, 0xFF252B30);
    draw_circle(buf, w, h, x - 25, ground - 11, 8, 0xFF171B20);
    draw_circle(buf, w, h, x + 17, ground - 11, 8, 0xFF171B20);
    draw_circle(buf, w, h, x - 25, ground - 11, 4, 0xFF929BA2);
    draw_circle(buf, w, h, x + 17, ground - 11, 4, 0xFF929BA2);
    // Green body, cab, and pale blue window
    draw_rect(buf, w, h, x - 38, ground - 41, 66, 24, 0xFF27864C);
    draw_rect(buf, w, h, x - 21, ground - 59, 31, 20, 0xFF27864C);
    draw_rect(buf, w, h, x - 17, ground - 55, 22, 13, 0xFFE2F5FF);
    draw_rect(buf, w, h, x + 13, ground - 34, 14, 7, 0xFFFFD35C);
    // Snowplow blade in front of the tractor (it drives toward the left).
    draw_rect(buf, w, h, x - 56, ground - 25, 14, 28, 0xFFB8C5CE);
    draw_rect(buf, w, h, x - 61, ground - 22, 6, 24, 0xFF75828A);
}

fn start_tractor(tractor: &mut Tractor, w: usize) {
    if !tractor.active { tractor.x = w as f32 + 90.0; tractor.active = true; }
}

unsafe extern "system" fn enum_windows_callback(hwnd: Hwnd, data: Lparam) -> i32 {
    let context = &mut *(data as *mut EnumContext);
    if hwnd == context.excluded_hwnd || IsWindowVisible(hwnd) == 0 || IsIconic(hwnd) != 0 { return 1; }
    let mut process_id = 0u32;
    GetWindowThreadProcessId(hwnd, &mut process_id);
    if process_id == context.excluded_pid { return 1; }

    let mut class_buffer = [0u16; 128];
    let class_len = GetClassNameW(hwnd, class_buffer.as_mut_ptr(), class_buffer.len() as i32).max(0) as usize;
    let class_name = String::from_utf16_lossy(&class_buffer[..class_len]);
    if matches!(class_name.as_str(), "Progman" | "WorkerW" | "Shell_TrayWnd" | "Shell_SecondaryTrayWnd") {
        return 1;
    }

    let mut rect = Rect { left: 0, top: 0, right: 0, bottom: 0 };
    if GetWindowRect(hwnd, &mut rect) == 0 { return 1; }
    let width = rect.right - rect.left;
    let height = rect.bottom - rect.top;
    if width < 120 || height < 70 || rect.bottom <= context.view.top || rect.top >= context.view.bottom
        || rect.right <= context.view.left || rect.left >= context.view.right { return 1; }
    if IsZoomed(hwnd) != 0 { return 1; }
    let monitor = MonitorFromWindow(hwnd, 2);
    if !monitor.is_null() {
        let mut info = MonitorInfo {
            size: mem::size_of::<MonitorInfo>() as u32,
            monitor: Rect { left: 0, top: 0, right: 0, bottom: 0 },
            work: Rect { left: 0, top: 0, right: 0, bottom: 0 },
            flags: 0,
        };
        if GetMonitorInfoW(monitor, &mut info) != 0 {
            let monitor_width = (info.monitor.right - info.monitor.left) as f32;
            let monitor_height = (info.monitor.bottom - info.monitor.top) as f32;
            if width as f32 >= monitor_width * 0.97 && height as f32 >= monitor_height * 0.97 {
                return 1;
            }
        }
    }

    context.windows.push(WindowCandidate { hwnd, rect });
    1
}

fn enumerate_windows(overlay: Hwnd, excluded_pid: u32, view: Rect) -> Vec<WindowCandidate> {
    let mut context = EnumContext { excluded_hwnd: overlay, excluded_pid, view, windows: Vec::new() };
    unsafe { EnumWindows(Some(enum_windows_callback), &mut context as *mut _ as Lparam); }
    context.windows
}

fn visible_ranges(hwnd: Hwnd, rect: Rect, view: Rect) -> Vec<(i32, i32)> {
    let left = rect.left.max(view.left);
    let right = rect.right.min(view.right);
    if left >= right { return Vec::new(); }
    let probe_y = (rect.top.max(view.top) + 2).min(rect.bottom - 1);
    let mut ranges: Vec<(i32, i32)> = Vec::new();
    // Sample the window's title-bar edge directly. EnumWindows does not promise
    // a useful ordering for clipping here, while WindowFromPoint reports the
    // window that is actually visible at this screen location.
    let step = 8;
    let mut x = left;
    while x < right {
        let probe_x = (x + step / 2).min(right - 1);
        let hit = unsafe { WindowFromPoint(Point { x: probe_x, y: probe_y }) };
        let root = if hit.is_null() { hit } else { unsafe { GetAncestor(hit, 2) } };
        if root == hwnd {
            let end = (x + step).min(right);
            if let Some(last) = ranges.last_mut() {
                if last.1 == x { last.1 = end; } else { ranges.push((x, end)); }
            } else { ranges.push((x, end)); }
        }
        x += step;
    }
    ranges
}

fn column_is_visible(ranges: &[(i32, i32)], x: i32) -> bool {
    ranges.iter().any(|(left, right)| x >= *left && x < *right)
}

fn shed_window_snow(surface: WindowSnow, flakes: &mut Vec<Flake>, seed: &mut u32) {
    for (column, mass) in surface.snowline.into_iter().enumerate() {
        if mass < 0.04 { continue; }
        flakes.push(Flake {
            x: (surface.rect.left + column as i32) as f32,
            y: if surface.snow_below { surface.snow_y as f32 + mass } else { surface.snow_y as f32 - mass },
            speed: 42.0 + random(seed) * 45.0,
            drift: (random(seed) - 0.5) * 18.0,
            radius: 1.0 + mass.sqrt().min(3.0) * 0.35,
            alpha: 0.78 + random(seed) * 0.20,
            mass,
            fresh: false,
        });
    }
}

fn refresh_window_snow(surfaces: &mut Vec<WindowSnow>, candidates: Vec<WindowCandidate>,
                       view: Rect, flakes: &mut Vec<Flake>, seed: &mut u32) {
    let mut refreshed = Vec::with_capacity(candidates.len());
    for candidate in &candidates {
        let ranges = visible_ranges(candidate.hwnd, candidate.rect, view);
        if let Some(old_index) = surfaces.iter().position(|surface| surface.hwnd == candidate.hwnd) {
            let mut old = surfaces.remove(old_index);
            if old.rect == candidate.rect {
                old.visible_ranges = ranges;
                refreshed.push(old);
            } else {
                shed_window_snow(old, flakes, seed);
                refreshed.push(WindowSnow { hwnd: candidate.hwnd, rect: candidate.rect,
                    snow_y: candidate.rect.top.max(view.top + 1),
                    snow_below: candidate.rect.top <= view.top + 2,
                    snowline: vec![0.0; (candidate.rect.right - candidate.rect.left) as usize],
                    visible_ranges: ranges });
            }
        } else {
            refreshed.push(WindowSnow { hwnd: candidate.hwnd, rect: candidate.rect,
                snow_y: candidate.rect.top.max(view.top + 1),
                snow_below: candidate.rect.top <= view.top + 2,
                snowline: vec![0.0; (candidate.rect.right - candidate.rect.left) as usize],
                visible_ranges: ranges });
        }
    }
    for old in surfaces.drain(..) { shed_window_snow(old, flakes, seed); }
    *surfaces = refreshed;
}

fn deposit_snow(line: &mut [f32], column: usize, amount: f32, max_depth: f32, seed: &mut u32) {
    if column >= line.len() { return; }
    line[column] = (line[column] + amount).min(max_depth);
    if column > 0 && random(seed) > 0.5 {
        line[column - 1] = (line[column - 1] + amount * 0.25).min(max_depth);
    }
    if column + 1 < line.len() && random(seed) > 0.5 {
        line[column + 1] = (line[column + 1] + amount * 0.25).min(max_depth);
    }
}

fn find_landing(x: i32, y: f32, view_x: i32, view_y: i32, w: usize, h: usize,
                floor_snow: &[f32], surfaces: &[WindowSnow]) -> Option<(f32, LandingTarget)> {
    let mut best: Option<(f32, LandingTarget)> = None;
    for (surface_index, surface) in surfaces.iter().enumerate() {
        if x < surface.rect.left || x >= surface.rect.right || !column_is_visible(&surface.visible_ranges, x) {
            continue;
        }
        let column = (x - surface.rect.left) as usize;
        let edge = (surface.snow_y - view_y) as f32;
        let surface_y = if surface.snow_below { edge } else { edge - surface.snowline[column] };
        if surface_y >= y && best.map_or(true, |(best_y, _)| surface_y < best_y) {
            best = Some((surface_y, LandingTarget::Window {
                surface: surface_index, column,
            }));
        }
    }
    let floor_column = (x - view_x) as usize;
    if floor_column < w {
        let floor_y = h as f32 - floor_snow[floor_column];
        if floor_y >= y && best.map_or(true, |(best_y, _)| floor_y < best_y) {
            best = Some((floor_y, LandingTarget::Floor { column: floor_column }));
        }
    }
    best
}

fn draw_window_snow(buf: &mut [u32], w: usize, h: usize, view_x: i32, view_y: i32,
                    surfaces: &[WindowSnow]) {
    for surface in surfaces {
        let edge_y = surface.snow_y - view_y;
        for (column, depth) in surface.snowline.iter().copied().enumerate() {
            let screen_x = surface.rect.left + column as i32;
            if depth < 0.04 || !column_is_visible(&surface.visible_ranges, screen_x) { continue; }
            let x = screen_x - view_x;
            if surface.snow_below {
                let boundary = edge_y as f32 + depth;
                for y in edge_y.max(0)..boundary.ceil().min(h as f32) as i32 {
                    let coverage = (boundary - y as f32).clamp(0.0, 1.0);
                    let a = (coverage * 255.0) as u32;
                    put(buf, w, h, x, y, (a << 24) | (a << 16) | (a << 8) | a);
                }
            } else {
                let boundary = edge_y as f32 - depth;
                let top = boundary.floor() as i32;
                let fractional = boundary.fract();
                if top >= 0 && top < h as i32 {
                    let a = ((1.0 - fractional) * 255.0) as u32;
                    put(buf, w, h, x, top, (a << 24) | (a << 16) | (a << 8) | a);
                }
                for y in (top + 1).max(0)..edge_y.min(h as i32) {
                    put(buf, w, h, x, y, 0xFFE8_F4_FF);
                }
            }
        }
    }
}

fn render(buf: &mut [u32], w: usize, h: usize, view_x: i32, view_y: i32,
          flakes: &mut Vec<Flake>, snowline: &mut [f32], surfaces: &mut [WindowSnow],
          elapsed: f32, enabled: bool, rate: f32, spawn_credit: &mut f32,
          tractor: &mut Tractor, seed: &mut u32) {
    buf.fill(0);
    let average_snow = snowline.iter().sum::<f32>() / w.max(1) as f32;
    if average_snow < 14.0 { tractor.auto_armed = true; }
    if average_snow >= 28.0 && tractor.auto_armed && !tractor.active {
        start_tractor(tractor, w);
        tractor.auto_armed = false;
    }
    if tractor.active {
        let old_x = tractor.x;
        tractor.x -= 170.0 * elapsed;
        let clear_start = (tractor.x - 62.0).floor() as i32;
        let clear_end = (old_x - 18.0).ceil() as i32;
        for x in clear_start.max(0)..clear_end.min(w as i32) {
            snowline[x as usize] = 0.0;
        }
        if tractor.x < -100.0 { tractor.active = false; }
    }
    // A small, gently uneven ridge makes the accumulated snow read as a soft bank.
    for x in 0..w {
        let boundary = (h as f32 - snowline[x]).max(0.0);
        let top = boundary.floor() as i32;
        let fractional = boundary.fract();
        if top >= 0 && top < h as i32 {
            let a = ((1.0 - fractional) * 255.0) as u32;
            put(buf, w, h, x as i32, top, (a << 24) | (a << 16) | (a << 8) | a);
        }
        for y in (top + 1).max(0)..h as i32 {
            put(buf, w, h, x as i32, y, 0xFFE8_F4_FF);
        }
    }
    draw_window_snow(buf, w, h, view_x, view_y, surfaces);

    // Rate is configured in flakes/second for a 1550 px wide screen.
    if enabled {
        *spawn_credit += rate * (w as f32 / 1550.0) * elapsed;
        while *spawn_credit >= 1.0 {
            flakes.push(Flake { x: random(seed) * w as f32, y: -3.0,
                speed: 22.0 + random(seed) * 38.0, drift: (random(seed) - 0.5) * 16.0,
                radius: 1.05 + random(seed) * 0.8, alpha: 0.68 + random(seed) * 0.30,
                mass: 0.45, fresh: true });
            *spawn_credit -= 1.0;
        }
    }
    for f in flakes.iter_mut() {
        f.y += f.speed * elapsed;
        f.x += f.drift * elapsed;
        if f.x < 0.0 { f.x += w as f32; }
        if f.x >= w as f32 { f.x -= w as f32; }
        let screen_x = view_x + f.x as i32;
        if let Some((landing_y, target)) = find_landing(screen_x, f.y, view_x, view_y, w, h, snowline, surfaces) {
            if f.y + f.radius >= landing_y {
                match target {
                    LandingTarget::Window { surface, column } => {
                        let max_depth = (h as f32 * 0.12).max(36.0);
                        let amount = if f.fresh { f.mass * 5.0 } else { f.mass };
                        deposit_snow(&mut surfaces[surface].snowline, column, amount, max_depth, seed);
                    }
                    LandingTarget::Floor { column } => {
                        deposit_snow(snowline, column, f.mass, h as f32 * 0.30, seed);
                    }
                }
                f.y = h as f32 + 10.0;
            } else {
                draw_flake(buf, w, h, f.x, f.y, f.radius, f.alpha);
            }
        } else {
            draw_flake(buf, w, h, f.x, f.y, f.radius, f.alpha);
        }
    }
    flakes.retain(|f| f.y < h as f32 + 5.0);
    if tractor.x > -100.0 && tractor.active { draw_tractor(buf, w, h, tractor.x); }
}

fn wide(s: &str) -> Vec<u16> { s.encode_utf16().chain(Some(0)).collect() }

fn choose_snow_rate(args: &[String]) -> f32 {
    if let Some(value) = args.iter().position(|arg| arg == "--rate")
        .and_then(|index| args.get(index + 1)).and_then(|arg| arg.parse::<f32>().ok())
        .or_else(|| args.iter().find_map(|arg| arg.parse::<f32>().ok()))
        .filter(|value| value.is_finite()) {
        return value.clamp(1.0, 60.0);
    }
    print!("Сколько снежинок в секунду? Введи число от 1 до 60 [12]: ");
    let _ = io::stdout().flush();
    let mut input = String::new();
    let rate = if io::stdin().read_line(&mut input).is_ok() {
        input.trim().parse::<f32>().ok().filter(|value| value.is_finite()).unwrap_or(12.0)
    } else { 12.0 };
    rate.clamp(1.0, 60.0)
}

fn read_controls() -> Receiver<Control> {
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let stdin = io::stdin();
        for line in stdin.lock().lines().flatten() {
            let mut parts = line.split_whitespace();
            match parts.next() {
                Some("enabled") => { let _ = sender.send(Control::Enabled(parts.next() == Some("1"))); }
                Some("rate") => if let Some(value) = parts.next().and_then(|s| s.parse::<f32>().ok()) {
                    if value.is_finite() { let _ = sender.send(Control::Rate(value.clamp(1.0, 60.0))); }
                },
                Some("tractor") => { let _ = sender.send(Control::Tractor); }
                Some("quit") => { let _ = sender.send(Control::Quit); break; }
                _ => {}
            }
        }
    });
    receiver
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let worker_mode = args.iter().any(|arg| arg == "--worker");
    let excluded_pid = args.iter().position(|arg| arg == "--exclude-pid")
        .and_then(|index| args.get(index + 1)).and_then(|arg| arg.parse::<u32>().ok()).unwrap_or(0);
    let snow_rate = if worker_mode { choose_worker_rate(&args) } else { choose_snow_rate(&args) };
    let controls = if worker_mode { Some(read_controls()) } else { None };
    unsafe {
        // Keep the prompt available at launch, then dismiss the console while
        // the transparent overlay continues running.
        if !worker_mode { let _ = FreeConsole(); }
        let x = GetSystemMetrics(SM_XVIRTUALSCREEN);
        let y = GetSystemMetrics(SM_YVIRTUALSCREEN);
        let w = GetSystemMetrics(SM_CXVIRTUALSCREEN).max(1) as usize;
        let h = GetSystemMetrics(SM_CYVIRTUALSCREEN).max(1) as usize;
        let view = Rect { left: x, top: y, right: x + w as i32, bottom: y + h as i32 };
        let instance = GetModuleHandleW(ptr::null());
        let class_name = wide("RustSnowOverlayWindow");
        let wc = WindowClassW { style: 0, wnd_proc: Some(window_proc), cls_extra: 0, wnd_extra: 0,
            instance, icon: ptr::null_mut(), cursor: ptr::null_mut(), background: ptr::null_mut(),
            menu_name: ptr::null(), class_name: class_name.as_ptr() };
        if RegisterClassW(&wc) == 0 { return; }
        let hwnd = CreateWindowExW(WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
            class_name.as_ptr(), wide("Snow").as_ptr(), WS_POPUP, x, y, w as i32, h as i32,
            ptr::null_mut(), ptr::null_mut(), instance, ptr::null_mut());
        if hwnd.is_null() { return; }
        let dc = CreateCompatibleDC(ptr::null_mut());
        let info = BitmapInfo { header: BitmapInfoHeader { size: mem::size_of::<BitmapInfoHeader>() as u32,
            width: w as i32, height: -(h as i32), planes: 1, bit_count: 32, compression: BI_RGB,
            size_image: 0, xppm: 0, yppm: 0, used: 0, important: 0 }, colors: [0; 3] };
        let mut pixels = ptr::null_mut();
        let bitmap = CreateDIBSection(dc, &info, DIB_RGB_COLORS, &mut pixels, ptr::null_mut(), 0);
        if bitmap.is_null() || pixels.is_null() { return; }
        SelectObject(dc, bitmap);
        ShowWindow(hwnd, SW_SHOWNOACTIVATE);
        let _ = SetWindowPos(hwnd, HWND_TOPMOST, x, y, w as i32, h as i32, 0x0010 | 0x0040);

        let frame_size = w.saturating_mul(h);
        let buffer = std::slice::from_raw_parts_mut(pixels.cast::<u32>(), frame_size);
        let mut flakes = Vec::new();
        let mut snowline = vec![0.0f32; w];
        let mut window_snow = Vec::new();
        let mut next_window_scan = Instant::now();
        let mut spawn_credit = 0.0f32;
        let mut enabled = true;
        let mut rate = snow_rate;
        let mut tractor = Tractor { x: -200.0, active: false, auto_armed: true };
        let mut seed = Instant::now().elapsed().as_nanos() as u32 ^ (w as u32).wrapping_mul(h as u32);
        let mut last = Instant::now();
        let mut msg: Msg = mem::zeroed();
        let mut running = true;
        while running {
            if let Some(receiver) = controls.as_ref() {
                while let Ok(command) = receiver.try_recv() {
                    match command {
                        Control::Enabled(value) => enabled = value,
                        Control::Rate(value) => rate = value,
                        Control::Tractor => start_tractor(&mut tractor, w),
                        Control::Quit => running = false,
                    }
                }
            }
            while PeekMessageW(&mut msg, ptr::null_mut(), 0, 0, 1) != 0 {
                if msg.message == 0x0012 { running = false; break; }
                TranslateMessage(&msg); DispatchMessageW(&msg);
            }
            if !running { break; }
            let now = Instant::now();
            if now >= next_window_scan {
                let windows = enumerate_windows(hwnd, excluded_pid, view);
                refresh_window_snow(&mut window_snow, windows, view, &mut flakes, &mut seed);
                next_window_scan = now + Duration::from_millis(100);
            }
            let dt = (now - last).as_secs_f32().min(0.05);
            last = now;
            render(buffer, w, h, x, y, &mut flakes, &mut snowline, &mut window_snow,
                dt, enabled, rate, &mut spawn_credit, &mut tractor, &mut seed);
            let dest = Point { x, y };
            let size = Size { cx: w as i32, cy: h as i32 };
            let src = Point { x: 0, y: 0 };
            let blend = Blend { op: AC_SRC_OVER, flags: 0, alpha: 255, format: AC_SRC_ALPHA };
            UpdateLayeredWindow(hwnd, ptr::null_mut(), &dest, &size, dc, &src, 0, &blend, ULW_ALPHA);
            thread::sleep(Duration::from_millis(16));
        }
    }
}

fn choose_worker_rate(args: &[String]) -> f32 {
    args.iter().position(|arg| arg == "--rate")
        .and_then(|index| args.get(index + 1)).and_then(|arg| arg.parse::<f32>().ok())
        .filter(|value| value.is_finite()).unwrap_or(12.0).clamp(1.0, 60.0)
}

unsafe extern "system" fn window_proc(hwnd: Hwnd, msg: u32, w: Wparam, l: Lparam) -> Lresult {
    DefWindowProcW(hwnd, msg, w, l)
}
