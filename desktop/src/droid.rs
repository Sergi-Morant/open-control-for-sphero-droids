//! Un droide conectado: estado, detección de direcciones y comandos de alto nivel.
//! Toda la lógica del protocolo es síncrona; la E/S Bluetooth vive en `backend.rs`.

use std::collections::{BTreeMap, HashMap, VecDeque};
use std::time::{Duration, Instant};

use tokio::sync::mpsc::UnboundedSender;

use crate::kind::{Animation, Kind, LightChannel, LightStyle, Sound};
use crate::protocol::*;

pub enum Outgoing {
    Data(Vec<u8>),
    Disconnect,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Link {
    Connecting,
    Ready,
    Disconnected,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Battery {
    Unknown,
    Charged,
    Charging,
    Ok,
    Low,
    Critical,
}

impl Battery {
    pub fn text(self) -> &'static str {
        match self {
            Battery::Unknown => "—",
            Battery::Charged => "Cargado",
            Battery::Charging => "Cargando",
            Battery::Ok => "Normal",
            Battery::Low => "Baja",
            Battery::Critical => "Crítica",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum LegAction {
    Stop = 0,
    ThreeLegs = 1,
    TwoLegs = 2,
    Waddle = 3,
}

impl LegAction {
    pub const ALL: [LegAction; 4] = [LegAction::Stop, LegAction::ThreeLegs, LegAction::TwoLegs, LegAction::Waddle];
    pub fn title(self) -> &'static str {
        ["Parar", "3 patas", "2 patas", "Contoneo"][self as usize]
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Interactions {
    Off,
    Idle,
    Social,
}

impl Interactions {
    pub const ALL: [Interactions; 3] = [Interactions::Off, Interactions::Idle, Interactions::Social];
    pub fn title(self) -> &'static str {
        ["Ninguna", "Animaciones de reposo", "Reposo + otros droides"][self as usize]
    }
}

const ERROR_NAMES: [&str; 11] = [
    "correcto", "DID no válido", "CID no válido", "no implementado", "restringido", "longitud de datos incorrecta",
    "falló", "parámetro no válido", "ocupado", "destino no válido", "destino no disponible",
];

const CHATTER_GROUPS: [&str; 7] = ["Chatty", "Excited", "Positive", "Hey", "Negative", "Sad", "Laugh"];

pub struct Droid {
    pub id: String,
    pub name: String,
    pub kind: Kind,
    pub link: Link,
    pub status: String,
    pub log: VecDeque<String>,
    pub battery: Battery,
    pub battery_text: String,
    pub versions: BTreeMap<String, String>,
    pub routing_text: String,
    /// Señal Bluetooth medida desde este ordenador (dBm).
    pub rssi: Option<i16>,
    pub busy_until: Option<Instant>,
    pub chatter: bool,
    pub react_to_hits: bool,
    pub idle_animations: bool,
    pub trophy_mode: bool,
    pub head_tracking: bool,

    hit_pending: bool,
    tx: UnboundedSender<Outgoing>,
    routing: Routing,
    routing_confirmed: bool,
    probes: HashMap<u8, Routing>,
    probe_start: Option<Instant>,
    probe_sent: usize,
    probe_warned: bool,
    seq: u8,
    dec_v2: DecoderV2,
    dec_v1: DecoderV1,
    v1_battery_seq: u8,
    last_poll: Instant,
    next_chatter: Instant,
    last_error: String,
    last_error_at: Option<Instant>,
    started: Instant,
}

impl Droid {
    pub fn new(id: String, name: String, kind: Kind, tx: UnboundedSender<Outgoing>) -> Self {
        let now = Instant::now();
        Droid {
            id,
            name,
            kind,
            link: Link::Connecting,
            status: "Conectando…".into(),
            log: VecDeque::new(),
            battery: Battery::Unknown,
            battery_text: "—".into(),
            versions: BTreeMap::new(),
            routing_text: "sin detectar".into(),
            rssi: None,
            busy_until: None,
            chatter: false,
            react_to_hits: true,
            idle_animations: false,
            trophy_mode: false,
            head_tracking: false,
            hit_pending: false,
            tx,
            routing: Routing::CANDIDATES[0],
            routing_confirmed: false,
            probes: HashMap::new(),
            probe_start: None,
            probe_sent: 0,
            probe_warned: false,
            seq: 0,
            dec_v2: DecoderV2::default(),
            dec_v1: DecoderV1::default(),
            v1_battery_seq: 0,
            last_poll: now,
            next_chatter: now,
            last_error: String::new(),
            last_error_at: None,
            started: now,
        }
    }

    pub fn is_ready(&self) -> bool {
        self.link == Link::Ready
    }

    pub fn busy(&self, now: Instant) -> bool {
        self.busy_until.is_some_and(|t| now < t)
    }

    pub fn note(&mut self, s: impl AsRef<str>) {
        let t = self.started.elapsed().as_secs_f32();
        self.log.push_back(format!("{:>8.3} {}", t, s.as_ref()));
        if self.log.len() > 600 {
            self.log.drain(..200);
        }
    }

    /// Devuelve true una sola vez por cada colisión detectada.
    pub fn take_hit(&mut self) -> bool {
        std::mem::take(&mut self.hit_pending)
    }

    // ------------------------------------------------------------- ciclo de vida

    pub fn on_ready(&mut self, now: Instant) {
        self.link = Link::Ready;
        self.status = "Conectado".into();
        self.last_poll = now;
        if self.kind.uses_v2() {
            self.wake();
            self.probe_start = Some(now + Duration::from_secs(1));
        } else {
            self.after_routing_confirmed();
        }
    }

    pub fn on_disconnected(&mut self) {
        self.link = Link::Disconnected;
        self.status = "Desconectado".into();
        self.chatter = false;
        self.routing_confirmed = false;
        self.probe_start = None;
        self.probe_sent = 0;
    }

    pub fn disconnect(&mut self) {
        let _ = self.tx.send(Outgoing::Disconnect);
    }

    /// Se llama unas diez veces por segundo.
    pub fn tick(&mut self, now: Instant) {
        if self.link != Link::Ready {
            return;
        }
        if self.kind.uses_v2() && !self.routing_confirmed {
            if let Some(start) = self.probe_start {
                let n = Routing::CANDIDATES.len();
                while self.probe_sent < n && now >= start + Duration::from_millis(500 * self.probe_sent as u64) {
                    self.send_probe(self.probe_sent);
                    self.probe_sent += 1;
                }
                if self.probe_sent == n && !self.probe_warned && now >= start + Duration::from_millis(500 * n as u64 + 1500) {
                    self.probe_warned = true;
                    self.routing_text = "no se pudo detectar".into();
                    self.note("AVISO: ningún esquema de direcciones obtuvo respuesta del procesador principal");
                }
            }
        }
        if now.duration_since(self.last_poll) >= Duration::from_secs(30) {
            self.last_poll = now;
            self.request_battery();
        }
        if self.chatter && now >= self.next_chatter {
            self.play_random_chatter();
            self.next_chatter = now + Duration::from_millis(fastrand::u64(6_000..16_000));
        }
        if self.busy_until.is_some_and(|t| now >= t) {
            self.busy_until = None;
        }
    }

    fn send_probe(&mut self, idx: usize) {
        let cand = Routing::CANDIDATES[idx];
        self.seq = self.seq.wrapping_add(1);
        self.probes.insert(self.seq, cand);
        let pkt = encode_v2(cand, true, dev::API_PROCESSOR, 0x00, self.seq, &[]);
        self.write(pkt);
        self.note(format!("probe {}", cand.title()));
    }

    fn after_routing_confirmed(&mut self) {
        self.wake();
        self.configure_collisions(70);
        if self.kind.uses_v2() {
            self.send_v2(false, dev::POWER, 0x05, &[1], false); // notificaciones de batería
            self.request_versions();
        } else {
            self.send_v1(0x00, 0x21, &[1], false); // notificaciones de energía (v1)
        }
        self.request_battery();
    }

    // ------------------------------------------------------------- envío

    fn write(&mut self, data: Vec<u8>) {
        let _ = self.tx.send(Outgoing::Data(data));
    }

    fn send_v2(&mut self, st: bool, did: u8, cid: u8, payload: &[u8], quiet: bool) {
        if !self.is_ready() {
            return;
        }
        self.seq = self.seq.wrapping_add(1);
        let data = encode_v2(self.routing, st, did, cid, self.seq, payload);
        if !quiet {
            self.note(format!("-> {}", hex(&data)));
        }
        self.write(data);
    }

    fn send_v1(&mut self, did: u8, cid: u8, payload: &[u8], quiet: bool) {
        if !self.is_ready() {
            return;
        }
        self.seq = self.seq.wrapping_add(1);
        let data = encode_v1(did, cid, self.seq, payload);
        if !quiet {
            self.note(format!("-> {}", hex(&data)));
        }
        self.write(data);
    }

    // ------------------------------------------------------------- energía y estado

    pub fn wake(&mut self) {
        if self.kind.uses_v2() {
            self.send_v2(false, dev::POWER, 0x0D, &[], false);
        }
    }

    pub fn sleep(&mut self, deep: bool) {
        if self.kind.uses_v2() {
            self.send_v2(false, dev::POWER, if deep { 0x00 } else { 0x01 }, &[], false);
        } else {
            self.send_v1(0x00, 0x22, &[0, 0, 0, 0, 0], false);
        }
    }

    pub fn request_battery(&mut self) {
        if self.kind.uses_v2() {
            self.send_v2(false, dev::POWER, 0x04, &[], true);
            self.send_v2(false, dev::POWER, 0x03, &[], true);
        } else {
            self.send_v1(0x00, 0x20, &[], true);
            self.v1_battery_seq = self.seq;
        }
    }

    pub fn request_versions(&mut self) {
        if self.kind.uses_v2() {
            self.send_v2(false, dev::SYSTEM_INFO, 0x00, &[], false);
            self.send_v2(false, dev::SYSTEM_INFO, 0x17, &[], false);
        }
    }

    pub fn rename(&mut self, new_name: &str) {
        if self.kind.uses_v2() {
            self.send_v2(false, dev::PEER_CONNECTION, 0x03, new_name.as_bytes(), false);
        }
    }

    // ------------------------------------------------------------- conducción

    /// speed 0…255; heading 0…359 relativo al último «fijar adelante».
    pub fn drive(&mut self, speed: i32, heading: i32, reverse: bool, now: Instant) {
        let s = speed.clamp(0, 255) as u8;
        let h = heading.rem_euclid(360) as u16;
        if self.kind.uses_v2() {
            // El droide rechaza conducir (error 8, ocupado) mientras reproduce una animación: se cancela.
            if self.busy(now) && s > 0 {
                self.stop_animation();
            }
            let mut p = vec![s];
            p.extend_from_slice(&be16(h));
            p.push(reverse as u8);
            self.send_v2(true, dev::DRIVING, 0x07, &p, true);
        } else {
            let mut p = vec![s];
            p.extend_from_slice(&be16(h));
            p.extend_from_slice(&[(s != 0) as u8, reverse as u8]);
            self.send_v1(0x02, 0x30, &p, true);
        }
    }

    pub fn stop(&mut self, heading: i32, now: Instant) {
        self.drive(0, heading, false, now);
    }

    /// Gira el droide hacia `heading` sin avanzar (para apuntar).
    pub fn aim(&mut self, heading: i32, now: Instant) {
        if self.kind.uses_v2() {
            self.drive(0, heading, false, now);
        } else {
            let mut p = vec![0];
            p.extend_from_slice(&be16(heading.rem_euclid(360) as u16));
            p.extend_from_slice(&[1, 0]);
            self.send_v1(0x02, 0x30, &p, true);
        }
    }

    /// Define la orientación actual como «adelante».
    pub fn set_forward(&mut self) {
        if self.kind.uses_v2() {
            self.send_v2(true, dev::DRIVING, 0x06, &[], false);
        } else {
            self.send_v1(0x02, 0x01, &[0, 0], false);
        }
    }

    pub fn set_stabilization(&mut self, on: bool) {
        if self.kind.uses_v2() {
            self.send_v2(true, dev::DRIVING, 0x0C, &[on as u8], false);
        } else {
            self.send_v1(0x02, 0x02, &[on as u8], false);
        }
    }

    /// Impulso de BB-8 (cid 0x31): `time` en décimas de segundo.
    pub fn boost_v1(&mut self, heading: i32) {
        if !self.kind.uses_v2() {
            let mut p = vec![20];
            p.extend_from_slice(&be16(heading.rem_euclid(360) as u16));
            self.send_v1(0x02, 0x31, &p, false);
        }
    }

    // ------------------------------------------------------------- cúpula y patas

    pub fn set_dome(&mut self, degrees: f32) {
        if self.kind.has_dome() {
            self.send_v2(true, dev::ANIMATRONICS, 0x0F, &be_float(degrees), true);
        }
    }

    pub fn leg_action(&mut self, a: LegAction) {
        if self.kind.has_dome() {
            self.send_v2(true, dev::ANIMATRONICS, 0x0D, &[a as u8], false);
        }
    }

    pub fn set_head_tracking(&mut self, on: bool) {
        self.head_tracking = on;
        if self.kind.has_dome() {
            self.send_v2(true, dev::ANIMATRONICS, 0x29, &[on as u8], false);
        }
    }

    // ------------------------------------------------------------- animaciones y sonido

    pub fn play_animation(&mut self, a: &Animation, now: Instant) {
        if !self.kind.uses_v2() {
            return;
        }
        self.send_v2(true, dev::ANIMATRONICS, 0x05, &be16(a.index), false);
        self.busy_until = Some(now + Duration::from_secs_f32(a.seconds + 0.5));
    }

    pub fn stop_animation(&mut self) {
        if self.kind.uses_v2() {
            self.send_v2(true, dev::ANIMATRONICS, 0x2B, &[], false);
            self.busy_until = None;
        }
    }

    pub fn play_sound(&mut self, s: &Sound) {
        if self.kind.has_audio() {
            let mut p = be16(s.sector).to_vec();
            p.push(0);
            self.send_v2(true, dev::USER_IO, 0x07, &p, false);
        }
    }

    pub fn stop_audio(&mut self) {
        if self.kind.has_audio() {
            self.send_v2(true, dev::USER_IO, 0x0A, &[], false);
        }
    }

    pub fn set_volume(&mut self, v: u8) {
        if self.kind.has_audio() {
            self.send_v2(true, dev::USER_IO, 0x08, &[v], false);
        }
    }

    pub fn set_chatter(&mut self, on: bool, now: Instant) {
        self.chatter = on;
        self.next_chatter = now;
    }

    fn play_random_chatter(&mut self) {
        let pool: Vec<&Sound> = self.kind.sounds().iter().filter(|s| CHATTER_GROUPS.contains(&s.group().as_str())).collect();
        if !pool.is_empty() {
            let s = pool[fastrand::usize(..pool.len())];
            self.play_sound(s);
        }
    }

    pub fn set_idle_animations(&mut self, on: bool) {
        self.idle_animations = on;
        if self.kind.uses_v2() {
            self.send_v2(true, dev::ANIMATRONICS, 0x2C, &[on as u8], false);
        }
    }

    pub fn set_trophy_mode(&mut self, on: bool) {
        self.trophy_mode = on;
        if self.kind.uses_v2() {
            self.send_v2(true, dev::ANIMATRONICS, 0x2D, &[on as u8], false);
        }
    }

    pub fn set_droid_to_droid(&mut self, on: bool) {
        if self.kind.uses_v2() {
            self.send_v2(true, dev::ANIMATRONICS, 0x2F, &[on as u8], false);
        }
    }

    pub fn set_interactions(&mut self, m: Interactions) {
        self.set_idle_animations(m != Interactions::Off);
        self.set_droid_to_droid(m == Interactions::Social);
    }

    // ------------------------------------------------------------- luces

    pub fn set_light_rgb(&mut self, ch: &LightChannel, rgb: [u8; 3]) {
        match ch.style {
            LightStyle::Rgb(r, g, b) => self.set_v2_leds(r | g | b, &rgb),
            LightStyle::V1Main => self.send_v1(0x02, 0x20, &[rgb[0], rgb[1], rgb[2], 0], true),
            _ => {}
        }
    }

    pub fn set_light_level(&mut self, ch: &LightChannel, level: u8) {
        match ch.style {
            LightStyle::Mono(m) => self.set_v2_leds(m, &[level]),
            LightStyle::V1Back => self.send_v1(0x02, 0x21, &[level], false),
            _ => {}
        }
    }

    fn set_v2_leds(&mut self, mask: u16, values: &[u8]) {
        let mut p = be16(mask).to_vec();
        p.extend_from_slice(values);
        self.send_v2(true, dev::USER_IO, 0x0E, &p, true);
    }

    pub fn set_aim_light(&mut self, on: bool) {
        let level = if on { 255 } else { 0 };
        match self.kind {
            Kind::Bb8 => self.send_v1(0x02, 0x21, &[level], false),
            Kind::Bb9e => self.set_v2_leds(0x08, &[level]),
            _ => {}
        }
    }

    pub fn reset_lights(&mut self) {
        if self.kind.uses_v2() {
            self.send_v2(true, dev::USER_IO, 0x19, &[], false);
        }
    }

    // ------------------------------------------------------------- colisiones

    pub fn configure_collisions(&mut self, sensitivity: u8) {
        let p = [1, sensitivity, sensitivity, sensitivity, sensitivity, 40];
        if self.kind.uses_v2() {
            self.send_v2(true, dev::SENSORS, 0x11, &p, false);
            self.send_v2(true, dev::SENSORS, 0x14, &[1], false);
        } else {
            self.send_v1(0x02, 0x12, &p, false);
        }
    }

    fn handle_hit(&mut self) {
        self.note("¡Colisión!");
        self.hit_pending = true;
        if !self.react_to_hits {
            return;
        }
        let pool: Vec<&Sound> = self.kind.sounds().iter().filter(|s| matches!(s.group().as_str(), "Hit" | "Negative")).collect();
        if !pool.is_empty() {
            let s = pool[fastrand::usize(..pool.len())];
            self.play_sound(s);
        }
    }

    // ------------------------------------------------------------- respuestas

    pub fn on_notification(&mut self, data: &[u8]) {
        if self.kind.uses_v2() {
            for p in self.dec_v2.feed(data) {
                self.handle_v2(p);
            }
        } else {
            for p in self.dec_v1.feed(data) {
                self.handle_v1(p);
            }
        }
    }

    fn note_error(&mut self, e: u8, did: u8, cid: u8) {
        let key = format!("{did:02X}/{cid:02X} e{e}");
        let now = Instant::now();
        // No repetir el mismo error al conducir a 10 Hz.
        if key == self.last_error && self.last_error_at.is_some_and(|t| now.duration_since(t) < Duration::from_secs(3)) {
            return;
        }
        self.last_error = key;
        self.last_error_at = Some(now);
        let name = ERROR_NAMES.get(e as usize).copied().unwrap_or("?");
        self.note(format!("ERROR {e} ({name}) en {did:02X}/{cid:02X}"));
    }

    fn handle_v2(&mut self, p: ParsedV2) {
        if p.is_response {
            if let Some(cand) = self.probes.remove(&p.seq) {
                if p.error == Some(0) && !self.routing_confirmed {
                    self.routing = cand;
                    self.routing_confirmed = true;
                    self.routing_text = cand.title();
                    self.note(format!("OK direcciones detectadas: {}", cand.title()));
                    self.after_routing_confirmed();
                } else {
                    self.note(format!("probe {} -> error {}", cand.title(), p.error.unwrap_or(0)));
                }
                return;
            }
            if let Some(e) = p.error.filter(|e| *e != 0) {
                self.note_error(e, p.did, p.cid);
            }
        }

        match (p.did, p.cid) {
            (dev::POWER, 0x04) | (dev::POWER, 0x06) => {
                if let Some(&s) = p.payload.first() {
                    self.battery = match s {
                        0 => Battery::Charged,
                        1 => Battery::Charging,
                        2 | 3 => Battery::Ok,
                        4 => Battery::Low,
                        5 => Battery::Critical,
                        _ => Battery::Unknown,
                    };
                }
            }
            (dev::POWER, 0x03) => {
                if let Some(cv) = read_u16(&p.payload, 0) {
                    self.battery_text = format!("{:.2} V", cv as f32 / 100.0); // centésimas de voltio
                }
            }
            (dev::SYSTEM_INFO, 0x00) => {
                self.versions.insert("Nordic".into(), version(&p.payload));
            }
            (dev::SYSTEM_INFO, 0x17) | (dev::SYSTEM_INFO, 0x18) => {
                self.versions.insert("Principal".into(), version(&p.payload));
            }
            (dev::SENSORS, 0x12) => self.handle_hit(),
            (dev::ANIMATRONICS, 0x11) => self.busy_until = None,
            _ => {
                // Los acuses correctos y vacíos (uno por cada orden de conducción) solo meterían ruido.
                if p.is_response && p.error == Some(0) && p.payload.is_empty() {
                    return;
                }
                self.note(format!("<- {:02X}/{:02X} {}", p.did, p.cid, hex(&p.payload)));
            }
        }
    }

    fn handle_v1(&mut self, p: ParsedV1) {
        if p.is_async {
            match p.code {
                0x07 => self.handle_hit(),
                0x01 => {
                    if let Some(&s) = p.payload.first() {
                        self.battery = v1_battery(s);
                    }
                }
                other => self.note(format!("<- async {other:02X} {}", hex(&p.payload))),
            }
            return;
        }
        if p.code != 0 {
            self.note(format!("ERROR {}", p.code));
        }
        // Respuesta de GetPowerState: [versión, estado, voltaje(2), cargas(2), tiempo(2)]
        if p.seq == self.v1_battery_seq && p.payload.len() >= 4 {
            self.battery = v1_battery(p.payload[1]);
            if let Some(cv) = read_u16(&p.payload, 2) {
                self.battery_text = format!("{:.2} V", cv as f32 / 100.0);
            }
        }
    }
}

fn v1_battery(s: u8) -> Battery {
    match s {
        1 => Battery::Charging,
        2 => Battery::Ok,
        3 => Battery::Low,
        4 => Battery::Critical,
        _ => Battery::Unknown,
    }
}

fn version(b: &[u8]) -> String {
    match (read_u16(b, 0), read_u16(b, 2), read_u16(b, 4)) {
        (Some(a), Some(m), Some(r)) => format!("{a}.{m}.{r}"),
        _ => hex(b),
    }
}
