//! 記録（セーブデータ）の置き場所とファイルダイアログ（Windows）
//!
//! 記録は %APPDATA%\yyempire\saves\slot{N}.yysav に保存し、見出しと保存日時を slot{N}.txt に置く。
//! 書き込みは一時ファイルに書いてから置き換えるので、途中で落ちても前の記録は壊れない。

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use windows::core::{w, PCWSTR, PWSTR};
use windows::Win32::System::SystemInformation::GetLocalTime;
use windows::Win32::UI::Controls::Dialogs::*;
use windows::Win32::UI::Input::KeyboardAndMouse::GetActiveWindow;

/// 記録画面に出す説明
pub const STORAGE_NOTE: &str = "記録はこのパソコン（%APPDATA%\\yyempire\\saves）に保存される。別のパソコンへ移すときや控えには、書き出したファイルを読み込む";

const SLOTS: usize = 4;

thread_local! {
    /// 読み込んだ記録（次のフレームで反映する）
    static LOADED: RefCell<Option<Vec<u8>>> = const { RefCell::new(None) };
    static NOTES: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
    /// 枠の見出しと保存日時（ファイルを毎フレーム読まないよう覚えておく）
    static INFO: RefCell<Option<Vec<Option<(String, String)>>>> = const { RefCell::new(None) };
}

fn note(s: String) {
    NOTES.with(|n| n.borrow_mut().push(s));
}

/// 記録の置き場所（無ければ作る）
fn save_dir() -> PathBuf {
    let base = std::env::var_os("APPDATA").map(PathBuf::from).or_else(|| std::env::current_exe().ok().and_then(|p| p.parent().map(Path::to_path_buf))).unwrap_or_else(|| PathBuf::from("."));
    let d = base.join("yyempire").join("saves");
    let _ = std::fs::create_dir_all(&d);
    d
}

fn data_path(slot: u8) -> PathBuf {
    save_dir().join(format!("slot{}.yysav", slot))
}

fn meta_path(slot: u8) -> PathBuf {
    save_dir().join(format!("slot{}.txt", slot))
}

/// いまの日時（ローカル時刻）
fn now_text() -> String {
    let t = unsafe { GetLocalTime() };
    format!("{}/{:02}/{:02} {:02}:{:02}", t.wYear, t.wMonth, t.wDay, t.wHour, t.wMinute)
}

/// 一時ファイルに書いてから置き換える
fn write_atomic(path: &Path, data: &[u8]) -> std::io::Result<()> {
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, data)?;
    std::fs::rename(&tmp, path)
}

fn read_info(slot: u8) -> Option<(String, String)> {
    if !data_path(slot).exists() {
        return None;
    }
    let s = std::fs::read_to_string(meta_path(slot)).unwrap_or_default();
    let mut it = s.lines();
    let meta = it.next().unwrap_or("（見出しなし）").to_string();
    let date = it.next().unwrap_or("").to_string();
    Some((meta, date))
}

fn with_info<T>(f: impl FnOnce(&mut Vec<Option<(String, String)>>) -> T) -> T {
    INFO.with(|i| {
        let mut i = i.borrow_mut();
        let v = i.get_or_insert_with(|| (0..SLOTS as u8).map(read_info).collect());
        f(v)
    })
}

/// 記録を枠に保存する
pub fn save_slot(slot: u8, data: &[u8], meta: &str) {
    let date = now_text();
    let r = write_atomic(&data_path(slot), data).and_then(|_| write_atomic(&meta_path(slot), format!("{}\n{}\n", meta, date).as_bytes()));
    match r {
        Ok(()) => with_info(|v| {
            if let Some(e) = v.get_mut(slot as usize) {
                *e = Some((meta.to_string(), date));
            }
        }),
        Err(e) => note(format!("記録できなかった：{}", e)),
    }
}

/// 枠の記録を読み込む（次のフレームで反映する）
pub fn load_slot(slot: u8) {
    match std::fs::read(data_path(slot)) {
        Ok(d) => LOADED.with(|l| *l.borrow_mut() = Some(d)),
        Err(e) => note(format!("読み込めなかった：{}", e)),
    }
}

/// 枠の見出しと保存日時（空きなら None）
pub fn slot_info(slot: u8) -> Option<(String, String)> {
    with_info(|v| v.get(slot as usize).cloned().flatten())
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn from_wide(buf: &[u16]) -> String {
    let n = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..n])
}

const FILTER: &str = "春秋戦国の記録 (*.yysav)\0*.yysav\0すべてのファイル (*.*)\0*.*\0\0";

/// 枠の記録をファイルとして書き出す（保存先を尋ねる）
pub fn export_slot(slot: u8) {
    let src = data_path(slot);
    let data = match std::fs::read(&src) {
        Ok(d) => d,
        Err(_) => {
            note("この枠には記録がない".into());
            return;
        }
    };
    let (date, _) = slot_info(slot).map(|(m, d)| (d, m)).unwrap_or_default();
    let stamp: String = date.chars().filter(|c| c.is_ascii_digit()).collect();
    let name = format!("yyempire-{}-{}.yysav", if slot == 0 { "auto".to_string() } else { format!("slot{}", slot) }, stamp);
    let mut buf = [0u16; 1024];
    let w = wide(&name);
    buf[..w.len().min(1023)].copy_from_slice(&w[..w.len().min(1023)]);
    let filter: Vec<u16> = FILTER.encode_utf16().collect();
    let mut ofn = OPENFILENAMEW {
        lStructSize: std::mem::size_of::<OPENFILENAMEW>() as u32,
        hwndOwner: unsafe { GetActiveWindow() },
        lpstrFilter: PCWSTR(filter.as_ptr()),
        lpstrFile: PWSTR(buf.as_mut_ptr()),
        nMaxFile: buf.len() as u32,
        lpstrDefExt: w!("yysav"),
        lpstrTitle: w!("記録を書き出す"),
        Flags: OFN_OVERWRITEPROMPT | OFN_PATHMUSTEXIST | OFN_NOCHANGEDIR,
        ..Default::default()
    };
    if !unsafe { GetSaveFileNameW(&mut ofn) }.as_bool() {
        return;
    }
    let path = from_wide(&buf);
    match std::fs::write(&path, data) {
        Ok(()) => note(format!("書き出した：{}", path)),
        Err(e) => note(format!("書き出せなかった：{}", e)),
    }
}

/// 記録ファイルを選んで読み込む
pub fn import_file() {
    let mut buf = [0u16; 1024];
    let filter: Vec<u16> = FILTER.encode_utf16().collect();
    let mut ofn = OPENFILENAMEW {
        lStructSize: std::mem::size_of::<OPENFILENAMEW>() as u32,
        hwndOwner: unsafe { GetActiveWindow() },
        lpstrFilter: PCWSTR(filter.as_ptr()),
        lpstrFile: PWSTR(buf.as_mut_ptr()),
        nMaxFile: buf.len() as u32,
        lpstrTitle: w!("記録ファイルを読み込む"),
        Flags: OFN_FILEMUSTEXIST | OFN_PATHMUSTEXIST | OFN_NOCHANGEDIR,
        ..Default::default()
    };
    if !unsafe { GetOpenFileNameW(&mut ofn) }.as_bool() {
        return;
    }
    match std::fs::read(from_wide(&buf)) {
        Ok(d) => LOADED.with(|l| *l.borrow_mut() = Some(d)),
        Err(e) => note(format!("読み込めなかった：{}", e)),
    }
}

/// 読み込んだ記録を受け取る
pub fn take_loaded() -> Option<Vec<u8>> {
    LOADED.with(|l| l.borrow_mut().take())
}

/// 通知を受け取る
pub fn take_notes() -> Vec<String> {
    NOTES.with(|n| std::mem::take(&mut *n.borrow_mut()))
}
