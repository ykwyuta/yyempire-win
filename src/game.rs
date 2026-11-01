//! 画面・入力・HUD（独自実装の UI）

mod chronicle;
mod tutorial;

use crate::art::{paint_aoe_terrain_pixel, Art, CmdIcon, ResIcon};
use crate::geo::*;
use crate::gfx::{Font, Gfx, Texture};
use crate::rng::hash2;
use crate::world::*;
use crate::Ev;
use windows::Win32::Graphics::Direct2D::ID2D1Bitmap;

const TEX_SCALE: usize = 8;
const OVERLAY_SCALE: usize = 4;
const TOP_H: f32 = 36.0;
const BOT_H: f32 = 176.0;
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
const VK_F3: u32 = 0x72;
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
    SelectAllWorkers,
    SelectAllMilitary,
    SelectKind(UnitKind),
    FilterKind(UnitKind),
    WorkersAuto,
    AllWorkersAuto,
    AllWorkersWood,
    AllWorkersMine,
    AllWorkersUpgrade,
    ArmyDefend,
    AllArmyAttack,
    AllArmyDefend,
    AllArmyUpgrade,
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
    Chron,
    ChronPage(i32),
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
    art: Art,
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
    show_chron: bool,
    chron_scroll: usize,
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
        let ow = (MAP_W * OVERLAY_SCALE) as u32;
        let oh = (MAP_H * OVERLAY_SCALE) as u32;
        let terrain_px = vec![0u32; (tw * th) as usize];
        let overlay_px = vec![0u32; (ow * oh) as usize];
        let terrain_tex = gfx.create_texture(tw, th, &terrain_px).expect("texture");
        let overlay_tex = gfx.create_texture(ow, oh, &overlay_px).expect("texture");
        let fog_px = vec![0u32; MAP_W * MAP_H];
        let fog_tex = gfx.create_texture(MAP_W as u32, MAP_H as u32, &fog_px).expect("texture");
        let mut g = Game {
            gfx,
            art: Art::new(),
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
            show_chron: false,
            chron_scroll: 0,
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
        let s = TEX_SCALE as i32;
        for py in 0..s {
            for px in 0..s {
                let gx = x * s + px;
                let gy = y * s + py;
                let c = paint_aoe_terrain_pixel(&self.world.map.terrain, &self.world.map.shade, x, y, px, py, s);
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
        let os = OVERLAY_SCALE;
        let ow = MAP_W * os;
        let owner_at = |nx: i32, ny: i32| -> u8 {
            let cx = nx.clamp(0, MAP_W as i32 - 1);
            let cy = ny.clamp(0, MAP_H as i32 - 1);
            let ni = Map::idx(cx, cy);
            if w.map.terrain[ni] == Terrain::Sea {
                NONE
            } else {
                w.owner[ni]
            }
        };
        for y in 0..MAP_H as i32 {
            for x in 0..MAP_W as i32 {
                let o = owner_at(x, y);
                for sy in 0..os {
                    for sx in 0..os {
                        let fx = x as f32 + (sx as f32 + 0.5) / os as f32 - 0.5;
                        let fy = y as f32 + (sy as f32 + 0.5) / os as f32 - 0.5;
                        let x0 = fx.floor() as i32;
                        let y0 = fy.floor() as i32;
                        let tx = fx - x0 as f32;
                        let ty = fy - y0 as f32;
                        let ux = tx * tx * (3.0 - 2.0 * tx);
                        let uy = ty * ty * (3.0 - 2.0 * ty);

                        let o00 = owner_at(x0, y0);
                        let o10 = owner_at(x0 + 1, y0);
                        let o01 = owner_at(x0, y0 + 1);
                        let o11 = owner_at(x0 + 1, y0 + 1);

                        // 最も近い有効な領主を決定
                        let dom = if o != NONE {
                            o
                        } else if o00 != NONE {
                            o00
                        } else if o10 != NONE {
                            o10
                        } else if o01 != NONE {
                            o01
                        } else {
                            o11
                        };
                        if dom == NONE {
                            self.overlay_px[(y as usize * os + sy) * ow + x as usize * os + sx] = 0;
                            continue;
                        }
                        let w00 = if o00 == dom { 1.0 } else { 0.0 };
                        let w10 = if o10 == dom { 1.0 } else { 0.0 };
                        let w01 = if o01 == dom { 1.0 } else { 0.0 };
                        let w11 = if o11 == dom { 1.0 } else { 0.0 };
                        let wt = (w00 * (1.0 - ux) + w10 * ux) * (1.0 - uy) + (w01 * (1.0 - ux) + w11 * ux) * uy;

                        let c = w.nations[dom as usize].color;
                        let px = if wt < 0.36 {
                            0
                        } else if wt < 0.60 {
                            rgba([c[0].min(1.0), c[1].min(1.0), c[2].min(1.0)], 0.86)
                        } else if wt < 0.80 {
                            rgba(c, 0.30)
                        } else {
                            rgba(c, 0.065)
                        };
                        self.overlay_px[(y as usize * os + sy) * ow + x as usize * os + sx] = px;
                    }
                }
            }
        }
        self.gfx.update_texture(&self.overlay_tex, 0, 0, ow as u32, (MAP_H * os) as u32, &self.overlay_px);
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
        if self.show_diplo || self.show_help || self.show_chron || self.world.game_over.is_some() {
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
                    self.speed = std::env::var("YY_SPEED").ok().and_then(|v| v.parse().ok()).unwrap_or(8);
                    self.zoom = z;
                    // 撮影直前にキーを押す（例：72 で史記を開く）
                    if let Some(k) = std::env::var("YY_PRESS").ok().and_then(|v| u32::from_str_radix(&v, 16).ok()) {
                        let t = (secs - 0.6).max(self.clock + 0.3);
                        self.script = vec![(t, Ev::Key(k, true)), (t + 0.05, Ev::Key(k, false))];
                    }
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
                        if x >= mx0 && x <= mx1 && y >= my0 && y <= my1 && !self.show_diplo && !self.show_help && !self.show_chron {
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
                } else if self.show_diplo || self.show_help || self.show_chron {
                    self.show_diplo = false;
                    self.show_help = false;
                    self.show_chron = false;
                } else {
                    self.clear_sel();
                }
            }
            VK_SPACE => self.paused = !self.paused,
            VK_F1 => self.show_help = !self.show_help,
            VK_F2 => self.show_diplo = !self.show_diplo,
            VK_F3 => self.do_act(Act::Chron),
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

    fn all_workers(&self) -> Vec<Handle> {
        let p = self.world.player;
        (0..self.world.units.len())
            .filter(|&i| {
                let u = &self.world.units[i];
                u.alive && u.nation as usize == p && u.kind == UnitKind::Worker
            })
            .map(|i| self.world.uh(i))
            .collect()
    }

    fn all_military(&self) -> Vec<Handle> {
        let p = self.world.player;
        (0..self.world.units.len())
            .filter(|&i| {
                let u = &self.world.units[i];
                u.alive && u.nation as usize == p && u.kind.military()
            })
            .map(|i| self.world.uh(i))
            .collect()
    }

    fn upgrade_handles(&mut self, mut hs: Vec<Handle>) {
        let mut err = None;
        let mut ok = 0;
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

    /// 民（選択民または全民）を空いている田・森・銅山へバランス配分する
    fn order_workers_balanced(&mut self, hs: &[Handle], skip_builders: bool) {
        let pl = self.world.player;
        let mut gathering = [0usize; 3];
        for u in self.world.units.iter() {
            if u.alive && u.nation as usize == pl && u.kind == UnitKind::Worker && !hs.iter().any(|&h| self.world.unit(h).map_or(false, |x| std::ptr::eq(x, u))) {
                if let Order::Gather { src, .. } = u.order {
                    gathering[src.res()] += 1;
                }
            }
        }
        let mut assigned = 0;
        let res = self.world.nations[pl].res;
        let mut want = [0.44f32, 0.33, 0.23];
        if self.world.nations[pl].upkeep > self.world.nations[pl].income[0] * 0.55 {
            want[0] += 0.18;
        }
        for k in 0..3 {
            if res[k] > 700.0 {
                want[k] *= 0.45;
            }
        }
        let sumw: f32 = want.iter().sum();

        for &h in hs {
            let Some(u) = self.world.unit(h) else { continue };
            if u.kind != UnitKind::Worker {
                continue;
            }
            if skip_builders && matches!(u.order, Order::Build(_)) {
                continue;
            }
            let p = u.pos;
            let total = (gathering[0] + gathering[1] + gathering[2] + 1) as f32;
            let mut best_k = 0;
            let mut best_def = f32::MIN;
            for k in 0..3 {
                let def = want[k] / sumw * total - gathering[k] as f32;
                if def > best_def {
                    best_def = def;
                    best_k = k;
                }
            }
            let src = match best_k {
                0 => self.world.free_farm(pl, p).map(Src::Farm),
                2 => self.world.nearest_mine(p, 28.0).map(Src::Mine),
                _ => self.world.nearest_forest(p, 22).map(|(x, y)| Src::Forest(x, y)),
            }
            .or_else(|| self.world.free_farm(pl, p).map(Src::Farm))
            .or_else(|| self.world.nearest_forest(p, 28).map(|(x, y)| Src::Forest(x, y)))
            .or_else(|| self.world.nearest_mine(p, 36.0).map(Src::Mine));
            if let Some(s) = src {
                gathering[s.res()] += 1;
                self.world.order_gather(&[h], s);
                assigned += 1;
            }
        }
        if assigned > 0 {
            self.notice(&format!("民{}体を田・森・銅山へ自動配分した", assigned));
        } else {
            self.notice("配分できる民や資源が近くにない");
        }
    }

    /// 民に一斉伐採（最寄りの森）または一斉採掘（最寄りの銅山）を命じる
    fn order_workers_resource(&mut self, hs: &[Handle], wood: bool) {
        let mut assigned = 0;
        let home = self.world.nations[self.world.player].home;
        for &h in hs {
            let Some(u) = self.world.unit(h) else { continue };
            if u.kind != UnitKind::Worker || matches!(u.order, Order::Build(_)) {
                continue;
            }
            let p = u.pos;
            let src = if wood {
                self.world
                    .nearest_forest(p, 28)
                    .or_else(|| self.world.nearest_forest(home, 36))
                    .map(|(x, y)| Src::Forest(x, y))
            } else {
                self.world
                    .nearest_mine(p, 35.0)
                    .or_else(|| self.world.nearest_mine(home, 45.0))
                    .map(Src::Mine)
            };
            if let Some(s) = src {
                self.world.order_gather(&[h], s);
                assigned += 1;
            }
        }
        if assigned > 0 {
            let kind_str = if wood { "森の伐採" } else { "銅山の採掘" };
            self.notice(&format!("民{}体に{}を命じた", assigned, kind_str));
        } else {
            self.notice(if wood { "近くに伐採できる森がない" } else { "近くに採掘できる銅山がない" });
        }
    }

    /// 軍事ユニットに領内防衛・侵入者迎撃・城邑集結を命じる
    fn order_army_defend(&mut self, hs: &[Handle]) {
        if hs.is_empty() {
            self.notice("軍事ユニットがいない");
            return;
        }
        let pl = self.world.player;
        let home = self.world.nations[pl].home;
        let mut intruder: Option<V2> = None;
        for u in self.world.units.iter() {
            if !u.alive || !self.world.at_war(pl, u.nation as usize) {
                continue;
            }
            let (x, y) = u.pos.tile();
            if Map::inb(x, y) && self.world.owner[Map::idx(x, y)] as usize == pl {
                if intruder.map_or(true, |p| p.dist(home) > u.pos.dist(home)) {
                    intruder = Some(u.pos);
                }
            }
        }
        if let Some(p) = intruder {
            self.world.order_move(hs, p, true);
            self.marker = Some((p, 0.6, true));
            self.notice(&format!("{}部隊に領内侵入者の迎撃を命じた", hs.len()));
            if let Some(t) = &mut self.tut {
                t.moved = true;
            }
            return;
        }
        let cities: Vec<V2> = self
            .world
            .blds
            .iter()
            .filter(|b| b.alive && b.nation as usize == pl && b.kind.city() && b.done())
            .map(|b| b.center())
            .collect();
        let rally = if cities.is_empty() {
            home
        } else {
            // 各部隊から最も近い自国城邑へ帰還・防衛配置
            for (k, &h) in hs.iter().enumerate() {
                if let Some(u) = self.world.unit(h) {
                    let up = u.pos;
                    let c = cities.iter().copied().min_by(|a, b| a.dist(up).partial_cmp(&b.dist(up)).unwrap()).unwrap_or(home);
                    let ang = (k as f32) * 1.1;
                    let goal = c + v2(ang.cos() * 2.2, ang.sin() * 2.2);
                    self.world.order_move(&[h], goal, true);
                }
            }
            cities[0]
        };
        self.marker = Some((rally, 0.6, false));
        self.notice(&format!("{}部隊に城邑の防衛集結を命じた", hs.len()));
        if let Some(t) = &mut self.tut {
            t.moved = true;
        }
    }

    /// 全軍に総進撃（交戦国の最寄り城邑・敵兵への攻撃移動＋進軍モード）を命じる
    fn order_all_army_attack(&mut self) {
        let hs = self.all_military();
        if hs.is_empty() {
            self.notice("軍事ユニットがいない");
            return;
        }
        self.set_sel(hs.clone());
        let pl = self.world.player;
        let home = self.world.nations[pl].home;
        // 交戦中の敵城邑または敵部隊を探す
        let mut best_target: Option<(f32, V2)> = None;
        for b in self.world.blds.iter() {
            if b.alive && self.world.at_war(pl, b.nation as usize) && b.kind.city() {
                let d = b.center().dist(home);
                if best_target.map_or(true, |x| d < x.0) {
                    best_target = Some((d, b.center()));
                }
            }
        }
        if best_target.is_none() {
            for u in self.world.units.iter() {
                if u.alive && self.world.at_war(pl, u.nation as usize) {
                    let d = u.pos.dist(home);
                    if best_target.map_or(true, |x| d < x.0) {
                        best_target = Some((d, u.pos));
                    }
                }
            }
        }
        if let Some((_, goal)) = best_target {
            self.world.order_move(&hs, goal, true);
            self.marker = Some((goal, 0.6, true));
            self.mode = Mode::AttackMove;
            self.notice(&format!("全軍{}部隊に敵陣への総進撃を命じた（地図クリックで目標変更可）", hs.len()));
            if let Some(t) = &mut self.tut {
                t.moved = true;
            }
        } else {
            self.mode = Mode::AttackMove;
            self.notice(&format!("全軍{}部隊を選択：進軍先を地図上でクリックしてください", hs.len()));
        }
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
                self.show_chron = false;
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
                let hs = self.own_sel();
                self.upgrade_handles(hs);
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
            Act::SelectAllWorkers => {
                let hs = self.all_workers();
                if hs.is_empty() {
                    self.notice("民がいない");
                } else {
                    self.notice(&format!("民全体（{}体）を選択した", hs.len()));
                    self.set_sel(hs);
                }
            }
            Act::SelectAllMilitary => {
                let hs = self.all_military();
                if hs.is_empty() {
                    self.notice("軍事ユニットがいない");
                } else {
                    self.notice(&format!("全軍事ユニット（{}体）を選択した", hs.len()));
                    self.set_sel(hs);
                }
            }
            Act::SelectKind(kind) => {
                let hs: Vec<Handle> = (0..self.world.units.len())
                    .filter(|&i| {
                        let u = &self.world.units[i];
                        u.alive && u.nation as usize == pl && u.kind == kind
                    })
                    .map(|i| self.world.uh(i))
                    .collect();
                if hs.is_empty() {
                    self.notice(&format!("{}はいない", kind.name(self.world.era)));
                } else {
                    self.notice(&format!("{}全体（{}体）を選択した", kind.name(self.world.era), hs.len()));
                    self.set_sel(hs);
                }
            }
            Act::FilterKind(kind) => {
                let hs: Vec<Handle> = self
                    .own_sel()
                    .into_iter()
                    .filter(|&h| self.world.unit(h).map_or(false, |u| u.kind == kind))
                    .collect();
                if !hs.is_empty() {
                    self.set_sel(hs);
                }
            }
            Act::WorkersAuto => {
                let hs = self.own_sel();
                self.order_workers_balanced(&hs, false);
            }
            Act::AllWorkersAuto => {
                let hs = self.all_workers();
                self.order_workers_balanced(&hs, true);
            }
            Act::AllWorkersWood => {
                let hs = self.all_workers();
                self.order_workers_resource(&hs, true);
            }
            Act::AllWorkersMine => {
                let hs = self.all_workers();
                self.order_workers_resource(&hs, false);
            }
            Act::AllWorkersUpgrade => {
                let hs = self.all_workers();
                if hs.is_empty() {
                    self.notice("民がいない");
                } else {
                    self.upgrade_handles(hs);
                }
            }
            Act::ArmyDefend => {
                let hs: Vec<Handle> = self.own_sel().into_iter().filter(|&h| self.world.unit(h).map_or(false, |u| u.kind.military())).collect();
                self.order_army_defend(&hs);
            }
            Act::AllArmyAttack => {
                self.order_all_army_attack();
            }
            Act::AllArmyDefend => {
                let hs = self.all_military();
                self.order_army_defend(&hs);
            }
            Act::AllArmyUpgrade => {
                let hs = self.all_military();
                if hs.is_empty() {
                    self.notice("軍事ユニットがいない");
                } else {
                    self.upgrade_handles(hs);
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
                self.show_chron = false;
            }
            Act::Help => {
                self.show_help = !self.show_help;
                self.show_diplo = false;
                self.show_chron = false;
            }
            Act::Chron => {
                self.show_chron = !self.show_chron;
                self.chron_scroll = 0;
                self.show_diplo = false;
                self.show_help = false;
            }
            Act::ChronPage(d) => {
                self.chron_scroll = (self.chron_scroll as i32 + d).max(0) as usize;
            }
            Act::CloseOverlay => {
                self.show_diplo = false;
                self.show_help = false;
                self.show_chron = false;
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
        self.gfx.begin([0.07, 0.06, 0.05, 1.0]);
        self.render_world();
        if self.gfx.d2d_begin() {
            self.art.ensure(&self.gfx);
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
                if self.show_chron {
                    self.render_chron();
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
        crate::art::draw_map_frame(g, mw, mh);
        g.draw_textured(&self.terrain_tex, 0.0, 0.0, mw, mh, [1.0; 4], true);
        g.draw_textured(&self.overlay_tex, 0.0, 0.0, mw, mh, [1.0; 4], true);
        let wd = &self.world;
        let view0 = v2(self.cam.x - w / 2.0 / z - 2.0, self.cam.y - h / 2.0 / z - 2.0);
        let view1 = v2(self.cam.x + w / 2.0 / z + 2.0, self.cam.y + h / 2.0 / z + 2.0);
        let inview = |p: V2| p.x >= view0.x && p.x <= view1.x && p.y >= view0.y && p.y <= view1.y;
        let px = 1.0 / z; // 1ピクセル
        // 森林の樹木・山岳の立体表現
        crate::art::draw_world_terrain_props(
            g,
            &wd.map,
            view0.x.floor() as i32,
            view0.y.floor() as i32,
            view1.x.ceil() as i32,
            view1.y.ceil() as i32,
            z,
            &fog_at,
        );
        // 銅山
        for m in wd.map.mines.iter() {
            if m.amount <= 0.0 {
                continue;
            }
            let c = v2(m.x as f32 + 0.5, m.y as f32 + 0.5);
            if !inview(c) || fog_at(c) == 0 {
                continue;
            }
            crate::art::draw_world_mine(g, c, m.amount, px);
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
            crate::art::draw_world_building(g, b, nc, px);
            let sel = self.sel_b == Some(wd.bh(bi));
            if sel {
                g.rect_outline(x0 - 0.32, y0 - 0.32, x1 + 0.32, y1 + 0.32, (2.2 * px).max(0.05), [1.0, 0.92, 0.55, 0.95]);
                if let Some(r) = b.rally {
                    g.line((c.x, c.y), (r.x, r.y), 1.5 * px, [1.0, 0.92, 0.55, 0.65]);
                    g.rect(r.x - 0.05, r.y - 0.65, r.x + 0.05, r.y, [0.92, 0.85, 0.65, 1.0]);
                    g.tri((r.x + 0.05, r.y - 0.65), (r.x + 0.48, r.y - 0.48), (r.x + 0.05, r.y - 0.32), c4(nc, 1.0));
                }
            }
        }
        // ユニット
        let era = wd.era;
        for u in wd.units.iter() {
            if !u.alive || !inview(u.pos) || (u.nation as usize != pl && fog_at(u.pos) != 2) {
                continue;
            }
            let nc = wd.nations[u.nation as usize].color;
            crate::art::draw_world_unit(g, u, nc, era, &RES_COLORS, px);
        }
        // 矢
        for s in wd.shots.iter() {
            let t = 1.0 - s.t / 0.3;
            let p = s.a + (s.b - s.a) * t.clamp(0.0, 1.0);
            if fog_at(p) != 2 {
                continue;
            }
            let d = (s.b - s.a).norm();
            g.line((p.x - d.x * 0.34, p.y - d.y * 0.34), (p.x, p.y), (2.2 * px).max(0.045), [0.85, 0.72, 0.45, 0.95]);
            g.circle(p.x, p.y, (1.8 * px).max(0.04), 5, [1.0, 0.95, 0.65, 1.0]);
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
        let fog_on = self.fog_active();
        let pl = self.world.player;

        enum WorldItem {
            Mountain { sx: f32, sy: f32, r: f32, flip_x: bool, fogged: bool },
            Forest { sx: f32, sy: f32, r: f32, flip_x: bool, fogged: bool },
            Mine { sx: f32, sy: f32, r: f32, fogged: bool },
            Building {
                sx: f32,
                sy: f32,
                r: f32,
                kind: BKind,
                nc: [f32; 3],
                done: bool,
                progress: f32,
                hp_frac: Option<f32>,
                fogged: bool,
            },
            Unit {
                sx: f32,
                sy: f32,
                kind: UnitKind,
                pw: f32,
                nc: [f32; 3],
                carry: Option<usize>,
                flip_x: bool,
                hp_frac: Option<f32>,
            },
        }

        if z >= 2.2 {
            let mut items: Vec<(f32, WorldItem)> = Vec::new();
            let view0 = v2(self.cam.x - w / 2.0 / z - 2.5, self.cam.y - h / 2.0 / z - 2.5);
            let view1 = v2(self.cam.x + w / 2.0 / z + 2.5, self.cam.y + h / 2.0 / z + 2.5);
            let x0 = (view0.x.floor() as i32).max(0);
            let y0 = (view0.y.floor() as i32).max(0);
            let x1 = (view1.x.ceil() as i32).min(MAP_W as i32 - 1);
            let y1 = (view1.y.ceil() as i32).min(MAP_H as i32 - 1);

            // 山岳・森林のAI生成スプライト
            let thin = z < 8.5;
            for ty in y0..=y1 {
                for tx in x0..=x1 {
                    let idx = Map::idx(tx, ty);
                    let t = self.world.map.terrain[idx];
                    if !matches!(t, Terrain::Mountain | Terrain::Forest) {
                        continue;
                    }
                    if z < 5.5 {
                        if ((tx * 3 + ty * 5) & 3) != 0 {
                            continue;
                        }
                    } else if thin && ((tx + ty) & 1) != 0 {
                        continue;
                    }
                    let f = if fog_on { self.fog[idx] } else { 2 };
                    if f == 0 {
                        continue;
                    }
                    let fogged = f == 1;
                    let h1 = crate::rng::hash2(tx, ty, 31);
                    let h2 = crate::rng::hash2(tx, ty, 47);
                    let flip_x = h1 > 0.5;
                    let ox = (h1 - 0.5) * 0.28;
                    let oy = (h2 - 0.5) * 0.24;
                    let wp = v2(tx as f32 + 0.5 + ox, ty as f32 + 0.5 + oy);
                    let (sx, sy) = self.world_to_screen(wp);
                    if sx < -64.0 || sy < -64.0 || sx > w + 64.0 || sy > h + 64.0 {
                        continue;
                    }
                    match t {
                        Terrain::Mountain => {
                            let mult = if z < 5.5 { 1.65 } else if thin { 1.25 } else { 0.92 + 0.22 * h2 };
                            let r = (z * mult).clamp(5.5, 44.0);
                            items.push((sy + r * 0.25, WorldItem::Mountain { sx, sy, r, flip_x, fogged }));
                        }
                        Terrain::Forest => {
                            let mult = if z < 5.5 { 1.45 } else if thin { 1.12 } else { 0.80 + 0.20 * h2 };
                            let r = (z * mult).clamp(5.0, 38.0);
                            items.push((sy + r * 0.22, WorldItem::Forest { sx, sy, r, flip_x, fogged }));
                        }
                        _ => {}
                    }
                }
            }

            // 銅山のAI生成スプライト
            for m in self.world.map.mines.iter() {
                if m.amount <= 0.0 {
                    continue;
                }
                let wp = v2(m.x as f32 + 0.5, m.y as f32 + 0.5);
                if !self.explored(wp) {
                    continue;
                }
                let (sx, sy) = self.world_to_screen(wp);
                if sx < -64.0 || sy < -64.0 || sx > w + 64.0 || sy > h + 64.0 {
                    continue;
                }
                let fogged = fog_on && !self.seen(wp);
                let s = 0.78 + 0.24 * (m.amount / 2500.0).clamp(0.2, 1.0);
                let r = (z * s).clamp(6.5, 38.0);
                items.push((sy + r * 0.28, WorldItem::Mine { sx, sy, r, fogged }));
            }

            // 建物（都・邑・兵営・田）のAI生成スプライト
            for (bi, b) in self.world.blds.iter().enumerate() {
                if !b.alive {
                    continue;
                }
                let c = b.center();
                if !self.explored(c) {
                    continue;
                }
                let (sx, sy) = self.world_to_screen(c);
                if sx < -96.0 || sy < -96.0 || sx > w + 96.0 || sy > h + 96.0 {
                    continue;
                }
                let fogged = fog_on && !self.seen(c);
                let nc = self.world.nations[b.nation as usize].color;
                let mult = match b.kind {
                    BKind::Capital => if z < 5.5 { 2.25 } else { 1.62 },
                    BKind::Town => if z < 5.5 { 1.75 } else { 1.38 },
                    BKind::Barracks => 1.25,
                    BKind::Farm => 1.16,
                };
                let r = (z * mult).clamp(9.0, 68.0);
                let sel = self.sel_b == Some(self.world.bh(bi));
                let hp_frac = if b.done() && (b.hp < b.kind.max_hp() - 0.5 || sel) {
                    Some((b.hp / b.kind.max_hp()).clamp(0.0, 1.0))
                } else {
                    None
                };
                let sort_y = if b.kind == BKind::Farm { sy - r * 0.2 } else { sy + r * 0.45 };
                items.push((
                    sort_y,
                    WorldItem::Building {
                        sx,
                        sy,
                        r,
                        kind: b.kind,
                        nc,
                        done: b.done(),
                        progress: b.progress,
                        hp_frac,
                        fogged,
                    },
                ));
            }

            // ユニットの高精細AI生成スプライト（マップや建物に対して適切なサイズに縮小）
            if z >= 6.5 && self.screen == Screen::Playing {
                for u in self.world.units.iter() {
                    if !u.alive || (u.nation as usize != pl && !self.seen(u.pos)) {
                        continue;
                    }
                    let (sx, sy) = self.world_to_screen(u.pos);
                    if sx < -48.0 || sy < -48.0 || sx > w + 48.0 || sy > h + 48.0 {
                        continue;
                    }
                    let nc = self.world.nations[u.nation as usize].color;
                    let carry = if u.carry > 0.5 { Some(u.carry_kind) } else { None };
                    let flip_x = u.dir.x < -0.05;
                    let mhp = u.max_hp();
                    let hp_frac = if u.hp < mhp - 0.5 || u.selected {
                        Some((u.hp / mhp).clamp(0.0, 1.0))
                    } else {
                        None
                    };
                    items.push((
                        sy + 2.0,
                        WorldItem::Unit {
                            sx,
                            sy,
                            kind: u.kind,
                            pw: u.power,
                            nc,
                            carry,
                            flip_x,
                            hp_frac,
                        },
                    ));
                }
            }

            items.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

            for (_, item) in items {
                match item {
                    WorldItem::Mountain { sx, sy, r, flip_x, fogged } => {
                        if let Some(bmp) = self.art.mountain_sprite(flip_x, fogged) {
                            self.gfx.draw_bitmap(bmp, sx - r, sy - r * 1.18, sx + r, sy + r * 0.82);
                        }
                    }
                    WorldItem::Forest { sx, sy, r, flip_x, fogged } => {
                        if let Some(bmp) = self.art.forest_sprite(flip_x, fogged) {
                            self.gfx.draw_bitmap(bmp, sx - r, sy - r * 1.12, sx + r, sy + r * 0.88);
                        }
                    }
                    WorldItem::Mine { sx, sy, r, fogged } => {
                        if let Some(bmp) = self.art.mine_sprite(fogged) {
                            self.gfx.draw_bitmap(bmp, sx - r, sy - r * 1.05, sx + r, sy + r * 0.95);
                        }
                    }
                    WorldItem::Building { sx, sy, r, kind, nc, done, progress, hp_frac, fogged } => {
                        let alpha = if done { 1.0 } else { 0.65 };
                        if let Some(bmp) = self.art.bld_sprite(kind, fogged) {
                            self.gfx.draw_bitmap_alpha(bmp, sx - r, sy - r * 1.08, sx + r, sy + r * 0.92, alpha);
                        }
                        // 建物右下の国色紋章バッジ（どの国の城邑・兵営か一目で判別可能に）
                        if kind != BKind::Farm && z >= 7.0 {
                            let br = (r * 0.16).clamp(3.2, 6.5);
                            let bx = sx + r * 0.56;
                            let by = sy + r * 0.48;
                            let k = if fogged { 0.55 } else { 1.0 };
                            self.gfx.fill_circle(bx, by, br + 1.2, [0.06, 0.05, 0.04, 0.92]);
                            self.gfx.fill_circle(bx, by, br, [nc[0] * k, nc[1] * k, nc[2] * k, 1.0]);
                            self.gfx.stroke_circle(bx, by, br + 0.5, 1.1, [0.95 * k, 0.82 * k, 0.38 * k, 0.95]);
                        }
                        if !done {
                            let bw = (r * 1.4).max(16.0);
                            let by = sy + r * 0.72;
                            self.gfx.fill_round(sx - bw * 0.5, by, sx + bw * 0.5, by + 3.5, 1.5, [0.08, 0.06, 0.04, 0.68]);
                            if progress > 0.01 {
                                self.gfx.fill_round(sx - bw * 0.5, by, sx - bw * 0.5 + bw * progress, by + 3.5, 1.5, [0.35, 0.78, 1.0, 0.95]);
                            }
                            self.gfx.stroke_round(sx - bw * 0.5, by, sx + bw * 0.5, by + 3.5, 1.5, 0.8, [0.78, 0.64, 0.34, 0.75]);
                        } else if let Some(f) = hp_frac {
                            let bw = (r * 1.4).max(16.0);
                            let by = sy - r * 1.02;
                            self.gfx.fill_round(sx - bw * 0.5, by, sx + bw * 0.5, by + 3.5, 1.5, [0.08, 0.06, 0.04, 0.75]);
                            self.gfx.fill_round(sx - bw * 0.5, by, sx - bw * 0.5 + bw * f, by + 3.5, 1.5, [1.0 - f, 0.22 + 0.75 * f, 0.15, 0.95]);
                        }
                    }
                    WorldItem::Unit { sx, sy, kind, pw, nc, carry, flip_x, hp_frac } => {
                        let scale = if kind == UnitKind::Chariot { 0.76 } else { 0.62 };
                        let r = (z * scale * (1.0 + 0.04 * (pw - 1.0).min(4.0))).clamp(7.5, 22.0);
                        if let Some(bmp) = self.art.unit_sprite(kind, era, flip_x) {
                            self.gfx.draw_bitmap(bmp, sx - r, sy - r * 1.35, sx + r, sy + r * 0.65);
                        }
                        // 足元右下の国色小紋章
                        let pr = (r * 0.19).clamp(2.2, 3.8);
                        let px0 = sx + r * 0.42;
                        let py0 = sy + r * 0.32;
                        self.gfx.fill_circle(px0, py0, pr + 0.9, [0.08, 0.06, 0.04, 0.92]);
                        self.gfx.fill_circle(px0, py0, pr, c4(nc, 1.0));
                        self.gfx.stroke_circle(px0, py0, pr + 0.4, 0.8, [0.95, 0.82, 0.42, 0.9]);
                        // 資源運搬中はミニ資源アイコンを左上に表示
                        if let Some(ck) = carry {
                            if z >= 11.0 {
                                let cr = (r * 0.38).clamp(3.8, 6.2);
                                let cx = sx - r * 0.55;
                                let cy = sy - r * 0.65;
                                self.gfx.fill_circle(cx, cy, cr + 0.8, [0.08, 0.06, 0.04, 0.85]);
                                if let Some(rbmp) = &self.art.res_icons[ck] {
                                    self.gfx.draw_bitmap(rbmp, cx - cr, cy - cr, cx + cr, cy + cr);
                                }
                            }
                        }
                        // 強化済みユニットは金の星（ランクピップ）を右下に表示
                        if pw > 1.01 && z >= 12.0 {
                            let stars = ((pw - 1.0) / 0.5).round().clamp(1.0, 6.0) as i32;
                            for s in 0..stars {
                                let px = sx + r * 0.44 - s as f32 * 3.8;
                                let py = sy + r * 0.48;
                                self.gfx.fill_circle(px, py, 2.3, [0.12, 0.08, 0.04, 0.95]);
                                self.gfx.fill_circle(px, py, 1.5, [1.0, 0.88, 0.32, 1.0]);
                            }
                        }
                        if let Some(f) = hp_frac {
                            let bw = (r * 1.4).max(11.0);
                            let by = sy - r * 1.40;
                            self.gfx.fill_rect(sx - bw * 0.5, by, sx + bw * 0.5, by + 2.8, [0.0, 0.0, 0.0, 0.8]);
                            self.gfx.fill_rect(sx - bw * 0.5, by, sx - bw * 0.5 + bw * f, by + 2.8, [1.0 - f, 0.25 + 0.7 * f, 0.15, 1.0]);
                        }
                    }
                }
            }
        }
        // 城邑名（Age of Empires 風の銘板バナー付き）
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
            let off = if z >= 9.0 { 1.58 * z + 14.0 } else { 1.25 * z + 6.0 };
            let ty = sy - off - 14.0;
            if z >= 8.0 {
                let bw = if cap { 64.0 } else { 72.0 };
                let bh = if cap { 24.0 } else { 20.0 };
                let bx0 = sx - bw * 0.5;
                let by0 = ty + (28.0 - bh) * 0.5;
                self.gfx.fill_round(bx0, by0, bx0 + bw, by0 + bh, 4.0, [0.08, 0.06, 0.04, 0.82]);
                self.gfx.stroke_round(bx0, by0, bx0 + bw, by0 + bh, 4.0, 1.2, [c[0] * 0.7 + 0.25, c[1] * 0.7 + 0.25, c[2] * 0.7 + 0.2, 0.9]);
            }
            let col = [c[0] * 0.45 + 0.55, c[1] * 0.45 + 0.55, c[2] * 0.45 + 0.55, 1.0];
            self.gfx.text_shadow(&name, sx - 80.0, ty, 160.0, 28.0, f, col);
        }
    }

    fn act_cmd_icon(&self, act: Act) -> Option<CmdIcon> {
        let era = self.world.era;
        match act {
            Act::Train(UnitKind::Worker) | Act::SelectAllWorkers | Act::SelectKind(UnitKind::Worker) | Act::FilterKind(UnitKind::Worker) => Some(CmdIcon::Worker),
            Act::Train(UnitKind::Infantry) | Act::SelectKind(UnitKind::Infantry) | Act::FilterKind(UnitKind::Infantry) => Some(CmdIcon::Infantry),
            Act::Train(UnitKind::Archer) | Act::SelectKind(UnitKind::Archer) | Act::FilterKind(UnitKind::Archer) => Some(CmdIcon::Archer),
            Act::Train(UnitKind::Chariot) | Act::SelectKind(UnitKind::Chariot) | Act::FilterKind(UnitKind::Chariot) => {
                Some(if era == Era::Spring { CmdIcon::Chariot } else { CmdIcon::Cavalry })
            }
            Act::SelectAllMilitary => Some(CmdIcon::AllArmy),
            Act::WorkersAuto | Act::AllWorkersAuto => Some(CmdIcon::GatherAuto),
            Act::AllWorkersWood => Some(CmdIcon::GatherWood),
            Act::AllWorkersMine => Some(CmdIcon::GatherBronze),
            Act::Place(BKind::Farm) => Some(CmdIcon::Farm),
            Act::Place(BKind::Barracks) => Some(CmdIcon::Barracks),
            Act::Place(BKind::Town) => Some(CmdIcon::Town),
            Act::Place(BKind::Capital) => Some(CmdIcon::Capital),
            Act::Upgrade | Act::AllWorkersUpgrade | Act::AllArmyUpgrade => Some(CmdIcon::Upgrade),
            Act::Convert => Some(CmdIcon::Convert),
            Act::Stop => Some(CmdIcon::Stop),
            Act::AttackMove | Act::AllArmyAttack => Some(CmdIcon::AttackMove),
            Act::ArmyDefend | Act::AllArmyDefend => Some(CmdIcon::Defend),
            Act::Demolish => Some(CmdIcon::Demolish),
            Act::Diplo => Some(CmdIcon::Diplo),
            Act::Help => Some(CmdIcon::Help),
            _ => None,
        }
    }

    fn add_button(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, label: &str, sub: &str, key: u32, enabled: bool, act: Act, tip: &str) {
        let (mx, my) = self.mouse;
        let hover = mx >= x0 && mx <= x1 && my >= y0 && my <= y1;
        let btn_bmp = if !enabled {
            self.art.btn_dis.as_ref()
        } else if hover {
            self.art.btn_hover.as_ref()
        } else {
            self.art.btn_norm.as_ref()
        };
        if let Some(bmp) = btn_bmp {
            self.gfx.draw_bitmap(bmp, x0, y0, x1, y1);
        } else {
            let bg = if !enabled {
                [0.18, 0.17, 0.15, 0.9]
            } else if hover {
                [0.45, 0.36, 0.20, 0.95]
            } else {
                [0.30, 0.24, 0.15, 0.95]
            };
            self.gfx.fill_round(x0, y0, x1, y1, 4.0, bg);
            self.gfx.stroke_round(x0, y0, x1, y1, 4.0, 1.0, [0.75, 0.62, 0.35, if enabled { 0.9 } else { 0.4 }]);
        }

        let bh = y1 - y0;
        let mut tx0 = x0;
        if let Some(ci) = self.act_cmd_icon(act) {
            if let Some(ibmp) = &self.art.cmd_icons[ci as usize] {
                if bh >= 36.0 {
                    let isz = (bh - 10.0).min(34.0);
                    let iy = y0 + (bh - isz) * 0.5;
                    self.gfx.draw_bitmap(ibmp, x0 + 3.0, iy, x0 + 3.0 + isz, iy + isz);
                    if !enabled {
                        self.gfx.fill_rect(x0 + 3.0, iy, x0 + 3.0 + isz, iy + isz, [0.05, 0.05, 0.05, 0.55]);
                    }
                    tx0 = x0 + isz + 4.0;
                } else if x1 - x0 >= 68.0 {
                    let isz = (bh - 6.0).max(14.0);
                    self.gfx.draw_bitmap(ibmp, x0 + 4.0, y0 + 3.0, x0 + 4.0 + isz, y0 + 3.0 + isz);
                    tx0 = x0 + isz + 4.0;
                }
            }
        }

        let tc = if enabled { [1.0, 0.95, 0.85, 1.0] } else { [0.55, 0.52, 0.48, 1.0] };
        let tw = (x1 - tx0).max(10.0);
        if sub.is_empty() {
            self.gfx.text_shadow(label, tx0, y0, tw, bh, Font::NormalCenter, tc);
        } else {
            self.gfx.text_shadow(label, tx0 + 2.0, y0 + 2.0, tw + 24.0, bh * 0.52, Font::Normal, tc);
            self.gfx.text(sub, tx0 + 2.0, y0 + bh * 0.53, tw + 24.0, bh * 0.45, Font::Small, [tc[0] * 0.92, tc[1] * 0.86, tc[2] * 0.68, 1.0]);
        }
        if key >= 0x41 && key <= 0x5A {
            let ks = ((key as u8) as char).to_string();
            self.gfx.fill_round(x0 + 2.0, y0 + 2.0, x0 + 15.0, y0 + 15.0, 2.0, [0.08, 0.06, 0.04, 0.84]);
            self.gfx.text(&ks, x0 + 4.0, y0 + 1.0, 11.0, 14.0, Font::Small, [1.0, 0.88, 0.45, 0.98]);
        }
        if hover && !tip.is_empty() {
            self.notice = None;
        }
        self.buttons.push(Button { x0, y0, x1, y1, label: label.to_string(), sub: sub.to_string(), key, enabled, act, tip: tip.to_string() });
    }

    fn cost_str(c: [f32; 3]) -> String {
        let cnt = c.iter().filter(|&&v| v > 0.0).count();
        let sep = if cnt >= 3 { "" } else { " " };
        let mut s = String::new();
        for k in 0..3 {
            if c[k] > 0.0 {
                if !s.is_empty() {
                    s.push_str(sep);
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
                self.gfx.fill_circle(sx, sy, 15.0, [1.0, 0.92, 0.55, 0.38]);
                self.gfx.stroke_circle(sx, sy, 15.0, 2.0, [1.0, 0.88, 0.40, 0.9]);
            }
            if seven {
                self.gfx.fill_circle(sx, sy, 4.5, [1.0, 0.85, 0.3, 1.0]);
            }
        }
        let px = w - 380.0;
        self.art.draw_stone_panel(&self.gfx, px, 0.0, w, h);
        self.art.draw_corner_ornaments(&self.gfx, px + 4.0, 4.0, w - 4.0, h - 4.0, 18.0);
        if let Some(crest) = &self.art.title_crest {
            self.gfx.draw_bitmap(crest, px + 20.0, 10.0, w - 20.0, 126.0);
        }
        self.gfx.text_shadow("春秋戦国", px, 19.0, 380.0, 56.0, Font::Title, [1.0, 0.86, 0.42, 1.0]);
        self.gfx.text_shadow("— 中華統一への道 —", px, 76.0, 380.0, 28.0, Font::BigCenter, [0.96, 0.90, 0.78, 1.0]);

        self.art.draw_inset_box(&self.gfx, px + 18.0, 132.0, w - 18.0, 306.0);
        self.gfx.text(
            "紀元前770年、周の東遷。数多の諸侯が並び立つ春秋の世から、七雄が争う戦国の世まで、時は全ての国に等しく流れる。\n\n地図上の国都をクリックして、率いる国を選べ。金の印は戦国の七雄（生産力が高い）。",
            px + 28.0,
            140.0,
            324.0,
            162.0,
            Font::Normal,
            [0.90, 0.86, 0.78, 1.0],
        );
        if let Some(i) = self.title_sel {
            let n = &self.world.nations[i];
            let d = &NATIONS[i];
            self.art.draw_inset_box(&self.gfx, px + 18.0, 316.0, w - 18.0, 522.0);
            self.art.draw_corner_ornaments(&self.gfx, px + 20.0, 318.0, w - 20.0, 520.0, 12.0);
            if let Some(cap_ic) = self.art.bld_icon(BKind::Capital) {
                self.gfx.draw_bitmap(cap_ic, px + 30.0, 328.0, px + 78.0, 376.0);
            }
            self.gfx.fill_rect(px + 86.0, 336.0, px + 104.0, 368.0, c4(n.color, 1.0));
            self.gfx.stroke_rect(px + 86.0, 336.0, px + 104.0, 368.0, 1.2, [0.92, 0.78, 0.40, 1.0]);
            self.gfx.text_shadow(n.name, px + 112.0, 332.0, 230.0, 40.0, Font::Big, [1.0, 0.95, 0.85, 1.0]);
            self.gfx.text(d.desc, px + 32.0, 382.0, 316.0, 44.0, Font::Normal, [0.88, 0.84, 0.76, 1.0]);
            let s = format!("生産力：×{:.2}{}", d.bias, if d.seven { "（戦国七雄）" } else { "" });
            self.gfx.text(&s, px + 32.0, 426.0, 316.0, 20.0, Font::Normal, [0.98, 0.86, 0.48, 1.0]);
            let cities = self.world.blds.iter().filter(|b| b.alive && b.nation as usize == i && b.kind.city()).count();
            let s = format!("初期城邑：{}　君主家：{}", cities, INITIAL_HOUSE[i]);
            self.gfx.text(&s, px + 32.0, 448.0, 316.0, 20.0, Font::Normal, [0.88, 0.84, 0.76, 1.0]);
            let diff = if d.seven {
                "難易度：易"
            } else if d.bias >= 0.95 {
                "難易度：中"
            } else {
                "難易度：難"
            };
            self.gfx.text(diff, px + 32.0, 470.0, 316.0, 20.0, Font::Normal, [0.88, 0.84, 0.76, 1.0]);
            self.add_button(px + 50.0, 530.0, w - 50.0, 576.0, "この国で覇業を開始", "", 0, true, Act::Start, "");
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
            self.art.draw_inset_box(&self.gfx, x0, ty - 104.0, w - 24.0, ty - 64.0);
            self.gfx.text(&t, x0 + 8.0, ty - 100.0, w - x0 - 40.0, 36.0, Font::Small, [0.95, 0.92, 0.85, 1.0]);
        }
    }

    fn render_hud(&mut self) {
        let (w, h) = (self.gfx.width as f32, self.gfx.height as f32);
        let pl = self.world.player;
        // ---- 上部バー（Age of Empires 風石彫パネル＋資源アイコン）
        self.art.draw_stone_panel(&self.gfx, 0.0, 0.0, w, TOP_H);
        let n = &self.world.nations[pl];
        let mut x = 8.0;
        let res = n.res;
        let inc = n.income;
        let upkeep = n.upkeep;
        let res_icons = [ResIcon::Food, ResIcon::Wood, ResIcon::Bronze];
        let (n_workers, w_cap, n_soldiers, m_cap) = (n.workers, n.worker_cap(), n.soldiers, n.mil_cap());
        for k in 0..3 {
            if let Some(ic) = &self.art.res_icons[res_icons[k] as usize] {
                self.gfx.draw_bitmap(ic, x, 4.0, x + 28.0, 32.0);
            }
            let rate = if k == 0 { inc[k] - upkeep } else { inc[k] };
            let s = format!("{}", res[k] as i32);
            self.gfx.text_shadow(&s, x + 31.0, 7.0, 72.0, 22.0, Font::Big, RES_COLORS[k]);
            let rs = format!("{:+.1}/秒", rate);
            let rc = if rate < 0.0 { [1.0, 0.42, 0.32, 1.0] } else { [0.80, 0.78, 0.70, 1.0] };
            self.gfx.text(&rs, x + 88.0, 11.0, 56.0, 18.0, Font::Small, rc);
            x += 148.0;
        }
        // 民全体・全軍をワンクリック選択できるトップバーアイコンボタン
        let (mx, my) = self.mouse;
        let w_hov = mx >= x && mx <= x + 88.0 && my >= 4.0 && my <= 32.0;
        self.gfx.fill_round(x, 4.0, x + 88.0, 32.0, 4.0, if w_hov { [0.35, 0.28, 0.16, 0.9] } else { [0.16, 0.13, 0.09, 0.75] });
        self.gfx.stroke_round(x, 4.0, x + 88.0, 32.0, 4.0, 1.0, [0.75, 0.62, 0.34, 0.85]);
        if let Some(ic) = &self.art.res_icons[ResIcon::Worker as usize] {
            self.gfx.draw_bitmap(ic, x + 3.0, 5.0, x + 29.0, 31.0);
        }
        let s = format!("{}/{}", n_workers, w_cap);
        self.gfx.text_shadow(&s, x + 32.0, 8.0, 54.0, 22.0, Font::Normal, [0.95, 0.92, 0.85, 1.0]);
        self.buttons.push(Button {
            x0: x,
            y0: 4.0,
            x1: x + 88.0,
            y1: 32.0,
            label: String::new(),
            sub: String::new(),
            key: 0,
            enabled: true,
            act: Act::SelectAllWorkers,
            tip: "民（生産ユニット）全体を選択する".to_string(),
        });
        x += 94.0;

        let s_hov = mx >= x && mx <= x + 88.0 && my >= 4.0 && my <= 32.0;
        self.gfx.fill_round(x, 4.0, x + 88.0, 32.0, 4.0, if s_hov { [0.35, 0.28, 0.16, 0.9] } else { [0.16, 0.13, 0.09, 0.75] });
        self.gfx.stroke_round(x, 4.0, x + 88.0, 32.0, 4.0, 1.0, [0.75, 0.62, 0.34, 0.85]);
        if let Some(ic) = &self.art.res_icons[ResIcon::Soldier as usize] {
            self.gfx.draw_bitmap(ic, x + 3.0, 5.0, x + 29.0, 31.0);
        }
        let s = format!("{}/{}", n_soldiers, m_cap);
        self.gfx.text_shadow(&s, x + 32.0, 8.0, 54.0, 22.0, Font::Normal, [0.95, 0.92, 0.85, 1.0]);
        self.buttons.push(Button {
            x0: x,
            y0: 4.0,
            x1: x + 88.0,
            y1: 32.0,
            label: String::new(),
            sub: String::new(),
            key: 0,
            enabled: true,
            act: Act::SelectAllMilitary,
            tip: "軍事ユニット全体（全軍）を選択する".to_string(),
        });
        x += 96.0;

        if let Some(ic) = &self.art.res_icons[ResIcon::Upkeep as usize] {
            self.gfx.draw_bitmap(ic, x, 5.0, x + 26.0, 31.0);
        }
        let s = format!("-{:.1}/秒", upkeep);
        self.gfx.text_shadow(&s, x + 28.0, 8.0, 88.0, 22.0, Font::Normal, [0.98, 0.68, 0.52, 1.0]);

        let era = if self.world.era == Era::Spring { "春秋時代" } else { "戦国時代" };
        let s = format!("紀元前 {}年　{}", self.world.year_bc(), era);
        // 速度・メニュー
        let bx = w - 344.0;
        let ex = (bx - 248.0).max(x + 118.0);
        if let Some(ic) = &self.art.res_icons[ResIcon::Era as usize] {
            self.gfx.draw_bitmap(ic, ex - 26.0, 6.0, ex - 2.0, 30.0);
        }
        self.gfx.text_shadow(&s, ex, 7.0, 240.0, 24.0, Font::Big, [1.0, 0.9, 0.6, 1.0]);
        let paused = self.paused;
        let sp = self.speed;
        self.add_button(bx, 5.0, bx + 36.0, 31.0, if paused { "▶" } else { "❚❚" }, "", 0, true, Act::Speed(0), "");
        for (k, s) in [1u32, 2, 4, 8].iter().enumerate() {
            let x0 = bx + 39.0 + k as f32 * 35.0;
            let lbl = format!("×{}", s);
            self.add_button(x0, 5.0, x0 + 33.0, 31.0, &lbl, "", 0, true, Act::Speed(*s), "");
            if sp == *s && !paused {
                self.gfx.stroke_round(x0, 5.0, x0 + 33.0, 31.0, 3.0, 2.0, [1.0, 0.9, 0.4, 1.0]);
            }
        }
        self.add_button(w - 162.0, 5.0, w - 84.0, 31.0, "外交 F2", "", 0, true, Act::Diplo, "");
        self.add_button(w - 80.0, 5.0, w - 4.0, 31.0, "ヘルプ F1", "", 0, true, Act::Help, "");
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
            self.gfx.fill_round(w - 472.0, cy, w - 6.0, cy + 23.0, 3.0, [0.38, 0.10, 0.08, 0.90]);
            self.gfx.stroke_round(w - 472.0, cy, w - 6.0, cy + 23.0, 3.0, 1.2, [0.92, 0.72, 0.34, 0.9]);
            self.gfx.text_shadow(&s, w - 464.0, cy + 2.0, 460.0, 20.0, Font::Normal, [1.0, 0.88, 0.52, 1.0]);
            cy += 25.0;
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
            let bw = 760.0f32.min(w - 40.0);
            let x0 = (w - bw) / 2.0;
            let y0 = TOP_H + 90.0;
            let th = self.gfx.measure_h(&text, Font::NormalCenter, bw - 40.0).max(24.0);
            let bh = 66.0 + th + 14.0;
            self.art.draw_stone_panel(&self.gfx, x0, y0, x0 + bw, y0 + bh);
            self.art.draw_corner_ornaments(&self.gfx, x0 + 3.0, y0 + 3.0, x0 + bw - 3.0, y0 + bh - 3.0, 14.0);
            self.gfx.text_shadow(&title, x0, y0 + 8.0, bw, 44.0, Font::Title, [1.0, 0.85, 0.4, a]);
            self.gfx.text_shadow(&text, x0 + 20.0, y0 + 58.0, bw - 40.0, th + 6.0, Font::NormalCenter, [1.0, 0.95, 0.85, a]);
        }

        // ---- 下部パネル（Age of Empires 風石彫レリーフ＆3区画フレーム）
        let py = h - BOT_H;
        self.art.draw_stone_panel(&self.gfx, 0.0, py, w, h);

        // ミニマップ枠
        let (mx0, my0, mx1, my1) = self.mm_rect();
        self.gfx.fill_rect(mx0 - 2.0, my0 - 2.0, mx1 + 2.0, my1 + 2.0, [0.05, 0.04, 0.03, 1.0]);
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
            1.2,
            [1.0, 0.96, 0.80, 0.92],
        );
        self.gfx.stroke_rect(mx0, my0, mx1, my1, 2.0, [0.78, 0.64, 0.34, 1.0]);
        self.art.draw_corner_ornaments(&self.gfx, mx0, my0, mx1, my1, 10.0);

        // 中央の情報欄ボックス
        let ix = MM_W + 12.0;
        let iw = w - ix - 394.0;
        self.art.draw_inset_box(&self.gfx, ix, py + 6.0, ix + iw, h - 4.0);
        self.art.draw_corner_ornaments(&self.gfx, ix + 2.0, py + 8.0, ix + iw - 2.0, h - 6.0, 11.0);
        self.render_info(ix + 12.0, py + 12.0, iw - 24.0);

        // 右側のコマンド欄ボックス
        let cx0 = w - 388.0;
        self.art.draw_inset_box(&self.gfx, cx0, py + 6.0, w - 4.0, h - 4.0);
        self.art.draw_corner_ornaments(&self.gfx, cx0 + 2.0, py + 8.0, w - 6.0, h - 6.0, 11.0);
        if !self.show_diplo && !self.show_help && !self.show_chron {
            self.render_commands(w - 382.0, py + 12.0);
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
            let tw = 310.0;
            let x0 = (mx - tw).max(4.0);
            self.art.draw_inset_box(&self.gfx, x0, my - 72.0, x0 + tw, my - 8.0);
            self.gfx.text(&t, x0 + 8.0, my - 68.0, tw - 16.0, 56.0, Font::Small, [0.95, 0.92, 0.85, 1.0]);
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
        let tc = [0.95, 0.92, 0.85, 1.0];
        if self.sel.len() == 1 {
            let u = self.world.unit(self.sel[0]).unwrap();
            let n = &self.world.nations[u.nation as usize];
            let nc = n.color;
            // Age of Empires 風の大型全身ユニット肖像カード＋体力バー
            let px0 = x + 2.0;
            let py0 = y + 2.0;
            let psz = 88.0;
            self.art.draw_inset_box(&self.gfx, px0 - 2.0, py0 - 2.0, px0 + psz + 2.0, py0 + psz + 2.0);
            self.gfx.fill_circle(px0 + psz * 0.5, py0 + psz * 0.74, psz * 0.34, c4(nc, 0.42));
            self.gfx.stroke_circle(px0 + psz * 0.5, py0 + psz * 0.74, psz * 0.34, 1.5, c4(nc, 0.9));
            if let Some(bmp) = self.art.unit_sprite(u.kind, era, false) {
                self.gfx.draw_bitmap(bmp, px0 + 2.0, py0 + 2.0, px0 + psz - 2.0, py0 + psz - 2.0);
            }
            // 肖像下のHPバー
            let f = (u.hp / u.max_hp()).clamp(0.0, 1.0);
            self.gfx.fill_rect(px0, py0 + psz + 4.0, px0 + psz, py0 + psz + 11.0, [0.08, 0.06, 0.05, 0.95]);
            self.gfx.fill_rect(px0 + 1.0, py0 + psz + 5.0, px0 + 1.0 + (psz - 2.0) * f, py0 + psz + 10.0, [1.0 - f, 0.25 + 0.7 * f, 0.15, 1.0]);
            self.gfx.stroke_rect(px0, py0 + psz + 4.0, px0 + psz, py0 + psz + 11.0, 1.0, [0.75, 0.62, 0.34, 0.9]);

            let tx = x + 102.0;
            let tw = (wd - 102.0).max(100.0);
            let title = format!("{}　【{}】", u.kind.name(era), n.name);
            let lines = format!(
                "兵力 {:.0}/{:.0}　　能力 {:.1}（時代上限 {:.1}）\n状態：{}{}",
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
            self.gfx.text_shadow(&title, tx, y + 2.0, tw, 28.0, Font::Big, tc);
            self.gfx.text(&lines, tx, y + 34.0, tw, 52.0, Font::Normal, tc);
            self.gfx.text(desc, x + 2.0, y + 106.0, wd, 38.0, Font::Small, [0.80, 0.76, 0.68, 1.0]);
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
            let title = format!("{} 部隊を編制選択中", self.sel.len());
            self.gfx.text_shadow(&title, x, y, wd, 26.0, Font::Big, tc);
            let kinds = [UnitKind::Worker, UnitKind::Infantry, UnitKind::Archer, UnitKind::Chariot];
            let (mx, my) = self.mouse;
            let mut ix = x;
            for k in 0..4 {
                if cnt[k] > 0 {
                    let bx0 = ix;
                    let by0 = y + 28.0;
                    let bx1 = ix + 86.0;
                    let by1 = y + 72.0;
                    let hov = mx >= bx0 && mx <= bx1 && my >= by0 && my <= by1;
                    self.gfx.fill_round(bx0, by0, bx1, by1, 4.0, if hov { [0.35, 0.28, 0.16, 0.95] } else { [0.16, 0.12, 0.08, 0.9] });
                    self.gfx.stroke_round(bx0, by0, bx1, by1, 4.0, 1.2, [0.78, 0.64, 0.34, 0.9]);
                    if let Some(bmp) = self.art.unit_icon(kinds[k], era) {
                        self.gfx.draw_bitmap(bmp, bx0 + 3.0, by0 + 3.0, bx0 + 41.0, by1 - 3.0);
                    }
                    let ks = format!("×{}", cnt[k]);
                    self.gfx.text_shadow(&ks, bx0 + 44.0, by0 + 10.0, 40.0, 24.0, Font::Big, tc);
                    self.buttons.push(Button {
                        x0: bx0,
                        y0: by0,
                        x1: bx1,
                        y1: by1,
                        label: String::new(),
                        sub: String::new(),
                        key: 0,
                        enabled: true,
                        act: Act::FilterKind(kinds[k]),
                        tip: format!("{}（{}体）：クリックでこの兵種のみに絞り込む", kinds[k].name(era), cnt[k]),
                    });
                    ix += 94.0;
                }
            }
            let s = format!("総能力 {:.1}（平均 {:.2}）　　総兵力 {:.0}/{:.0}", pw, pw / self.sel.len() as f32, hp, mhp);
            self.gfx.text(&s, x, y + 76.0, wd, 30.0, Font::Normal, tc);
            let f = (hp / mhp.max(1.0)).clamp(0.0, 1.0);
            let bw = wd.min(320.0);
            self.gfx.fill_rect(x, y + 106.0, x + bw, y + 114.0, [0.08, 0.06, 0.05, 0.95]);
            self.gfx.fill_rect(x + 1.0, y + 107.0, x + 1.0 + (bw - 2.0) * f, y + 113.0, [1.0 - f, 0.25 + 0.7 * f, 0.15, 1.0]);
            self.gfx.stroke_rect(x, y + 106.0, x + bw, y + 114.0, 1.0, [0.75, 0.62, 0.34, 0.9]);
        } else if let Some(bh) = self.sel_b {
            let b = self.world.bld(bh).unwrap();
            let n = &self.world.nations[b.nation as usize];
            let nc = n.color;
            let px0 = x + 2.0;
            let py0 = y + 4.0;
            let psz = 68.0;
            self.gfx.fill_rect(px0 - 2.0, py0 - 2.0, px0 + psz + 2.0, py0 + psz + 2.0, c4(nc, 0.9));
            if let Some(bmp) = self.art.bld_icon(b.kind) {
                self.gfx.draw_bitmap(bmp, px0, py0, px0 + psz, py0 + psz);
            }
            let f = (b.hp / b.kind.max_hp()).clamp(0.0, 1.0);
            self.gfx.fill_rect(px0, py0 + psz + 4.0, px0 + psz, py0 + psz + 11.0, [0.08, 0.06, 0.05, 0.95]);
            self.gfx.fill_rect(px0 + 1.0, py0 + psz + 5.0, px0 + 1.0 + (psz - 2.0) * f, py0 + psz + 10.0, [1.0 - f, 0.25 + 0.7 * f, 0.15, 1.0]);
            self.gfx.stroke_rect(px0, py0 + psz + 4.0, px0 + psz, py0 + psz + 11.0, 1.0, [0.75, 0.62, 0.34, 0.9]);

            let tx = x + 84.0;
            let tw = (wd - 84.0).max(100.0);
            let title = format!("{}　【{}】", b.kind.name(), n.name);
            let mut s = format!("耐久 {:.0}/{:.0}", b.hp.max(0.0), b.kind.max_hp());
            if !b.done() {
                s.push_str(&format!("　（建設中 {:.0}%）", b.progress * 100.0));
            }
            let desc = match b.kind {
                BKind::Capital => "国の中心。民を訓練し、資源の集積地となる。失うと遷都し、全ての城邑を失うと滅亡する。",
                BKind::Town => "領土を広げ、民を訓練し、資源の集積地となる。城邑が増えるほど人口上限が上がる。",
                BKind::Barracks => "戈兵・弩兵・戦車（騎兵）を訓練する。",
                BKind::Farm => "民が耕すと糧を産む。枯れることはない。",
            };
            let queue: Vec<(UnitKind, f32)> = b.queue.iter().enumerate().map(|(i, k)| (*k, if i == 0 { b.queue_t / k.train_time() } else { 0.0 })).collect();
            self.gfx.text_shadow(&title, tx, y, tw, 28.0, Font::Big, tc);
            self.gfx.text(&s, tx, y + 30.0, tw, 24.0, Font::Normal, tc);
            self.gfx.text(desc, tx, y + 56.0, tw, 40.0, Font::Small, [0.80, 0.76, 0.68, 1.0]);
            for (i, (k, f)) in queue.iter().enumerate() {
                let qx = tx + i as f32 * 44.0;
                let qy = y + 98.0;
                self.gfx.fill_round(qx, qy, qx + 40.0, qy + 40.0, 3.0, [0.18, 0.14, 0.10, 1.0]);
                if let Some(ubmp) = self.art.unit_icon(*k, era) {
                    self.gfx.draw_bitmap(ubmp, qx + 2.0, qy + 2.0, qx + 38.0, qy + 38.0);
                }
                if *f > 0.0 {
                    self.gfx.fill_rect(qx + 2.0, qy + 34.0, qx + 2.0 + 36.0 * f, qy + 38.0, [0.35, 0.9, 0.4, 1.0]);
                }
            }
        } else {
            let n = &self.world.nations[pl];
            let nc = n.color;
            let px0 = x + 2.0;
            let py0 = y + 4.0;
            let psz = 68.0;
            self.gfx.fill_rect(px0 - 2.0, py0 - 2.0, px0 + psz + 2.0, py0 + psz + 2.0, c4(nc, 0.9));
            if let Some(bmp) = self.art.bld_icon(BKind::Capital) {
                self.gfx.draw_bitmap(bmp, px0, py0, px0 + psz, py0 + psz);
            }
            let tx = x + 84.0;
            let tw = (wd - 84.0).max(100.0);
            let title = format!("{} の国勢（右のボタンで民全体・全軍へ命令可）", n.name);
            let wars: Vec<&str> = (0..self.world.nations.len()).filter(|&o| self.world.at_war(pl, o)).map(|o| self.world.nations[o].name).collect();
            let s = format!(
                "城邑 {}　領土 {}　国力 {:.0}　生産 ×{:.2}　時代 ×{:.2}　上限 {:.1}\n交戦中：{}",
                n.cities,
                n.tiles,
                n.strength,
                n.bias,
                self.world.era_mult(),
                cap,
                if wars.is_empty() { "なし".to_string() } else { wars.join("・") }
            );
            self.gfx.text_shadow(&title, tx, y, tw, 26.0, Font::Big, tc);
            self.gfx.text(&s, tx, y + 28.0, tw, 48.0, Font::Normal, tc);

            // 自国の各兵種アイコンボタン（クリックでその兵種全体を選択）
            let mut cnt = [0usize; 4];
            for u in self.world.units.iter() {
                if u.alive && u.nation as usize == pl {
                    cnt[u.kind as usize] += 1;
                }
            }
            let kinds = [UnitKind::Worker, UnitKind::Infantry, UnitKind::Archer, UnitKind::Chariot];
            let (mx, my) = self.mouse;
            let mut ix = tx;
            for k in 0..4 {
                let bx0 = ix;
                let by0 = y + 82.0;
                let bx1 = ix + 78.0;
                let by1 = y + 122.0;
                let hov = mx >= bx0 && mx <= bx1 && my >= by0 && my <= by1;
                self.gfx.fill_round(bx0, by0, bx1, by1, 4.0, if hov { [0.35, 0.28, 0.16, 0.95] } else { [0.16, 0.12, 0.08, 0.9] });
                self.gfx.stroke_round(bx0, by0, bx1, by1, 4.0, 1.1, [0.78, 0.64, 0.34, 0.85]);
                if let Some(bmp) = self.art.unit_icon(kinds[k], era) {
                    self.gfx.draw_bitmap(bmp, bx0 + 3.0, by0 + 3.0, bx0 + 37.0, by1 - 3.0);
                }
                let ks = format!("×{}", cnt[k]);
                self.gfx.text_shadow(&ks, bx0 + 40.0, by0 + 8.0, 36.0, 22.0, Font::Normal, tc);
                self.buttons.push(Button {
                    x0: bx0,
                    y0: by0,
                    x1: bx1,
                    y1: by1,
                    label: String::new(),
                    sub: String::new(),
                    key: 0,
                    enabled: cnt[k] > 0,
                    act: Act::SelectKind(kinds[k]),
                    tip: format!("{}全体（{}体）を選択する", kinds[k].name(era), cnt[k]),
                });
                ix += 84.0;
            }
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
        let (n_workers, n_soldiers) = (self.world.nations[pl].workers, self.world.nations[pl].soldiers);
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
            let u0_kind = u0.kind;
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
            self.add_button(x0, y0, x1, y1, "停止", "", 'S' as u32, true, Act::Stop, "選択中の部隊を停止して待機させる。");
            i += 1;

            if any_worker && !any_mil {
                // 下段：選択民および民全体への一括命令
                let (x0, y0, x1, y1) = pos(6);
                self.add_button(x0, y0, x1, y1, "自動採集", "田・森・銅へ", 'F' as u32, true, Act::WorkersAuto, "選択中の民を、空いている田・森・銅山へバランスよく自動配分する。");
                let all_sel = own.len() >= n_workers as usize;
                let (x0, y0, x1, y1) = pos(7);
                if all_sel {
                    self.add_button(x0, y0, x1, y1, "全民・伐採", "最寄りの森へ", 'G' as u32, true, Act::AllWorkersWood, "すべての民（建設中を除く）に最寄りの森の伐採を命じる。");
                } else {
                    self.add_button(x0, y0, x1, y1, "全民を選択", &format!("全{}体", n_workers), 'G' as u32, n_workers > 0, Act::SelectAllWorkers, "自国のすべての民を選択する。");
                }
                let (x0, y0, x1, y1) = pos(8);
                if all_sel {
                    self.add_button(x0, y0, x1, y1, "全民・採掘", "最寄りの銅山", 'V' as u32, true, Act::AllWorkersMine, "すべての民（建設中を除く）に最寄りの銅山の採掘を命じる。");
                } else {
                    self.add_button(x0, y0, x1, y1, "全民・自動", "民全体を配分", 'V' as u32, n_workers > 0, Act::AllWorkersAuto, "自国のすべての民（建設中を除く）を田・森・銅山へ自動でバランス配分する。");
                }
            } else if any_mil && !any_worker {
                let (x0, y0, x1, y1) = pos(i);
                self.add_button(x0, y0, x1, y1, "進軍", "攻撃移動", 'A' as u32, true, Act::AttackMove, "クリックした地点へ、途中の敵と戦いながら進む。");
                i += 1;
                let (x0, y0, x1, y1) = pos(i);
                self.add_button(x0, y0, x1, y1, "防衛・帰還", "領内迎撃・集結", 'D' as u32, true, Act::ArmyDefend, "領内の侵入者を迎え撃ち、平時は最寄りの自国城邑へ帰還・防衛配置する。");
                i += 1;
                let (x0, y0, x1, y1) = pos(i);
                self.add_button(x0, y0, x1, y1, "同兵種選択", u0_kind.name(era), 'F' as u32, true, Act::SelectKind(u0_kind), "自国の同じ兵種のユニットをすべて選択する。");
                // 下段：全軍（軍事ユニット全体）への命令
                let (x0, y0, x1, y1) = pos(6);
                self.add_button(x0, y0, x1, y1, "全軍を選択", &format!("全{}体", n_soldiers), 'G' as u32, n_soldiers > 0, Act::SelectAllMilitary, "自国のすべての軍事ユニットを選択する。");
                let (x0, y0, x1, y1) = pos(7);
                self.add_button(x0, y0, x1, y1, "全軍・進撃", "全軍で敵陣へ", 'Q' as u32, n_soldiers > 0, Act::AllArmyAttack, "全軍事ユニットを選択し、交戦国の城邑・敵部隊へ一斉進撃（または地図クリックで進軍先指定）する。");
                let (x0, y0, x1, y1) = pos(8);
                self.add_button(x0, y0, x1, y1, "全軍・防衛", "全軍で防衛", 'W' as u32, n_soldiers > 0, Act::AllArmyDefend, "全軍事ユニットに領内侵入者の迎撃および城邑への防衛帰還を命じる。");
            } else {
                let (x0, y0, x1, y1) = pos(i);
                self.add_button(x0, y0, x1, y1, "進軍", "攻撃移動", 'A' as u32, true, Act::AttackMove, "クリックした地点へ、途中の敵と戦いながら進む。");
                let (x0, y0, x1, y1) = pos(7);
                self.add_button(x0, y0, x1, y1, "全民を選択", &format!("全{}体", n_workers), 'G' as u32, n_workers > 0, Act::SelectAllWorkers, "自国のすべての民を選択する。");
                let (x0, y0, x1, y1) = pos(8);
                self.add_button(x0, y0, x1, y1, "全軍を選択", &format!("全{}体", n_soldiers), 'V' as u32, n_soldiers > 0, Act::SelectAllMilitary, "自国のすべての軍事ユニットを選択する。");
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
        } else {
            // 未選択時：民全体・軍事ユニット全体に対する全体命令パネル
            let (x0, y0, x1, y1) = pos(0);
            self.add_button(x0, y0, x1, y1, "全民・自動", "田・森・銅へ", 'Q' as u32, n_workers > 0, Act::AllWorkersAuto, "自国のすべての民（建設中を除く）を、空いている田・近くの森・銅山へバランスよく自動配分する。");
            let (x0, y0, x1, y1) = pos(1);
            self.add_button(x0, y0, x1, y1, "全民・伐採", "最寄りの森へ", 'W' as u32, n_workers > 0, Act::AllWorkersWood, "自国のすべての民（建設中を除く）に、最寄りの森の伐採を命じる。");
            let (x0, y0, x1, y1) = pos(2);
            self.add_button(x0, y0, x1, y1, "全民・採掘", "最寄りの銅山", 'E' as u32, n_workers > 0, Act::AllWorkersMine, "自国のすべての民（建設中を除く）に、最寄りの銅山の採掘を命じる。");

            let (x0, y0, x1, y1) = pos(3);
            self.add_button(x0, y0, x1, y1, "全民・教化", "能力+0.5", 'T' as u32, n_workers > 0, Act::AllWorkersUpgrade, "自国領内の民を、能力の低い順に資源が続く限り一斉に教化する。");
            let (x0, y0, x1, y1) = pos(4);
            self.add_button(x0, y0, x1, y1, "全民を選択", &format!("全{}体", n_workers), 'G' as u32, n_workers > 0, Act::SelectAllWorkers, "自国のすべての民を選択する（建設や一斉移動を行えます）。");
            let (x0, y0, x1, y1) = pos(5);
            self.add_button(x0, y0, x1, y1, "全軍を選択", &format!("全{}体", n_soldiers), 'V' as u32, n_soldiers > 0, Act::SelectAllMilitary, "自国のすべての軍事ユニットを選択する。");

            let (x0, y0, x1, y1) = pos(6);
            self.add_button(x0, y0, x1, y1, "全軍・進撃", "敵城・目標へ", 'A' as u32, n_soldiers > 0, Act::AllArmyAttack, "全軍事ユニットを選択し、交戦国の最寄り城邑・敵部隊へ総進撃（または地図クリックで進軍先指定）する。");
            let (x0, y0, x1, y1) = pos(7);
            self.add_button(x0, y0, x1, y1, "全軍・防衛", "領内迎撃・帰還", 'D' as u32, n_soldiers > 0, Act::AllArmyDefend, "全軍事ユニットに、領内の侵入者の迎撃または自国城邑への防衛帰還を命じる。");
            let (x0, y0, x1, y1) = pos(8);
            self.add_button(x0, y0, x1, y1, "全軍・訓練", "能力+0.5", 'R' as u32, n_soldiers > 0, Act::AllArmyUpgrade, "自国領内の軍事ユニットを、能力の低い順に資源が続く限り一斉に訓練する。");
        }
    }

    fn render_diplo(&mut self) {
        let (w, h) = (self.gfx.width as f32, self.gfx.height as f32);
        let pl = self.world.player;
        let pw = 960.0f32.min(w - 20.0);
        let ph = 620.0f32.min(h - 60.0);
        let x0 = (w - pw) / 2.0;
        let y0 = (h - ph) / 2.0;
        self.gfx.fill_rect(0.0, 0.0, w, h, [0.0, 0.0, 0.0, 0.45]);
        self.art.draw_stone_panel(&self.gfx, x0, y0, x0 + pw, y0 + ph);
        self.art.draw_corner_ornaments(&self.gfx, x0 + 3.0, y0 + 3.0, x0 + pw - 3.0, y0 + ph - 3.0, 16.0);
        if let Some(ic) = &self.art.cmd_icons[CmdIcon::Diplo as usize] {
            self.gfx.draw_bitmap(ic, x0 + 16.0, y0 + 8.0, x0 + 48.0, y0 + 40.0);
        }
        self.gfx.text_shadow("諸侯外交", x0 + 56.0, y0 + 10.0, 200.0, 34.0, Font::Big, [1.0, 0.85, 0.5, 1.0]);
        let my = self.world.nations[pl].strength;
        let s = format!("自国の国力 {:.0}　（国力は兵の能力・城邑・民・領土から算出）", my);
        self.gfx.text(&s, x0 + 160.0, y0 + 16.0, pw - 370.0, 24.0, Font::Normal, [0.85, 0.82, 0.75, 1.0]);
        self.add_button(x0 + pw - 90.0, y0 + 10.0, x0 + pw - 14.0, y0 + 40.0, "閉じる", "", 0, true, Act::CloseOverlay, "");
        self.add_button(x0 + pw - 196.0, y0 + 10.0, x0 + pw - 98.0, y0 + 40.0, "史記 F3", "", 0, true, Act::Chron, "英雄・諸子百家・政治指導者、反乱・簒奪の記録と各国の君主家");
        self.art.draw_inset_box(&self.gfx, x0 + 10.0, y0 + 48.0, x0 + pw - 10.0, y0 + ph - 10.0);
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
        self.gfx.fill_rect(0.0, 0.0, w, h, [0.0, 0.0, 0.0, 0.45]);
        self.art.draw_stone_panel(&self.gfx, x0, y0, x0 + pw, y0 + ph);
        self.art.draw_corner_ornaments(&self.gfx, x0 + 3.0, y0 + 3.0, x0 + pw - 3.0, y0 + ph - 3.0, 16.0);
        if let Some(ic) = &self.art.cmd_icons[CmdIcon::Help as usize] {
            self.gfx.draw_bitmap(ic, x0 + 16.0, y0 + 8.0, x0 + 48.0, y0 + 40.0);
        }
        self.gfx.text_shadow("兵法・遊び方", x0 + 56.0, y0 + 10.0, 220.0, 34.0, Font::Big, [1.0, 0.85, 0.5, 1.0]);
        self.add_button(x0 + pw - 90.0, y0 + 10.0, x0 + pw - 14.0, y0 + 40.0, "閉じる", "", 0, true, Act::CloseOverlay, "");
        self.art.draw_inset_box(&self.gfx, x0 + 14.0, y0 + 46.0, x0 + pw - 14.0, y0 + ph - 14.0);
        let text = "\
■ 目的　紀元前221年まで生き残り、最も強い国となるか、天下を統一せよ。時間は全ての国で共通に流れ（1年＝4秒）、春秋時代（〜前453年）から戦国時代へ移る。どの国も同じ時代補正を受け、一部の国だけ文明が進むことはない。

■ 資源　糧（田を耕す）・木（森を伐採）・銅（銅山を採掘）。民が集めて都・邑へ運ぶ。城邑からも少しずつ税収がある。

■ ユニット　動かせる数には上限がある（城邑の数で増える）。その代わり「訓練／教化」で一体ごとの能力を高められる（時代ごとに上限あり）。軍事ユニットは存在するだけで糧を消費する（維持費）。糧が尽きると兵は弱っていく。兵を解散して民に、民を徴兵して戈兵にできるが、能力は2/3に目減りする。
　戈兵＞戦車（騎兵）＞弩兵＞戈兵 の相性がある。

■ 外交　F2で外交画面。宣戦しないと他国を攻撃できない。城邑を攻め落とすと自国のものになる。全ての城邑を失った国は滅亡する。

■ 覇者の呼びかけ（春秋）　宣戦や攻略を繰り返すと「諸侯の警戒」が高まり、100に達すると最強国が諸侯に呼びかけ、周辺国が15年間一斉に攻めてくる。
■ 合従軍（戦国）　突出して強い国が現れると、周辺国が合従して15年間一斉に攻撃する。
■ 三家分晋　戦国時代の到来とともに晋は韓・趙・魏に分かれる。
■ 史実の出来事　史実の年になると、英雄・諸子百家・政治指導者が現れて国を強くし（一定年数、生産や武が上がる）、反乱や簒奪で兵や蓄えを失い君主家（国姓）が変わる。その国が滅んでいれば起こらない。記録と各国の君主家・人物は「史記」（F3、外交画面から）で見られる。
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
        self.art.draw_stone_panel(&self.gfx, x0, y0, x0 + pw, y0 + 260.0);
        self.art.draw_corner_ornaments(&self.gfx, x0 + 3.0, y0 + 3.0, x0 + pw - 3.0, y0 + 257.0, 16.0);
        let title = if win { "勝利" } else { "敗北" };
        self.gfx.text_shadow(title, x0, y0 + 20.0, pw, 60.0, Font::Title, if win { [1.0, 0.85, 0.4, 1.0] } else { [0.9, 0.4, 0.35, 1.0] });
        self.gfx.text(&text, x0 + 20.0, y0 + 100.0, pw - 40.0, 60.0, Font::BigCenter, [0.95, 0.92, 0.85, 1.0]);
        self.add_button(x0 + pw / 2.0 - 100.0, y0 + 180.0, x0 + pw / 2.0 + 100.0, y0 + 228.0, "新しい天下へ", "", 0, true, Act::Restart, "");
    }
}
