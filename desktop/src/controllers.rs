//! Modos automáticos: emotes en bucle, patrulla, dibujar y conducir, y seguir al ordenador.
//! Son máquinas de estados que se avanzan con `tick` unas diez veces por segundo.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use crate::droid::Droid;
use crate::kind::{Animation, Kind};
use crate::translate;

fn secs(s: f32) -> Duration {
    Duration::from_secs_f32(s.max(0.0))
}

// ---------------------------------------------------------------- emotes automáticos

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum LoopMode {
    /// «Vamos a ver una película»: los emotes de Watch With Me.
    Wwm,
    /// «Haz monerías»: los emotes de Emociones.
    Emotions,
}

pub struct EmoteLoop {
    pub running: Option<LoopMode>,
    pub wwm_interval: f32,
    pub emotions_interval: f32,
    pub include_ion_blast: bool,
    pub last_played: String,
    next_at: Instant,
    last: Option<u16>,
}

impl EmoteLoop {
    pub fn new() -> Self {
        EmoteLoop {
            running: None,
            wwm_interval: 15.0,
            emotions_interval: 15.0,
            include_ion_blast: false,
            last_played: String::new(),
            next_at: Instant::now(),
            last: None,
        }
    }

    pub fn pool(&self, kind: Kind, mode: LoopMode) -> Vec<&'static Animation> {
        kind.animations()
            .iter()
            .filter(|a| match mode {
                LoopMode::Wwm => a.category == "WWM",
                LoopMode::Emotions => a.category == "Emote" && (self.include_ion_blast || !a.is_ion_blast()),
            })
            .collect()
    }

    pub fn start(&mut self, mode: LoopMode, now: Instant) {
        self.running = Some(mode);
        self.next_at = now;
    }

    /// Para el bucle. La animación en curso termina sola.
    pub fn stop(&mut self) {
        self.running = None;
    }

    pub fn tick(&mut self, d: &mut Droid, now: Instant) {
        let Some(mode) = self.running else { return };
        if !d.is_ready() {
            self.running = None;
            return;
        }
        // Si la animación anterior dura más que el intervalo, se espera a que acabe.
        if now < self.next_at || d.busy(now) {
            return;
        }
        let all = self.pool(d.kind, mode);
        let candidates: Vec<&Animation> =
            if all.len() > 1 { all.iter().copied().filter(|a| Some(a.index) != self.last).collect() } else { all };
        if candidates.is_empty() {
            return;
        }
        let a = candidates[fastrand::usize(..candidates.len())];
        d.play_animation(a, now);
        self.last = Some(a.index);
        self.last_played = translate::title(a.name);
        self.next_at = now + secs(if mode == LoopMode::Wwm { self.wwm_interval } else { self.emotions_interval });
    }
}

// ---------------------------------------------------------------- patrulla

enum PatrolPhase {
    Forward,
    Backing(Instant),
    Settling(Instant),
    Alarm(Instant),
}

pub struct Patrol {
    pub running: bool,
    pub speed: f32,
    pub entries: VecDeque<String>,
    phase: PatrolPhase,
    heading: i32,
    next_alarm: Instant,
    started: Instant,
}

impl Patrol {
    pub fn new() -> Self {
        let now = Instant::now();
        Patrol { running: false, speed: 70.0, entries: VecDeque::new(), phase: PatrolPhase::Forward, heading: 0, next_alarm: now, started: now }
    }

    fn log(&mut self, s: impl AsRef<str>) {
        self.entries.push_front(format!("[{:>6.1} s] {}", self.started.elapsed().as_secs_f32(), s.as_ref()));
        self.entries.truncate(200);
    }

    pub fn start(&mut self, d: &mut Droid, now: Instant) {
        self.running = true;
        self.started = now;
        self.heading = 0;
        self.phase = PatrolPhase::Forward;
        self.next_alarm = now + Duration::from_secs(fastrand::u64(20..35));
        d.configure_collisions(70);
        self.log(format!("Patrulla iniciada con {}", d.name));
    }

    pub fn stop(&mut self, d: &mut Droid, now: Instant) {
        if self.running {
            d.stop(self.heading, now);
            self.log("Patrulla detenida");
        }
        self.running = false;
    }

    pub fn tick(&mut self, d: &mut Droid, now: Instant) {
        if !self.running {
            return;
        }
        if !d.is_ready() {
            self.running = false;
            return;
        }
        if d.take_hit() && matches!(self.phase, PatrolPhase::Forward) {
            self.log("Colisión: retrocediendo");
            self.phase = PatrolPhase::Backing(now + Duration::from_millis(800));
        }
        let speed = self.speed as i32;
        match self.phase {
            PatrolPhase::Forward => {
                if now >= self.next_alarm {
                    d.stop(self.heading, now);
                    let pool: Vec<&Animation> = d.kind.animations().iter().filter(|a| a.category == "Patrol").collect();
                    let pool = if pool.is_empty() { d.kind.animations().iter().filter(|a| a.category == "Emote").collect() } else { pool };
                    if pool.is_empty() {
                        self.phase = PatrolPhase::Settling(now + Duration::from_millis(500));
                    } else {
                        let a = pool[fastrand::usize(..pool.len())];
                        d.play_animation(a, now);
                        self.log(format!("Alarma: {}", translate::title(a.name)));
                        self.phase = PatrolPhase::Alarm(now + secs(a.seconds + 0.5));
                    }
                    self.next_alarm = now + Duration::from_secs(fastrand::u64(20..40));
                } else {
                    d.drive(speed, self.heading, false, now);
                }
            }
            PatrolPhase::Backing(until) => {
                d.drive((self.speed * 0.6) as i32, self.heading, true, now);
                if now >= until {
                    let turn = fastrand::i32(100..=200) * if fastrand::bool() { 1 } else { -1 };
                    self.heading = (self.heading + turn).rem_euclid(360);
                    self.log(format!("Giro de {turn}°"));
                    self.phase = PatrolPhase::Settling(now + Duration::from_millis(800));
                }
            }
            PatrolPhase::Settling(until) => {
                d.drive(0, self.heading, false, now);
                if now >= until {
                    self.phase = PatrolPhase::Forward;
                }
            }
            PatrolPhase::Alarm(until) => {
                if now >= until {
                    self.phase = PatrolPhase::Forward;
                }
            }
        }
    }
}

// ---------------------------------------------------------------- dibujar y conducir

struct Segment {
    heading: i32,
    seconds: f32,
    speed: i32,
}

/// Dibujas un recorrido y el droide lo sigue. Sin sensores de posición se calcula por tiempo:
/// la distancia recorrida depende de la calibración de velocidad.
pub struct DrawDrive {
    pub points: Vec<(f32, f32)>,
    pub running: bool,
    pub progress: f32,
    pub speed: f32,
    pub cm_per_second: f32,
    pub cm_per_point: f32,
    segments: Vec<Segment>,
    started: Instant,
    total: f32,
}

impl DrawDrive {
    pub fn new() -> Self {
        DrawDrive {
            points: Vec::new(),
            running: false,
            progress: 0.0,
            speed: 80.0,
            cm_per_second: 30.0,
            cm_per_point: 0.4,
            segments: Vec::new(),
            started: Instant::now(),
            total: 0.0,
        }
    }

    pub fn clear(&mut self, d: &mut Droid, now: Instant) {
        self.abort(d, now);
        self.points.clear();
    }

    pub fn run(&mut self, d: &mut Droid, now: Instant) {
        self.abort(d, now);
        self.segments = self.build_segments();
        self.total = self.segments.iter().map(|s| s.seconds).sum();
        if self.total <= 0.0 {
            return;
        }
        d.configure_collisions(70);
        self.started = now;
        self.running = true;
    }

    pub fn abort(&mut self, d: &mut Droid, now: Instant) {
        if self.running {
            d.stop(0, now);
        }
        self.running = false;
        self.progress = 0.0;
    }

    pub fn tick(&mut self, d: &mut Droid, now: Instant) {
        if !self.running {
            return;
        }
        if !d.is_ready() || d.take_hit() {
            self.abort(d, now);
            return;
        }
        let t = now.duration_since(self.started).as_secs_f32();
        self.progress = (t / self.total).min(1.0);
        let mut acc = 0.0;
        for s in &self.segments {
            if t < acc + s.seconds {
                d.drive(s.speed, s.heading, false, now);
                return;
            }
            acc += s.seconds;
        }
        self.abort(d, now);
    }

    /// Simplifica el trazo y lo convierte en tramos (rumbo, duración).
    fn build_segments(&self) -> Vec<Segment> {
        let mut pts: Vec<(f32, f32)> = Vec::new();
        for &p in &self.points {
            if pts.last().is_none_or(|l| (l.0 - p.0).hypot(l.1 - p.1) >= 14.0) {
                pts.push(p);
            }
        }
        if pts.len() < 2 {
            return Vec::new();
        }
        let mut merged: Vec<(f32, f32)> = Vec::new(); // (rumbo, distancia en puntos)
        for w in pts.windows(2) {
            let (dx, dy) = (w[1].0 - w[0].0, w[1].1 - w[0].1);
            let mut deg = dx.atan2(-dy).to_degrees();
            if deg < 0.0 {
                deg += 360.0;
            }
            let dist = dx.hypot(dy);
            match merged.last_mut() {
                Some(last) if angle_diff(last.0, deg).abs() < 15.0 => last.1 += dist,
                _ => merged.push((deg, dist)),
            }
        }
        let mut out = Vec::new();
        let mut prev = merged[0].0;
        for (deg, dist) in merged {
            if angle_diff(prev, deg).abs() > 30.0 {
                out.push(Segment { heading: deg as i32, seconds: 0.4, speed: 0 });
            }
            out.push(Segment { heading: deg as i32, seconds: dist * self.cm_per_point / self.cm_per_second, speed: self.speed as i32 });
            prev = deg;
        }
        out
    }
}

fn angle_diff(a: f32, b: f32) -> f32 {
    let mut d = (b - a) % 360.0;
    if d > 180.0 {
        d -= 360.0;
    } else if d < -180.0 {
        d += 360.0;
    }
    d
}

// ---------------------------------------------------------------- seguir al ordenador

enum FollowPhase {
    Forward,
    Settling(Instant),
    Arrived,
}

/// Sigue al ordenador usando solo la intensidad de la señal Bluetooth (RSSI).
/// El RSSI no da dirección y fluctúa mucho, así que se hace «avanzar y rectificar»: el droide avanza,
/// corrige el rumbo con giros suaves si la señal empeora poco a poco, y solo da media vuelta si cae de golpe.
/// Es aproximado y funciona mejor en un espacio despejado.
pub struct Follow {
    pub running: bool,
    pub smoothed: Option<f32>,
    pub status: String,
    /// dBm; por encima de esto se considera «ya he llegado».
    pub near_threshold: f32,
    pub speed: f32,
    phase: FollowPhase,
    heading: i32,
    last_eval: Option<f32>,
    next_eval: Instant,
    turn_sign: i32,
    corrected: bool,
    last_rssi: Option<i16>,
}

const WINDOW: Duration = Duration::from_secs(3);
const MILD_DROP: f32 = -2.5;
const SHARP_DROP: f32 = -6.0;
const LOST_LEVEL: f32 = -85.0;

impl Follow {
    pub fn new() -> Self {
        Follow {
            running: false,
            smoothed: None,
            status: "Detenido".into(),
            near_threshold: -55.0,
            speed: 45.0,
            phase: FollowPhase::Forward,
            heading: 0,
            last_eval: None,
            next_eval: Instant::now(),
            turn_sign: 1,
            corrected: false,
            last_rssi: None,
        }
    }

    pub fn start(&mut self, d: &mut Droid, now: Instant) {
        self.running = true;
        self.phase = FollowPhase::Forward;
        self.heading = 0;
        self.smoothed = None;
        self.last_eval = None;
        self.last_rssi = d.rssi;
        self.next_eval = now + WINDOW;
        self.corrected = false;
        self.status = "Esperando señal…".into();
        d.configure_collisions(70);
    }

    pub fn stop(&mut self, d: &mut Droid, now: Instant) {
        if self.running {
            d.stop(self.heading, now);
        }
        self.running = false;
        self.status = "Detenido".into();
    }

    /// Usa la señal actual como «cerca» (coloca el droide junto al ordenador).
    pub fn calibrate_near(&mut self) {
        if let Some(s) = self.smoothed {
            self.near_threshold = (s - 2.0).round();
        }
    }

    pub fn tick(&mut self, d: &mut Droid, now: Instant) {
        if !self.running {
            return;
        }
        if !d.is_ready() {
            self.running = false;
            return;
        }
        // Incorpora la medida nueva, si la hay.
        if let Some(r) = d.rssi.filter(|r| Some(*r) != self.last_rssi || self.smoothed.is_none()) {
            self.last_rssi = Some(r);
            self.smoothed = Some(self.smoothed.map_or(r as f32, |s| 0.75 * s + 0.25 * r as f32));
        }
        if d.take_hit() {
            self.heading = (self.heading + fastrand::i32(120..=240)).rem_euclid(360);
            self.phase = FollowPhase::Settling(now + Duration::from_secs(1));
            self.status = "Obstáculo: cambio de rumbo".into();
        }
        let Some(s) = self.smoothed else { return };
        match self.phase {
            FollowPhase::Arrived => {
                d.stop(self.heading, now);
                self.status = format!("He llegado ({} dBm)", s as i32);
                if s < self.near_threshold - 6.0 {
                    self.phase = FollowPhase::Forward;
                    self.last_eval = Some(s);
                    self.next_eval = now + WINDOW;
                }
            }
            FollowPhase::Settling(until) => {
                d.drive(0, self.heading, false, now);
                if now >= until {
                    self.phase = FollowPhase::Forward;
                    self.last_eval = Some(s);
                    self.next_eval = now + WINDOW;
                }
            }
            FollowPhase::Forward => {
                if s >= self.near_threshold {
                    self.phase = FollowPhase::Arrived;
                    return;
                }
                if s <= LOST_LEVEL {
                    self.stop(d, now);
                    self.status = format!("Señal muy débil ({} dBm): parado para no perderlo", s as i32);
                    return;
                }
                d.drive(self.speed as i32, self.heading, false, now);
                self.status = format!("Buscando… ({} dBm)", s as i32);
                if now < self.next_eval {
                    return;
                }
                let delta = s - self.last_eval.unwrap_or(s);
                self.last_eval = Some(s);
                self.next_eval = now + WINDOW;
                if delta < SHARP_DROP {
                    // Caída brusca: vamos claramente en mala dirección.
                    self.heading = (self.heading + 150 + fastrand::i32(-30..=30)).rem_euclid(360);
                    self.phase = FollowPhase::Settling(now + Duration::from_millis(600));
                    self.corrected = false;
                    self.status = "Caída brusca: media vuelta".into();
                } else if delta < MILD_DROP {
                    // Empeora un poco: giro suave sin parar. Si ya se corrigió y sigue peor, se prueba el otro lado.
                    if self.corrected {
                        self.turn_sign = -self.turn_sign;
                    }
                    self.heading = (self.heading + self.turn_sign * if self.corrected { 60 } else { 30 }).rem_euclid(360);
                    self.corrected = true;
                    self.status = format!("Corrigiendo rumbo ({} dBm)", s as i32);
                } else {
                    self.corrected = false; // estable o mejora: seguimos recto
                }
            }
        }
    }
}
