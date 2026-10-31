//! 画面・入力・HUD（独自実装の UI）

mod tutorial;

use crate::geo::*;
use crate::gfx::{Font, Gfx, Texture};
use crate::rng::hash2;
use crate::world::*;
use crate::Ev;
use windows::Win32::Graphics::Direct2D::ID2D1Bitmap;

const TEX_SCALE: usize = 4;
const TOP_H: f32 = 34.0;
const BOT_H: f32 = 172.0;
const MM_W: f32 = 196.0;
const MM_H: f32 = 168.0;

const VK_BACK: u32 = 0x08;
const VK_TAB: u32 = 0x09;
const VK_SHIFT: u32 = 0x10;
const VK_CONTROL: u32 = 0x11;
const VK_ESCAPE: u32 = 0x1B;
const VK_SPACE: u32 = 0x20;
const VK_HOME: u32 = 0x24;
const VK_LEFT: u32 = 0x25;
const VK_UP: u32 = 0x26;
const VK_RIGHT: u32 = 0x27;
const VK_DOWN: u32 = 0x28;
const VK_DELETE: u32 = 0x2E;
const VK_F1: u32 = 0x70;
const VK_F2: u32 = 0x71;
const VK_F11: u32 = 0x7A;
const VK_ADD: u32 = 0x6B;
const VK_SUB: u32 = 0x6D;
const VK_OEM_PLUS: u32 = 0xBB;
const VK_OEM_MINUS: u32 = 0xBD;
const VK_OEM_PERIOD: u32 = 0xBE;

const RES_NAMES: [&str; 3] = ["糧", "木", "銅"];
const RES_COLORS: [[f32; 4]; 3] = [[0.95, 0.85, 0.35, 1.0], [0.55, 0.85, 0.45, 1.0], [0.95, 0.6, 0.35, 1.0]];

#[derive(Clone, Copy, PartialEq, Eq)]
enum Screen {
    Title,
    Playing,
}

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Normal,
    Place(BKind),
    AttackMove,
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Act {
    Train(UnitKind),
    Place(BKind),
    Upgrade,
    Convert,
    Stop,
    AttackMove,
    Speed(u32),
    Diplo,
    Help,
    War(usize),
    Peace(usize),
    Start,
    PickNation(usize),
    Home,
    Restart,
    CloseOverlay,
    Demolish,
    IdleWorker,
    Tutorial,
    TutNext,
    TutQuit,
    Scale(i32),
    Fullscreen,
    Noop,
}

struct Button {
    x0: f32,
    y0: f32,
    x1: f32,
    y1: f32,
    label: String,
    sub: String,
    key: u32,
    enabled: bool,
    act: Act,
    tip: String,
}

pub struct Game {
    gfx: Gfx,
    world: World,
    seed: u32,
    terrain_px: Vec<u32>,
    terrain_tex: Texture,
    overlay_px: Vec<u32>,
    overlay_tex: Texture,
    overlay_ver: u32,
    minimap_px: Vec<u32>,
    minimap_bmp: Option<ID2D1Bitmap>,
    minimap_gen: u32,
    minimap_ver: u32,
    minimap_t: f32,
    minimap_fog: u32,
    /// 視界：0=未踏、1=探索済み（霧）、2=視界内
    fog: Vec<u8>,
    fog_px: Vec<u32>,
    fog_tex: Texture,
    fog_t: f32,
    fog_ver: u32,
    cam: V2,
    zoom: f32,
    mouse: (f32, f32),
    keys: [bool; 256],
    ldown: Option<(f32, f32)>,
    mdrag: Option<(f32, f32, V2)>,
    mm_drag: bool,
    screen: Screen,
    mode: Mode,
    sel: Vec<Handle>,
    sel_b: Option<Handle>,
    groups: Vec<Vec<Handle>>,
    speed: u32,
    paused: bool,
    acc: f32,
    show_diplo: bool,
    show_help: bool,
    title_sel: Option<usize>,
    notice: Option<(String, f32)>,
    buttons: Vec<Button>,
    last_click: (f32, Option<Handle>),
    clock: f32,
    marker: Option<(V2, f32, bool)>,
    idle_cycle: usize,
    /// 自動撮影（YY_AUTO="国番号,秒,出力パス[,ズーム]"）
    auto: Option<(Option<usize>, f32, String, f32)>,
    script: Vec<(f32, Ev)>,
    tut: Option<tutorial::Tut>,
}

fn lerp3(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
}

fn rgba(c: [f32; 3], a: f32) -> u32 {
    let r = (c[0].clamp(0.0, 1.0) * 255.0) as u32;
    let g = (c[1].clamp(0.0, 1.0) * 255.0) as u32;
    let b = (c[2].clamp(0.0, 1.0) * 255.0) as u32;
    let a = (a.clamp(0.0, 1.0) * 255.0) as u32;
    r | (g << 8) | (b << 16) | (a << 24)
}

fn bgra(c: [f32; 3]) -> u32 {
    let r = (c[0].clamp(0.0, 1.0) * 255.0) as u32;
    let g = (c[1].clamp(0.0, 1.0) * 255.0) as u32;
    let b = (c[2].clamp(0.0, 1.0) * 255.0) as u32;
    b | (g << 8) | (r << 16) | (255 << 24)
}

fn c4(c: [f32; 3], a: f32) -> [f32; 4] {
    [c[0], c[1], c[2], a]
}

fn darker(c: [f32; 3], k: f32) -> [f32; 3] {
    [c[0] * k, c[1] * k, c[2] * k]
}

/// タイルの基本色
fn base_color(t: Terrain, x: i32, y: i32, shade: f32) -> [f32; 3] {
    let lat_t = (y as f32 / MAP_H as f32).clamp(0.0, 1.0);
    match t {
        Terrain::Sea => lerp3([0.10, 0.22, 0.42], [0.14, 0.30, 0.52], shade),
        Terrain::Lake => [0.20, 0.38, 0.58],
        Terrain::River => [0.24, 0.44, 0.66],
        Terrain::Plain => {
            let north = [0.70, 0.64, 0.42];
            let south = [0.40, 0.58, 0.30];
            let c = lerp3(north, south, ((lat_t - 0.30) * 2.2).clamp(0.0, 1.0));
            lerp3(c, darker(c, 0.85), shade)
        }
        Terrain::Steppe => lerp3([0.74, 0.70, 0.48], [0.66, 0.64, 0.42], shade),
        Terrain::Forest => lerp3([0.16, 0.36, 0.17], [0.22, 0.42, 0.20], shade),
        Terrain::Hill => lerp3([0.58, 0.50, 0.34], [0.50, 0.44, 0.30], shade),
        Terrain::Mountain => {
            let h = hash2(x, y, 99);
            if h > 0.93 {
                [0.78, 0.76, 0.74]
            } else {
                lerp3([0.42, 0.38, 0.35], [0.52, 0.48, 0.44], h)
            }
        }
    }
}

impl Game {
    pub fn new(gfx: Gfx) -> Game {
        let seed = 20240 + (std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0) % 1000) as u32;
        let world = World::new(seed);
        let tw = (MAP_W * TEX_SCALE) as u32;
        let th = (MAP_H * TEX_SCALE) as u32;
        let terrain_px = vec![0u32; (tw * th) as usize];
        let overlay_px = vec![0u32; MAP_W * MAP_H];
        let terrain_tex = gfx.create_texture(tw, th, &terrain_px).expect("texture");
        let overlay_tex = gfx.create_texture(MAP_W as u32, MAP_H as u32, &overlay_px).expect("texture");
        let fog_px = vec![0u32; MAP_W * MAP_H];
        let fog_tex = gfx.create_texture(MAP_W as u32, MAP_H as u32, &fog_px).expect("texture");
        let mut g = Game {
            gfx,
            world,
            seed,
            terrain_px,
            terrain_tex,
            overlay_px,
            overlay_tex,
            overlay_ver: u32::MAX,
            minimap_px: vec![0; MAP_W * MAP_H],
            minimap_bmp: None,
            minimap_gen: 0,
            minimap_ver: u32::MAX,
            minimap_t: 0.0,
            minimap_fog: 0,
            fog: vec![2; MAP_W * MAP_H],
            fog_px,
            fog_tex,
            fog_t: 0.0,
            fog_ver: 0,
            cam: v2(MAP_W as f32 / 2.0, MAP_H as f32 / 2.0),
            zoom: 4.0,
            mouse: (-100.0, -100.0),
            keys: [false; 256],
            ldown: None,
            mdrag: None,
            mm_drag: false,
            screen: Screen::Title,
            mode: Mode::Normal,
            sel: Vec::new(),
            sel_b: None,
            groups: vec![Vec::new(); 10],
            speed: 1,
            paused: false,
            acc: 0.0,
            show_diplo: false,
            show_help: false,
            title_sel: None,
            notice: None,
            buttons: Vec::new(),
            last_click: (-10.0, None),
            clock: 0.0,
            marker: None,
            idle_cycle: 0,
            script: Vec::new(),
            tut: None,
            auto: std::env::var("YY_AUTO").ok().and_then(|v| {
                let p: Vec<&str> = v.split(',').collect();
                Some((p.first()?.parse().ok(), p.get(1)?.parse().ok()?, p.get(2)?.to_string(), p.get(3).and_then(|z| z.parse().ok()).unwrap_or(22.0)))
            }),
        };
        g.rebuild_terrain();
        g.fit_title_camera();
        if std::env::var("YY_FULL").is_ok() {
            g.gfx.toggle_fullscreen();
        }
        g
    }

    fn fit_title_camera(&mut self) {
        let (w, h) = (self.gfx.width as f32, self.gfx.height as f32);
        let avail_w = (w - 380.0).max(300.0);
        self.zoom = (avail_w / MAP_W as f32).min(h / MAP_H as f32) * 0.98;
        self.cam = v2(MAP_W as f32 / 2.0 + 380.0 / 2.0 / self.zoom, MAP_H as f32 / 2.0);
    }

    fn rebuild_terrain(&mut self) {
        let tw = MAP_W * TEX_SCALE;
        for y in 0..MAP_H as i32 {
            for x in 0..MAP_W as i32 {
                self.paint_tile(x, y, tw);
            }
        }
        let t = &self.terrain_tex;
        self.gfx.update_texture(t, 0, 0, t.w, t.h, &self.terrain_px);
        self.overlay_ver = u32::MAX;
        self.minimap_ver = u32::MAX;
    }

    fn paint_tile(&mut self, x: i32, y: i32, tw: usize) {
        let i = Map::idx(x, y);
        let t = self.world.map.terrain[i];
        let shade = self.world.map.shade[i];
        let base = base_color(t, x, y, shade);
        for py in 0..TEX_SCALE as i32 {
            for px in 0..TEX_SCALE as i32 {
                let gx = x * TEX_SCALE as i32 + px;
                let gy = y * TEX_SCALE as i32 + py;
                let n = hash2(gx, gy, 7) - 0.5;
                let mut c = [base[0] + n * 0.06, base[1] + n * 0.06, base[2] + n * 0.05];
                match t {
                    Terrain::Forest => {
                        if hash2(gx, gy, 13) > 0.55 {
                            c = darker(c, 0.7);
                        }
                    }
                    Terrain::Mountain => {
                        // 斜面の陰影
                        if px + py < 3 {
                            c = [c[0] * 1.15, c[1] * 1.15, c[2] * 1.15];
                        } else if px + py > 4 {
                            c = darker(c, 0.75);
                        }
                    }
                    Terrain::Hill => {
                        if py == 0 && hash2(x, y, 5) > 0.5 {
                            c = [c[0] * 1.1, c[1] * 1.1, c[2] * 1.1];
                        }
                    }
                    Terrain::Sea | Terrain::Lake | Terrain::River => {
                        if hash2(gx / 2, gy, 21) > 0.93 {
                            c = [c[0] * 1.2, c[1] * 1.2, c[2] * 1.2];
                        }
                    }
                    _ => {}
                }
                self.terrain_px[gy as usize * tw + gx as usize] = rgba(c, 1.0);
            }
        }
    }

    fn update_dirty_terrain(&mut self) {
        if self.world.terrain_dirty.is_empty() {
            return;
        }
        let tw = MAP_W * TEX_SCALE;
        let dirty = std::mem::take(&mut self.world.terrain_dirty);
        for &(x, y) in dirty.iter() {
            self.paint_tile(x, y, tw);
            let s = TEX_SCALE;
            let mut buf = vec![0u32; s * s];
            for py in 0..s {
                for px in 0..s {
                    buf[py * s + px] = self.terrain_px[(y as usize * s + py) * tw + x as usize * s + px];
                }
            }
            self.gfx.update_texture(&self.terrain_tex, (x as usize * s) as u32, (y as usize * s) as u32, s as u32, s as u32, &buf);
        }
        self.minimap_ver = u32::MAX;
    }

    fn update_overlay(&mut self) {
        if self.overlay_ver == self.world.territory_version {
            return;
        }
        self.overlay_ver = self.world.territory_version;
        let w = &self.world;
        for y in 0..MAP_H as i32 {
            for x in 0..MAP_W as i32 {
                let i = Map::idx(x, y);
                let o = w.owner[i];
                if o == NONE {
                    self.overlay_px[i] = 0;
                    continue;
                }
                let mut border = false;
                for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                    let (nx, ny) = (x + dx, y + dy);
                    if Map::inb(nx, ny) {
                        let no = w.owner[Map::idx(nx, ny)];
                        if no != o && w.map.terrain[Map::idx(nx, ny)] != Terrain::Sea {
                            border = true;
                        }
                    }
                }
                let c = w.nations[o as usize].color;
                self.overlay_px[i] = if border { rgba(c, 0.75) } else { rgba(c, 0.22) };
            }
        }
        self.gfx.update_texture(&self.overlay_tex, 0, 0, MAP_W as u32, MAP_H as u32, &self.overlay_px);
    }

    fn update_minimap(&mut self, dt: f32) {
        self.minimap_t -= dt;
        let gen_changed = self.minimap_gen != self.gfx.d2d_gen;
        let unchanged = self.minimap_ver == self.world.territory_version && self.minimap_fog == self.fog_ver;
        if !gen_changed && (self.minimap_t > 0.0 || unchanged) {
            return;
        }
        self.minimap_t = 1.0;
        self.minimap_ver = self.world.territory_version;
        self.minimap_fog = self.fog_ver;
        let fogged = self.fog_active();
        let w = &self.world;
        for y in 0..MAP_H as i32 {
            for x in 0..MAP_W as i32 {
                let i = Map::idx(x, y);
                let mut c = base_color(w.map.terrain[i], x, y, w.map.shade[i]);
                let o = w.owner[i];
                if o != NONE {
                    c = lerp3(c, w.nations[o as usize].color, 0.55);
                }
                if fogged {
                    match self.fog[i] {
                        0 => c = [0.06, 0.06, 0.07],
                        1 => c = darker(c, 0.55),
                        _ => {}
                    }
                }
                self.minimap_px[i] = bgra(c);
            }
        }
        if gen_changed || self.minimap_bmp.is_none() {
            self.minimap_bmp = self.gfx.create_bitmap(MAP_W as u32, MAP_H as u32, &self.minimap_px);
            self.minimap_gen = self.gfx.d2d_gen;
        } else if let Some(b) = &self.minimap_bmp {
            self.gfx.update_bitmap(b, MAP_W as u32, &self.minimap_px);
        }
    }

    // ------------------------------------------------------------ 視界（戦場の霧）
    /// 霧を表示・適用するか（タイトル画面と終局後は全域を見せる）
    fn fog_active(&self) -> bool {
        self.screen == Screen::Playing && self.world.game_over.is_none()
    }

    fn reset_fog(&mut self) {
        self.fog.fill(0);
        // 諸侯の都の所在は天下に知られている
        let homes: Vec<V2> = self.world.nations.iter().filter(|n| n.alive).map(|n| n.home).collect();
        for h in homes {
            let (x, y) = h.tile();
            Self::reveal(&mut self.fog, x, y, 3, 1);
        }
        self.fog_t = 0.0;
        self.update_fog(0.0);
    }

    fn reveal(fog: &mut [u8], cx: i32, cy: i32, r: i32, level: u8) {
        for dy in -r..=r {
            for dx in -r..=r {
                if dx * dx + dy * dy > r * r + r {
                    continue;
                }
                let (x, y) = (cx + dx, cy + dy);
                if Map::inb(x, y) {
                    let f = &mut fog[Map::idx(x, y)];
                    *f = (*f).max(level);
                }
            }
        }
    }

    fn update_fog(&mut self, dt: f32) {
        self.fog_t -= dt;
        if self.fog_t > 0.0 {
            return;
        }
        self.fog_t = 0.15;
        for f in self.fog.iter_mut() {
            if *f == 2 {
                *f = 1;
            }
        }
        let pl = self.world.player;
        for u in self.world.units.iter() {
            if u.alive && u.nation as usize == pl {
                let (x, y) = u.pos.tile();
                Self::reveal(&mut self.fog, x, y, u.kind.vision(), 2);
            }
        }
        for b in self.world.blds.iter() {
            if b.alive && b.nation as usize == pl {
                let (x, y) = b.center().tile();
                let r = if b.kind.city() { b.kind.radius() as i32 + 3 } else { 5 };
                Self::reveal(&mut self.fog, x, y, r, 2);
            }
        }
        let fog_c = [0.04, 0.05, 0.07];
        for (px, &f) in self.fog_px.iter_mut().zip(self.fog.iter()) {
            *px = match f {
                0 => rgba(fog_c, 1.0),
                1 => rgba(fog_c, 0.5),
                _ => 0,
            };
        }
        self.gfx.update_texture(&self.fog_tex, 0, 0, MAP_W as u32, MAP_H as u32, &self.fog_px);
        self.fog_ver = self.fog_ver.wrapping_add(1);
    }

    /// その地点が今見えているか
    fn seen(&self, p: V2) -> bool {
        if !self.fog_active() {
            return true;
        }
        let (x, y) = p.tile();
        Map::inb(x, y) && self.fog[Map::idx(x, y)] == 2
    }

    /// その地点を一度でも見たか
    fn explored(&self, p: V2) -> bool {
        if !self.fog_active() {
            return true;
        }
        let (x, y) = p.tile();
        Map::inb(x, y) && self.fog[Map::idx(x, y)] >= 1
    }

    // ------------------------------------------------------------ 座標
    fn screen_to_world(&self, sx: f32, sy: f32) -> V2 {
        let (w, h) = (self.gfx.width as f32, self.gfx.height as f32);
        v2(self.cam.x + (sx - w / 2.0) / self.zoom, self.cam.y + (sy - h / 2.0) / self.zoom)
    }
    fn world_to_screen(&self, p: V2) -> (f32, f32) {
        let (w, h) = (self.gfx.width as f32, self.gfx.height as f32);
        ((p.x - self.cam.x) * self.zoom + w / 2.0, (p.y - self.cam.y) * self.zoom + h / 2.0)
    }
    fn clamp_cam(&mut self) {
        self.cam.x = self.cam.x.clamp(0.0, MAP_W as f32);
        self.cam.y = self.cam.y.clamp(0.0, MAP_H as f32);
    }
    fn mm_rect(&self) -> (f32, f32, f32, f32) {
        let h = self.gfx.height as f32;
        (2.0, h - MM_H - 2.0, 2.0 + MM_W, h - 2.0)
    }
    fn over_ui(&self, x: f32, y: f32) -> bool {
        if self.screen == Screen::Title {
            return x > self.gfx.width as f32 - 380.0;
        }
        if let Some(t) = &self.tut {
            let (x0, y0, x1, y1) = t.panel;
            if x >= x0 && x <= x1 && y >= y0 && y <= y1 {
                return true;
            }
        }
        if self.show_diplo || self.show_help || self.world.game_over.is_some() {
            return true;
        }
        y < TOP_H || y > self.gfx.height as f32 - BOT_H
    }

    fn notice(&mut self, s: &str) {
        if !s.is_empty() {
            self.notice = Some((s.to_string(), 3.0));
        }
    }

    // ------------------------------------------------------------ フレーム
    /// true を返すと終了
    pub fn frame(&mut self, evs: &[Ev], dt: f32, focused: bool) -> bool {
        self.clock += dt;
        // OS からのマウス座標（実ピクセル）を UI 座標系へ
        let ui = self.gfx.ui;
        let mut evs: Vec<Ev> = evs
            .iter()
            .map(|e| match *e {
                Ev::MouseMove(x, y) => Ev::MouseMove(x / ui, y / ui),
                Ev::Down(b, x, y) => Ev::Down(b, x / ui, y / ui),
                Ev::Up(b, x, y) => Ev::Up(b, x / ui, y / ui),
                o => o,
            })
            .collect();
        while !self.script.is_empty() && self.script[0].0 <= self.clock {
            evs.push(self.script.remove(0).1);
        }
        for e in evs {
            if self.handle_event(e) {
                return true;
            }
        }
        if self.screen == Screen::Playing {
            self.scroll(dt, focused);
            if !self.paused && self.world.game_over.is_none() && !self.show_help {
                self.acc += dt * self.speed as f32;
                let mut steps = 0;
                while self.acc >= DT && steps < 16 {
                    self.world.step();
                    self.acc -= DT;
                    steps += 1;
                }
                if steps >= 16 {
                    self.acc = 0.0;
                }
            }
            self.update_fog(dt);
            self.update_tutorial(dt);
            let pl = self.world.player;
            let sel: Vec<Handle> = self.sel.iter().copied().filter(|&h| self.world.unit(h).map_or(false, |u| u.nation as usize == pl || self.seen(u.pos))).collect();
            self.sel = sel;
            if let Some(b) = self.sel_b {
                if self.world.bld(b).is_none() {
                    self.sel_b = None;
                }
            }
        }
        if let Some((_, t)) = &mut self.notice {
            *t -= dt;
            if *t <= 0.0 {
                self.notice = None;
            }
        }
        if let Some(m) = &mut self.marker {
            m.1 -= dt;
            if m.1 <= 0.0 {
                self.marker = None;
            }
        }
        if let Some((nation, secs, _, z)) = self.auto.clone() {
            if self.screen == Screen::Title {
                if let Some(step) = std::env::var("YY_TUT").ok().and_then(|v| v.parse::<usize>().ok()) {
                    self.start_tutorial();
                    self.tut_enter(step);
                    self.zoom = z;
                    if let Some(k) = std::env::var("YY_PRESS").ok().and_then(|v| u32::from_str_radix(&v, 16).ok()) {
                        self.script = vec![(self.clock + 0.5, Ev::Key(k, true)), (self.clock + 0.55, Ev::Key(k, false))];
                    }
                }
            }
            if self.screen == Screen::Title {
                if let Some(n) = nation {
                    self.title_sel = Some(n);
                    self.do_act(Act::Start);
                    self.speed = 8;
                    self.zoom = z;
                    if std::env::var("YY_DEMO").is_ok() {
                        self.speed = 1;
                        let (cx, cy) = (self.gfx.width as f32 / 2.0, self.gfx.height as f32 / 2.0);
                        let c = self.clock;
                        self.script = vec![
                            (c + 0.3, Ev::MouseMove(cx, cy)),
                            (c + 0.4, Ev::Down(0, cx, cy)),
                            (c + 0.45, Ev::Up(0, cx, cy)),
                            (c + 0.6, Ev::Key(0x51, true)),
                            (c + 0.65, Ev::Key(0x51, false)),
                            (c + 0.9, Ev::Down(0, cx - 160.0, cy - 160.0)),
                            (c + 0.95, Ev::MouseMove(cx + 160.0, cy + 160.0)),
                            (c + 1.0, Ev::Up(0, cx + 160.0, cy + 160.0)),
                            (c + 1.2, Ev::Key(0x51, true)),
                            (c + 1.25, Ev::Key(0x51, false)),
                            (c + 1.4, Ev::MouseMove(cx + 140.0, cy + 80.0)),
                            (c + 1.6, Ev::Down(0, cx + 140.0, cy + 80.0)),
                            (c + 1.65, Ev::Up(0, cx + 140.0, cy + 80.0)),
                            (c + 1.8, Ev::Key(0x57, true)),
                            (c + 1.85, Ev::Key(0x57, false)),
                            (c + 2.0, Ev::MouseMove(cx - 150.0, cy + 60.0)),
                            (c + 2.4, Ev::Key(0x71, true)),
                            (c + 2.45, Ev::Key(0x71, false)),
                        ];
                    }
                }
            }
            if let Some(n) = nation {
                let _ = n;
                // 自国の兵に追従
                if self.clock > 1.0 {
                    let pl = self.world.player;
                    if let Some(u) = self.world.units.iter().find(|u| u.alive && u.nation as usize == pl && u.kind.military()) {
                        let _ = u;
                    }
                }
            }
            if self.clock >= secs {
                self.update_dirty_terrain();
                self.update_overlay();
                self.render_nopresent(dt);
                let path = self.auto.as_ref().unwrap().2.clone();
                let _ = self.gfx.save_backbuffer(&path);
                return true;
            }
        }
        self.update_dirty_terrain();
        self.update_overlay();
        self.render(dt);
        false
    }

    fn scroll(&mut self, dt: f32, focused: bool) {
        let sp = 900.0 / self.zoom * dt;
        let (w, h) = (self.gfx.width as f32, self.gfx.height as f32);
        let (mx, my) = self.mouse;
        let mut d = v2(0.0, 0.0);
        if self.keys[VK_LEFT as usize] {
            d.x -= 1.0;
        }
        if self.keys[VK_RIGHT as usize] {
            d.x += 1.0;
        }
        if self.keys[VK_UP as usize] {
            d.y -= 1.0;
        }
        if self.keys[VK_DOWN as usize] {
            d.y += 1.0;
        }
        if focused && self.mdrag.is_none() && self.ldown.is_none() {
            if mx <= 3.0 && mx >= 0.0 {
                d.x -= 1.0;
            }
            if mx >= w - 4.0 && mx <= w {
                d.x += 1.0;
            }
            if my <= 3.0 && my >= 0.0 {
                d.y -= 1.0;
            }
            if my >= h - 4.0 && my <= h {
                d.y += 1.0;
            }
        }
        self.cam = self.cam + d * sp;
        self.clamp_cam();
    }

    fn handle_event(&mut self, e: Ev) -> bool {
        match e {
            Ev::Resize(w, h) => {
                self.gfx.resize(w, h);
                self.clamp_cam();
                if self.screen == Screen::Title {
                    self.fit_title_camera();
                }
            }
            Ev::MouseMove(x, y) => {
                self.mouse = (x, y);
                if let Some((sx, sy, c0)) = self.mdrag {
                    self.cam = c0 - v2(x - sx, y - sy) * (1.0 / self.zoom);
                    self.clamp_cam();
                }
                if self.mm_drag {
                    self.minimap_jump(x, y);
                }
            }
            Ev::Wheel(d) => {
                if self.screen == Screen::Playing && !self.over_ui(self.mouse.0, self.mouse.1) {
                    let before = self.screen_to_world(self.mouse.0, self.mouse.1);
                    self.zoom = (self.zoom * 1.15f32.powf(d)).clamp(3.0, 64.0);
                    let after = self.screen_to_world(self.mouse.0, self.mouse.1);
                    self.cam = self.cam + (before - after);
                    self.clamp_cam();
                }
            }
            Ev::Down(b, x, y) => {
                self.mouse = (x, y);
                match b {
                    0 => {
                        if let Some(a) = self.button_at(x, y) {
                            self.do_act(a);
                            return false;
                        }
                        if self.screen == Screen::Title {
                            self.title_click(x, y);
                            return false;
                        }
                        let (mx0, my0, mx1, my1) = self.mm_rect();
                        if x >= mx0 && x <= mx1 && y >= my0 && y <= my1 && !self.show_diplo && !self.show_help {
                            self.mm_drag = true;
                            self.minimap_jump(x, y);
                            return false;
                        }
                        if self.over_ui(x, y) {
                            return false;
                        }
                        match self.mode {
                            Mode::Place(k) => {
                                self.try_place(k);
                                if !self.keys[VK_SHIFT as usize] {
                                    self.mode = Mode::Normal;
                                }
                            }
                            Mode::AttackMove => {
                                let p = self.screen_to_world(x, y);
                                let hs = self.own_sel();
                                self.world.order_move(&hs, p, true);
                                self.marker = Some((p, 0.6, true));
                                self.mode = Mode::Normal;
                                if let Some(t) = &mut self.tut {
                                    t.moved = true;
                                }
                            }
                            Mode::Normal => self.ldown = Some((x, y)),
                        }
                    }
                    1 => {
                        if self.mode != Mode::Normal {
                            self.mode = Mode::Normal;
                            return false;
                        }
                        if self.screen != Screen::Playing {
                            return false;
                        }
                        let (mx0, my0, mx1, my1) = self.mm_rect();
                        if x >= mx0 && x <= mx1 && y >= my0 && y <= my1 && !self.show_diplo {
                            let p = v2((x - mx0) / MM_W * MAP_W as f32, (y - my0) / MM_H * MAP_H as f32);
                            self.right_click_world(p);
                            return false;
                        }
                        if !self.over_ui(x, y) {
                            let p = self.screen_to_world(x, y);
                            self.right_click_world(p);
                        }
                    }
                    _ => {
                        if self.screen == Screen::Playing {
                            self.mdrag = Some((x, y, self.cam));
                        }
                    }
                }
            }
            Ev::Up(b, x, y) => {
                self.mouse = (x, y);
                match b {
                    0 => {
                        self.mm_drag = false;
                        if let Some((sx, sy)) = self.ldown.take() {
                            self.select_release(sx, sy, x, y);
                        }
                    }
                    2 => self.mdrag = None,
                    _ => {}
                }
            }
            Ev::Key(k, down) => {
                if (k as usize) < 256 {
                    self.keys[k as usize] = down;
                }
                if down {
                    return self.key_down(k);
                }
            }
            Ev::Char(_) => {}
        }
        false
    }

    fn minimap_jump(&mut self, x: f32, y: f32) {
        let (mx0, my0, _, _) = self.mm_rect();
        self.cam = v2(((x - mx0) / MM_W * MAP_W as f32).clamp(0.0, MAP_W as f32), ((y - my0) / MM_H * MAP_H as f32).clamp(0.0, MAP_H as f32));
    }

    fn key_down(&mut self, k: u32) -> bool {
        let ctrl = self.keys[VK_CONTROL as usize];
        match k {
            VK_F11 => {
                self.do_act(Act::Fullscreen);
                return false;
            }
            VK_ADD | VK_OEM_PLUS if ctrl => {
                self.do_act(Act::Scale(1));
                return false;
            }
            VK_SUB | VK_OEM_MINUS if ctrl => {
                self.do_act(Act::Scale(-1));
                return false;
            }
            _ => {}
        }
        if self.screen == Screen::Title {
            if k == VK_ESCAPE {
                return true;
            }
            if k == VK_SPACE || k == 0x0D {
                if self.title_sel.is_some() {
                    self.do_act(Act::Start);
                }
            }
            return false;
        }
        // ボタンのホットキー
        if !ctrl {
            let mut hit = None;
            for b in self.buttons.iter() {
                if b.key == k && b.enabled {
                    hit = Some(b.act);
                    break;
                }
            }
            if let Some(a) = hit {
                self.do_act(a);
                return false;
            }
        }
        match k {
            VK_ESCAPE => {
                if self.mode != Mode::Normal {
                    self.mode = Mode::Normal;
                } else if self.show_diplo || self.show_help {
                    self.show_diplo = false;
                    self.show_help = false;
                } else {
                    self.clear_sel();
                }
            }
            VK_SPACE => self.paused = !self.paused,
            VK_F1 => self.show_help = !self.show_help,
            VK_F2 => self.show_diplo = !self.show_diplo,
            VK_HOME => self.do_act(Act::Home),
            VK_ADD | VK_OEM_PLUS => self.speed = (self.speed * 2).min(8),
            VK_SUB | VK_OEM_MINUS => self.speed = (self.speed / 2).max(1),
            VK_OEM_PERIOD => self.do_act(Act::IdleWorker),
            VK_DELETE => self.do_act(Act::Demolish),
            VK_TAB | VK_BACK => {}
            0x30..=0x39 => {
                let g = (k - 0x30) as usize;
                if ctrl {
                    self.groups[g] = self.own_sel();
                    self.notice(&format!("部隊{}に登録", g));
                } else {
                    let hs: Vec<Handle> = self.groups[g].iter().copied().filter(|&h| self.world.unit(h).is_some()).collect();
                    if !hs.is_empty() {
                        let double = self.last_click.0 > self.clock - 0.4 && self.last_click.1 == Some(hs[0]);
                        self.set_sel(hs.clone());
                        if double {
                            let p = self.world.unit(hs[0]).unwrap().pos;
                            self.cam = p;
                        }
                        self.last_click = (self.clock, Some(hs[0]));
                    }
                }
            }
            _ => {}
        }
        false
    }

    fn clear_sel(&mut self) {
        for h in self.sel.drain(..) {
            if let Some(u) = self.world.unit_mut(h) {
                u.selected = false;
            }
        }
        self.sel_b = None;
    }

    fn set_sel(&mut self, hs: Vec<Handle>) {
        self.clear_sel();
        for &h in hs.iter() {
            if let Some(u) = self.world.unit_mut(h) {
                u.selected = true;
            }
        }
        self.sel = hs;
    }

    fn own_sel(&self) -> Vec<Handle> {
        let p = self.world.player;
        self.sel.iter().copied().filter(|&h| self.world.unit(h).map_or(false, |u| u.nation as usize == p)).collect()
    }

    fn unit_at(&self, p: V2) -> Option<Handle> {
        let mut best: Option<(f32, Handle)> = None;
        for (i, u) in self.world.units.iter().enumerate() {
            if !u.alive {
                continue;
            }
            if u.nation as usize != self.world.player && !self.seen(u.pos) {
                continue;
            }
            let r = if u.kind == UnitKind::Worker { 0.45 } else { 0.55 };
            let d = u.pos.dist(p);
            if d < r && best.map_or(true, |b| d < b.0) {
                best = Some((d, self.world.uh(i)));
            }
        }
        best.map(|b| b.1)
    }

    fn bld_at(&self, p: V2) -> Option<Handle> {
        let (x, y) = p.tile();
        if !Map::inb(x, y) {
            return None;
        }
        let b = self.world.bmap[Map::idx(x, y)];
        if b == 0 || !self.explored(p) {
            return None;
        }
        Some(self.world.bh(b as usize - 1))
    }

    fn select_release(&mut self, sx: f32, sy: f32, x: f32, y: f32) {
        let p = self.world.player;
        if (sx - x).abs() < 5.0 && (sy - y).abs() < 5.0 {
            let wp = self.screen_to_world(x, y);
            if let Some(h) = self.unit_at(wp) {
                let shift = self.keys[VK_SHIFT as usize];
                let double = self.last_click.0 > self.clock - 0.35 && self.last_click.1 == Some(h);
                self.last_click = (self.clock, Some(h));
                let u = self.world.unit(h).unwrap();
                if double && u.nation as usize == p {
                    // 画面内の同種ユニットを全選択
                    let kind = u.kind;
                    let (w, hh) = (self.gfx.width as f32, self.gfx.height as f32);
                    let a = self.screen_to_world(0.0, TOP_H);
                    let b = self.screen_to_world(w, hh - BOT_H);
                    let hs: Vec<Handle> = (0..self.world.units.len())
                        .filter(|&i| {
                            let u = &self.world.units[i];
                            u.alive && u.nation as usize == p && u.kind == kind && u.pos.x >= a.x && u.pos.x <= b.x && u.pos.y >= a.y && u.pos.y <= b.y
                        })
                        .map(|i| self.world.uh(i))
                        .collect();
                    self.set_sel(hs);
                } else if shift && u.nation as usize == p {
                    let mut hs = self.own_sel();
                    if let Some(k) = hs.iter().position(|&x| x == h) {
                        hs.remove(k);
                    } else {
                        hs.push(h);
                    }
                    self.set_sel(hs);
                } else {
                    self.set_sel(vec![h]);
                }
            } else if let Some(b) = self.bld_at(wp) {
                self.clear_sel();
                self.sel_b = Some(b);
            } else {
                self.clear_sel();
            }
        } else {
            let a = self.screen_to_world(sx.min(x), sy.min(y));
            let b = self.screen_to_world(sx.max(x), sy.max(y));
            let hs: Vec<Handle> = (0..self.world.units.len())
                .filter(|&i| {
                    let u = &self.world.units[i];
                    u.alive && u.nation as usize == p && u.pos.x >= a.x && u.pos.x <= b.x && u.pos.y >= a.y && u.pos.y <= b.y
                })
                .map(|i| self.world.uh(i))
                .collect();
            if !hs.is_empty() {
                let mut hs = hs;
                if self.keys[VK_SHIFT as usize] {
                    for h in self.own_sel() {
                        if !hs.contains(&h) {
                            hs.push(h);
                        }
                    }
                }
                self.set_sel(hs);
            } else if !self.keys[VK_SHIFT as usize] {
                self.clear_sel();
            }
        }
    }

    fn right_click_world(&mut self, p: V2) {
        let pl = self.world.player;
        // 建物選択中：集合地点
        if self.sel.is_empty() {
            if let Some(bh) = self.sel_b {
                if let Some(b) = self.world.bld_mut(bh) {
                    if b.nation as usize == pl && b.kind != BKind::Farm {
                        b.rally = Some(p);
                        self.marker = Some((p, 0.6, false));
                    }
                }
            }
            return;
        }
        let hs = self.own_sel();
        if hs.is_empty() {
            return;
        }
        let workers: Vec<Handle> = hs.iter().copied().filter(|&h| self.world.unit(h).map_or(false, |u| u.kind == UnitKind::Worker)).collect();
        let soldiers: Vec<Handle> = hs.iter().copied().filter(|&h| self.world.unit(h).map_or(false, |u| u.kind.military())).collect();
        if !soldiers.is_empty() {
            if let Some(t) = &mut self.tut {
                t.moved = true;
            }
        }
        // 敵ユニット
        if let Some(t) = self.unit_at(p) {
            let tn = self.world.unit(t).unwrap().nation as usize;
            if tn != pl {
                if self.world.at_war(pl, tn) {
                    self.world.order_attack(&hs, Target::Unit(t));
                    self.marker = Some((p, 0.6, true));
                } else {
                    self.notice(&format!("{} とは交戦していない（外交 F2 で宣戦できる）", self.world.nations[tn].name));
                    self.world.order_move(&hs, p, false);
                }
                return;
            }
        }
        if let Some(bh) = self.bld_at(p) {
            let b = self.world.bld(bh).unwrap();
            let (bn, kind, done) = (b.nation as usize, b.kind, b.done());
            if bn != pl {
                if self.world.at_war(pl, bn) {
                    self.world.order_attack(&hs, Target::Bld(bh));
                    self.marker = Some((p, 0.6, true));
                } else {
                    self.notice(&format!("{} とは交戦していない（外交 F2 で宣戦できる）", self.world.nations[bn].name));
                }
                return;
            }
            if !done {
                self.world.order_build(&workers, bh);
                self.world.order_move(&soldiers, p, false);
                return;
            }
            if kind == BKind::Farm && !workers.is_empty() {
                self.world.order_gather(&workers[..1], Src::Farm(bh));
                if workers.len() > 1 {
                    self.world.order_move(&workers[1..], p, false);
                }
                self.world.order_move(&soldiers, p, false);
                return;
            }
        }
        let (tx, ty) = p.tile();
        if !workers.is_empty() && Map::inb(tx, ty) {
            if self.world.map.terrain[Map::idx(tx, ty)] == Terrain::Forest {
                self.world.order_gather(&workers, Src::Forest(tx, ty));
                self.world.order_move(&soldiers, p, false);
                self.marker = Some((p, 0.6, false));
                return;
            }
            if let Some(m) = self.world.map.mines.iter().position(|m| m.x == tx && m.y == ty && m.amount > 0.0) {
                self.world.order_gather(&workers, Src::Mine(m));
                self.world.order_move(&soldiers, p, false);
                self.marker = Some((p, 0.6, false));
                return;
            }
        }
        self.world.order_move(&hs, p, false);
        self.marker = Some((p, 0.6, false));
    }

    fn placement_tile(&self) -> (i32, i32) {
        let p = self.screen_to_world(self.mouse.0, self.mouse.1);
        ((p.x - 0.5).round() as i32, (p.y - 0.5).round() as i32)
    }

    fn try_place(&mut self, k: BKind) {
        let (tx, ty) = self.placement_tile();
        let pl = self.world.player;
        match self.world.place_building(pl, k, tx, ty) {
            Ok(bh) => {
                let ws: Vec<Handle> = self.own_sel().into_iter().filter(|&h| self.world.unit(h).map_or(false, |u| u.kind == UnitKind::Worker)).collect();
                self.world.order_build(&ws, bh);
            }
            Err(e) => self.notice(e),
        }
    }

    fn title_click(&mut self, x: f32, y: f32) {
        let p = self.screen_to_world(x, y);
        let mut best: Option<(f32, usize)> = None;
        for (i, n) in self.world.nations.iter().enumerate() {
            if !n.alive {
                continue;
            }
            let d = n.home.dist(p);
            if d < 6.0 && best.map_or(true, |b| d < b.0) {
                best = Some((d, i));
            }
        }
        if let Some((_, i)) = best {
            self.title_sel = Some(i);
        }
    }

    fn button_at(&self, x: f32, y: f32) -> Option<Act> {
        for b in self.buttons.iter().rev() {
            if x >= b.x0 && x <= b.x1 && y >= b.y0 && y <= b.y1 {
                return Some(if b.enabled { b.act } else { Act::Noop });
            }
        }
        None
    }

    fn do_act(&mut self, a: Act) {
        let pl = self.world.player;
        match a {
            Act::Noop => {}
            Act::PickNation(i) => self.title_sel = Some(i),
            Act::Start => {
                if let Some(n) = self.title_sel {
                    self.world.player = n;
                    self.screen = Screen::Playing;
                    self.zoom = 22.0;
                    self.cam = self.world.nations[n].home;
                    self.reset_fog();
                    let name = self.world.nations[n].name;
                    self.world.push_msg(format!("紀元前770年、周の東遷。あなたは {} の君主となった。", name), [1.0, 0.9, 0.6]);
                    self.world.push_msg("F1でヘルプ、F2で外交画面を開く".into(), [0.8, 0.8, 0.8]);
                }
            }
            Act::Tutorial => self.start_tutorial(),
            Act::TutNext => self.tut_next(),
            Act::TutQuit => self.end_tutorial(),
            Act::Scale(d) => {
                self.gfx.adjust_scale(d);
                if self.screen == Screen::Title {
                    self.fit_title_camera();
                }
                self.notice(&format!("文字の大きさ ×{:.2}", self.gfx.ui));
            }
            Act::Fullscreen => self.gfx.toggle_fullscreen(),
            Act::Restart => {
                self.tut = None;
                self.seed = self.seed.wrapping_add(17);
                self.world = World::new(self.seed);
                self.rebuild_terrain();
                self.screen = Screen::Title;
                self.title_sel = None;
                self.sel.clear();
                self.sel_b = None;
                self.show_diplo = false;
                self.show_help = false;
                self.paused = false;
                self.speed = 1;
                self.fit_title_camera();
            }
            Act::Train(k) => {
                if let Some(bh) = self.sel_b {
                    if let Err(e) = self.world.enqueue(bh, k) {
                        self.notice(e);
                    }
                }
            }
            Act::Place(k) => {
                let cost = k.cost();
                let r = self.world.nations[pl].res;
                if r[0] < cost[0] || r[1] < cost[1] || r[2] < cost[2] {
                    self.notice("資源が足りない");
                } else {
                    self.mode = Mode::Place(k);
                }
            }
            Act::Upgrade => {
                let mut err = None;
                let mut ok = 0;
                let mut hs = self.own_sel();
                hs.sort_by(|a, b| self.world.unit(*a).unwrap().power.partial_cmp(&self.world.unit(*b).unwrap().power).unwrap());
                for h in hs {
                    match self.world.upgrade_unit(h) {
                        Ok(()) => ok += 1,
                        Err(e) => err = Some(e),
                    }
                }
                if ok == 0 {
                    if let Some(e) = err {
                        self.notice(e);
                    }
                } else {
                    self.notice(&format!("{}部隊の能力を高めた", ok));
                }
            }
            Act::Convert => {
                let mut err = None;
                for h in self.own_sel() {
                    if let Err(e) = self.world.convert_unit(h) {
                        err = Some(e);
                    }
                }
                if let Some(e) = err {
                    self.notice(e);
                }
            }
            Act::Stop => {
                for h in self.own_sel() {
                    if let Some(u) = self.world.unit_mut(h) {
                        u.order = Order::Idle;
                        u.resume = None;
                        u.path.clear();
                    }
                }
            }
            Act::AttackMove => {
                if !self.own_sel().is_empty() {
                    self.mode = Mode::AttackMove;
                }
            }
            Act::Speed(s) => {
                if s == 0 {
                    self.paused = !self.paused;
                } else {
                    self.speed = s;
                    self.paused = false;
                }
            }
            Act::Diplo => {
                self.show_diplo = !self.show_diplo;
                self.show_help = false;
            }
            Act::Help => {
                self.show_help = !self.show_help;
                self.show_diplo = false;
            }
            Act::CloseOverlay => {
                self.show_diplo = false;
                self.show_help = false;
            }
            Act::War(o) => {
                if self.world.truce[pl][o] > self.world.years {
                    let r = (self.world.truce[pl][o] - self.world.years).ceil() as i32;
                    self.notice(&format!("休戦期間中（残り{}年）", r));
                } else {
                    self.world.declare_war(pl, o, false);
                }
            }
            Act::Peace(o) => {
                let w = &mut self.world;
                if w.coalition_locked(pl, o) {
                    self.notice("連合軍の討伐中は和睦できない");
                    return;
                }
                let dur = w.years - w.war_start[pl][o];
                let accept = w.peace_offer[o] || (dur > 5.0 && (w.nations[o].strength < w.nations[pl].strength * 1.1 || w.rng.chance(0.25)));
                if accept {
                    w.make_peace(pl, o, 10.0);
                } else {
                    let t = format!("{} は和睦を拒否した", w.nations[o].name);
                    self.notice(&t);
                }
            }
            Act::Home => {
                if let Some(c) = self.world.nations[pl].capital.and_then(|h| self.world.bld(h)) {
                    self.cam = c.center();
                }
            }
            Act::Demolish => {
                if let Some(bh) = self.sel_b {
                    if let Some(b) = self.world.bld(bh) {
                        if b.nation as usize == pl && !b.kind.city() {
                            let i = bh.idx as usize;
                            self.world.remove_building(i);
                            self.sel_b = None;
                        }
                    }
                }
            }
            Act::IdleWorker => {
                let idle: Vec<usize> = (0..self.world.units.len())
                    .filter(|&i| {
                        let u = &self.world.units[i];
                        u.alive && u.nation as usize == pl && u.kind == UnitKind::Worker && u.order == Order::Idle
                    })
                    .collect();
                if idle.is_empty() {
                    self.notice("待機中の民はいない");
                } else {
                    self.idle_cycle = (self.idle_cycle + 1) % idle.len();
                    let i = idle[self.idle_cycle];
                    let h = self.world.uh(i);
                    self.cam = self.world.units[i].pos;
                    self.set_sel(vec![h]);
                }
            }
        }
    }

    // ------------------------------------------------------------ 描画
    fn render(&mut self, dt: f32) {
        self.render_nopresent(dt);
        self.gfx.present();
    }

    fn render_nopresent(&mut self, dt: f32) {
        self.update_minimap(dt);
        self.gfx.begin([0.05, 0.07, 0.10, 1.0]);
        self.render_world();
        if self.gfx.d2d_begin() {
            self.buttons.clear();
            self.render_world_labels();
            if self.screen == Screen::Title {
                self.render_title();
            } else {
                self.render_hud();
                if self.show_diplo {
                    self.render_diplo();
                }
                if self.show_help {
                    self.render_help();
                }
                if self.world.game_over.is_some() {
                    self.render_gameover();
                }
                self.render_tutorial();
            }
            self.gfx.d2d_end();
        }
    }

    fn render_world(&mut self) {
        let (w, h) = (self.gfx.width as f32, self.gfx.height as f32);
        let z = self.zoom;
        let sx = 2.0 * z / w;
        let sy = -2.0 * z / h;
        self.gfx.set_xform(sx, sy, -self.cam.x * sx, -self.cam.y * sy);
        let place_tile = self.placement_tile();
        let fog_on = self.fog_active();
        let pl = self.world.player;
        let fog = &self.fog;
        let fog_at = move |p: V2| -> u8 {
            if !fog_on {
                return 2;
            }
            let (x, y) = p.tile();
            if Map::inb(x, y) {
                fog[Map::idx(x, y)]
            } else {
                0
            }
        };
        let g = &mut self.gfx;
        let mw = MAP_W as f32;
        let mh = MAP_H as f32;
        g.draw_textured(&self.terrain_tex, 0.0, 0.0, mw, mh, [1.0; 4], true);
        g.draw_textured(&self.overlay_tex, 0.0, 0.0, mw, mh, [1.0; 4], false);
        let wd = &self.world;
        let view0 = v2(self.cam.x - w / 2.0 / z - 2.0, self.cam.y - h / 2.0 / z - 2.0);
        let view1 = v2(self.cam.x + w / 2.0 / z + 2.0, self.cam.y + h / 2.0 / z + 2.0);
        let inview = |p: V2| p.x >= view0.x && p.x <= view1.x && p.y >= view0.y && p.y <= view1.y;
        let px = 1.0 / z; // 1ピクセル
        // 銅山
        for m in wd.map.mines.iter() {
            if m.amount <= 0.0 {
                continue;
            }
            let c = v2(m.x as f32 + 0.5, m.y as f32 + 0.5);
            if !inview(c) || fog_at(c) == 0 {
                continue;
            }
            let s = 0.25 + 0.2 * (m.amount / 2500.0);
            g.tri((c.x - s - px, c.y + s * 0.7 + px), (c.x, c.y - s - px), (c.x + s + px, c.y + s * 0.7 + px), [0.15, 0.1, 0.05, 1.0]);
            g.tri((c.x - s, c.y + s * 0.7), (c.x, c.y - s), (c.x + s, c.y + s * 0.7), [0.85, 0.55, 0.25, 1.0]);
            g.circle(c.x, c.y + 0.05, s * 0.25, 6, [1.0, 0.85, 0.5, 1.0]);
        }
        // 建物
        for (bi, b) in wd.blds.iter().enumerate() {
            if !b.alive {
                continue;
            }
            let c = b.center();
            if !inview(c) || fog_at(c) == 0 {
                continue;
            }
            let nc = wd.nations[b.nation as usize].color;
            let (x0, y0, x1, y1) = (b.tx as f32, b.ty as f32, b.tx as f32 + 2.0, b.ty as f32 + 2.0);
            let alpha = if b.done() { 1.0 } else { 0.55 };
            match b.kind {
                BKind::Farm => {
                    g.rect(x0 + 0.05, y0 + 0.05, x1 - 0.05, y1 - 0.05, [0.45, 0.38, 0.18, alpha]);
                    for k in 0..4 {
                        let yy = y0 + 0.15 + k as f32 * 0.45;
                        g.rect(x0 + 0.15, yy, x1 - 0.15, yy + 0.25, [0.78, 0.72, 0.30, alpha]);
                    }
                    g.rect_outline(x0 + 0.05, y0 + 0.05, x1 - 0.05, y1 - 0.05, 0.08, c4(nc, alpha));
                }
                BKind::Barracks => {
                    g.rect(x0 + 0.1, y0 + 0.1, x1 - 0.1, y1 - 0.1, c4(darker(nc, 0.35), alpha));
                    g.rect(x0 + 0.25, y0 + 0.25, x1 - 0.25, y1 - 0.25, c4(nc, alpha));
                    g.line((x0 + 0.35, y0 + 0.35), (x1 - 0.35, y1 - 0.35), 0.18, [0.15, 0.12, 0.1, alpha]);
                    g.line((x1 - 0.35, y0 + 0.35), (x0 + 0.35, y1 - 0.35), 0.18, [0.15, 0.12, 0.1, alpha]);
                }
                BKind::Capital | BKind::Town => {
                    let cap = b.kind == BKind::Capital;
                    let e = if cap { -0.25 } else { 0.0 };
                    g.rect(x0 + e - px * 2.0, y0 + e - px * 2.0, x1 - e + px * 2.0, y1 - e + px * 2.0, [0.05, 0.05, 0.05, alpha]);
                    g.rect(x0 + e, y0 + e, x1 - e, y1 - e, [0.55, 0.50, 0.42, alpha]);
                    g.rect(x0 + e + 0.22, y0 + e + 0.22, x1 - e - 0.22, y1 - e - 0.22, c4(nc, alpha));
                    // 四隅の望楼
                    for (cx, cy) in [(x0 + e, y0 + e), (x1 - e, y0 + e), (x0 + e, y1 - e), (x1 - e, y1 - e)] {
                        g.rect(cx - 0.16, cy - 0.16, cx + 0.16, cy + 0.16, [0.35, 0.3, 0.25, alpha]);
                    }
                    if cap {
                        g.rect(c.x - 0.35, c.y - 0.35, c.x + 0.35, c.y + 0.35, [0.95, 0.80, 0.30, alpha]);
                        g.rect(c.x - 0.22, c.y - 0.22, c.x + 0.22, c.y + 0.22, c4(nc, alpha));
                    }
                }
            }
            let sel = self.sel_b == Some(wd.bh(bi));
            if sel {
                g.rect_outline(x0 - 0.3, y0 - 0.3, x1 + 0.3, y1 + 0.3, 2.0 * px, [1.0, 1.0, 1.0, 0.9]);
            }
            if !b.done() {
                g.rect(x0, y1 + 0.1, x1, y1 + 0.3, [0.0, 0.0, 0.0, 0.7]);
                g.rect(x0, y1 + 0.1, x0 + 2.0 * b.progress, y1 + 0.3, [0.4, 0.7, 1.0, 1.0]);
            } else if b.hp < b.kind.max_hp() - 0.5 || sel {
                let f = (b.hp / b.kind.max_hp()).clamp(0.0, 1.0);
                g.rect(x0, y0 - 0.45, x1, y0 - 0.25, [0.0, 0.0, 0.0, 0.7]);
                g.rect(x0, y0 - 0.45, x0 + 2.0 * f, y0 - 0.25, [1.0 - f, 0.2 + 0.7 * f, 0.15, 1.0]);
            }
            if sel {
                if let Some(r) = b.rally {
                    g.line((c.x, c.y), (r.x, r.y), 1.5 * px, [1.0, 1.0, 1.0, 0.5]);
                    g.rect(r.x - 0.05, r.y - 0.6, r.x + 0.05, r.y, [0.9, 0.9, 0.9, 1.0]);
                    g.tri((r.x + 0.05, r.y - 0.6), (r.x + 0.45, r.y - 0.45), (r.x + 0.05, r.y - 0.3), c4(nc, 1.0));
                }
            }
        }
        // ユニット
        let era = wd.era;
        for u in wd.units.iter() {
            if !u.alive || !inview(u.pos) || (u.nation as usize != pl && fog_at(u.pos) != 2) {
                continue;
            }
            let p = u.pos;
            let nc = wd.nations[u.nation as usize].color;
            let body = if u.flash > 0.0 { [1.0, 1.0, 1.0, 1.0] } else { c4(nc, 1.0) };
            let dark = c4(darker(nc, 0.3), 1.0);
            // 影
            g.circle(p.x + 0.06, p.y + 0.1, 0.4, 10, [0.0, 0.0, 0.0, 0.25]);
            if u.selected {
                g.ring(p.x, p.y, 0.55, 2.0 * px, 16, [0.4, 1.0, 0.4, 0.95]);
            }
            let sz = 1.3 + 0.12 * (u.power - 1.0).min(5.0);
            match u.kind {
                UnitKind::Worker => {
                    let r = 0.2 * sz;
                    g.circle(p.x, p.y, r + 1.5 * px, 10, dark);
                    g.circle(p.x, p.y, r, 10, body);
                    g.circle(p.x, p.y, r * 0.45, 8, [0.95, 0.9, 0.8, 1.0]);
                    if u.carry > 0.5 {
                        let cc = RES_COLORS[u.carry_kind];
                        g.circle(p.x + 0.18, p.y - 0.15, 0.09, 6, cc);
                    }
                }
                UnitKind::Infantry => {
                    let r = 0.27 * sz;
                    g.circle(p.x, p.y, r + 1.5 * px, 12, dark);
                    g.circle(p.x, p.y, r, 12, body);
                    let d = u.dir;
                    g.line((p.x, p.y), (p.x + d.x * r * 1.6, p.y + d.y * r * 1.6), 2.0 * px.max(0.04), [0.85, 0.8, 0.7, 1.0]);
                }
                UnitKind::Archer => {
                    let r = 0.32 * sz;
                    let d = u.dir;
                    let n = v2(-d.y, d.x);
                    let a = p + d * r;
                    let b = p - d * (r * 0.7) + n * (r * 0.85);
                    let c = p - d * (r * 0.7) - n * (r * 0.85);
                    let o = 1.8 * px;
                    g.tri((a.x + d.x * o, a.y + d.y * o), (b.x + n.x * o - d.x * o, b.y + n.y * o - d.y * o), (c.x - n.x * o - d.x * o, c.y - n.y * o - d.y * o), dark);
                    g.tri((a.x, a.y), (b.x, b.y), (c.x, c.y), body);
                }
                UnitKind::Chariot => {
                    let r = 0.3 * sz;
                    let d = u.dir;
                    let n = v2(-d.y, d.x);
                    let q = |s: f32, t: f32| {
                        let v = p + d * s + n * t;
                        (v.x, v.y)
                    };
                    let o = 1.8 * px;
                    g.tri(q(r + o, -r * 0.8 - o), q(r + o, r * 0.8 + o), q(-r - o, r * 0.8 + o), dark);
                    g.tri(q(r + o, -r * 0.8 - o), q(-r - o, r * 0.8 + o), q(-r - o, -r * 0.8 - o), dark);
                    g.tri(q(r, -r * 0.8), q(r, r * 0.8), q(-r, r * 0.8), body);
                    g.tri(q(r, -r * 0.8), q(-r, r * 0.8), q(-r, -r * 0.8), body);
                    g.tri(q(r * 1.6, 0.0), q(r, r * 0.35), q(r, -r * 0.35), [0.9, 0.85, 0.7, 1.0]);
                }
            }
            let mhp = u.max_hp();
            if u.hp < mhp - 0.5 || u.selected {
                let f = (u.hp / mhp).clamp(0.0, 1.0);
                g.rect(p.x - 0.35, p.y - 0.68, p.x + 0.35, p.y - 0.6, [0.0, 0.0, 0.0, 0.7]);
                g.rect(p.x - 0.35, p.y - 0.68, p.x - 0.35 + 0.7 * f, p.y - 0.6, [1.0 - f, 0.25 + 0.7 * f, 0.15, 1.0]);
            }
        }
        let _ = era;
        // 矢
        for s in wd.shots.iter() {
            let t = 1.0 - s.t / 0.3;
            let p = s.a + (s.b - s.a) * t.clamp(0.0, 1.0);
            if fog_at(p) != 2 {
                continue;
            }
            let d = (s.b - s.a).norm();
            g.line((p.x - d.x * 0.3, p.y - d.y * 0.3), (p.x, p.y), 2.0 * px, [1.0, 0.95, 0.6, 0.9]);
        }
        if fog_on {
            g.draw_textured(&self.fog_tex, 0.0, 0.0, mw, mh, [1.0; 4], true);
        }
        // 移動目標の印
        if let Some((p, t, atk)) = self.marker {
            let r = 0.2 + t * 0.6;
            let c = if atk { [1.0, 0.3, 0.2, t / 0.6] } else { [0.4, 1.0, 0.4, t / 0.6] };
            g.ring(p.x, p.y, r, 2.0 * px, 16, c);
        }
        // 建設予定地
        if let Mode::Place(k) = self.mode {
            let (tx, ty) = place_tile;
            let ok = wd.can_place(wd.player, k, tx, ty).is_ok();
            let c = if ok { [0.3, 1.0, 0.3, 0.45] } else { [1.0, 0.25, 0.2, 0.45] };
            g.rect(tx as f32, ty as f32, tx as f32 + 2.0, ty as f32 + 2.0, c);
            if k.city() {
                g.ring(tx as f32 + 1.0, ty as f32 + 1.0, k.radius() as f32, 2.0 * px, 48, [1.0, 1.0, 1.0, 0.5]);
            }
        }
        g.flush();
        // 画面座標での選択矩形
        if let Some((sx, sy)) = self.ldown {
            let (mx, my) = self.mouse;
            if (sx - mx).abs() > 4.0 || (sy - my).abs() > 4.0 {
                self.gfx.set_screen_xform();
                let g = &mut self.gfx;
                g.rect(sx.min(mx), sy.min(my), sx.max(mx), sy.max(my), [0.4, 1.0, 0.4, 0.12]);
                g.rect_outline(sx.min(mx), sy.min(my), sx.max(mx), sy.max(my), 1.5, [0.4, 1.0, 0.4, 0.9]);
                g.flush();
            }
        }
    }

    fn render_world_labels(&mut self) {
        let z = self.zoom;
        let (w, h) = (self.gfx.width as f32, self.gfx.height as f32);
        let era = self.world.era;
        // ユニットの字
        if z >= 20.0 && self.screen == Screen::Playing {
            let mut labels = Vec::new();
            for u in self.world.units.iter() {
                if !u.alive || (u.nation as usize != self.world.player && !self.seen(u.pos)) {
                    continue;
                }
                let (sx, sy) = self.world_to_screen(u.pos);
                if sx < -20.0 || sy < -20.0 || sx > w + 20.0 || sy > h + 20.0 {
                    continue;
                }
                labels.push((sx, sy, u.kind.glyph(era), u.power));
            }
            for (sx, sy, gl, pw) in labels {
                let s = 11.0;
                self.gfx.text(gl, sx - s, sy - s * 0.95, s * 2.0, s * 2.0, Font::NormalCenter, [1.0, 1.0, 1.0, 0.95]);
                if pw > 1.01 && z >= 30.0 {
                    let t = format!("{:.1}", pw);
                    self.gfx.text_shadow(&t, sx + 8.0, sy + 2.0, 40.0, 16.0, Font::Small, [1.0, 0.95, 0.6, 1.0]);
                }
            }
        }
        // 城邑名
        let mut labels = Vec::new();
        for b in self.world.blds.iter() {
            if !b.alive || !b.kind.city() || !self.explored(b.center()) {
                continue;
            }
            let (sx, sy) = self.world_to_screen(b.center());
            if sx < -100.0 || sy < -50.0 || sx > w + 100.0 || sy > h + 50.0 {
                continue;
            }
            let n = &self.world.nations[b.nation as usize];
            let cap = b.kind == BKind::Capital;
            if z < 9.0 && !cap {
                continue;
            }
            let name = if cap { n.name.to_string() } else { format!("{}・邑", n.name) };
            labels.push((sx, sy, name, cap, n.color));
        }
        for (sx, sy, name, cap, c) in labels {
            let f = if cap && z >= 9.0 { Font::BigCenter } else { Font::NormalCenter };
            let off = if z >= 9.0 { 1.4 * z + 14.0 } else { 0.0 };
            let col = [c[0] * 0.5 + 0.5, c[1] * 0.5 + 0.5, c[2] * 0.5 + 0.5, 1.0];
            self.gfx.text_shadow(&name, sx - 80.0, sy - off - 14.0, 160.0, 28.0, f, col);
        }
    }

    fn add_button(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, label: &str, sub: &str, key: u32, enabled: bool, act: Act, tip: &str) {
        let (mx, my) = self.mouse;
        let hover = mx >= x0 && mx <= x1 && my >= y0 && my <= y1;
        let bg = if !enabled {
            [0.18, 0.17, 0.15, 0.9]
        } else if hover {
            [0.45, 0.36, 0.20, 0.95]
        } else {
            [0.30, 0.24, 0.15, 0.95]
        };
        self.gfx.fill_round(x0, y0, x1, y1, 4.0, bg);
        self.gfx.stroke_round(x0, y0, x1, y1, 4.0, 1.0, [0.75, 0.62, 0.35, if enabled { 0.9 } else { 0.4 }]);
        let tc = if enabled { [1.0, 0.95, 0.85, 1.0] } else { [0.55, 0.52, 0.48, 1.0] };
        if sub.is_empty() {
            self.gfx.text(label, x0, y0, x1 - x0, y1 - y0, Font::NormalCenter, tc);
        } else {
            self.gfx.text(label, x0, y0 + 1.0, x1 - x0, (y1 - y0) * 0.58, Font::NormalCenter, tc);
            self.gfx.text(sub, x0 + 2.0, y0 + (y1 - y0) * 0.55, x1 - x0 - 4.0, (y1 - y0) * 0.42, Font::Small, [tc[0] * 0.85, tc[1] * 0.85, tc[2] * 0.8, 1.0]);
        }
        if key >= 0x41 && key <= 0x5A {
            let ks = ((key as u8) as char).to_string();
            self.gfx.text(&ks, x1 - 14.0, y0 + 1.0, 13.0, 14.0, Font::Small, [0.9, 0.8, 0.5, 0.8]);
        }
        if hover && !tip.is_empty() {
            self.notice = None;
        }
        self.buttons.push(Button { x0, y0, x1, y1, label: label.to_string(), sub: sub.to_string(), key, enabled, act, tip: tip.to_string() });
    }

    fn cost_str(c: [f32; 3]) -> String {
        let mut s = String::new();
        for k in 0..3 {
            if c[k] > 0.0 {
                if !s.is_empty() {
                    s.push(' ');
                }
                s.push_str(&format!("{}{}", RES_NAMES[k], c[k] as i32));
            }
        }
        s
    }

    fn render_title(&mut self) {
        let (w, h) = (self.gfx.width as f32, self.gfx.height as f32);
        // 国都マーカーの強調
        let hov = {
            let p = self.screen_to_world(self.mouse.0, self.mouse.1);
            let mut best: Option<(f32, usize)> = None;
            for (i, n) in self.world.nations.iter().enumerate() {
                if n.alive && n.home.dist(p) < 6.0 && best.map_or(true, |b| n.home.dist(p) < b.0) {
                    best = Some((n.home.dist(p), i));
                }
            }
            best.map(|b| b.1)
        };
        for (i, n) in self.world.nations.iter().enumerate() {
            if !n.alive {
                continue;
            }
            let (sx, sy) = self.world_to_screen(n.home);
            let seven = NATIONS[i].seven;
            if Some(i) == self.title_sel || Some(i) == hov {
                self.gfx.fill_circle(sx, sy, 14.0, [1.0, 1.0, 1.0, 0.35]);
            }
            if seven {
                self.gfx.fill_circle(sx, sy, 4.0, [1.0, 0.85, 0.3, 1.0]);
            }
        }
        let px = w - 380.0;
        self.gfx.fill_rect(px, 0.0, w, h, [0.08, 0.06, 0.05, 0.95]);
        self.gfx.fill_rect(px, 0.0, px + 2.0, h, [0.7, 0.55, 0.3, 1.0]);
        self.gfx.text("春秋戦国", px, 30.0, 380.0, 60.0, Font::Title, [0.95, 0.8, 0.4, 1.0]);
        self.gfx.text("— 中華統一への道 —", px, 90.0, 380.0, 30.0, Font::BigCenter, [0.9, 0.85, 0.75, 1.0]);
        self.gfx.text(
            "紀元前770年、周の東遷。数多の諸侯が並び立つ春秋の世から、七雄が争う戦国の世まで、時は全ての国に等しく流れる。\n\n地図上の国都をクリックして、率いる国を選べ。金の印は戦国の七雄（生産力が高い）。",
            px + 24.0,
            140.0,
            332.0,
            170.0,
            Font::Normal,
            [0.85, 0.82, 0.75, 1.0],
        );
        if let Some(i) = self.title_sel {
            let n = &self.world.nations[i];
            let d = &NATIONS[i];
            self.gfx.fill_round(px + 20.0, 320.0, w - 20.0, 520.0, 6.0, [0.2, 0.15, 0.1, 0.9]);
            self.gfx.fill_rect(px + 34.0, 338.0, px + 64.0, 368.0, c4(n.color, 1.0));
            self.gfx.text(n.name, px + 76.0, 332.0, 260.0, 40.0, Font::Big, [1.0, 0.95, 0.85, 1.0]);
            self.gfx.text(d.desc, px + 34.0, 378.0, 320.0, 44.0, Font::Normal, [0.85, 0.82, 0.75, 1.0]);
            let s = format!("生産力：×{:.2}{}", d.bias, if d.seven { "（七雄）" } else { "" });
            self.gfx.text(&s, px + 34.0, 426.0, 320.0, 20.0, Font::Normal, [0.95, 0.85, 0.5, 1.0]);
            let cities = self.world.blds.iter().filter(|b| b.alive && b.nation as usize == i && b.kind.city()).count();
            let s = format!("城邑：{}", cities);
            self.gfx.text(&s, px + 34.0, 448.0, 320.0, 20.0, Font::Normal, [0.85, 0.82, 0.75, 1.0]);
            let diff = if d.seven {
                "難易度：易"
            } else if d.bias >= 0.95 {
                "難易度：中"
            } else {
                "難易度：難"
            };
            self.gfx.text(diff, px + 34.0, 470.0, 320.0, 20.0, Font::Normal, [0.85, 0.82, 0.75, 1.0]);
            self.add_button(px + 60.0, 530.0, w - 60.0, 576.0, "この国で開始", "", 0, true, Act::Start, "");
        }
        let ty = (h - 128.0).max(592.0);
        self.add_button(px + 24.0, ty, w - 24.0, ty + 48.0, "はじめての方は：チュートリアル", "", 0, true, Act::Tutorial, "晋の君主として、操作と遊び方を1段ずつ学ぶ");
        let ty = ty + 60.0;
        self.add_button(px + 24.0, ty, px + 104.0, ty + 40.0, "文字 −", "", 0, true, Act::Scale(-1), "文字と画面部品を小さくする（Ctrl＋−）");
        self.add_button(px + 110.0, ty, px + 190.0, ty + 40.0, "文字 ＋", "", 0, true, Act::Scale(1), "文字と画面部品を大きくする（Ctrl＋＋）");
        let fl = if self.gfx.is_fullscreen() { "ウィンドウ F11" } else { "全画面 F11" };
        self.add_button(px + 196.0, ty, w - 24.0, ty + 40.0, fl, "", 0, true, Act::Fullscreen, "全画面表示を切り替える");
        // タイトル画面のツールチップ
        let (mx, my) = self.mouse;
        let tip = self.buttons.iter().find(|b| mx >= b.x0 && mx <= b.x1 && my >= b.y0 && my <= b.y1 && !b.tip.is_empty()).map(|b| b.tip.clone());
        if let Some(t) = tip {
            let x0 = px + 24.0;
            self.gfx.fill_round(x0, ty - 104.0, w - 24.0, ty - 64.0, 4.0, [0.05, 0.04, 0.03, 0.95]);
            self.gfx.text(&t, x0 + 8.0, ty - 100.0, w - x0 - 40.0, 36.0, Font::Small, [0.95, 0.92, 0.85, 1.0]);
        }
    }

    fn render_hud(&mut self) {
        let (w, h) = (self.gfx.width as f32, self.gfx.height as f32);
        let pl = self.world.player;
        // ---- 上部バー
        self.gfx.fill_rect(0.0, 0.0, w, TOP_H, [0.10, 0.08, 0.06, 0.92]);
        self.gfx.fill_rect(0.0, TOP_H - 2.0, w, TOP_H, [0.6, 0.48, 0.28, 1.0]);
        let n = &self.world.nations[pl];
        let mut x = 10.0;
        let res = n.res;
        let inc = n.income;
        let upkeep = n.upkeep;
        for k in 0..3 {
            let rate = if k == 0 { inc[k] - upkeep } else { inc[k] };
            let s = format!("{} {}", RES_NAMES[k], res[k] as i32);
            self.gfx.text(&s, x, 7.0, 110.0, 22.0, Font::Big, RES_COLORS[k]);
            let rs = format!("{:+.1}/秒", rate);
            let rc = if rate < 0.0 { [1.0, 0.4, 0.3, 1.0] } else { [0.75, 0.75, 0.7, 1.0] };
            self.gfx.text(&rs, x + 92.0, 12.0, 60.0, 18.0, Font::Small, rc);
            x += 150.0;
        }
        let s = format!("民 {}/{}", n.workers, n.worker_cap());
        self.gfx.text(&s, x, 9.0, 90.0, 22.0, Font::Normal, [0.9, 0.9, 0.85, 1.0]);
        x += 80.0;
        let s = format!("兵 {}/{}", n.soldiers, n.mil_cap());
        self.gfx.text(&s, x, 9.0, 90.0, 22.0, Font::Normal, [0.9, 0.9, 0.85, 1.0]);
        x += 80.0;
        let s = format!("維持費 {:.1}/秒", upkeep);
        self.gfx.text(&s, x, 9.0, 130.0, 22.0, Font::Normal, [0.95, 0.65, 0.5, 1.0]);
        let era = if self.world.era == Era::Spring { "春秋時代" } else { "戦国時代" };
        let s = format!("紀元前 {}年　{}", self.world.year_bc(), era);
        // 速度・メニュー
        let bx = w - 330.0;
        self.gfx.text(&s, (bx - 260.0).max(x + 140.0), 6.0, 250.0, 24.0, Font::Big, [1.0, 0.9, 0.6, 1.0]);
        let paused = self.paused;
        let sp = self.speed;
        self.add_button(bx, 5.0, bx + 36.0, 29.0, if paused { "▶" } else { "❚❚" }, "", 0, true, Act::Speed(0), "");
        for (k, s) in [1u32, 2, 4, 8].iter().enumerate() {
            let x0 = bx + 40.0 + k as f32 * 36.0;
            let lbl = format!("×{}", s);
            self.add_button(x0, 5.0, x0 + 34.0, 29.0, &lbl, "", 0, true, Act::Speed(*s), "");
            if sp == *s && !paused {
                self.gfx.stroke_round(x0, 5.0, x0 + 34.0, 29.0, 4.0, 2.0, [1.0, 0.9, 0.4, 1.0]);
            }
        }
        self.add_button(w - 150.0, 5.0, w - 80.0, 29.0, "外交 F2", "", 0, true, Act::Diplo, "");
        self.add_button(w - 76.0, 5.0, w - 6.0, 29.0, "ヘルプ F1", "", 0, true, Act::Help, "");
        if paused {
            self.gfx.text_shadow("一時停止中（Space）", w / 2.0 - 120.0, TOP_H + 60.0, 240.0, 30.0, Font::BigCenter, [1.0, 1.0, 0.8, 1.0]);
        }

        // ---- 連合情報
        let mut cy = TOP_H + 6.0;
        let years = self.world.years;
        let coal: Vec<String> = self
            .world
            .coalitions
            .iter()
            .map(|c| {
                let k = if c.kind == CoalKind::Hegemon { "覇者の呼びかけ" } else { "合従軍" };
                let me = if c.target == pl {
                    "（標的：自国！）"
                } else if c.members.contains(&pl) {
                    "（参加中）"
                } else {
                    ""
                };
                format!("【{}】{}討伐 盟主{} {}国 残り{}年{}", k, self.world.nations[c.target].name, self.world.nations[c.leader].name, c.members.len(), (c.end - years).ceil() as i32, me)
            })
            .collect();
        for s in coal {
            self.gfx.fill_rect(w - 470.0, cy, w - 6.0, cy + 22.0, [0.35, 0.1, 0.08, 0.85]);
            self.gfx.text(&s, w - 464.0, cy + 2.0, 460.0, 20.0, Font::Normal, [1.0, 0.85, 0.5, 1.0]);
            cy += 24.0;
        }
        // 覇者の警戒度
        if self.world.era == Era::Spring {
            let ag = self.world.nations[pl].aggression;
            if ag > 20.0 {
                let s = format!("諸侯の警戒 {:.0}/100", ag);
                let col = if ag > 75.0 { [1.0, 0.4, 0.3, 1.0] } else { [0.95, 0.8, 0.5, 1.0] };
                self.gfx.text_shadow(&s, w - 220.0, cy + 2.0, 214.0, 20.0, Font::Normal, col);
            }
        }

        // ---- メッセージ
        let now = self.world.time;
        let mut y = TOP_H + 8.0;
        let msgs: Vec<(String, [f32; 3], f32)> = self.world.msgs.iter().rev().take(7).filter(|m| now - m.time < 25.0).map(|m| (m.text.clone(), m.color, now - m.time)).collect();
        for (t, c, age) in msgs.into_iter().rev() {
            let a = if age > 20.0 { (25.0 - age) / 5.0 } else { 1.0 };
            self.gfx.text_shadow(&t, 10.0, y, w * 0.55, 20.0, Font::Normal, [c[0], c[1], c[2], a]);
            y += 20.0;
        }

        // ---- イベントバナー
        if let Some(b) = &self.world.banner {
            let a = b.t.min(1.0);
            let (title, text) = (b.title.clone(), b.text.clone());
            let bw = 720.0f32.min(w - 40.0);
            let x0 = (w - bw) / 2.0;
            let y0 = TOP_H + 90.0;
            self.gfx.fill_round(x0, y0, x0 + bw, y0 + 110.0, 8.0, [0.25, 0.08, 0.05, 0.85 * a]);
            self.gfx.stroke_round(x0, y0, x0 + bw, y0 + 110.0, 8.0, 2.0, [0.95, 0.75, 0.35, a]);
            self.gfx.text(&title, x0, y0 + 6.0, bw, 44.0, Font::Title, [1.0, 0.85, 0.4, a]);
            self.gfx.text(&text, x0 + 20.0, y0 + 56.0, bw - 40.0, 50.0, Font::NormalCenter, [1.0, 0.95, 0.85, a]);
        }

        // ---- 下部パネル
        let py = h - BOT_H;
        self.gfx.fill_rect(0.0, py, w, h, [0.10, 0.08, 0.06, 1.0]);
        self.gfx.fill_rect(0.0, py, w, py + 2.0, [0.6, 0.48, 0.28, 1.0]);
        // ミニマップ
        let (mx0, my0, mx1, my1) = self.mm_rect();
        if let Some(b) = &self.minimap_bmp {
            self.gfx.draw_bitmap(b, mx0, my0, mx1, my1);
        }
        let sxs = MM_W / MAP_W as f32;
        let sys = MM_H / MAP_H as f32;
        let dots: Vec<(f32, f32, [f32; 3], bool)> = self
            .world
            .units
            .iter()
            .filter(|u| u.alive && u.kind.military() && (u.nation as usize == pl || self.seen(u.pos)))
            .map(|u| (mx0 + u.pos.x * sxs, my0 + u.pos.y * sys, self.world.nations[u.nation as usize].color, u.nation as usize == pl))
            .collect();
        for (x, y, c, mine) in dots {
            let s = if mine { 1.6 } else { 1.0 };
            self.gfx.fill_rect(x - s, y - s, x + s, y + s, if mine { [1.0, 1.0, 1.0, 1.0] } else { c4(darker(c, 0.6), 1.0) });
        }
        let a = self.screen_to_world(0.0, TOP_H);
        let b = self.screen_to_world(w, h - BOT_H);
        self.gfx.stroke_rect(
            (mx0 + a.x * sxs).max(mx0),
            (my0 + a.y * sys).max(my0),
            (mx0 + b.x * sxs).min(mx1),
            (my0 + b.y * sys).min(my1),
            1.0,
            [1.0, 1.0, 1.0, 0.9],
        );
        self.gfx.stroke_rect(mx0, my0, mx1, my1, 1.0, [0.6, 0.48, 0.28, 1.0]);

        // 情報欄
        let ix = MM_W + 16.0;
        let iw = w - ix - 396.0;
        self.render_info(ix, py + 10.0, iw);
        // コマンド
        if !self.show_diplo && !self.show_help {
            self.render_commands(w - 384.0, py + 12.0);
        }

        if let Some((t, life)) = &self.notice {
            let a = life.min(1.0);
            let t = t.clone();
            self.gfx.text_shadow(&t, w / 2.0 - 300.0, py - 34.0, 600.0, 26.0, Font::BigCenter, [1.0, 0.85, 0.5, a]);
        }
        // ボタンのツールチップ
        let (mx, my) = self.mouse;
        let tip = self.buttons.iter().find(|b| mx >= b.x0 && mx <= b.x1 && my >= b.y0 && my <= b.y1 && !b.tip.is_empty()).map(|b| b.tip.clone());
        if let Some(t) = tip {
            let tw = 300.0;
            let x0 = (mx - tw).max(4.0);
            self.gfx.fill_round(x0, my - 70.0, x0 + tw, my - 8.0, 4.0, [0.05, 0.04, 0.03, 0.95]);
            self.gfx.stroke_round(x0, my - 70.0, x0 + tw, my - 8.0, 4.0, 1.0, [0.7, 0.6, 0.4, 1.0]);
            self.gfx.text(&t, x0 + 8.0, my - 66.0, tw - 16.0, 56.0, Font::Small, [0.95, 0.92, 0.85, 1.0]);
        }
    }

    fn order_text(&self, u: &Unit) -> &'static str {
        match u.order {
            Order::Idle => "待機中",
            Order::Move(_) => "移動中",
            Order::AttackMove(_) => "進軍中",
            Order::Attack(_) => "交戦中",
            Order::Gather { src, phase } => match (src.res(), phase) {
                (_, 2) => "運搬中",
                (0, _) => "農耕中",
                (1, _) => "伐採中",
                _ => "採掘中",
            },
            Order::Build(_) => "建設中",
        }
    }

    fn render_info(&mut self, x: f32, y: f32, wd: f32) {
        let pl = self.world.player;
        let era = self.world.era;
        let cap = self.world.power_cap();
        let tc = [0.92, 0.9, 0.84, 1.0];
        if self.sel.len() == 1 {
            let u = self.world.unit(self.sel[0]).unwrap();
            let n = &self.world.nations[u.nation as usize];
            let title = format!("{}　{}", u.kind.name(era), n.name);
            let lines = format!(
                "兵力 {:.0}/{:.0}\n能力 {:.1}（上限 {:.1}）\n状態：{}{}",
                u.hp.max(0.0),
                u.max_hp(),
                u.power,
                cap,
                self.order_text(u),
                if u.carry > 0.5 { format!("（{} {:.0}）", RES_NAMES[u.carry_kind], u.carry) } else { String::new() }
            );
            let desc = match u.kind {
                UnitKind::Worker => "生産ユニット。田・森・銅山で資源を集め、建物を建てる。",
                UnitKind::Infantry => "戈を持つ歩兵。戦車・騎兵に強い。建物攻撃に優れる。",
                UnitKind::Archer => "遠距離から射撃する。歩兵に強いが戦車・騎兵に弱い。",
                UnitKind::Chariot => "高速の突撃兵。弩兵と民に強いが、戈兵に弱い。",
            };
            let nc = n.color;
            self.gfx.fill_rect(x, y + 4.0, x + 22.0, y + 26.0, c4(nc, 1.0));
            self.gfx.text(&title, x + 30.0, y, wd, 30.0, Font::Big, tc);
            self.gfx.text(&lines, x, y + 36.0, wd, 70.0, Font::Normal, tc);
            self.gfx.text(desc, x, y + 104.0, wd, 50.0, Font::Small, [0.75, 0.72, 0.66, 1.0]);
        } else if self.sel.len() > 1 {
            let mut cnt = [0usize; 4];
            let mut pw = 0.0;
            let mut hp = 0.0;
            let mut mhp = 0.0;
            for &h in self.sel.iter() {
                let u = self.world.unit(h).unwrap();
                cnt[u.kind as usize] += 1;
                pw += u.power;
                hp += u.hp;
                mhp += u.max_hp();
            }
            let title = format!("{} 部隊を選択中", self.sel.len());
            self.gfx.text(&title, x, y, wd, 30.0, Font::Big, tc);
            let kinds = [UnitKind::Worker, UnitKind::Infantry, UnitKind::Archer, UnitKind::Chariot];
            let mut s = String::new();
            for k in 0..4 {
                if cnt[k] > 0 {
                    s.push_str(&format!("{} ×{}　", kinds[k].name(era), cnt[k]));
                }
            }
            let s = format!("{}\n総能力 {:.1}（平均 {:.2}）\n兵力 {:.0}/{:.0}", s, pw, pw / self.sel.len() as f32, hp, mhp);
            self.gfx.text(&s, x, y + 36.0, wd, 90.0, Font::Normal, tc);
        } else if let Some(bh) = self.sel_b {
            let b = self.world.bld(bh).unwrap();
            let n = &self.world.nations[b.nation as usize];
            let title = format!("{}　{}", b.kind.name(), n.name);
            let mut s = format!("耐久 {:.0}/{:.0}", b.hp.max(0.0), b.kind.max_hp());
            if !b.done() {
                s.push_str(&format!("\n建設中 {:.0}%", b.progress * 100.0));
            }
            let desc = match b.kind {
                BKind::Capital => "国の中心。民を訓練し、資源の集積地となる。失うと遷都し、全ての城邑を失うと滅亡する。",
                BKind::Town => "領土を広げ、民を訓練し、資源の集積地となる。城邑が増えるほど人口上限が上がる。",
                BKind::Barracks => "戈兵・弩兵・戦車（騎兵）を訓練する。",
                BKind::Farm => "民が耕すと糧を産む。枯れることはない。",
            };
            let queue: Vec<(UnitKind, f32)> = b.queue.iter().enumerate().map(|(i, k)| (*k, if i == 0 { b.queue_t / k.train_time() } else { 0.0 })).collect();
            let nc = n.color;
            self.gfx.fill_rect(x, y + 4.0, x + 22.0, y + 26.0, c4(nc, 1.0));
            self.gfx.text(&title, x + 30.0, y, wd, 30.0, Font::Big, tc);
            self.gfx.text(&s, x, y + 36.0, wd, 44.0, Font::Normal, tc);
            self.gfx.text(desc, x, y + 80.0, wd, 40.0, Font::Small, [0.75, 0.72, 0.66, 1.0]);
            for (i, (k, f)) in queue.iter().enumerate() {
                let qx = x + i as f32 * 58.0;
                let qy = y + 118.0;
                self.gfx.fill_round(qx, qy, qx + 54.0, qy + 30.0, 3.0, [0.25, 0.2, 0.14, 1.0]);
                if *f > 0.0 {
                    self.gfx.fill_rect(qx, qy + 26.0, qx + 54.0 * f, qy + 30.0, [0.4, 0.8, 0.4, 1.0]);
                }
                self.gfx.text(k.name(era), qx, qy, 54.0, 26.0, Font::NormalCenter, tc);
            }
        } else {
            let n = &self.world.nations[pl];
            let title = format!("{} の国勢", n.name);
            let wars: Vec<&str> = (0..self.world.nations.len()).filter(|&o| self.world.at_war(pl, o)).map(|o| self.world.nations[o].name).collect();
            let s = format!(
                "城邑 {}　領土 {}　国力 {:.0}\n生産バイアス ×{:.2}　時代補正 ×{:.2}　能力上限 {:.1}\n交戦中：{}",
                n.cities,
                n.tiles,
                n.strength,
                n.bias,
                self.world.era_mult(),
                cap,
                if wars.is_empty() { "なし".to_string() } else { wars.join("・") }
            );
            self.gfx.text(&title, x, y, wd, 30.0, Font::Big, tc);
            self.gfx.text(&s, x, y + 36.0, wd, 80.0, Font::Normal, tc);
            self.gfx.text("「.」キー：待機中の民を選択　Home：国都へ", x, y + 120.0, wd, 20.0, Font::Small, [0.7, 0.68, 0.62, 1.0]);
        }
    }

    fn render_commands(&mut self, x: f32, y: f32) {
        let pl = self.world.player;
        let era = self.world.era;
        let (bw, bh, gap) = (122.0, 46.0, 4.0);
        let pos = |i: usize| -> (f32, f32, f32, f32) {
            let c = (i % 3) as f32;
            let r = (i / 3) as f32;
            let x0 = x + c * (bw + gap);
            let y0 = y + r * (bh + gap);
            (x0, y0, x0 + bw, y0 + bh)
        };
        let res = self.world.nations[pl].res;
        let afford = |c: [f32; 3]| res[0] >= c[0] && res[1] >= c[1] && res[2] >= c[2];
        let own = self.own_sel();
        if !own.is_empty() {
            let any_worker = own.iter().any(|&h| self.world.unit(h).unwrap().kind == UnitKind::Worker);
            let any_mil = own.iter().any(|&h| self.world.unit(h).unwrap().kind.military());
            let mut i = 0;
            if any_worker {
                for (k, key, tip) in [
                    (BKind::Farm, 'Q', "田：民が耕すと糧を産む。自国領内に建設。"),
                    (BKind::Barracks, 'W', "兵営：軍事ユニットを訓練する。自国領内に建設。"),
                    (BKind::Town, 'E', "邑：領土を広げ人口上限を上げる。他の城邑から9以上離し、自国城邑から22以内。"),
                ] {
                    let (x0, y0, x1, y1) = pos(i);
                    let c = k.cost();
                    self.add_button(x0, y0, x1, y1, k.name(), &Self::cost_str(c), key as u32, afford(c), Act::Place(k), tip);
                    i += 1;
                }
            }
            // 鍛錬
            let u0 = self.world.unit(own[0]).unwrap();
            let up_cost = World::upgrade_cost(u0);
            let label = if any_mil && !any_worker { "訓練" } else if any_worker && !any_mil { "教化" } else { "鍛錬" };
            let (x0, y0, x1, y1) = pos(i);
            self.add_button(x0, y0, x1, y1, label, &Self::cost_str(up_cost), 'T' as u32, true, Act::Upgrade, "選択部隊の能力を +0.5 高める（自国領内のみ）。兵は戦闘力と兵力、民は採集量が上がる。費用は現在の能力に比例。");
            i += 1;
            let (x0, y0, x1, y1) = pos(i);
            let (cl, ct) = if any_mil && !any_worker {
                ("解散→民", "兵を解散して民にする（能力は2/3に目減り）。")
            } else if any_worker && !any_mil {
                ("徴兵→戈兵", "民を徴兵して戈兵にする（能力は2/3に目減り）。")
            } else {
                ("兵⇔民", "兵は民に、民は戈兵に転換する（能力は2/3に目減り）。")
            };
            self.add_button(x0, y0, x1, y1, cl, "能力×2/3", 'C' as u32, true, Act::Convert, ct);
            i += 1;
            let (x0, y0, x1, y1) = pos(i);
            self.add_button(x0, y0, x1, y1, "停止", "", 'S' as u32, true, Act::Stop, "");
            i += 1;
            if any_mil {
                let (x0, y0, x1, y1) = pos(i);
                self.add_button(x0, y0, x1, y1, "進軍", "攻撃しつつ移動", 'A' as u32, true, Act::AttackMove, "クリックした地点へ、途中の敵と戦いながら進む。");
            }
        } else if let Some(bh) = self.sel_b {
            let b = self.world.bld(bh).unwrap();
            if b.nation as usize != pl {
                return;
            }
            let (kind, done) = (b.kind, b.done());
            let mut i = 0;
            if done {
                let list: Vec<(UnitKind, char, &str)> = match kind {
                    BKind::Capital | BKind::Town => vec![(UnitKind::Worker, 'Q', "民：資源を集め、建物を建てる。")],
                    BKind::Barracks => vec![
                        (UnitKind::Infantry, 'Q', "戈兵：戦車・騎兵に強い歩兵。"),
                        (UnitKind::Archer, 'W', "弩兵：遠距離攻撃。歩兵に強い。"),
                        (UnitKind::Chariot, 'E', if era == Era::Spring { "戦車：高速。弩兵に強い。" } else { "騎兵：高速。弩兵に強い。" }),
                    ],
                    BKind::Farm => vec![],
                };
                for (k, key, tip) in list {
                    let (x0, y0, x1, y1) = pos(i);
                    let c = k.cost();
                    let tip = format!("{}\n訓練時間 {:.0}秒　維持費 {}", tip, k.train_time(), if k.military() { "糧0.07/秒×能力" } else { "なし" });
                    self.add_button(x0, y0, x1, y1, k.name(era), &Self::cost_str(c), key as u32, afford(c), Act::Train(k), &tip);
                    i += 1;
                }
            }
            if !kind.city() {
                let (x0, y0, x1, y1) = pos(8);
                self.add_button(x0, y0, x1, y1, "取り壊す", "", 0, true, Act::Demolish, "この建物を取り壊す（Delete）");
            }
        }
    }

    fn render_diplo(&mut self) {
        let (w, h) = (self.gfx.width as f32, self.gfx.height as f32);
        let pl = self.world.player;
        let pw = 960.0f32.min(w - 20.0);
        let ph = 620.0f32.min(h - 60.0);
        let x0 = (w - pw) / 2.0;
        let y0 = (h - ph) / 2.0;
        self.gfx.fill_rect(0.0, 0.0, w, h, [0.0, 0.0, 0.0, 0.4]);
        self.gfx.fill_round(x0, y0, x0 + pw, y0 + ph, 8.0, [0.10, 0.08, 0.06, 0.97]);
        self.gfx.stroke_round(x0, y0, x0 + pw, y0 + ph, 8.0, 2.0, [0.75, 0.6, 0.35, 1.0]);
        self.gfx.text("外交", x0 + 20.0, y0 + 10.0, 200.0, 34.0, Font::Big, [1.0, 0.85, 0.5, 1.0]);
        let my = self.world.nations[pl].strength;
        let s = format!("自国の国力 {:.0}　（国力は兵の能力・城邑・民・領土から算出）", my);
        self.gfx.text(&s, x0 + 100.0, y0 + 16.0, pw - 200.0, 24.0, Font::Normal, [0.85, 0.82, 0.75, 1.0]);
        self.add_button(x0 + pw - 90.0, y0 + 10.0, x0 + pw - 14.0, y0 + 40.0, "閉じる", "", 0, true, Act::CloseOverlay, "");
        let home = self.world.nations[pl].home;
        let mut list: Vec<usize> = (0..self.world.nations.len()).filter(|&i| i != pl && self.world.nations[i].alive).collect();
        list.sort_by(|&a, &b| self.world.nations[a].home.dist(home).partial_cmp(&self.world.nations[b].home.dist(home)).unwrap());
        let cols = 2;
        let colw = (pw - 30.0) / cols as f32;
        let rowh = 25.0;
        let rows_per = ((ph - 70.0) / rowh) as usize;
        for (k, &o) in list.iter().enumerate() {
            let c = k / rows_per;
            if c >= cols {
                break;
            }
            let r = k % rows_per;
            let rx = x0 + 15.0 + c as f32 * colw;
            let ry = y0 + 54.0 + r as f32 * rowh;
            let n = &self.world.nations[o];
            let war = self.world.at_war(pl, o);
            let truce = self.world.truce[pl][o] - self.world.years;
            let locked = self.world.coalition_locked(pl, o);
            let offer = self.world.peace_offer[o];
            if r % 2 == 0 {
                self.gfx.fill_rect(rx, ry, rx + colw - 8.0, ry + rowh, [1.0, 1.0, 1.0, 0.04]);
            }
            self.gfx.fill_rect(rx + 4.0, ry + 5.0, rx + 18.0, ry + 19.0, c4(n.color, 1.0));
            let name = format!("{}{}", n.name, if NATIONS[o].seven { "★" } else { "" });
            self.gfx.text(&name, rx + 24.0, ry + 3.0, 80.0, 20.0, Font::Normal, [0.95, 0.92, 0.85, 1.0]);
            let st = format!("国力{:.0} 城{} 距離{:.0}", n.strength, n.cities, n.home.dist(home));
            self.gfx.text(&st, rx + 90.0, ry + 5.0, 170.0, 20.0, Font::Small, [0.75, 0.72, 0.66, 1.0]);
            let (rel, rc) = if war {
                if locked {
                    ("交戦中（連合）", [1.0, 0.35, 0.3, 1.0])
                } else if offer {
                    ("和睦の申し入れ", [0.6, 0.9, 1.0, 1.0])
                } else {
                    ("交戦中", [1.0, 0.45, 0.35, 1.0])
                }
            } else if truce > 0.0 {
                ("休戦中", [0.7, 0.9, 0.7, 1.0])
            } else {
                ("平和", [0.75, 0.75, 0.7, 1.0])
            };
            let rel = if !war && truce > 0.0 { format!("休戦 残{}年", truce.ceil() as i32) } else { rel.to_string() };
            self.gfx.text(&rel, rx + 260.0, ry + 4.0, 110.0, 20.0, Font::Small, rc);
            let bx = rx + colw - 82.0;
            if war {
                let lbl = if offer { "受諾" } else { "和睦" };
                self.add_button(bx, ry + 2.0, bx + 70.0, ry + rowh - 2.0, lbl, "", 0, !locked, Act::Peace(o), "");
            } else {
                self.add_button(bx, ry + 2.0, bx + 70.0, ry + rowh - 2.0, "宣戦", "", 0, truce <= 0.0, Act::War(o), "");
            }
        }
    }

    fn render_help(&mut self) {
        let (w, h) = (self.gfx.width as f32, self.gfx.height as f32);
        let pw = 880.0f32.min(w - 20.0);
        let ph = 640.0f32.min(h - 40.0);
        let x0 = (w - pw) / 2.0;
        let y0 = (h - ph) / 2.0;
        self.gfx.fill_rect(0.0, 0.0, w, h, [0.0, 0.0, 0.0, 0.4]);
        self.gfx.fill_round(x0, y0, x0 + pw, y0 + ph, 8.0, [0.10, 0.08, 0.06, 0.97]);
        self.gfx.stroke_round(x0, y0, x0 + pw, y0 + ph, 8.0, 2.0, [0.75, 0.6, 0.35, 1.0]);
        self.gfx.text("遊び方", x0 + 20.0, y0 + 10.0, 200.0, 34.0, Font::Big, [1.0, 0.85, 0.5, 1.0]);
        self.add_button(x0 + pw - 90.0, y0 + 10.0, x0 + pw - 14.0, y0 + 40.0, "閉じる", "", 0, true, Act::CloseOverlay, "");
        let text = "\
■ 目的　紀元前221年まで生き残り、最も強い国となるか、天下を統一せよ。時間は全ての国で共通に流れ（1年＝4秒）、春秋時代（〜前453年）から戦国時代へ移る。どの国も同じ時代補正を受け、一部の国だけ文明が進むことはない。

■ 資源　糧（田を耕す）・木（森を伐採）・銅（銅山を採掘）。民が集めて都・邑へ運ぶ。城邑からも少しずつ税収がある。

■ ユニット　動かせる数には上限がある（城邑の数で増える）。その代わり「訓練／教化」で一体ごとの能力を高められる（時代ごとに上限あり）。軍事ユニットは存在するだけで糧を消費する（維持費）。糧が尽きると兵は弱っていく。兵を解散して民に、民を徴兵して戈兵にできるが、能力は2/3に目減りする。
　戈兵＞戦車（騎兵）＞弩兵＞戈兵 の相性がある。

■ 外交　F2で外交画面。宣戦しないと他国を攻撃できない。城邑を攻め落とすと自国のものになる。全ての城邑を失った国は滅亡する。

■ 覇者の呼びかけ（春秋）　宣戦や攻略を繰り返すと「諸侯の警戒」が高まり、100に達すると最強国が諸侯に呼びかけ、周辺国が15年間一斉に攻めてくる。
■ 合従軍（戦国）　突出して強い国が現れると、周辺国が合従して15年間一斉に攻撃する。
■ 三家分晋　戦国時代の到来とともに晋は韓・趙・魏に分かれる。
■ 戦場の霧　自国の民・兵・建物の周囲しか見えない。一度見た土地は暗く残り、地形と城は見えるが敵兵は見えない。諸侯の都の所在だけは初めから分かっている。

■ 操作
　左クリック：選択　ドラッグ：範囲選択　ダブルクリック：同種を全選択　Shift：追加選択
　右クリック：移動／攻撃／採集（森・銅山・田）／建設の手伝い　建物選択中の右クリック：集合地点
　A：進軍（攻撃移動）　S：停止　T：訓練・教化　C：兵⇔民の転換　Q/W/E：建設・訓練
　Ctrl+数字：部隊登録　数字：部隊呼び出し　.：待機中の民　Home：国都へ
　ホイール：拡大縮小　中ボタンドラッグ・矢印キー・画面端：スクロール　ミニマップクリック：移動
　Space：一時停止　＋／－：速度　Esc：取り消し　F11：全画面　Ctrl＋＋／Ctrl＋−：文字の大きさ";
        self.gfx.text(text, x0 + 24.0, y0 + 52.0, pw - 48.0, ph - 64.0, Font::Normal, [0.92, 0.9, 0.84, 1.0]);
    }

    fn render_gameover(&mut self) {
        let (w, h) = (self.gfx.width as f32, self.gfx.height as f32);
        let Some((win, text)) = self.world.game_over.clone() else { return };
        self.gfx.fill_rect(0.0, 0.0, w, h, [0.0, 0.0, 0.0, 0.55]);
        let pw = 640.0;
        let x0 = (w - pw) / 2.0;
        let y0 = h / 2.0 - 140.0;
        self.gfx.fill_round(x0, y0, x0 + pw, y0 + 260.0, 10.0, [0.12, 0.09, 0.06, 0.97]);
        self.gfx.stroke_round(x0, y0, x0 + pw, y0 + 260.0, 10.0, 2.0, [0.85, 0.7, 0.4, 1.0]);
        let title = if win { "勝利" } else { "敗北" };
        self.gfx.text(title, x0, y0 + 20.0, pw, 60.0, Font::Title, if win { [1.0, 0.85, 0.4, 1.0] } else { [0.9, 0.4, 0.35, 1.0] });
        self.gfx.text(&text, x0 + 20.0, y0 + 100.0, pw - 40.0, 60.0, Font::BigCenter, [0.95, 0.92, 0.85, 1.0]);
        self.add_button(x0 + pw / 2.0 - 100.0, y0 + 180.0, x0 + pw / 2.0 + 100.0, y0 + 228.0, "新しい天下へ", "", 0, true, Act::Restart, "");
    }
}
