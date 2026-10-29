//! Interfaz egui: una ventana con una pestaña por función, igual que la app de iOS.

use std::collections::{BTreeMap, HashMap};
use std::sync::mpsc::{channel, Receiver};
use std::time::{Duration, Instant};

use egui::{Color32, RichText, Sense, Stroke, Vec2};

use crate::backend::{Backend, Event};
use crate::controllers::{Follow, DrawDrive, EmoteLoop, LoopMode, Patrol};
use crate::droid::{Droid, Interactions, LegAction, Link};
use crate::gamepad::{self, Frame, Pad, PadAction, PadButton};
use crate::kind::{Kind, LightStyle, Sound};
use crate::translate;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Connect,
    Drive,
    Emotes,
    Auto,
    Sounds,
    Follow,
    Patrol,
    Draw,
    Lights,
    Gamepad,
    Energy,
    Settings,
    Log,
}

const TABS: [(Tab, &str); 13] = [
    (Tab::Connect, "Conectar"),
    (Tab::Drive, "Conducir"),
    (Tab::Emotes, "Emociones"),
    (Tab::Auto, "Automático"),
    (Tab::Sounds, "Sonidos"),
    (Tab::Follow, "Seguir"),
    (Tab::Patrol, "Patrulla"),
    (Tab::Draw, "Dibujar"),
    (Tab::Lights, "Luces"),
    (Tab::Gamepad, "Mando"),
    (Tab::Energy, "Energía"),
    (Tab::Settings, "Ajustes"),
    (Tab::Log, "Log"),
];

/// Cómo se conduce cuando no hay mando conectado.
#[derive(Clone, Copy, PartialEq, Eq)]
enum InputMode {
    /// W A S D o flechas.
    Keyboard,
    /// Círculo en pantalla que se arrastra con el ratón.
    Mouse,
}

struct Found {
    id: String,
    name: String,
    kind: Kind,
    rssi: Option<i16>,
}

/// Estado de interfaz y controladores de un droide.
struct DroidUi {
    emote_loop: EmoteLoop,
    patrol: Patrol,
    draw: DrawDrive,
    follow: Follow,
    // conducción
    heading: f32,
    max_speed: f32,
    turbo: bool,
    stabilize: bool,
    aiming: bool,
    aim_heading: f32,
    dome: f32,
    driving: bool,
    last_drive: Instant,
    pad_driving: bool,
    pad_dome_active: bool,
    last_dome_sent: Instant,
    // resto de pestañas
    volume: f32,
    search: String,
    light_rgb: HashMap<&'static str, [u8; 3]>,
    light_level: HashMap<&'static str, f32>,
    rename: String,
    interactions: Interactions,
}

impl DroidUi {
    fn new() -> Self {
        let now = Instant::now();
        DroidUi {
            emote_loop: EmoteLoop::new(),
            patrol: Patrol::new(),
            draw: DrawDrive::new(),
            follow: Follow::new(),
            heading: 0.0,
            max_speed: 0.6,
            turbo: false,
            stabilize: true,
            aiming: false,
            aim_heading: 0.0,
            dome: 0.0,
            driving: false,
            last_drive: now,
            pad_driving: false,
            pad_dome_active: false,
            last_dome_sent: now,
            volume: 150.0,
            search: String::new(),
            light_rgb: HashMap::new(),
            light_level: HashMap::new(),
            rename: String::new(),
            interactions: Interactions::Off,
        }
    }
}

struct Entry {
    droid: Droid,
    ui: DroidUi,
}

impl Entry {
    /// Detiene todo lo que la app haya puesto en marcha.
    fn stop_all(&mut self, now: Instant) {
        self.ui.emote_loop.stop();
        self.ui.follow.stop(&mut self.droid, now);
        self.ui.patrol.stop(&mut self.droid, now);
        self.ui.draw.abort(&mut self.droid, now);
        self.droid.set_chatter(false, now);
        self.droid.stop_animation();
        self.droid.stop_audio();
        self.droid.stop(self.ui.heading as i32, now);
    }
}

pub struct App {
    backend: Backend,
    rx: Receiver<Event>,
    entries: Vec<Entry>,
    active: usize,
    found: Vec<Found>,
    scanning: bool,
    bt_error: Option<String>,
    tab: Tab,
    pad: Pad,
    pad_frame_connected: bool,
    input_mode: InputMode,
    last_logic: Instant,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let (tx, rx) = channel();
        App {
            backend: Backend::new(cc.egui_ctx.clone(), tx),
            rx,
            entries: Vec::new(),
            active: 0,
            found: Vec::new(),
            scanning: false,
            bt_error: None,
            tab: Tab::Connect,
            pad: Pad::new(),
            pad_frame_connected: false,
            input_mode: InputMode::Mouse,
            last_logic: Instant::now(),
        }
    }

    fn entry_mut(&mut self, id: &str) -> Option<&mut Entry> {
        self.entries.iter_mut().find(|e| e.droid.id == id)
    }

    fn handle_event(&mut self, ev: Event, now: Instant) {
        match ev {
            Event::ScanStarted => {
                self.scanning = true;
                self.bt_error = None;
            }
            Event::ScanFinished => self.scanning = false,
            Event::BluetoothError(e) => self.bt_error = Some(e),
            Event::Found { id, name, kind, rssi } => {
                if let Some(f) = self.found.iter_mut().find(|f| f.id == id) {
                    f.rssi = rssi;
                } else {
                    self.found.push(Found { id, name, kind, rssi });
                }
            }
            Event::Ready(id) => {
                if let Some(e) = self.entry_mut(&id) {
                    e.droid.on_ready(now);
                }
            }
            Event::Notification(id, data) => {
                if let Some(e) = self.entry_mut(&id) {
                    e.droid.on_notification(&data);
                }
            }
            Event::Rssi(id, r) => {
                if let Some(e) = self.entry_mut(&id) {
                    e.droid.rssi = Some(r);
                }
            }
            Event::Failed(id, msg) => {
                if let Some(e) = self.entry_mut(&id) {
                    e.droid.status = format!("Error: {msg}");
                    e.droid.note(format!("ERROR de conexión: {msg}"));
                }
            }
            Event::Disconnected(id) => {
                if let Some(e) = self.entry_mut(&id) {
                    e.droid.on_disconnected();
                }
            }
        }
    }
}

impl eframe::App for App {
    /// Trabajo que no dibuja: eventos Bluetooth, mando y controladores.
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let now = Instant::now();
        let dt = now.duration_since(self.last_logic).as_secs_f32().min(0.5);
        self.last_logic = now;

        while let Ok(ev) = self.rx.try_recv() {
            self.handle_event(ev, now);
        }

        let frame = self.pad.poll();
        self.pad_frame_connected = frame.connected;
        if let Some(e) = self.entries.get_mut(self.active) {
            apply_pad(e, &self.pad, &frame, dt, now);
        }

        for e in &mut self.entries {
            e.droid.tick(now);
            e.ui.emote_loop.tick(&mut e.droid, now);
            e.ui.patrol.tick(&mut e.droid, now);
            e.ui.draw.tick(&mut e.droid, now);
            e.ui.follow.tick(&mut e.droid, now);
        }
        ctx.request_repaint_after(Duration::from_millis(100));
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let now = Instant::now();
        let App { backend, entries, active, found, scanning, bt_error, tab, pad, pad_frame_connected, input_mode, .. } = self;
        if entries.is_empty() {
            *tab = Tab::Connect;
        }
        *active = (*active).min(entries.len().saturating_sub(1));

        egui::Panel::top("droids").show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                for (i, e) in entries.iter().enumerate() {
                    let color = match e.droid.link {
                        Link::Ready => Color32::from_rgb(90, 200, 120),
                        Link::Connecting => Color32::from_rgb(230, 170, 60),
                        Link::Disconnected => Color32::GRAY,
                    };
                    let text = RichText::new(format!("{} · {}", e.droid.kind.title(), e.droid.battery.text())).color(color);
                    if ui.selectable_label(i == *active, text).clicked() {
                        *active = i;
                    }
                }
                if ui.button("+ Droides").clicked() {
                    *tab = Tab::Connect;
                }
                if *pad_frame_connected {
                    ui.label(RichText::new("Mando conectado").color(Color32::from_rgb(90, 200, 120)));
                }
            });
        });

        egui::Panel::bottom("bottom").show(ui, |ui| {
            ui.horizontal(|ui| {
                if let Some(e) = entries.get_mut(*active) {
                    let stop = egui::Button::new(RichText::new("Parar todo").strong()).fill(Color32::from_rgb(170, 50, 50));
                    if ui.add(stop).clicked() {
                        e.stop_all(now);
                    }
                    if e.ui.emote_loop.running == Some(LoopMode::Wwm) && ui.button("Viendo una película  ✕").clicked() {
                        e.ui.emote_loop.stop();
                    }
                    if e.ui.emote_loop.running == Some(LoopMode::Emotions) && ui.button("Haciendo monerías  ✕").clicked() {
                        e.ui.emote_loop.stop();
                    }
                    if e.ui.follow.running && ui.button("Siguiéndote  ✕").clicked() {
                        e.ui.follow.stop(&mut e.droid, now);
                    }
                    if e.droid.chatter && ui.button("Charla aleatoria  ✕").clicked() {
                        e.droid.set_chatter(false, now);
                    }
                    ui.label(format!("{}: {}", e.droid.name, e.droid.status));
                }
            });
        });

        egui::CentralPanel::default().show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                for (t, name) in TABS {
                    if entries.is_empty() && t != Tab::Connect {
                        continue;
                    }
                    ui.selectable_value(tab, t, name);
                }
            });
            ui.separator();

            if *tab == Tab::Connect {
                connect_tab(ui, backend, entries, active, found, *scanning, bt_error.as_deref());
                return;
            }
            if *tab == Tab::Gamepad {
                gamepad_tab(ui, pad, *pad_frame_connected);
                return;
            }
            let Some(e) = entries.get_mut(*active) else { return };
            if !e.droid.is_ready() {
                ui.colored_label(Color32::from_rgb(230, 170, 60), format!("{}: {}", e.droid.name, e.droid.status));
            }
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| match *tab {
                Tab::Drive => drive_tab(ui, e, now, *pad_frame_connected, input_mode),
                Tab::Emotes => emotes_tab(ui, e, now),
                Tab::Auto => auto_tab(ui, e, now),
                Tab::Sounds => sounds_tab(ui, e, now),
                Tab::Follow => follow_tab(ui, e, now),
                Tab::Patrol => patrol_tab(ui, e, now),
                Tab::Draw => draw_tab(ui, e, now),
                Tab::Lights => lights_tab(ui, e),
                Tab::Energy => energy_tab(ui, e),
                Tab::Settings => settings_tab(ui, e),
                Tab::Log => log_tab(ui, e),
                Tab::Connect | Tab::Gamepad => {}
            });
        });

        // Si se sale de la pestaña Conducir mientras se conduce con teclado o ratón, se para.
        if *tab != Tab::Drive {
            for e in entries.iter_mut() {
                if e.ui.driving {
                    e.ui.driving = false;
                    e.droid.stop(e.ui.heading as i32, now);
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------- mando

/// Aplica el estado del mando al droide activo.
fn apply_pad(e: &mut Entry, pad: &Pad, f: &Frame, dt: f32, now: Instant) {
    if !f.connected {
        return;
    }
    // Gatillos: RT sube y LT baja la velocidad máxima.
    e.ui.max_speed = (e.ui.max_speed + (f.rt - f.lt) * 0.5 * dt).clamp(0.15, 1.0);
    e.ui.turbo = f.held.iter().any(|b| pad.mapping.get(b) == Some(&PadAction::Turbo));

    // Stick izquierdo: mueve el droide.
    let (lx, ly) = f.left;
    let mag = lx.hypot(ly);
    if mag > 0.0 {
        let cap = if e.ui.turbo { 1.0 } else { e.ui.max_speed };
        let speed = (mag * 255.0 * cap) as i32;
        let heading = gamepad::heading_of(lx, ly);
        e.ui.heading = heading as f32;
        e.ui.pad_driving = true;
        if now.duration_since(e.ui.last_drive) >= Duration::from_millis(100) {
            e.ui.last_drive = now;
            e.droid.drive(speed, heading, false, now);
        }
    } else if e.ui.pad_driving {
        e.ui.pad_driving = false;
        e.droid.stop(e.ui.heading as i32, now);
    }

    // Stick derecho (eje X): cúpula. Al soltarlo vuelve al centro.
    if e.droid.kind.has_dome() {
        let moving = f.right_x != 0.0;
        if moving || e.ui.pad_dome_active {
            let target = if moving { gamepad::dome_of(f.right_x) } else { 0.0 };
            if (target - e.ui.dome).abs() >= 2.0 && now.duration_since(e.ui.last_dome_sent) >= Duration::from_millis(100) {
                e.ui.last_dome_sent = now;
                e.ui.dome = target;
                e.droid.set_dome(target);
            }
            e.ui.pad_dome_active = moving || (target - e.ui.dome).abs() >= 2.0;
        }
    }

    for b in &f.pressed {
        if let Some(a) = pad.mapping.get(b).copied() {
            run_pad_action(e, a, now);
        }
    }
}

fn run_pad_action(e: &mut Entry, a: PadAction, now: Instant) {
    match a {
        PadAction::Legs3 => e.droid.leg_action(LegAction::ThreeLegs),
        PadAction::Legs2 => e.droid.leg_action(LegAction::TwoLegs),
        PadAction::Waddle => e.droid.leg_action(LegAction::Waddle),
        PadAction::LegsStop => e.droid.leg_action(LegAction::Stop),
        PadAction::StopAll => e.stop_all(now),
        PadAction::SetForward => e.droid.set_forward(),
        PadAction::RandomSound => {
            let pool: Vec<&Sound> = e.droid.kind.sounds().iter().filter(|s| ["Chatty", "Excited", "Positive", "Hey"].contains(&s.group().as_str())).collect();
            if !pool.is_empty() {
                e.droid.play_sound(pool[fastrand::usize(..pool.len())]);
            }
        }
        PadAction::RandomEmote => {
            let pool = e.ui.emote_loop.pool(e.droid.kind, LoopMode::Emotions);
            if !pool.is_empty() && !e.droid.busy(now) {
                e.droid.play_animation(pool[fastrand::usize(..pool.len())], now);
            }
        }
        PadAction::ToggleMovie => toggle_loop(e, LoopMode::Wwm, now),
        PadAction::ToggleMonkeyBusiness => toggle_loop(e, LoopMode::Emotions, now),
        PadAction::Boost => e.droid.boost_v1(e.ui.heading as i32),
        PadAction::Turbo => {} // se aplica mientras se mantiene pulsado
    }
}

fn toggle_loop(e: &mut Entry, mode: LoopMode, now: Instant) {
    if e.ui.emote_loop.running == Some(mode) {
        e.ui.emote_loop.stop();
    } else {
        e.ui.emote_loop.start(mode, now);
    }
}

fn gamepad_tab(ui: &mut egui::Ui, pad: &mut Pad, connected: bool) {
    if !pad.available() {
        ui.colored_label(Color32::from_rgb(230, 170, 60), "No se pudo iniciar la lectura de mandos en este sistema.");
    }
    match (&pad.name, connected) {
        (Some(n), true) => ui.colored_label(Color32::from_rgb(90, 200, 120), format!("Mando conectado: {n}")),
        _ => ui.label("Conecta un mando (Xbox, PlayStation…) por USB o Bluetooth."),
    };
    ui.add_space(6.0);
    ui.group(|ui| {
        ui.strong("Controles fijos");
        ui.label("Stick izquierdo: mueve el droide.");
        ui.label("Stick derecho (eje X): gira la cúpula; al soltarlo vuelve al centro.");
        ui.label("RT: sube la velocidad máxima · LT: la baja.");
        ui.label("En PlayStation: Cruz = A, Círculo = B, Cuadrado = X, Triángulo = Y, R2/L2 = RT/LT.");
    });
    ui.add_space(6.0);
    ui.strong("Botones");
    let mut change: Option<(PadButton, Option<PadAction>)> = None;
    for b in PadButton::ALL {
        let current = pad.mapping.get(&b).copied();
        ui.horizontal(|ui| {
            ui.label(format!("{:<28}", b.title()));
            egui::ComboBox::from_id_salt(b.title())
                .selected_text(current.map_or("Sin asignar", |a| a.title()))
                .width(320.0)
                .show_ui(ui, |ui| {
                    if ui.selectable_label(current.is_none(), "Sin asignar").clicked() {
                        change = Some((b, None));
                    }
                    for a in PadAction::ALL {
                        if ui.selectable_label(current == Some(a), a.title()).clicked() {
                            change = Some((b, Some(a)));
                        }
                    }
                });
        });
    }
    if let Some((b, a)) = change {
        pad.set(b, a);
    }
    ui.add_space(6.0);
    if ui.button("Restablecer valores (A = 3 patas, B = 2 patas, Y = contoneo, X = parar)").clicked() {
        pad.reset();
    }
}

// ---------------------------------------------------------------------------- conexión

fn connect_tab(
    ui: &mut egui::Ui,
    backend: &Backend,
    entries: &mut Vec<Entry>,
    active: &mut usize,
    found: &[Found],
    scanning: bool,
    bt_error: Option<&str>,
) {
    if let Some(err) = bt_error {
        ui.colored_label(Color32::from_rgb(220, 80, 80), err);
    }
    ui.horizontal(|ui| {
        if ui.add_enabled(!scanning, egui::Button::new(if scanning { "Buscando…" } else { "Buscar droides" })).clicked() {
            backend.start_scan();
        }
        ui.label("Busca BB-8, R2-D2, BB-9E y R2-Q5. Despierta al droide sacándolo del cargador.");
    });

    if !entries.is_empty() {
        ui.separator();
        ui.strong("Conectados");
        let mut remove: Option<usize> = None;
        for (i, e) in entries.iter_mut().enumerate() {
            ui.horizontal(|ui| {
                ui.label(format!("{} · {} — {}", e.droid.name, e.droid.kind.title(), e.droid.status));
                match e.droid.link {
                    Link::Disconnected => {
                        if ui.button("Quitar").clicked() {
                            remove = Some(i);
                        }
                    }
                    _ => {
                        if ui.button("Desconectar").clicked() {
                            e.droid.disconnect();
                        }
                    }
                }
            });
        }
        if let Some(i) = remove {
            entries.remove(i);
        }
    }

    ui.separator();
    ui.strong("Encontrados");
    if found.is_empty() {
        ui.label("Nada todavía");
    }
    let mut connect: Option<(String, String, Kind)> = None;
    for f in found {
        ui.horizontal(|ui| {
            ui.label(format!("{} · {}   {}", f.name, f.kind.title(), f.rssi.map_or(String::new(), |r| format!("{r} dBm"))));
            let already = entries.iter().any(|e| e.droid.id == f.id && e.droid.link != Link::Disconnected);
            if ui.add_enabled(!already, egui::Button::new("Conectar")).clicked() {
                connect = Some((f.id.clone(), f.name.clone(), f.kind));
            }
        });
    }
    if let Some((id, name, kind)) = connect {
        let tx = backend.connect(id.clone(), kind);
        entries.retain(|e| e.droid.id != id);
        entries.push(Entry { droid: Droid::new(id, name, kind, tx), ui: DroidUi::new() });
        *active = entries.len() - 1;
    }
}

// ---------------------------------------------------------------------------- conducir

fn pad_widget(ui: &mut egui::Ui) -> Option<Vec2> {
    let size = 200.0;
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(size), Sense::drag());
    let (c, r) = (rect.center(), size / 2.0);
    let painter = ui.painter();
    painter.circle_filled(c, r, Color32::from_gray(40));
    painter.circle_stroke(c, r, Stroke::new(2.0, Color32::GRAY));
    let mut v = None;
    let mut knob = c;
    if resp.dragged() || resp.is_pointer_button_down_on() {
        if let Some(pos) = resp.interact_pointer_pos() {
            let d = (pos - c) / r;
            let m = d.length();
            let d = if m > 1.0 { d / m } else { d };
            v = Some(d);
            knob = c + d * r;
        }
    }
    painter.circle_filled(knob, 22.0, Color32::from_rgb(110, 170, 230));
    v
}

fn drive_tab(ui: &mut egui::Ui, e: &mut Entry, now: Instant, pad_connected: bool, mode: &mut InputMode) {
    if e.droid.kind.has_dome() {
        ui.horizontal(|ui| {
            ui.label("Patas:");
            for a in LegAction::ALL {
                if ui.button(a.title()).clicked() {
                    e.droid.leg_action(a);
                }
            }
        });
    }

    // Con mando conectado manda el mando; si no, el usuario elige teclado o joystick con el ratón.
    ui.horizontal(|ui| {
        if pad_connected {
            ui.colored_label(Color32::from_rgb(90, 200, 120), "Mando conectado: stick izquierdo mueve el droide, derecho gira la cúpula, RT/LT cambian la velocidad.");
        } else {
            ui.label("Control:");
            ui.radio_value(mode, InputMode::Mouse, "Joystick con el ratón");
            ui.radio_value(mode, InputMode::Keyboard, "Teclado (WASD / flechas)");
        }
    });
    let use_mouse = !pad_connected && *mode == InputMode::Mouse;
    let use_keys = !pad_connected && *mode == InputMode::Keyboard;

    ui.horizontal(|ui| {
        let pad = if use_mouse { pad_widget(ui) } else { None };
        ui.vertical(|ui| {
            if use_mouse {
                ui.label("Arrastra el círculo para mover el droide.");
            } else if use_keys {
                ui.label("W avanza, S retrocede, A y D giran (también las flechas).");
            }
            ui.checkbox(&mut e.ui.turbo, "Turbo");
            ui.add(egui::Slider::new(&mut e.ui.max_speed, 0.15..=1.0).text("velocidad máxima"));
        });

        // Entrada: círculo táctil o teclado.
        let dt = ui.input(|i| i.stable_dt);
        let typing = ui.ctx().egui_wants_keyboard_input();
        let key = |k: egui::Key| use_keys && !typing && ui.input(|i| i.key_down(k));
        let cap = if e.ui.turbo { 1.0 } else { e.ui.max_speed };
        let mut cmd: Option<(i32, i32, bool)> = None;
        if let Some(v) = pad.filter(|v| v.length() > 0.1) {
            e.ui.heading = gamepad::heading_of(v.x, -v.y) as f32;
            cmd = Some(((v.length().min(1.0) * 255.0 * cap) as i32, e.ui.heading as i32, false));
        } else {
            let (fwd, back) = (key(egui::Key::W) || key(egui::Key::ArrowUp), key(egui::Key::S) || key(egui::Key::ArrowDown));
            let (left, right) = (key(egui::Key::A) || key(egui::Key::ArrowLeft), key(egui::Key::D) || key(egui::Key::ArrowRight));
            if left {
                e.ui.heading = (e.ui.heading - 120.0 * dt).rem_euclid(360.0);
            }
            if right {
                e.ui.heading = (e.ui.heading + 120.0 * dt).rem_euclid(360.0);
            }
            if fwd || back {
                cmd = Some(((255.0 * cap) as i32, e.ui.heading as i32, back));
            } else if left || right {
                cmd = Some((0, e.ui.heading as i32, false));
            }
        }
        match cmd {
            Some((speed, heading, reverse)) => {
                e.ui.driving = true;
                if now.duration_since(e.ui.last_drive) >= Duration::from_millis(100) {
                    e.ui.last_drive = now;
                    e.droid.drive(speed, heading, reverse, now);
                }
            }
            None if e.ui.driving => {
                e.ui.driving = false;
                e.droid.stop(e.ui.heading as i32, now);
            }
            None => {}
        }
    });

    ui.add_space(8.0);
    ui.label(format!("Rumbo: {}°", e.ui.heading as i32));
    if ui.checkbox(&mut e.ui.stabilize, "Estabilización").changed() {
        e.droid.set_stabilization(e.ui.stabilize);
    }
    if !e.droid.kind.uses_v2() && ui.button("Impulso (boost)").clicked() {
        e.droid.boost_v1(e.ui.heading as i32);
    }

    ui.group(|ui| {
        ui.strong("Apuntar");
        if ui.checkbox(&mut e.ui.aiming, "Modo apuntado").changed() {
            e.droid.set_aim_light(e.ui.aiming);
            e.ui.aim_heading = 0.0;
        }
        if e.ui.aiming {
            ui.label("Gira el droide hasta que mire hacia donde quieres que sea «adelante».");
            if ui.add(egui::Slider::new(&mut e.ui.aim_heading, 0.0..=359.0)).changed() {
                e.droid.aim(e.ui.aim_heading as i32, now);
            }
            if ui.button("Fijar como adelante").clicked() {
                e.droid.set_forward();
                e.ui.aiming = false;
                e.droid.set_aim_light(false);
            }
        }
    });

    if e.droid.kind.has_dome() {
        ui.group(|ui| {
            ui.strong(format!("Cúpula: {}°", e.ui.dome as i32));
            if ui.add(egui::Slider::new(&mut e.ui.dome, -160.0..=180.0)).changed() {
                e.droid.set_dome(e.ui.dome);
            }
        });
    }
}

// ---------------------------------------------------------------------------- emociones y sonidos

fn emotes_tab(ui: &mut egui::Ui, e: &mut Entry, now: Instant) {
    let anims = e.droid.kind.animations();
    if anims.is_empty() {
        ui.label("Las animaciones de BB-8 son programas internos de la app original y no están incluidas.");
        return;
    }
    if ui.button("Parar animación").clicked() {
        e.droid.stop_animation();
    }
    let busy = e.droid.busy(now);
    if busy {
        ui.label("Reproduciendo…");
    }
    let order = ["Emociones", "Reposo", "Patrulla", "Reacciones", "Reacciones (Watch With Me)", "Otras"];
    for title in order {
        let items: Vec<_> = anims.iter().filter(|a| a.category_title() == title).collect();
        if items.is_empty() {
            continue;
        }
        egui::CollapsingHeader::new(format!("{title} ({})", items.len())).default_open(title == "Emociones").show(ui, |ui| {
            for a in items {
                ui.horizontal(|ui| {
                    if ui.add_enabled(!busy, egui::Button::new(a.title())).clicked() {
                        e.droid.play_animation(a, now);
                    }
                    ui.weak(format!("{:.1} s", a.seconds));
                });
            }
        });
    }
}

/// Los dos modos automáticos. Se usan en la pestaña Automático y en Sonidos (comparten el mismo estado).
fn auto_sections(ui: &mut egui::Ui, e: &mut Entry, now: Instant) {
    let kind = e.droid.kind;
    if kind.animations().is_empty() {
        ui.label("BB-8 no tiene animaciones en esta app.");
        return;
    }
    for (mode, title) in [(LoopMode::Wwm, "Vamos a ver una película"), (LoopMode::Emotions, "Haz monerías")] {
        ui.group(|ui| {
            let count = e.ui.emote_loop.pool(kind, mode).len();
            ui.strong(format!("{title} · {count} emotes"));
            let interval = if mode == LoopMode::Wwm { &mut e.ui.emote_loop.wwm_interval } else { &mut e.ui.emote_loop.emotions_interval };
            ui.add(egui::Slider::new(interval, 3.0..=120.0).step_by(1.0).text("segundos entre emotes"));
            if mode == LoopMode::Emotions && kind.animations().iter().any(|a| a.is_ion_blast()) {
                ui.checkbox(&mut e.ui.emote_loop.include_ion_blast, "Incluir Ion Blast");
            }
            let on = e.ui.emote_loop.running == Some(mode);
            let fill = if on { Color32::from_rgb(170, 50, 50) } else { Color32::from_rgb(50, 110, 180) };
            if ui.add(egui::Button::new(if on { "Apagar" } else { "Iniciar" }).fill(fill)).clicked() {
                if on {
                    e.ui.emote_loop.stop();
                } else {
                    e.ui.emote_loop.start(mode, now);
                }
            }
        });
    }
    if e.ui.emote_loop.running.is_some() {
        let last = if e.ui.emote_loop.last_played.is_empty() { "—" } else { &e.ui.emote_loop.last_played };
        ui.label(format!("Último: {last}"));
    }
    ui.small(
        "«Vamos a ver una película» usa los emotes de Watch With Me; «Haz monerías» usa los de Emociones. \
         Elige un emote al azar cada cierto tiempo; si dura más que el intervalo, espera a que acabe. \
         Mover el droide cancela el emote en curso. Al apagar, el que está sonando termina solo.",
    );
}

fn auto_tab(ui: &mut egui::Ui, e: &mut Entry, now: Instant) {
    auto_sections(ui, e, now);
}

/// Orden natural: "Hit_2" antes que "Hit_10".
fn natural_key(s: &str) -> (String, u32) {
    let digits = s.chars().rev().take_while(|c| c.is_ascii_digit()).count();
    let split = s.len() - digits;
    (s[..split].to_string(), s[split..].parse().unwrap_or(0))
}

fn sounds_tab(ui: &mut egui::Ui, e: &mut Entry, now: Instant) {
    if !e.droid.kind.has_audio() {
        ui.label("BB-8 reproducía sus sonidos desde el teléfono con el motor Wwise de la app original, que no está incluido.");
        return;
    }
    ui.horizontal(|ui| {
        ui.label("Volumen");
        let r = ui.add(egui::Slider::new(&mut e.ui.volume, 0.0..=255.0));
        if r.drag_stopped() || (r.changed() && !r.dragged()) {
            e.droid.set_volume(e.ui.volume as u8);
        }
        if ui.button("Parar audio").clicked() {
            e.droid.stop_audio();
        }
    });
    let mut chatter = e.droid.chatter;
    if ui.checkbox(&mut chatter, "Charla aleatoria").changed() {
        e.droid.set_chatter(chatter, now);
    }
    ui.separator();
    egui::CollapsingHeader::new("Modos automáticos").default_open(true).show(ui, |ui| auto_sections(ui, e, now));
    ui.separator();
    if e.droid.kind == Kind::R2q5 {
        ui.small("El R2-Q5 usa aquí la lista de sonidos del R2-D2; si alguno no suena, avísanos.");
    }
    ui.horizontal(|ui| {
        ui.label("Buscar sonido:");
        ui.text_edit_singleline(&mut e.ui.search);
    });
    let query = e.ui.search.to_lowercase();
    let mut groups: BTreeMap<String, Vec<&Sound>> = BTreeMap::new();
    for s in e.droid.kind.sounds() {
        if query.is_empty() || s.title().to_lowercase().contains(&query) {
            groups.entry(s.group_title()).or_default().push(s);
        }
    }
    for (title, mut items) in groups {
        items.sort_by_key(|s| natural_key(s.name));
        egui::CollapsingHeader::new(format!("{title} ({})", items.len())).id_salt(&title).default_open(!query.is_empty()).show(ui, |ui| {
            for s in items {
                if ui.button(s.title()).clicked() {
                    e.droid.play_sound(s);
                }
            }
        });
    }
}

// ---------------------------------------------------------------------------- seguir, patrulla, dibujar

fn follow_tab(ui: &mut egui::Ui, e: &mut Entry, now: Instant) {
    ui.label(format!("Señal actual: {}", e.ui.follow.smoothed.map_or("—".into(), |s| format!("{} dBm", s as i32))));
    ui.label(format!("Estado: {}", e.ui.follow.status));
    let on = e.ui.follow.running;
    let fill = if on { Color32::from_rgb(170, 50, 50) } else { Color32::from_rgb(50, 110, 180) };
    if ui.add(egui::Button::new(if on { "Dejar de seguir" } else { "Seguir al ordenador" }).fill(fill)).clicked() {
        if on {
            e.ui.follow.stop(&mut e.droid, now);
        } else {
            e.ui.follow.start(&mut e.droid, now);
        }
    }
    ui.separator();
    ui.add(egui::Slider::new(&mut e.ui.follow.near_threshold, -80.0..=-35.0).step_by(1.0).text("dBm para considerar «cerca»"));
    if ui.add_enabled(e.ui.follow.smoothed.is_some(), egui::Button::new("Usar la señal actual como «cerca»")).clicked() {
        e.ui.follow.calibrate_near();
    }
    ui.add(egui::Slider::new(&mut e.ui.follow.speed, 30.0..=120.0).text("velocidad"));
    ui.small("Para calibrar: coloca el droide junto al ordenador, pulsa «Seguir» y luego «Usar la señal actual».");
    ui.separator();
    ui.label(
        "El droide solo conoce la intensidad de la señal Bluetooth, no la dirección. Avanza despacio y, si la señal \
         empeora poco a poco, corrige el rumbo con giros suaves; solo da media vuelta si la señal cae de golpe. \
         Si la señal baja de -85 dBm se para para no perderse. Es aproximado: úsalo en un espacio despejado y vigílalo. \
         En Windows la señal solo se actualiza mientras hay un escaneo activo.",
    );
}

fn patrol_tab(ui: &mut egui::Ui, e: &mut Entry, now: Instant) {
    ui.add(egui::Slider::new(&mut e.ui.patrol.speed, 30.0..=150.0).text("velocidad"));
    let on = e.ui.patrol.running;
    let fill = if on { Color32::from_rgb(170, 50, 50) } else { Color32::from_rgb(50, 110, 180) };
    if ui.add(egui::Button::new(if on { "Detener patrulla" } else { "Iniciar patrulla" }).fill(fill)).clicked() {
        if on {
            e.ui.patrol.stop(&mut e.droid, now);
        } else {
            e.ui.patrol.start(&mut e.droid, now);
        }
    }
    ui.small("El droide avanza solo, retrocede y gira al chocar, y de vez en cuando lanza una alarma. Déjalo en un espacio despejado.");
    ui.separator();
    ui.strong("Registro de eventos");
    for line in &e.ui.patrol.entries {
        ui.monospace(line);
    }
}

fn draw_tab(ui: &mut egui::Ui, e: &mut Entry, now: Instant) {
    let size = Vec2::new(ui.available_width(), 320.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::drag());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 6.0, Color32::from_gray(30));
    painter.text(rect.center_top() + Vec2::new(0.0, 6.0), egui::Align2::CENTER_TOP, "▲ delante del droide", egui::FontId::proportional(12.0), Color32::GRAY);
    if (resp.dragged() || resp.drag_started()) && !e.ui.draw.running && e.ui.draw.points.len() < 2000 {
        if let Some(p) = resp.interact_pointer_pos() {
            e.ui.draw.points.push((p.x - rect.min.x, p.y - rect.min.y));
        }
    }
    let at = |p: &(f32, f32)| rect.min + Vec2::new(p.0, p.1);
    for w in e.ui.draw.points.windows(2) {
        painter.line_segment([at(&w[0]), at(&w[1])], Stroke::new(3.0, Color32::from_rgb(110, 170, 230)));
    }
    if let Some(p) = e.ui.draw.points.first() {
        painter.circle_filled(at(p), 7.0, Color32::from_rgb(90, 200, 120));
    }
    ui.add(egui::ProgressBar::new(e.ui.draw.progress));
    ui.horizontal(|ui| {
        let on = e.ui.draw.running;
        let fill = if on { Color32::from_rgb(170, 50, 50) } else { Color32::from_rgb(50, 110, 180) };
        if ui.add_enabled(on || e.ui.draw.points.len() >= 2, egui::Button::new(if on { "Detener" } else { "Ejecutar recorrido" }).fill(fill)).clicked() {
            if on {
                e.ui.draw.abort(&mut e.droid, now);
            } else {
                e.ui.draw.run(&mut e.droid, now);
            }
        }
        if ui.button("Borrar").clicked() {
            e.ui.draw.clear(&mut e.droid, now);
        }
    });
    egui::CollapsingHeader::new("Calibración").show(ui, |ui| {
        ui.add(egui::Slider::new(&mut e.ui.draw.speed, 40.0..=160.0).text("velocidad"));
        ui.add(egui::Slider::new(&mut e.ui.draw.cm_per_second, 10.0..=100.0).text("cm/s a esa velocidad"));
        ui.add(egui::Slider::new(&mut e.ui.draw.cm_per_point, 0.1..=1.0).text("cm por punto del dibujo"));
    });
}

// ---------------------------------------------------------------------------- luces, energía, ajustes, log

fn lights_tab(ui: &mut egui::Ui, e: &mut Entry) {
    for ch in e.droid.kind.lights() {
        match ch.style {
            LightStyle::Rgb(..) | LightStyle::V1Main => {
                let rgb = e.ui.light_rgb.entry(ch.title).or_insert([0, 0, 255]);
                ui.horizontal(|ui| {
                    ui.label(ch.title);
                    if ui.color_edit_button_srgb(rgb).changed() {
                        let v = *rgb;
                        e.droid.set_light_rgb(&ch, v);
                    }
                });
            }
            LightStyle::Mono(_) | LightStyle::V1Back => {
                let level = e.ui.light_level.entry(ch.title).or_insert(0.0);
                if ui.add(egui::Slider::new(level, 0.0..=255.0).text(ch.title)).changed() {
                    let v = *level as u8;
                    e.droid.set_light_level(&ch, v);
                }
            }
        }
    }
    if e.droid.kind.uses_v2() && ui.button("Restablecer luces").clicked() {
        e.droid.reset_lights();
    }
}

fn energy_tab(ui: &mut egui::Ui, e: &mut Entry) {
    ui.strong("Energía");
    if ui.button("Despertar").clicked() {
        e.droid.wake();
    }
    if ui.button("Dormir").clicked() {
        e.droid.sleep(false);
    }
    if e.droid.kind.uses_v2() && ui.button("Sueño profundo").clicked() {
        e.droid.sleep(true);
    }
    if ui.add(egui::Button::new("Desconectar").fill(Color32::from_rgb(170, 50, 50))).clicked() {
        e.droid.disconnect();
    }
}

fn settings_tab(ui: &mut egui::Ui, e: &mut Entry) {
    let kind = e.droid.kind;
    ui.strong("Droide");
    ui.label(format!("Nombre: {}", e.droid.name));
    ui.label(format!("Modelo: {}", kind.title()));
    ui.label(format!("Batería: {} · {}", e.droid.battery.text(), e.droid.battery_text));
    if ui.button("Actualizar batería").clicked() {
        e.droid.request_battery();
    }
    if kind.uses_v2() {
        ui.label(format!("Firmware Nordic: {}", e.droid.versions.get("Nordic").map_or("—", |s| s.as_str())));
        ui.label(format!("Firmware principal: {}", e.droid.versions.get("Principal").map_or("—", |s| s.as_str())));
        if ui.button("Leer versiones").clicked() {
            e.droid.request_versions();
        }
        ui.label(format!("Direcciones: {}", e.droid.routing_text));
    }
    ui.label(format!("Firmware en el APK: {}", kind.bundled_firmware()));

    if kind.uses_v2() {
        ui.separator();
        ui.strong("Comportamiento autónomo");
        let before = e.ui.interactions;
        egui::ComboBox::from_label("Interacciones").selected_text(before.title()).show_ui(ui, |ui| {
            for m in Interactions::ALL {
                ui.selectable_value(&mut e.ui.interactions, m, m.title());
            }
        });
        if e.ui.interactions != before {
            e.droid.set_interactions(e.ui.interactions);
        }
        let mut trophy = e.droid.trophy_mode;
        if ui.checkbox(&mut trophy, "Modo trofeo").changed() {
            e.droid.set_trophy_mode(trophy);
        }
        if kind.has_dome() {
            let mut tracking = e.droid.head_tracking;
            if ui.checkbox(&mut tracking, "La cúpula sigue al movimiento").changed() {
                e.droid.set_head_tracking(tracking);
            }
        }
    }

    ui.separator();
    ui.checkbox(&mut e.droid.react_to_hits, "Reaccionar a los golpes");

    if kind.uses_v2() {
        ui.separator();
        ui.strong("Renombrar");
        ui.text_edit_singleline(&mut e.ui.rename);
        let prefix = format!("{}-", kind.prefix());
        let valid = e.ui.rename.len() == prefix.len() + 4
            && e.ui.rename.starts_with(&prefix)
            && e.ui.rename[prefix.len()..].chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit());
        ui.small(format!("Debe conservar el prefijo {prefix} y tener 4 letras o números; si no, la app no volverá a encontrarlo."));
        if ui.add_enabled(valid, egui::Button::new("Cambiar nombre Bluetooth")).clicked() {
            let name = e.ui.rename.clone();
            e.droid.rename(&name);
        }
    }
    ui.separator();
    ui.small(
        "No incluido: actualización de firmware (OTA), realidad aumentada, tienda, Force Band y sincronía de Watch With Me \
         con películas. Dependen de servicios o recursos de la app original, o pueden dejar el droide inservible si fallan.",
    );
}

fn log_tab(ui: &mut egui::Ui, e: &mut Entry) {
    ui.horizontal(|ui| {
        if ui.button("Copiar log").clicked() {
            let head = format!(
                "Droide: {} ({})\nDirecciones: {}\nVersiones: {:?}\nBatería: {} {}\n---\n",
                e.droid.name, e.droid.kind.title(), e.droid.routing_text, e.droid.versions, e.droid.battery.text(), e.droid.battery_text
            );
            let body: Vec<&str> = e.droid.log.iter().map(|s| s.as_str()).collect();
            ui.ctx().copy_text(head + &body.join("\n"));
        }
        if ui.button("Batería").clicked() {
            e.droid.request_battery();
        }
        if ui.button("Despertar").clicked() {
            e.droid.wake();
        }
        if ui.button("Borrar").clicked() {
            e.droid.log.clear();
        }
    });
    ui.separator();
    for line in &e.droid.log {
        ui.monospace(line);
    }
}

#[allow(dead_code)]
fn _title_is_used(raw: &str) -> String {
    translate::title(raw)
}
