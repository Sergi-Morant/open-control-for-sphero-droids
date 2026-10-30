//! Mandos (Xbox, PlayStation, etc.) con gilrs. Esta capa solo lee el estado; `app.rs` aplica las acciones.
//!
//! Los botones se identifican por su posición, no por su dibujo, así que PlayStation sigue la misma
//! estructura que Xbox: Cruz = A, Círculo = B, Cuadrado = X, Triángulo = Y, R2/L2 = RT/LT.

use std::collections::{BTreeMap, HashSet};
use std::path::PathBuf;

use gilrs::{Axis, Button, EventType, Gilrs};

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum PadButton {
    South,
    East,
    West,
    North,
    LeftBumper,
    RightBumper,
    DPadUp,
    DPadDown,
    DPadLeft,
    DPadRight,
    Start,
    Select,
    LeftStick,
    RightStick,
}

impl PadButton {
    pub const ALL: [PadButton; 14] = [
        PadButton::South, PadButton::East, PadButton::West, PadButton::North,
        PadButton::LeftBumper, PadButton::RightBumper,
        PadButton::DPadUp, PadButton::DPadDown, PadButton::DPadLeft, PadButton::DPadRight,
        PadButton::Start, PadButton::Select, PadButton::LeftStick, PadButton::RightStick,
    ];

    pub fn title(self) -> &'static str {
        match self {
            PadButton::South => "A / Cruz",
            PadButton::East => "B / Círculo",
            PadButton::West => "X / Cuadrado",
            PadButton::North => "Y / Triángulo",
            PadButton::LeftBumper => "LB / L1",
            PadButton::RightBumper => "RB / R1",
            PadButton::DPadUp => "Cruceta arriba",
            PadButton::DPadDown => "Cruceta abajo",
            PadButton::DPadLeft => "Cruceta izquierda",
            PadButton::DPadRight => "Cruceta derecha",
            PadButton::Start => "Menú / Options",
            PadButton::Select => "Vista / Share",
            PadButton::LeftStick => "Clic stick izquierdo (L3)",
            PadButton::RightStick => "Clic stick derecho (R3)",
        }
    }

    fn key(self) -> &'static str {
        match self {
            PadButton::South => "south",
            PadButton::East => "east",
            PadButton::West => "west",
            PadButton::North => "north",
            PadButton::LeftBumper => "lb",
            PadButton::RightBumper => "rb",
            PadButton::DPadUp => "up",
            PadButton::DPadDown => "down",
            PadButton::DPadLeft => "left",
            PadButton::DPadRight => "right",
            PadButton::Start => "start",
            PadButton::Select => "select",
            PadButton::LeftStick => "l3",
            PadButton::RightStick => "r3",
        }
    }

    fn from_gilrs(b: Button) -> Option<PadButton> {
        Some(match b {
            Button::South => PadButton::South,
            Button::East => PadButton::East,
            Button::West => PadButton::West,
            Button::North => PadButton::North,
            Button::LeftTrigger => PadButton::LeftBumper,
            Button::RightTrigger => PadButton::RightBumper,
            Button::DPadUp => PadButton::DPadUp,
            Button::DPadDown => PadButton::DPadDown,
            Button::DPadLeft => PadButton::DPadLeft,
            Button::DPadRight => PadButton::DPadRight,
            Button::Start => PadButton::Start,
            Button::Select => PadButton::Select,
            Button::LeftThumb => PadButton::LeftStick,
            Button::RightThumb => PadButton::RightStick,
            _ => return None,
        })
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PadAction {
    Legs3,
    Legs2,
    Waddle,
    LegsStop,
    StopAll,
    SetForward,
    RandomSound,
    RandomEmote,
    ToggleMovie,
    ToggleMonkeyBusiness,
    Turbo,
    Boost,
    AimToggle,
    DomeCenter,
}

impl PadAction {
    pub const ALL: [PadAction; 14] = [
        PadAction::Legs3, PadAction::Legs2, PadAction::Waddle, PadAction::LegsStop, PadAction::StopAll,
        PadAction::SetForward, PadAction::RandomSound, PadAction::RandomEmote, PadAction::ToggleMovie,
        PadAction::ToggleMonkeyBusiness, PadAction::Turbo, PadAction::Boost, PadAction::AimToggle, PadAction::DomeCenter,
    ];

    pub fn title(self) -> &'static str {
        match self {
            PadAction::Legs3 => "3 patas",
            PadAction::Legs2 => "2 patas",
            PadAction::Waddle => "Contoneo",
            PadAction::LegsStop => "Parar patas",
            PadAction::StopAll => "Parar todo",
            PadAction::SetForward => "Fijar «adelante»",
            PadAction::RandomSound => "Sonido al azar",
            PadAction::RandomEmote => "Emote al azar",
            PadAction::ToggleMovie => "Vamos a ver una película (activar/apagar)",
            PadAction::ToggleMonkeyBusiness => "Haz monerías (activar/apagar)",
            PadAction::Turbo => "Turbo (mientras se mantiene)",
            PadAction::Boost => "Impulso (BB-8)",
            PadAction::AimToggle => "Apuntar / fijar «adelante» (alternar)",
            PadAction::DomeCenter => "Centrar la cúpula",
        }
    }

    fn key(self) -> &'static str {
        match self {
            PadAction::Legs3 => "legs3",
            PadAction::Legs2 => "legs2",
            PadAction::Waddle => "waddle",
            PadAction::LegsStop => "legsstop",
            PadAction::StopAll => "stopall",
            PadAction::SetForward => "setforward",
            PadAction::RandomSound => "randomsound",
            PadAction::RandomEmote => "randomemote",
            PadAction::ToggleMovie => "movie",
            PadAction::ToggleMonkeyBusiness => "monkey",
            PadAction::Turbo => "turbo",
            PadAction::Boost => "boost",
            PadAction::AimToggle => "aim",
            PadAction::DomeCenter => "domecenter",
        }
    }
}

/// Estado leído en un instante.
#[derive(Default)]
pub struct Frame {
    pub connected: bool,
    /// Stick izquierdo, ya sin zona muerta; y positiva = arriba.
    pub left: (f32, f32),
    pub right_x: f32,
    /// Gatillos RT y LT (0…1).
    pub rt: f32,
    pub lt: f32,
    /// Botones pulsados desde la lectura anterior.
    pub pressed: Vec<PadButton>,
    pub held: HashSet<PadButton>,
}

pub struct Pad {
    gilrs: Option<Gilrs>,
    pub name: Option<String>,
    pub mapping: BTreeMap<PadButton, PadAction>,
    held: HashSet<PadButton>,
}

fn default_mapping() -> BTreeMap<PadButton, PadAction> {
    // Por defecto: A = 3 patas, B = 2 patas, Y = contoneo, X = parar patas,
    // clic del stick izquierdo = apuntar / fijar «adelante» y clic del derecho = centrar la cúpula.
    BTreeMap::from([
        (PadButton::South, PadAction::Legs3),
        (PadButton::East, PadAction::Legs2),
        (PadButton::North, PadAction::Waddle),
        (PadButton::West, PadAction::LegsStop),
        (PadButton::LeftStick, PadAction::AimToggle),
        (PadButton::RightStick, PadAction::DomeCenter),
    ])
}

/// Carpeta de configuración del usuario, según el sistema.
fn config_path() -> Option<PathBuf> {
    const APP: &str = "open-control-for-sphero-droids";
    let base = if cfg!(target_os = "windows") {
        std::env::var_os("APPDATA").map(PathBuf::from)
    } else if cfg!(target_os = "macos") {
        std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Library/Application Support"))
    } else {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
    };
    base.map(|b| b.join(APP).join("gamepad.txt"))
}

impl Pad {
    pub fn new() -> Self {
        let mut pad = Pad { gilrs: Gilrs::new().ok(), name: None, mapping: default_mapping(), held: HashSet::new() };
        pad.load();
        pad.refresh_name();
        pad
    }

    pub fn available(&self) -> bool {
        self.gilrs.is_some()
    }

    fn refresh_name(&mut self) {
        self.name = self.gilrs.as_ref().and_then(|g| g.gamepads().next().map(|(_, p)| p.name().to_string()));
    }

    /// Lee los eventos pendientes y el estado actual de sticks y gatillos.
    pub fn poll(&mut self) -> Frame {
        let mut frame = Frame::default();
        let Some(g) = self.gilrs.as_mut() else { return frame };
        let mut changed = false;
        while let Some(ev) = g.next_event() {
            match ev.event {
                EventType::ButtonPressed(b, _) => {
                    if let Some(pb) = PadButton::from_gilrs(b) {
                        self.held.insert(pb);
                        frame.pressed.push(pb);
                    }
                }
                EventType::ButtonReleased(b, _) => {
                    if let Some(pb) = PadButton::from_gilrs(b) {
                        self.held.remove(&pb);
                    }
                }
                EventType::Connected | EventType::Disconnected => changed = true,
                _ => {}
            }
        }
        if changed {
            self.refresh_name();
        }
        let Some(g) = self.gilrs.as_ref() else { return frame };
        if let Some((_, p)) = g.gamepads().next() {
            frame.connected = true;
            let ax = |a| p.value(a);
            frame.left = deadzone(ax(Axis::LeftStickX), ax(Axis::LeftStickY));
            frame.right_x = deadzone(ax(Axis::RightStickX), 0.0).0;
            let trigger = |b: Button, z: Axis| p.button_data(b).map(|d| d.value()).unwrap_or(0.0).max(p.value(z).max(0.0));
            frame.rt = trigger(Button::RightTrigger2, Axis::RightZ);
            frame.lt = trigger(Button::LeftTrigger2, Axis::LeftZ);
        }
        frame.held = self.held.clone();
        frame
    }

    pub fn set(&mut self, b: PadButton, a: Option<PadAction>) {
        match a {
            Some(a) => {
                self.mapping.insert(b, a);
            }
            None => {
                self.mapping.remove(&b);
            }
        }
        self.save();
    }

    pub fn reset(&mut self) {
        self.mapping = default_mapping();
        self.save();
    }

    fn save(&self) {
        let Some(path) = config_path() else { return };
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let body: String = self.mapping.iter().map(|(b, a)| format!("{}={}\n", b.key(), a.key())).collect();
        let _ = std::fs::write(path, format!("version=2\n{body}"));
    }

    fn load(&mut self) {
        let Some(path) = config_path() else { return };
        let Ok(text) = std::fs::read_to_string(path) else { return };
        let (map, migrated) = parse_mapping(&text);
        self.mapping = map;
        if migrated {
            self.save();
        }
    }
}

/// Lee la asignación guardada. Devuelve también si hubo que migrarla de una versión anterior.
fn parse_mapping(text: &str) -> (BTreeMap<PadButton, PadAction>, bool) {
    let mut map = BTreeMap::new();
    let mut version = 1;
    for line in text.lines() {
        let Some((k, v)) = line.split_once('=') else { continue };
        if k.trim() == "version" {
            version = v.trim().parse().unwrap_or(1);
            continue;
        }
        let button = PadButton::ALL.into_iter().find(|b| b.key() == k.trim());
        let action = PadAction::ALL.into_iter().find(|a| a.key() == v.trim());
        if let (Some(b), Some(a)) = (button, action) {
            map.insert(b, a);
        }
    }
    // Versión 2: los clics de los sticks pasan a apuntar y centrar la cúpula (solo si no estaban asignados).
    let migrated = version < 2;
    if migrated {
        for b in [PadButton::LeftStick, PadButton::RightStick] {
            if let Some(a) = default_mapping().get(&b) {
                map.entry(b).or_insert(*a);
            }
        }
    }
    (map, migrated)
}

/// Zona muerta radial con reescalado, para que el stick no «tiemble» en reposo.
pub fn deadzone(x: f32, y: f32) -> (f32, f32) {
    const DEAD: f32 = 0.15;
    let m = x.hypot(y);
    if m < DEAD {
        return (0.0, 0.0);
    }
    let scaled = ((m - DEAD) / (1.0 - DEAD)).min(1.0);
    (x / m * scaled, y / m * scaled)
}

/// Rumbo (0…359, 0 = adelante, sentido horario) de un stick con y positiva hacia arriba.
pub fn heading_of(x: f32, y: f32) -> i32 {
    (x.atan2(y).to_degrees().round() as i32).rem_euclid(360)
}

/// Nueva posición de la cúpula tras mantener el stick `x` durante `dt` segundos (-160°…180°).
/// La curva cuadrática da precisión cerca del centro; a tope barre unos 120°/s.
pub fn dome_step(current: f32, x: f32, dt: f32) -> f32 {
    (current + x * x.abs() * 120.0 * dt).clamp(-160.0, 180.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deadzone_ignores_rest_and_scales_to_full() {
        assert_eq!(deadzone(0.1, 0.05), (0.0, 0.0));
        let (x, y) = deadzone(1.0, 0.0);
        assert!((x - 1.0).abs() < 1e-5 && y == 0.0);
    }

    #[test]
    fn heading_zero_is_forward_and_clockwise() {
        assert_eq!(heading_of(0.0, 1.0), 0); // arriba
        assert_eq!(heading_of(1.0, 0.0), 90); // derecha
        assert_eq!(heading_of(0.0, -1.0), 180); // abajo
        assert_eq!(heading_of(-1.0, 0.0), 270); // izquierda
    }

    #[test]
    fn dome_moves_gradually_and_stays_in_range() {
        assert_eq!(dome_step(0.0, 0.0, 0.1), 0.0); // soltar el stick no mueve ni devuelve la cúpula
        assert!((dome_step(0.0, 1.0, 0.1) - 12.0).abs() < 1e-4); // a tope, ~120°/s
        assert!(dome_step(0.0, 0.5, 0.1) < dome_step(0.0, 1.0, 0.1)); // más suave con menos inclinación
        assert!(dome_step(0.0, -1.0, 0.1) < 0.0);
        assert_eq!(dome_step(179.0, 1.0, 1.0), 180.0);
        assert_eq!(dome_step(-159.0, -1.0, 1.0), -160.0);
    }

    #[test]
    fn old_saved_mapping_gains_stick_clicks_without_losing_user_choices() {
        // Archivo de la versión 1: sin «version» y con el stick izquierdo ya asignado por el usuario.
        let (m, migrated) = parse_mapping("south=legs3\nnorth=waddle\nl3=randomsound\n");
        assert!(migrated);
        assert_eq!(m[&PadButton::South], PadAction::Legs3);
        assert_eq!(m[&PadButton::LeftStick], PadAction::RandomSound); // se respeta la elección del usuario
        assert_eq!(m[&PadButton::RightStick], PadAction::DomeCenter); // se añade el valor nuevo
    }

    #[test]
    fn current_version_mapping_is_not_touched() {
        let (m, migrated) = parse_mapping("version=2\nsouth=legs3\n");
        assert!(!migrated);
        assert_eq!(m.len(), 1); // quien quitó los clics de stick los sigue teniendo sin asignar
    }

    #[test]
    fn default_mapping_is_a_b_y_x_and_stick_clicks() {
        let m = default_mapping();
        assert_eq!(m[&PadButton::LeftStick], PadAction::AimToggle);
        assert_eq!(m[&PadButton::RightStick], PadAction::DomeCenter);
        assert_eq!(m[&PadButton::South], PadAction::Legs3);
        assert_eq!(m[&PadButton::East], PadAction::Legs2);
        assert_eq!(m[&PadButton::North], PadAction::Waddle);
        assert_eq!(m[&PadButton::West], PadAction::LegsStop);
    }
}
