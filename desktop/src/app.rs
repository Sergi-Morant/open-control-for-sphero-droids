//! Interfaz egui: barra lateral con una pantalla por función, droides arriba y parada siempre a mano abajo.

use std::collections::{BTreeMap, HashMap};
use std::sync::mpsc::{channel, Receiver};
use std::time::{Duration, Instant};

use egui::{Color32, Margin, RichText, Sense, Stroke, Vec2};

use crate::backend::{Backend, Event};
use crate::controllers::{DrawDrive, EmoteLoop, Follow, LoopMode, Patrol, FOLLOW_HISTORY};
use crate::droid::{Droid, Interactions, LegAction, Link};
use crate::gamepad::{self, Frame, Pad, PadAction, PadButton};
use crate::kind::{Kind, LightStyle, Sound};
use crate::theme;
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
    Settings,
    Log,
}

/// Navegación de la barra lateral, por grupos.
const NAV: [(&str, &[(Tab, &str)]); 3] = [
    ("Droide", &[(Tab::Drive, "Conducir"), (Tab::Emotes, "Emociones"), (Tab::Sounds, "Sonidos"), (Tab::Lights, "Luces")]),
    ("Automático", &[(Tab::Auto, "Modos automáticos"), (Tab::Follow, "Seguir"), (Tab::Patrol, "Patrulla"), (Tab::Draw, "Dibujar recorrido")]),
    ("Sistema", &[(Tab::Connect, "Droides"), (Tab::Gamepad, "Mando"), (Tab::Settings, "Ajustes"), (Tab::Log, "Registro")]),
];

impl Tab {
    fn title(self) -> &'static str {
        NAV.iter().flat_map(|(_, items)| items.iter()).find(|(t, _)| *t == self).map_or("", |(_, name)| name)
    }

    /// Pantallas que tienen sentido sin ningún droide conectado.
    fn works_without_droid(self) -> bool {
        matches!(self, Tab::Connect | Tab::Gamepad)
    }
}

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
    pad_dome_dirty: bool,
    pad_aiming: bool,
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
            pad_dome_dirty: false,
            pad_aiming: false,
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
        theme::apply(&cc.egui_ctx);
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
                    e.droid.rssi_at = Some(now);
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
        if entries.is_empty() && !tab.works_without_droid() {
            *tab = Tab::Connect;
        }
        *active = (*active).min(entries.len().saturating_sub(1));

        // Esc para todo, salvo que se esté escribiendo en un campo de texto.
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) && !ui.ctx().egui_wants_keyboard_input() {
            if let Some(e) = entries.get_mut(*active) {
                e.stop_all(now);
            }
        }

        // Barra lateral: navegación agrupada.
        let side = egui::Frame::new().fill(theme::PANEL).inner_margin(Margin::same(12));
        egui::Panel::left("nav").exact_size(200.0).resizable(false).show_separator_line(false).frame(side).show(ui, |ui| {
            ui.label(RichText::new("Open control").strong().size(19.0));
            ui.label(RichText::new("for Sphero Droids").color(theme::MUTED).size(12.5));
            ui.add_space(12.0);
            for (group, items) in NAV {
                theme::section(ui, group);
                for &(t, name) in items {
                    let enabled = !entries.is_empty() || t.works_without_droid();
                    let button = egui::Button::selectable(*tab == t, name).min_size(Vec2::new(ui.available_width(), 36.0));
                    if ui.add_enabled(enabled, button).clicked() {
                        *tab = t;
                    }
                }
                ui.add_space(10.0);
            }
            ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                if *pad_frame_connected {
                    theme::pill(ui, theme::GREEN, "Mando conectado");
                } else {
                    theme::pill(ui, theme::BORDER, "Sin mando");
                }
            });
        });

        // Arriba: los droides conectados.
        let top = egui::Frame::new().fill(theme::BG).inner_margin(Margin::symmetric(20, 10));
        egui::Panel::top("droids").show_separator_line(false).frame(top).show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                for (i, e) in entries.iter().enumerate() {
                    let color = link_color(e.droid.link);
                    let text = format!("      {} · {}", e.droid.kind.title(), e.droid.battery.text());
                    let r = ui.add(egui::Button::selectable(i == *active, text)).on_hover_text(format!("{}: {}", e.droid.name, e.droid.status));
                    ui.painter().circle_filled(r.rect.left_center() + Vec2::new(16.0, 0.0), 5.0, color);
                    if r.clicked() {
                        *active = i;
                    }
                }
                if entries.is_empty() {
                    ui.label(RichText::new("Ningún droide conectado").color(theme::MUTED));
                } else if ui.button("+ Añadir droide").clicked() {
                    *tab = Tab::Connect;
                }
            });
        });

        // Abajo: parada de emergencia, lo que está en marcha y el estado.
        let bottom = egui::Frame::new().fill(theme::PANEL).inner_margin(Margin::symmetric(20, 10));
        egui::Panel::bottom("bottom").show_separator_line(false).frame(bottom).show(ui, |ui| {
            ui.horizontal(|ui| {
                let Some(e) = entries.get_mut(*active) else {
                    ui.label(RichText::new("Busca y conecta un droide para empezar.").color(theme::MUTED));
                    return;
                };
                if ui.add(theme::danger("PARAR TODO").min_size(Vec2::new(150.0, 42.0))).on_hover_text("Detiene movimiento, modos automáticos, animaciones y sonido (tecla Esc)").clicked() {
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
                if e.ui.patrol.running && ui.button("Patrullando  ✕").clicked() {
                    e.ui.patrol.stop(&mut e.droid, now);
                }
                if e.ui.draw.running && ui.button("Recorrido  ✕").clicked() {
                    e.ui.draw.abort(&mut e.droid, now);
                }
                if e.droid.chatter && ui.button("Charla aleatoria  ✕").clicked() {
                    e.droid.set_chatter(false, now);
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(RichText::new(format!("{}: {}", e.droid.name, e.droid.status)).color(theme::MUTED));
                });
            });
        });

        let center = egui::Frame::new().fill(theme::BG).inner_margin(Margin::symmetric(20, 8));
        egui::CentralPanel::default().frame(center).show(ui, |ui| {
            ui.heading(tab.title());
            ui.add_space(4.0);
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                match *tab {
                    Tab::Connect => return connect_tab(ui, backend, entries, active, tab, found, *scanning, bt_error.as_deref()),
                    Tab::Gamepad => return gamepad_tab(ui, pad, *pad_frame_connected),
                    _ => {}
                }
                let Some(e) = entries.get_mut(*active) else { return };
                if !e.droid.is_ready() {
                    theme::card(ui, "", |ui| {
                        ui.horizontal(|ui| {
                            if e.droid.link == Link::Connecting {
                                ui.spinner();
                            }
                            ui.colored_label(theme::AMBER, format!("{}: {}", e.droid.name, e.droid.status));
                        });
                    });
                }
                match *tab {
                    Tab::Drive => drive_tab(ui, e, now, *pad_frame_connected, input_mode),
                    Tab::Emotes => emotes_tab(ui, e, now),
                    Tab::Auto => auto_tab(ui, e, now),
                    Tab::Sounds => sounds_tab(ui, e, now),
                    Tab::Follow => follow_tab(ui, e, now),
                    Tab::Patrol => patrol_tab(ui, e, now),
                    Tab::Draw => draw_tab(ui, e, now),
                    Tab::Lights => lights_tab(ui, e),
                    Tab::Settings => settings_tab(ui, e),
                    Tab::Log => log_tab(ui, e),
                    Tab::Connect | Tab::Gamepad => {}
                }
                ui.add_space(8.0);
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

fn link_color(link: Link) -> Color32 {
    match link {
        Link::Ready => theme::GREEN,
        Link::Connecting => theme::AMBER,
        Link::Disconnected => theme::MUTED,
    }
}

/// Fila con contenido a la izquierda y acciones pegadas a la derecha.
fn row(ui: &mut egui::Ui, left: impl FnOnce(&mut egui::Ui), right: impl FnOnce(&mut egui::Ui)) {
    ui.horizontal(|ui| {
        left(ui);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), right);
    });
}

fn muted(text: impl Into<String>) -> RichText {
    RichText::new(text).color(theme::MUTED)
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

    // Stick izquierdo: mueve el droide; en modo apuntado lo orienta sin avanzar.
    let (lx, ly) = f.left;
    let mag = lx.hypot(ly);
    if e.ui.pad_aiming {
        if mag > 0.0 {
            let heading = gamepad::heading_of(lx, ly);
            e.ui.heading = heading as f32;
            if now.duration_since(e.ui.last_drive) >= Duration::from_millis(100) {
                e.ui.last_drive = now;
                e.droid.aim(heading, now);
            }
        }
    } else if mag > 0.0 {
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

    // Stick derecho (eje X): la cúpula gira poco a poco y se queda donde se suelta.
    if e.droid.kind.has_dome() {
        if f.right_x != 0.0 {
            let target = gamepad::dome_step(e.ui.dome, f.right_x, dt);
            if target != e.ui.dome {
                e.ui.dome = target;
                e.ui.pad_dome_dirty = true;
            }
        }
        if e.ui.pad_dome_dirty && now.duration_since(e.ui.last_dome_sent) >= Duration::from_millis(100) {
            e.ui.last_dome_sent = now;
            e.ui.pad_dome_dirty = false;
            e.droid.set_dome(e.ui.dome);
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
        PadAction::AimToggle => toggle_aim(e, now),
        PadAction::DomeCenter => {
            e.ui.dome = 0.0;
            e.ui.pad_dome_dirty = false;
            e.droid.set_dome(0.0);
        }
    }
}

/// Primera pulsación: modo apuntado (luz de apuntado y el stick orienta el droide).
/// Segunda: la orientación actual se fija como «adelante».
fn toggle_aim(e: &mut Entry, now: Instant) {
    if e.ui.pad_aiming {
        e.droid.set_forward();
        e.droid.set_aim_light(false);
        e.ui.pad_aiming = false;
        e.ui.heading = 0.0;
    } else {
        if e.ui.pad_driving {
            e.ui.pad_driving = false;
            e.droid.stop(e.ui.heading as i32, now);
        }
        e.ui.pad_aiming = true;
        e.droid.set_aim_light(true);
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
    theme::card(ui, "Estado", |ui| {
        if !pad.available() {
            ui.colored_label(theme::AMBER, "No se pudo iniciar la lectura de mandos en este sistema.");
        }
        match (&pad.name, connected) {
            (Some(n), true) => theme::pill(ui, theme::GREEN, &format!("Conectado: {n}")),
            _ => {
                ui.label("Conecta un mando (Xbox, PlayStation…) por USB o Bluetooth.");
                ui.label(muted("En Steam Deck: añade la app a Steam como juego que no es de Steam para que los controles lleguen como mando."));
            }
        }
    });
    ui.columns(2, |cols| {
        theme::card(&mut cols[0], "Controles fijos", |ui| {
            egui::Grid::new("pad_fixed").num_columns(2).spacing([14.0, 8.0]).show(ui, |ui| {
                for (k, v) in [
                    ("Stick izquierdo", "Mueve el droide"),
                    ("Stick derecho", "Gira la cúpula poco a poco"),
                    ("Clic stick derecho", "Centra la cúpula"),
                    ("Clic stick izquierdo", "Apuntar; otra vez para fijar «adelante»"),
                    ("RT / LT", "Sube / baja la velocidad máxima"),
                ] {
                    ui.strong(k);
                    ui.label(v);
                    ui.end_row();
                }
            });
            ui.label(muted("PlayStation: Cruz = A, Círculo = B, Cuadrado = X, Triángulo = Y, R2/L2 = RT/LT."));
        });
        theme::card(&mut cols[1], "Botones", |ui| {
            let mut change: Option<(PadButton, Option<PadAction>)> = None;
            egui::Grid::new("pad_map").num_columns(2).spacing([14.0, 8.0]).show(ui, |ui| {
                for b in PadButton::ALL {
                    let current = pad.mapping.get(&b).copied();
                    ui.label(b.title());
                    egui::ComboBox::from_id_salt(b.title()).selected_text(current.map_or("Sin asignar", |a| a.title())).width(200.0).truncate().show_ui(ui, |ui| {
                        if ui.selectable_label(current.is_none(), "Sin asignar").clicked() {
                            change = Some((b, None));
                        }
                        for a in PadAction::ALL {
                            if ui.selectable_label(current == Some(a), a.title()).clicked() {
                                change = Some((b, Some(a)));
                            }
                        }
                    });
                    ui.end_row();
                }
            });
            if let Some((b, a)) = change {
                pad.set(b, a);
            }
            ui.add_space(4.0);
            if ui.button("Restablecer").on_hover_text("A = 3 patas, B = 2 patas, Y = contoneo, X = parar, clics de stick = apuntar y centrar").clicked() {
                pad.reset();
            }
        });
    });
}

// ---------------------------------------------------------------------------- conexión

#[allow(clippy::too_many_arguments)]
fn connect_tab(
    ui: &mut egui::Ui,
    backend: &Backend,
    entries: &mut Vec<Entry>,
    active: &mut usize,
    tab: &mut Tab,
    found: &[Found],
    scanning: bool,
    bt_error: Option<&str>,
) {
    if let Some(err) = bt_error {
        theme::card(ui, "Bluetooth", |ui| {
            ui.colored_label(theme::RED, err);
        });
    }
    theme::card(ui, "", |ui| {
        ui.horizontal(|ui| {
            let label = if scanning { "Buscando…" } else { "Buscar droides" };
            if ui.add_enabled(!scanning, theme::primary(label).min_size(Vec2::new(180.0, 42.0))).clicked() {
                backend.start_scan();
            }
            if scanning {
                ui.spinner();
            }
        });
        ui.label(muted("Busca BB-8, R2-D2, BB-9E y R2-Q5. Despierta al droide sacándolo del cargador."));
    });

    if !entries.is_empty() {
        theme::card(ui, "Conectados", |ui| {
            let mut remove: Option<usize> = None;
            for (i, e) in entries.iter_mut().enumerate() {
                let link = e.droid.link;
                let text = format!("{} · {}", e.droid.name, e.droid.kind.title());
                let status = e.droid.status.clone();
                row(
                    ui,
                    |ui| {
                        theme::dot(ui, link_color(link));
                        ui.strong(text);
                        ui.label(muted(status));
                    },
                    |ui| {
                        if link == Link::Disconnected {
                            if ui.button("Quitar").clicked() {
                                remove = Some(i);
                            }
                        } else if ui.button("Desconectar").clicked() {
                            e.droid.disconnect();
                        }
                    },
                );
            }
            if let Some(i) = remove {
                entries.remove(i);
            }
        });
    }

    let mut connect: Option<(String, String, Kind)> = None;
    theme::card(ui, "Encontrados", |ui| {
        if found.is_empty() {
            ui.label(muted(if scanning { "Buscando droides cercanos…" } else { "Nada todavía. Pulsa «Buscar droides»." }));
        }
        for f in found {
            let already = entries.iter().any(|e| e.droid.id == f.id && e.droid.link != Link::Disconnected);
            row(
                ui,
                |ui| {
                    theme::signal_bars(ui, f.rssi.map(f32::from));
                    ui.strong(format!("{} · {}", f.name, f.kind.title()));
                    if let Some(r) = f.rssi {
                        ui.label(muted(format!("{r} dBm")));
                    }
                },
                |ui| {
                    if already {
                        ui.label(muted("Conectado"));
                    } else if ui.add(theme::primary("Conectar")).clicked() {
                        connect = Some((f.id.clone(), f.name.clone(), f.kind));
                    }
                },
            );
        }
    });
    if let Some((id, name, kind)) = connect {
        let tx = backend.connect(id.clone(), kind);
        entries.retain(|e| e.droid.id != id);
        entries.push(Entry { droid: Droid::new(id, name, kind, tx), ui: DroidUi::new() });
        *active = entries.len() - 1;
        *tab = Tab::Drive;
    }
}

// ---------------------------------------------------------------------------- conducir

/// Joystick en pantalla. Muestra siempre el rumbo actual; solo responde al puntero si `interactive`.
fn joystick(ui: &mut egui::Ui, heading: f32, interactive: bool) -> Option<Vec2> {
    let size = 230.0;
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(size), if interactive { Sense::drag() } else { Sense::hover() });
    let (c, r) = (rect.center(), size / 2.0 - 4.0);
    let painter = ui.painter();
    painter.circle_filled(c, r, ui.visuals().extreme_bg_color);
    painter.circle_stroke(c, r, Stroke::new(2.0, theme::BORDER));
    painter.circle_stroke(c, r * 0.5, Stroke::new(1.0, theme::BORDER));
    painter.line_segment([c - Vec2::new(r, 0.0), c + Vec2::new(r, 0.0)], Stroke::new(1.0, theme::BORDER));
    painter.line_segment([c - Vec2::new(0.0, r), c + Vec2::new(0.0, r)], Stroke::new(1.0, theme::BORDER));
    // Marca del rumbo en el borde.
    let a = heading.to_radians();
    let dir = Vec2::new(a.sin(), -a.cos());
    painter.line_segment([c + dir * (r - 14.0), c + dir * r], Stroke::new(4.0, theme::AMBER));
    let mut v = None;
    let mut knob = c;
    if interactive && (resp.dragged() || resp.is_pointer_button_down_on()) {
        if let Some(pos) = resp.interact_pointer_pos() {
            let d = (pos - c) / r;
            let m = d.length();
            let d = if m > 1.0 { d / m } else { d };
            v = Some(d);
            knob = c + d * r;
        }
    }
    let color = if !interactive { theme::WIDGET_HOVER } else if v.is_some() { theme::ACCENT } else { theme::ACCENT.gamma_multiply(0.8) };
    painter.circle_filled(knob, 26.0, color);
    painter.circle_stroke(knob, 26.0, Stroke::new(2.0, Color32::from_white_alpha(40)));
    v
}

fn drive_tab(ui: &mut egui::Ui, e: &mut Entry, now: Instant, pad_connected: bool, mode: &mut InputMode) {
    // Con mando conectado manda el mando; si no, el usuario elige teclado o joystick en pantalla.
    let use_mouse = !pad_connected && *mode == InputMode::Mouse;
    let use_keys = !pad_connected && *mode == InputMode::Keyboard;
    let mut stick: Option<Vec2> = None;

    ui.columns(2, |cols| {
        theme::card(&mut cols[0], "Movimiento", |ui| {
            if pad_connected {
                theme::pill(ui, theme::GREEN, "Conduciendo con el mando");
            } else {
                ui.horizontal(|ui| {
                    ui.selectable_value(mode, InputMode::Mouse, "Ratón / táctil");
                    ui.selectable_value(mode, InputMode::Keyboard, "Teclado");
                });
            }
            ui.vertical_centered(|ui| {
                stick = joystick(ui, e.ui.heading, use_mouse);
                ui.label(RichText::new(format!("Rumbo {}°", e.ui.heading as i32)).strong());
            });
            ui.label(muted(if pad_connected {
                "Stick izquierdo: mover · stick derecho: cúpula · RT/LT: velocidad."
            } else if use_mouse {
                "Arrastra el círculo para mover el droide."
            } else {
                "W avanza, S retrocede, A y D giran (también las flechas)."
            }));
            if e.ui.pad_aiming {
                ui.colored_label(theme::AMBER, "Modo apuntado: orienta el droide con el stick izquierdo y púlsalo otra vez para fijar «adelante».");
            }
        });

        let ui = &mut cols[1];
        theme::card(ui, "Velocidad", |ui| {
            ui.add(egui::Slider::new(&mut e.ui.max_speed, 0.15..=1.0).custom_formatter(|v, _| format!("{:.0} %", v * 100.0)).text("máxima"));
            ui.horizontal_wrapped(|ui| {
                ui.checkbox(&mut e.ui.turbo, "Turbo");
                if ui.checkbox(&mut e.ui.stabilize, "Estabilización").changed() {
                    e.droid.set_stabilization(e.ui.stabilize);
                }
                if !e.droid.kind.uses_v2() && ui.button("Impulso").clicked() {
                    e.droid.boost_v1(e.ui.heading as i32);
                }
            });
        });
        if e.droid.kind.has_dome() {
            theme::card(ui, "Patas", |ui| {
                ui.horizontal_wrapped(|ui| {
                    for a in LegAction::ALL {
                        if ui.button(a.title()).clicked() {
                            e.droid.leg_action(a);
                        }
                    }
                });
            });
            theme::card(ui, "Cúpula", |ui| {
                ui.horizontal(|ui| {
                    if ui.add(egui::Slider::new(&mut e.ui.dome, -160.0..=180.0).suffix("°").fixed_decimals(0)).changed() {
                        e.droid.set_dome(e.ui.dome);
                    }
                    if ui.button("Centrar").clicked() {
                        e.ui.dome = 0.0;
                        e.droid.set_dome(0.0);
                    }
                });
            });
        }
        theme::card(ui, "Apuntar", |ui| {
            if e.ui.aiming {
                ui.label("Gira el droide hasta que mire hacia donde quieres que sea «adelante».");
                if ui.add(egui::Slider::new(&mut e.ui.aim_heading, 0.0..=359.0).suffix("°").fixed_decimals(0)).changed() {
                    e.droid.aim(e.ui.aim_heading as i32, now);
                }
                ui.horizontal(|ui| {
                    if ui.add(theme::primary("Fijar como adelante")).clicked() {
                        e.droid.set_forward();
                        e.ui.aiming = false;
                        e.droid.set_aim_light(false);
                    }
                    if ui.button("Cancelar").clicked() {
                        e.ui.aiming = false;
                        e.droid.set_aim_light(false);
                    }
                });
            } else {
                ui.label(muted("Si «adelante» no coincide con lo que ves, reoriéntalo."));
                if ui.button("Cambiar «adelante»").clicked() {
                    e.ui.aiming = true;
                    e.ui.aim_heading = 0.0;
                    e.droid.set_aim_light(true);
                }
            }
        });
    });

    // Entrada: joystick en pantalla o teclado.
    let dt = ui.input(|i| i.stable_dt);
    let typing = ui.ctx().egui_wants_keyboard_input();
    let key = |k: egui::Key| use_keys && !typing && ui.input(|i| i.key_down(k));
    let cap = if e.ui.turbo { 1.0 } else { e.ui.max_speed };
    let mut cmd: Option<(i32, i32, bool)> = None;
    if let Some(v) = stick.filter(|v| v.length() > 0.1) {
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
}

// ---------------------------------------------------------------------------- emociones y sonidos

fn emotes_tab(ui: &mut egui::Ui, e: &mut Entry, now: Instant) {
    let anims = e.droid.kind.animations();
    if anims.is_empty() {
        theme::card(ui, "", |ui| {
            ui.label("Las animaciones de BB-8 son programas internos de la app original y no están incluidas.");
        });
        return;
    }
    let busy = e.droid.busy(now);
    ui.horizontal(|ui| {
        if ui.button("Parar animación").clicked() {
            e.droid.stop_animation();
        }
        if busy {
            theme::pill(ui, theme::AMBER, "Reproduciendo…");
        }
    });
    let order = ["Emociones", "Reposo", "Patrulla", "Reacciones", "Reacciones (Watch With Me)", "Otras"];
    for title in order {
        let items: Vec<_> = anims.iter().filter(|a| a.category_title() == title).collect();
        if items.is_empty() {
            continue;
        }
        theme::card(ui, &format!("{title} · {}", items.len()), |ui| {
            ui.horizontal_wrapped(|ui| {
                for a in items {
                    if ui.add_enabled(!busy, egui::Button::new(a.title())).on_hover_text(format!("{:.1} s", a.seconds)).clicked() {
                        e.droid.play_animation(a, now);
                    }
                }
            });
        });
    }
}

fn auto_tab(ui: &mut egui::Ui, e: &mut Entry, now: Instant) {
    let kind = e.droid.kind;
    if kind.animations().is_empty() {
        theme::card(ui, "", |ui| {
            ui.label("BB-8 no tiene animaciones en esta app.");
        });
        return;
    }
    ui.columns(2, |cols| {
        for (col, (mode, title, hint)) in [
            (LoopMode::Wwm, "Vamos a ver una película", "Reacciones de Watch With Me, al azar."),
            (LoopMode::Emotions, "Haz monerías", "Emotes de Emociones, al azar."),
        ]
        .into_iter()
        .enumerate()
        {
            theme::card(&mut cols[col], title, |ui| {
                let count = e.ui.emote_loop.pool(kind, mode).len();
                ui.label(muted(format!("{hint} {count} emotes.")));
                let interval = if mode == LoopMode::Wwm { &mut e.ui.emote_loop.wwm_interval } else { &mut e.ui.emote_loop.emotions_interval };
                ui.add(egui::Slider::new(interval, 3.0..=120.0).step_by(1.0).suffix(" s").text("entre emotes"));
                if mode == LoopMode::Emotions && kind.animations().iter().any(|a| a.is_ion_blast()) {
                    ui.checkbox(&mut e.ui.emote_loop.include_ion_blast, "Incluir Ion Blast");
                }
                let on = e.ui.emote_loop.running == Some(mode);
                if theme::start_stop(ui, on, "Iniciar", "Apagar") {
                    if on {
                        e.ui.emote_loop.stop();
                    } else {
                        e.ui.emote_loop.start(mode, now);
                    }
                }
            });
        }
    });
    if e.ui.emote_loop.running.is_some() {
        let last = if e.ui.emote_loop.last_played.is_empty() { "—" } else { &e.ui.emote_loop.last_played };
        ui.label(format!("Último: {last}"));
    }
    ui.label(muted(
        "Elige un emote al azar cada cierto tiempo; si dura más que el intervalo, espera a que acabe. \
         Mover el droide cancela el emote en curso. Al apagar, el que está sonando termina solo.",
    ));
}

/// Orden natural: "Hit_2" antes que "Hit_10".
fn natural_key(s: &str) -> (String, u32) {
    let digits = s.chars().rev().take_while(|c| c.is_ascii_digit()).count();
    let split = s.len() - digits;
    (s[..split].to_string(), s[split..].parse().unwrap_or(0))
}

fn sounds_tab(ui: &mut egui::Ui, e: &mut Entry, now: Instant) {
    if !e.droid.kind.has_audio() {
        theme::card(ui, "", |ui| {
            ui.label("BB-8 reproducía sus sonidos desde el teléfono con el motor Wwise de la app original, que no está incluido.");
        });
        return;
    }
    theme::card(ui, "Audio", |ui| {
        ui.horizontal_wrapped(|ui| {
            let r = ui.add(egui::Slider::new(&mut e.ui.volume, 0.0..=255.0).fixed_decimals(0).text("volumen"));
            if r.drag_stopped() || (r.changed() && !r.dragged()) {
                e.droid.set_volume(e.ui.volume as u8);
            }
            if ui.button("Parar audio").clicked() {
                e.droid.stop_audio();
            }
            let mut chatter = e.droid.chatter;
            if ui.checkbox(&mut chatter, "Charla aleatoria").changed() {
                e.droid.set_chatter(chatter, now);
            }
        });
    });
    theme::card(ui, "Sonidos", |ui| {
        ui.add(egui::TextEdit::singleline(&mut e.ui.search).hint_text("Buscar sonido…").desired_width(320.0));
        if e.droid.kind == Kind::R2q5 {
            ui.label(muted("El R2-Q5 usa aquí la lista de sonidos del R2-D2; puede que alguno no suene."));
        }
        let query = e.ui.search.to_lowercase();
        let mut groups: BTreeMap<String, Vec<&Sound>> = BTreeMap::new();
        for s in e.droid.kind.sounds() {
            if query.is_empty() || s.title().to_lowercase().contains(&query) {
                groups.entry(s.group_title()).or_default().push(s);
            }
        }
        if groups.is_empty() {
            ui.label(muted("Ningún sonido coincide."));
        }
        for (title, mut items) in groups {
            items.sort_by_key(|s| natural_key(s.name));
            egui::CollapsingHeader::new(format!("{title} ({})", items.len())).id_salt(&title).default_open(!query.is_empty()).show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    for s in items {
                        if ui.button(s.title()).clicked() {
                            e.droid.play_sound(s);
                        }
                    }
                });
            });
        }
    });
}

// ---------------------------------------------------------------------------- seguir, patrulla, dibujar

fn follow_tab(ui: &mut egui::Ui, e: &mut Entry, now: Instant) {
    let fresh = e.ui.follow.signal_fresh(&e.droid, now);
    let on = e.ui.follow.running;
    ui.columns(2, |cols| {
        theme::card(&mut cols[0], "Señal Bluetooth", |ui| {
            let s = e.ui.follow.smoothed.filter(|_| fresh);
            ui.horizontal(|ui| {
                theme::signal_bars(ui, s);
                ui.label(RichText::new(s.map_or("— dBm".into(), |s| format!("{} dBm", s.round() as i32))).size(26.0).strong());
                if let Some(s) = s {
                    let near = e.ui.follow.near_threshold;
                    ui.label(muted(if s >= near { "cerca" } else if s >= near - 12.0 { "a media distancia" } else { "lejos" }));
                }
            });
            let marks = [(e.ui.follow.near_threshold, theme::GREEN, "cerca"), (-85.0, theme::RED, "demasiado lejos")];
            theme::signal_chart(ui, e.ui.follow.history.iter().copied(), FOLLOW_HISTORY, &marks);
            if !fresh {
                ui.colored_label(theme::AMBER, "No llegan medidas de señal. Sin ellas el droide no se mueve en este modo; puede que este sistema no las dé con el droide conectado.");
            }
        });

        let ui = &mut cols[1];
        theme::card(ui, "Seguir al ordenador", |ui| {
            ui.label(RichText::new(&e.ui.follow.status).strong());
            if let Some(t) = e.ui.follow.target_heading() {
                ui.label(muted(format!("Rumbo estimado hacia ti: {}°", t as i32)));
            }
            if theme::start_stop(ui, on, "Empezar a seguir", "Dejar de seguir") {
                if on {
                    e.ui.follow.stop(&mut e.droid, now);
                } else {
                    e.ui.follow.start(&mut e.droid, now);
                }
            }
        });
        theme::card(ui, "Ajustes", |ui| {
            ui.add(egui::Slider::new(&mut e.ui.follow.near_threshold, -80.0..=-35.0).step_by(1.0).suffix(" dBm").text("«cerca»"));
            if ui.add_enabled(e.ui.follow.smoothed.is_some() && fresh, egui::Button::new("Usar la señal actual como «cerca»")).clicked() {
                e.ui.follow.calibrate_near();
            }
            ui.label(muted("Para calibrar, deja el droide a la distancia a la que quieres que se pare y pulsa el botón."));
            ui.add(egui::Slider::new(&mut e.ui.follow.speed, 30.0..=120.0).fixed_decimals(0).text("velocidad"));
        });
    });
    theme::card(ui, "Cómo funciona", |ui| {
        ui.label(muted(
            "El droide solo conoce la intensidad de la señal Bluetooth, no la dirección. Avanza zigzagueando, apunta la señal \
             en cada punto del recorrido y con las medidas de los últimos segundos calcula hacia dónde crece; hacia ahí corrige \
             el rumbo. Frena al acercarse, se para si deja de recibir medidas y, si la señal baja de -85 dBm, vuelve sobre sus \
             pasos una vez antes de detenerse. Es aproximado: úsalo en un espacio despejado y vigílalo. \
             En Windows la señal solo se actualiza mientras hay un escaneo activo.",
        ));
    });
}

fn patrol_tab(ui: &mut egui::Ui, e: &mut Entry, now: Instant) {
    theme::card(ui, "Patrulla", |ui| {
        ui.label(muted("El droide avanza solo, retrocede y gira al chocar, y de vez en cuando lanza una alarma. Déjalo en un espacio despejado."));
        ui.add(egui::Slider::new(&mut e.ui.patrol.speed, 30.0..=150.0).fixed_decimals(0).text("velocidad"));
        let on = e.ui.patrol.running;
        if theme::start_stop(ui, on, "Iniciar patrulla", "Detener patrulla") {
            if on {
                e.ui.patrol.stop(&mut e.droid, now);
            } else {
                e.ui.patrol.start(&mut e.droid, now);
            }
        }
    });
    theme::card(ui, "Registro de eventos", |ui| {
        if e.ui.patrol.entries.is_empty() {
            ui.label(muted("Sin eventos todavía."));
        }
        for line in &e.ui.patrol.entries {
            ui.monospace(line);
        }
    });
}

fn draw_tab(ui: &mut egui::Ui, e: &mut Entry, now: Instant) {
    theme::card(ui, "", |ui| {
        ui.label(muted("Dibuja el recorrido con el ratón o el dedo. El punto verde es la salida."));
        let size = Vec2::new(ui.available_width(), 320.0);
        let (rect, resp) = ui.allocate_exact_size(size, Sense::drag());
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 8.0, ui.visuals().extreme_bg_color);
        painter.rect_stroke(rect, 8.0, Stroke::new(1.0, theme::BORDER), egui::StrokeKind::Inside);
        painter.text(rect.center_top() + Vec2::new(0.0, 8.0), egui::Align2::CENTER_TOP, "▲ delante del droide", egui::FontId::proportional(12.5), theme::MUTED);
        if (resp.dragged() || resp.drag_started()) && !e.ui.draw.running && e.ui.draw.points.len() < 2000 {
            if let Some(p) = resp.interact_pointer_pos() {
                e.ui.draw.points.push((p.x - rect.min.x, p.y - rect.min.y));
            }
        }
        let at = |p: &(f32, f32)| rect.min + Vec2::new(p.0, p.1);
        for w in e.ui.draw.points.windows(2) {
            painter.line_segment([at(&w[0]), at(&w[1])], Stroke::new(3.0, theme::ACCENT));
        }
        if let Some(p) = e.ui.draw.points.first() {
            painter.circle_filled(at(p), 7.0, theme::GREEN);
        }
        ui.add(egui::ProgressBar::new(e.ui.draw.progress));
        ui.horizontal(|ui| {
            let on = e.ui.draw.running;
            ui.add_enabled_ui(on || e.ui.draw.points.len() >= 2, |ui| {
                if theme::start_stop(ui, on, "Ejecutar recorrido", "Detener") {
                    if on {
                        e.ui.draw.abort(&mut e.droid, now);
                    } else {
                        e.ui.draw.run(&mut e.droid, now);
                    }
                }
            });
            if ui.button("Borrar").clicked() {
                e.ui.draw.clear(&mut e.droid, now);
            }
        });
    });
    theme::card(ui, "Calibración", |ui| {
        ui.label(muted("Sin sensores de posición, el recorrido se calcula por tiempo. Ajusta estos valores a tu suelo."));
        ui.add(egui::Slider::new(&mut e.ui.draw.speed, 40.0..=160.0).fixed_decimals(0).text("velocidad"));
        ui.add(egui::Slider::new(&mut e.ui.draw.cm_per_second, 10.0..=100.0).fixed_decimals(0).text("cm/s a esa velocidad"));
        ui.add(egui::Slider::new(&mut e.ui.draw.cm_per_point, 0.1..=1.0).text("cm por punto del dibujo"));
    });
}

// ---------------------------------------------------------------------------- luces, ajustes, registro

fn lights_tab(ui: &mut egui::Ui, e: &mut Entry) {
    theme::card(ui, "Luces", |ui| {
        egui::Grid::new("lights").num_columns(2).spacing([18.0, 10.0]).show(ui, |ui| {
            for ch in e.droid.kind.lights() {
                ui.label(ch.title);
                match ch.style {
                    LightStyle::Rgb(..) | LightStyle::V1Main => {
                        let rgb = e.ui.light_rgb.entry(ch.title).or_insert([0, 0, 255]);
                        if ui.color_edit_button_srgb(rgb).changed() {
                            let v = *rgb;
                            e.droid.set_light_rgb(&ch, v);
                        }
                    }
                    LightStyle::Mono(_) | LightStyle::V1Back => {
                        let level = e.ui.light_level.entry(ch.title).or_insert(0.0);
                        if ui.add(egui::Slider::new(level, 0.0..=255.0).fixed_decimals(0)).changed() {
                            let v = *level as u8;
                            e.droid.set_light_level(&ch, v);
                        }
                    }
                }
                ui.end_row();
            }
        });
        if e.droid.kind.uses_v2() && ui.button("Restablecer luces").clicked() {
            e.droid.reset_lights();
        }
    });
}

fn settings_tab(ui: &mut egui::Ui, e: &mut Entry) {
    let kind = e.droid.kind;
    ui.columns(2, |cols| {
        theme::card(&mut cols[0], "Droide", |ui| {
            egui::Grid::new("info").num_columns(2).spacing([18.0, 8.0]).show(ui, |ui| {
                let mut line = |k: &str, v: String| {
                    ui.label(muted(k));
                    ui.label(v);
                    ui.end_row();
                };
                line("Nombre", e.droid.name.clone());
                line("Modelo", kind.title().into());
                line("Batería", format!("{} · {}", e.droid.battery.text(), e.droid.battery_text));
                if kind.uses_v2() {
                    line("Firmware Nordic", e.droid.versions.get("Nordic").cloned().unwrap_or("—".into()));
                    line("Firmware principal", e.droid.versions.get("Principal").cloned().unwrap_or("—".into()));
                    line("Direcciones", e.droid.routing_text.clone());
                }
                line("Firmware en el APK", kind.bundled_firmware().into());
            });
            ui.horizontal_wrapped(|ui| {
                if ui.button("Actualizar batería").clicked() {
                    e.droid.request_battery();
                }
                if kind.uses_v2() && ui.button("Leer versiones").clicked() {
                    e.droid.request_versions();
                }
            });
        });

        let ui = &mut cols[1];
        theme::card(ui, "Energía", |ui| {
            ui.horizontal_wrapped(|ui| {
                if ui.button("Despertar").clicked() {
                    e.droid.wake();
                }
                if ui.button("Dormir").clicked() {
                    e.droid.sleep(false);
                }
                if kind.uses_v2() && ui.button("Sueño profundo").clicked() {
                    e.droid.sleep(true);
                }
                if ui.add(theme::danger("Desconectar")).clicked() {
                    e.droid.disconnect();
                }
            });
        });
        theme::card(ui, "Comportamiento", |ui| {
            if kind.uses_v2() {
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
            ui.checkbox(&mut e.droid.react_to_hits, "Reaccionar a los golpes");
        });
    });

    if kind.uses_v2() {
        theme::card(ui, "Renombrar", |ui| {
            let prefix = format!("{}-", kind.prefix());
            let valid = e.ui.rename.len() == prefix.len() + 4
                && e.ui.rename.starts_with(&prefix)
                && e.ui.rename[prefix.len()..].chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit());
            ui.horizontal(|ui| {
                ui.add(egui::TextEdit::singleline(&mut e.ui.rename).hint_text(format!("{prefix}XXXX")).desired_width(160.0));
                if ui.add_enabled(valid, egui::Button::new("Cambiar nombre Bluetooth")).clicked() {
                    let name = e.ui.rename.clone();
                    e.droid.rename(&name);
                }
            });
            ui.label(muted(format!("Debe conservar el prefijo {prefix} y tener 4 letras o números; si no, la app no volverá a encontrarlo.")));
        });
    }
    ui.label(muted(
        "No incluido: actualización de firmware (OTA), realidad aumentada, tienda, Force Band y sincronía de Watch With Me \
         con películas. Dependen de servicios o recursos de la app original, o pueden dejar el droide inservible si fallan.",
    ));
}

fn log_tab(ui: &mut egui::Ui, e: &mut Entry) {
    ui.horizontal(|ui| {
        if ui.add(theme::primary("Copiar registro")).clicked() {
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
    theme::card(ui, "", |ui| {
        if e.droid.log.is_empty() {
            ui.label(muted("Registro vacío."));
        }
        for line in &e.droid.log {
            ui.monospace(line);
        }
    });
}

#[allow(dead_code)]
fn _title_is_used(raw: &str) -> String {
    translate::title(raw)
}
