#![windows_subsystem = "windows"]
//! 春秋戦国 RTS — Rust + Direct3D11 / Direct2D

mod ai;
mod art;
mod game;
mod geo;
mod gfx;
mod path;
mod platform;
mod rng;
mod save;
mod world;

use std::cell::RefCell;
use windows::core::*;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::HiDpi::*;
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::WindowsAndMessaging::*;

#[derive(Clone, Copy, Debug)]
pub enum Ev {
    MouseMove(f32, f32),
    Down(u8, f32, f32),
    Up(u8, f32, f32),
    Wheel(f32),
    Key(u32, bool),
    Char(char),
    Resize(u32, u32),
}

thread_local! {
    static EVENTS: RefCell<Vec<Ev>> = const { RefCell::new(Vec::new()) };
}

fn push(e: Ev) {
    EVENTS.with(|v| v.borrow_mut().push(e));
}

fn lp_xy(lp: LPARAM) -> (f32, f32) {
    let x = (lp.0 & 0xffff) as i16 as f32;
    let y = ((lp.0 >> 16) & 0xffff) as i16 as f32;
    (x, y)
}

extern "system" fn wndproc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    unsafe {
        match msg {
            WM_DESTROY => {
                PostQuitMessage(0);
                LRESULT(0)
            }
            WM_SIZE => {
                let w = (lp.0 & 0xffff) as u32;
                let h = ((lp.0 >> 16) & 0xffff) as u32;
                push(Ev::Resize(w, h));
                LRESULT(0)
            }
            WM_MOUSEMOVE => {
                let (x, y) = lp_xy(lp);
                push(Ev::MouseMove(x, y));
                LRESULT(0)
            }
            WM_LBUTTONDOWN | WM_RBUTTONDOWN | WM_MBUTTONDOWN => {
                let (x, y) = lp_xy(lp);
                SetCapture(hwnd);
                let b = match msg {
                    WM_LBUTTONDOWN => 0,
                    WM_RBUTTONDOWN => 1,
                    _ => 2,
                };
                push(Ev::Down(b, x, y));
                LRESULT(0)
            }
            WM_LBUTTONUP | WM_RBUTTONUP | WM_MBUTTONUP => {
                let (x, y) = lp_xy(lp);
                let _ = ReleaseCapture();
                let b = match msg {
                    WM_LBUTTONUP => 0,
                    WM_RBUTTONUP => 1,
                    _ => 2,
                };
                push(Ev::Up(b, x, y));
                LRESULT(0)
            }
            WM_MOUSEWHEEL => {
                let d = ((wp.0 >> 16) & 0xffff) as i16 as f32 / 120.0;
                push(Ev::Wheel(d));
                LRESULT(0)
            }
            WM_KEYDOWN | WM_SYSKEYDOWN => {
                if wp.0 as u32 == VK_F4.0 as u32 && msg == WM_SYSKEYDOWN {
                    return DefWindowProcW(hwnd, msg, wp, lp);
                }
                push(Ev::Key(wp.0 as u32, true));
                LRESULT(0)
            }
            WM_KEYUP | WM_SYSKEYUP => {
                push(Ev::Key(wp.0 as u32, false));
                LRESULT(0)
            }
            WM_CHAR => {
                if let Some(c) = char::from_u32(wp.0 as u32) {
                    push(Ev::Char(c));
                }
                LRESULT(0)
            }
            WM_ERASEBKGND => LRESULT(1),
            _ => DefWindowProcW(hwnd, msg, wp, lp),
        }
    }
}

fn main() -> Result<()> {
    unsafe {
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        let inst = GetModuleHandleW(None)?;
        let class = w!("YYEmpireWnd");
        // アプリのアイコン（build.rs が assets/app_icon.ico を資源番号1として埋め込む）
        let icon = |sx: i32, sy: i32| -> HICON {
            LoadImageW(Some(inst.into()), PCWSTR(1 as *const u16), IMAGE_ICON, sx, sy, LR_DEFAULTCOLOR)
                .map(|h| HICON(h.0))
                .unwrap_or_default()
        };
        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(wndproc),
            hInstance: inst.into(),
            hIcon: icon(GetSystemMetrics(SM_CXICON), GetSystemMetrics(SM_CYICON)),
            hIconSm: icon(GetSystemMetrics(SM_CXSMICON), GetSystemMetrics(SM_CYSMICON)),
            hCursor: LoadCursorW(None, IDC_ARROW)?,
            hbrBackground: HBRUSH(GetStockObject(BLACK_BRUSH).0),
            lpszClassName: class,
            ..Default::default()
        };
        RegisterClassExW(&wc);
        let (w, h) = (1600, 900);
        let mut rc = RECT { left: 0, top: 0, right: w, bottom: h };
        let _ = AdjustWindowRect(&mut rc, WS_OVERLAPPEDWINDOW, false);
        let hwnd = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            class,
            w!("春秋戦国 — 中華統一への道"),
            WS_OVERLAPPEDWINDOW | WS_VISIBLE,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            rc.right - rc.left,
            rc.bottom - rc.top,
            None,
            None,
            Some(inst.into()),
            None,
        )?;
        let mut cr = RECT::default();
        GetClientRect(hwnd, &mut cr)?;
        let gfx = gfx::Gfx::new(hwnd, (cr.right - cr.left).max(1) as u32, (cr.bottom - cr.top).max(1) as u32)?;
        let mut game = game::Game::new(gfx);
        let mut last = std::time::Instant::now();
        let mut msg = MSG::default();
        'outer: loop {
            while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                if msg.message == WM_QUIT {
                    break 'outer;
                }
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
            let evs: Vec<Ev> = EVENTS.with(|v| std::mem::take(&mut *v.borrow_mut()));
            let now = std::time::Instant::now();
            let dt = (now - last).as_secs_f32().min(0.1);
            last = now;
            let focused = GetForegroundWindow() == hwnd;
            if game.frame(&evs, dt, focused) {
                break;
            }
            if IsIconic(hwnd).as_bool() {
                std::thread::sleep(std::time::Duration::from_millis(30));
            }
        }
    }
    Ok(())
}
