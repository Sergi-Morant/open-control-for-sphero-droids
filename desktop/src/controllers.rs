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

#[derive(Clone, Copy, PartialEq)]
enum FollowPhase {
    Forward,
    Settling(Instant),
    Arrived,
}

/// Una medida de señal y dónde estaba el droide (posición estimada por las órdenes enviadas).
#[derive(Clone, Copy)]
struct Sample {
    x: f32,
    y: f32,
    rssi: f32,
}

/// Sigue al ordenador usando solo la intensidad de la señal Bluetooth (RSSI).
///
/// El RSSI no da dirección y fluctúa mucho. En vez de reaccionar a medidas sueltas, el droide avanza
/// zigzagueando y apunta cada medida junto a su posición estimada (se calcula con las órdenes enviadas,
/// sin sensores). Con las medidas de los últimos segundos se ajusta un plano: su pendiente indica hacia
/// dónde crece la señal, y hacia ahí se corrige el rumbo. El zigzag hace falta para ver también la
/// pendiente lateral. Frena al acercarse, se para si deja de recibir medidas y sigue siendo aproximado.
pub struct Follow {
    pub running: bool,
    /// Señal filtrada (dBm). Se actualiza siempre que haya medidas, también con el modo parado.
    pub smoothed: Option<f32>,
    pub status: String,
    /// dBm; por encima de esto se considera «ya he llegado».
    pub near_threshold: f32,
    pub speed: f32,
    /// Últimas señales filtradas, para la gráfica.
    pub history: VecDeque<f32>,
    /// Última orden enviada: (velocidad, rumbo).
    pub command: (i32, i32),
    phase: FollowPhase,
    /// Rumbo hacia el que se cree que está el ordenador; el rumbo real zigzaguea alrededor.
    target: f32,
    weave_sign: f32,
    heading: i32,
    pos: (f32, f32),
    last_tick: Instant,
    samples: VecDeque<Sample>,
    recent: VecDeque<f32>,
    seen_at: Option<Instant>,
    leg_end: Instant,
    retreated: bool,
    stalled: bool,
}

/// Duración de cada tramo del zigzag.
const LEG: Duration = Duration::from_millis(1800);
/// Grados que se desvía cada tramo a un lado y a otro del rumbo.
const WEAVE: f32 = 30.0;
/// Medidas que se usan para estimar la pendiente (unos 12 s).
const FIT_WINDOW: usize = 36;
const FIT_MIN: usize = 10;
/// La pendiente tiene que explicar al menos estos dB a lo largo del recorrido para fiarse de ella.
const FIT_MIN_STRENGTH: f32 = 0.8;
const LOST_LEVEL: f32 = -85.0;
/// Sin medidas durante este tiempo, el droide se para.
const STALE: Duration = Duration::from_millis(2500);
/// dB que tiene que alejarse la señal del umbral para volver a moverse tras llegar.
const HYSTERESIS: f32 = 5.0;
pub const FOLLOW_HISTORY: usize = 160;

/// Ajusta un plano a las medidas y devuelve (rumbo hacia el que crece la señal, dB que explica).
fn gradient<'a>(samples: impl Iterator<Item = &'a Sample> + Clone) -> Option<(f32, f32)> {
    let n = samples.clone().count();
    if n < FIT_MIN {
        return None;
    }
    let nf = n as f32;
    let (mx, my, mr) = samples.clone().fold((0.0, 0.0, 0.0), |a, s| (a.0 + s.x / nf, a.1 + s.y / nf, a.2 + s.rssi / nf));
    let (mut sxx, mut sxy, mut syy, mut sxr, mut syr) = (0.0, 0.0, 0.0, 0.0, 0.0);
    for s in samples {
        let (dx, dy, dr) = (s.x - mx, s.y - my, s.rssi - mr);
        sxx += dx * dx;
        sxy += dx * dy;
        syy += dy * dy;
        sxr += dx * dr;
        syr += dy * dr;
    }
    let spread = sxx + syy;
    if spread < 1e-3 {
        return None;
    }
    // Regularización: si el recorrido es casi una línea, la pendiente lateral se queda en cero en vez de dispararse.
    let ridge = 0.08 * spread;
    let (a, d) = (sxx + ridge, syy + ridge);
    let det = a * d - sxy * sxy;
    let gx = (d * sxr - sxy * syr) / det;
    let gy = (a * syr - sxy * sxr) / det;
    let strength = gx.hypot(gy) * (spread / nf).sqrt();
    Some((gx.atan2(gy).to_degrees().rem_euclid(360.0), strength))
}

impl Follow {
    pub fn new() -> Self {
        let now = Instant::now();
        Follow {
            running: false,
            smoothed: None,
            status: "Detenido".into(),
            near_threshold: -55.0,
            speed: 45.0,
            history: VecDeque::new(),
            command: (0, 0),
            phase: FollowPhase::Forward,
            target: 0.0,
            weave_sign: 1.0,
            heading: 0,
            pos: (0.0, 0.0),
            last_tick: now,
            samples: VecDeque::new(),
            recent: VecDeque::new(),
            seen_at: None,
            leg_end: now,
            retreated: false,
            stalled: false,
        }
    }

    pub fn start(&mut self, d: &mut Droid, now: Instant) {
        self.running = true;
        self.phase = FollowPhase::Forward;
        self.target = 0.0;
        self.weave_sign = 1.0;
        self.pos = (0.0, 0.0);
        self.samples.clear();
        self.retreated = false;
        self.stalled = false;
        self.last_tick = now;
        self.new_leg(now);
        self.status = "Esperando señal…".into();
        d.configure_collisions(70);
    }

    pub fn stop(&mut self, d: &mut Droid, now: Instant) {
        if self.running {
            self.halt(d, now);
        }
        self.running = false;
        self.status = "Detenido".into();
    }

    /// Usa la señal actual como «cerca» (coloca el droide junto al ordenador).
    pub fn calibrate_near(&mut self) {
        if let Some(s) = self.smoothed {
            self.near_threshold = (s - 3.0).round().clamp(-80.0, -35.0);
        }
    }

    /// Hay medidas recientes de señal.
    pub fn signal_fresh(&self, d: &Droid, now: Instant) -> bool {
        d.rssi_at.is_some_and(|t| now.saturating_duration_since(t) < STALE)
    }

    /// Rumbo estimado hacia el ordenador, si el modo está en marcha.
    pub fn target_heading(&self) -> Option<f32> {
        self.running.then_some(self.target)
    }

    fn new_leg(&mut self, now: Instant) {
        self.leg_end = now + LEG;
        self.weave_sign = -self.weave_sign;
        self.heading = (self.target + WEAVE * self.weave_sign).rem_euclid(360.0) as i32;
    }

    fn go(&mut self, d: &mut Droid, speed: i32, now: Instant) {
        self.command = (speed, self.heading);
        d.drive(speed, self.heading, false, now);
    }

    fn halt(&mut self, d: &mut Droid, now: Instant) {
        self.command = (0, self.heading);
        d.stop(self.heading, now);
    }

    /// Cambio brusco de rumbo: gira sobre sí mismo un momento y olvida las medidas viejas a medias.
    fn turn_to(&mut self, target: f32, now: Instant, ms: u64) {
        self.target = target.rem_euclid(360.0);
        self.heading = self.target as i32;
        self.phase = FollowPhase::Settling(now + Duration::from_millis(ms));
    }

    /// Incorpora la medida nueva, si la hay: mediana de 3 (quita picos) y media exponencial.
    fn observe(&mut self, d: &Droid) {
        let (Some(r), Some(at)) = (d.rssi, d.rssi_at) else { return };
        if self.seen_at == Some(at) {
            return;
        }
        self.seen_at = Some(at);
        self.recent.push_back(r as f32);
        if self.recent.len() > 3 {
            self.recent.pop_front();
        }
        let mut v: Vec<f32> = self.recent.iter().copied().collect();
        v.sort_by(f32::total_cmp);
        let median = v[v.len() / 2];
        let s = self.smoothed.map_or(median, |s| 0.65 * s + 0.35 * median);
        self.smoothed = Some(s);
        self.history.push_back(s);
        if self.history.len() > FOLLOW_HISTORY {
            self.history.pop_front();
        }
        if self.running && self.phase == FollowPhase::Forward {
            // Para el ajuste se usa la medida sin suavizar: suavizarla la retrasaría respecto a la posición.
            self.samples.push_back(Sample { x: self.pos.0, y: self.pos.1, rssi: r as f32 });
            if self.samples.len() > FIT_WINDOW {
                self.samples.pop_front();
            }
        }
    }

    pub fn tick(&mut self, d: &mut Droid, now: Instant) {
        let dt = now.saturating_duration_since(self.last_tick).as_secs_f32().min(0.5);
        self.last_tick = now;
        if !d.is_ready() {
            self.running = false;
            self.smoothed = None;
            self.recent.clear();
            return;
        }
        // Posición estimada: lo que se le ha mandado hacer desde el tick anterior.
        if self.running && self.command.0 > 0 {
            let a = (self.command.1 as f32).to_radians();
            let step = self.command.0 as f32 * dt;
            self.pos = (self.pos.0 + a.sin() * step, self.pos.1 + a.cos() * step);
        }
        self.observe(d);
        if !self.running {
            return;
        }
        // Sin medidas no se puede decidir nada: mejor quieto que avanzando a ciegas.
        if !self.signal_fresh(d, now) {
            if !self.stalled {
                self.stalled = true;
                self.halt(d, now);
            }
            self.status = "Sin lectura de señal: esperando…".into();
            return;
        }
        if self.stalled {
            self.stalled = false;
            self.new_leg(now);
        }
        if d.take_hit() {
            self.samples.clear();
            self.turn_to(self.heading as f32 + fastrand::i32(120..=240) as f32, now, 1000);
            self.status = "Obstáculo: cambio de rumbo".into();
        }
        let Some(s) = self.smoothed else { return };
        match self.phase {
            FollowPhase::Arrived => {
                self.status = format!("He llegado ({} dBm)", s as i32);
                if s < self.near_threshold - HYSTERESIS {
                    self.phase = FollowPhase::Forward;
                    self.samples.clear();
                    self.new_leg(now);
                }
            }
            FollowPhase::Settling(until) => {
                self.go(d, 0, now); // gira sobre sí mismo hacia el rumbo nuevo
                if now >= until {
                    self.phase = FollowPhase::Forward;
                    self.new_leg(now);
                }
            }
            FollowPhase::Forward => {
                if s >= self.near_threshold {
                    self.phase = FollowPhase::Arrived;
                    self.halt(d, now);
                    self.status = format!("He llegado ({} dBm)", s as i32);
                    return;
                }
                if s <= LOST_LEVEL {
                    if self.retreated {
                        self.stop(d, now);
                        self.status = format!("Señal muy débil ({} dBm): parado para no perderlo", s as i32);
                    } else {
                        // Una oportunidad: deshacer el camino antes de rendirse.
                        self.retreated = true;
                        self.samples.clear();
                        self.turn_to(self.target + 180.0, now, 700);
                        self.status = "Señal muy débil: vuelvo sobre mis pasos".into();
                    }
                    return;
                }
                if s > LOST_LEVEL + 6.0 {
                    self.retreated = false;
                }
                // Frena al acercarse para no pasarse de largo.
                let slow = ((self.near_threshold - s) / 10.0).clamp(0.5, 1.0);
                self.go(d, (self.speed * slow) as i32, now);
                if now < self.leg_end {
                    return;
                }
                self.status = format!("Acercándome… ({} dBm)", s as i32);
                if let Some((bearing, strength)) = gradient(self.samples.iter()) {
                    if strength >= FIT_MIN_STRENGTH {
                        let diff = angle_diff(self.target, bearing);
                        if diff.abs() > 100.0 {
                            self.turn_to(bearing, now, 700);
                            self.status = "La señal cae: media vuelta".into();
                            return;
                        }
                        self.target = (self.target + 0.7 * diff).rem_euclid(360.0);
                        if diff.abs() > 20.0 {
                            self.status = format!("Corrigiendo rumbo ({} dBm)", s as i32);
                        }
                    }
                }
                self.new_leg(now);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::droid::Link;

    #[test]
    fn gradient_points_where_signal_grows() {
        // La señal crece hacia el este (x): rumbo 90°.
        let pts: Vec<Sample> = (0..20).map(|i| { let (x, y) = ((i % 5) as f32, (i / 5) as f32); Sample { x, y, rssi: -70.0 + 2.0 * x } }).collect();
        let (bearing, strength) = gradient(pts.iter()).unwrap();
        assert!(angle_diff(bearing, 90.0).abs() < 5.0, "rumbo {bearing}");
        assert!(strength > 1.0);
        // En línea recta alejándose: la pendiente apunta hacia atrás (180°), sin componente lateral inventada.
        let line: Vec<Sample> = (0..20).map(|i| Sample { x: 0.0, y: i as f32, rssi: -60.0 - 0.5 * i as f32 }).collect();
        let (bearing, _) = gradient(line.iter()).unwrap();
        assert!(angle_diff(bearing, 180.0).abs() < 5.0, "rumbo {bearing}");
        assert!(gradient(pts[..5].iter()).is_none());
    }

    fn droid() -> (Droid, tokio::sync::mpsc::UnboundedReceiver<crate::droid::Outgoing>) {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let mut d = Droid::new("id".into(), "D2-TEST".into(), Kind::R2d2, tx);
        d.link = Link::Ready;
        (d, rx)
    }

    /// Mundo simulado: el ordenador está en el origen, la señal cae con la distancia (más ruido) y el droide
    /// se mueve según la última orden. Devuelve la distancia final y si el modo acabó en «he llegado».
    fn simulate(seed: u64, start: (f32, f32), facing: f32, noise: f32, seconds: u32) -> (f32, bool) {
        fastrand::seed(seed);
        let (mut d, _rx) = droid();
        let mut f = Follow::new();
        let t0 = Instant::now();
        let (mut x, mut y) = start;
        let signal = |x: f32, y: f32| -45.0 - 22.0 * x.hypot(y).max(0.2).log10() + (fastrand::f32() - 0.5) * 2.0 * noise;
        f.start(&mut d, t0);
        for step in 0..seconds * 10 {
            let now = t0 + Duration::from_millis(step as u64 * 100);
            if step % 3 == 0 {
                d.rssi = Some(signal(x, y).round() as i16);
                d.rssi_at = Some(now);
            }
            f.tick(&mut d, now);
            let (speed, heading) = f.command;
            let a = (facing + heading as f32).to_radians();
            let v = speed as f32 / 45.0 * 0.35; // m/s
            x += a.sin() * v * 0.1;
            y += a.cos() * v * 0.1;
        }
        (x.hypot(y), f.phase == FollowPhase::Arrived)
    }

    fn campaign(noise: f32) -> (u32, f32, f32) {
        let runs = 60;
        let mut arrived = 0;
        let mut dists = Vec::new();
        for seed in 0..runs {
            let facing = (seed * 47 % 360) as f32;
            let (dist, ok) = simulate(seed, (6.0, 1.0), facing, noise, 240);
            arrived += ok as u32;
            dists.push(dist);
        }
        dists.sort_by(f32::total_cmp);
        let (median, worst) = (dists[dists.len() / 2], dists[dists.len() - 1]);
        println!("ruido ±{noise} dB: llegaron {arrived}/{runs}, mediana {median:.1} m, peor {worst:.1} m");
        (arrived * 100 / runs as u32, median, worst)
    }

    /// Sale a 6 m, mirando hacia cualquier lado, y tiene que acabar junto al ordenador (umbral ≈ 2,8 m).
    #[test]
    fn approaches_the_computer_in_simulation() {
        let (pct, median, _) = campaign(3.0);
        assert!(pct >= 85, "solo llegó el {pct} %");
        assert!(median < 3.5);
        let (pct, _, _) = campaign(5.0);
        assert!(pct >= 70, "con mucho ruido solo llegó el {pct} %");
    }

    #[test]
    fn stops_when_signal_goes_stale() {
        let (mut d, _rx) = droid();
        let mut f = Follow::new();
        let t0 = Instant::now();
        f.start(&mut d, t0);
        for i in 0..10u64 {
            d.rssi = Some(-70);
            d.rssi_at = Some(t0 + Duration::from_millis(i * 300));
            f.tick(&mut d, t0 + Duration::from_millis(i * 300));
        }
        assert!(f.command.0 > 0);
        f.tick(&mut d, t0 + Duration::from_secs(10));
        assert_eq!(f.command.0, 0);
        assert!(f.running);
    }
}
