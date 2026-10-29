//! Tipos de droide (ToyTypes de la app original) y sus catálogos.

use crate::catalog;
use crate::translate;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    Bb8,
    R2d2,
    Bb9e,
    R2q5,
}

pub struct Sound {
    pub name: &'static str,
    pub sector: u16,
}

pub struct Animation {
    pub name: &'static str,
    pub index: u16,
    pub category: &'static str,
    pub seconds: f32,
}

#[derive(Clone, Copy)]
pub enum LightStyle {
    /// Máscaras v2 de rojo, verde y azul.
    Rgb(u16, u16, u16),
    /// Máscara v2 de un solo LED.
    Mono(u16),
    V1Main,
    V1Back,
}

pub struct LightChannel {
    pub title: &'static str,
    pub style: LightStyle,
}

impl Kind {
    pub const ALL: [Kind; 4] = [Kind::Bb8, Kind::R2d2, Kind::Bb9e, Kind::R2q5];

    /// Prefijo del nombre BLE (ToyPrefix de la app original).
    pub fn prefix(self) -> &'static str {
        match self {
            Kind::Bb8 => "BB",
            Kind::R2d2 => "D2",
            Kind::Bb9e => "GB",
            Kind::R2q5 => "Q5",
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Kind::Bb8 => "BB-8",
            Kind::R2d2 => "R2-D2",
            Kind::Bb9e => "BB-9E",
            Kind::R2q5 => "R2-Q5",
        }
    }

    pub fn from_ble_name(name: &str) -> Option<Kind> {
        Kind::ALL.into_iter().find(|k| name.starts_with(&format!("{}-", k.prefix())))
    }

    pub fn uses_v2(self) -> bool {
        self != Kind::Bb8
    }
    pub fn has_audio(self) -> bool {
        self != Kind::Bb8
    }
    pub fn has_dome(self) -> bool {
        matches!(self, Kind::R2d2 | Kind::R2q5)
    }

    pub fn sounds(self) -> &'static [Sound] {
        match self {
            Kind::Bb8 => &[],
            Kind::Bb9e => catalog::BB9E_SOUNDS,
            Kind::R2d2 | Kind::R2q5 => catalog::R2D2_SOUNDS, // el R2-Q5 comparte la lista del R2-D2
        }
    }

    pub fn animations(self) -> &'static [Animation] {
        match self {
            Kind::Bb8 => &[],
            Kind::R2d2 => catalog::R2D2_ANIMATIONS,
            Kind::R2q5 => catalog::R2Q5_ANIMATIONS,
            Kind::Bb9e => catalog::BB9E_ANIMATIONS,
        }
    }

    pub fn lights(self) -> Vec<LightChannel> {
        use LightStyle::*;
        match self {
            Kind::R2d2 | Kind::R2q5 => vec![
                LightChannel { title: "LED frontal", style: Rgb(0x01, 0x02, 0x04) },
                LightChannel { title: "LED trasero", style: Rgb(0x10, 0x20, 0x40) },
                LightChannel { title: "Pantalla lógica", style: Mono(0x08) },
                LightChannel { title: "Holoproyector", style: Mono(0x80) },
            ],
            Kind::Bb9e => vec![
                LightChannel { title: "Cuerpo", style: Rgb(0x01, 0x02, 0x04) },
                LightChannel { title: "Luz de apuntado", style: Mono(0x08) },
                LightChannel { title: "Luz de la cabeza", style: Mono(0x10) },
            ],
            Kind::Bb8 => vec![
                LightChannel { title: "LED principal", style: V1Main },
                LightChannel { title: "LED trasero (apuntado)", style: V1Back },
            ],
        }
    }

    /// Versiones de firmware que trae el APK original.
    pub fn bundled_firmware(self) -> &'static str {
        match self {
            Kind::Bb8 => "ST 4.69",
            Kind::R2d2 => "Nordic 7.0.101 · ST 7.1.86",
            Kind::Bb9e => "Nordic 7.0.68 · ST 7.0.62",
            Kind::R2q5 => "Nordic 7.0.102 · ST 7.1.87",
        }
    }
}

impl Animation {
    pub fn title(&self) -> String {
        translate::title_with_original(self.name)
    }

    pub fn category_title(&self) -> &'static str {
        match self.category {
            "Emote" => "Emociones",
            "Idle" => "Reposo",
            "Patrol" => "Patrulla",
            "WWM" => "Reacciones (Watch With Me)",
            "Reaction" => "Reacciones",
            _ => "Otras",
        }
    }

    pub fn is_ion_blast(&self) -> bool {
        self.name.to_lowercase().contains("ionblast")
    }
}

impl Sound {
    pub fn title(&self) -> String {
        translate::title_with_original(self.name)
    }

    /// Grupo a partir del nombre ("R2_Chatty_12" → "Chatty").
    pub fn group(&self) -> String {
        let mut n = self
            .name
            .replace("Play_BB9E_", "")
            .replace("R2_", "")
            .replace("R2 ", "")
            .replace('_', " ");
        while n.chars().last().is_some_and(|c| c.is_numeric() || c == ' ') {
            n.pop();
        }
        if n.is_empty() { self.name.to_string() } else { n }
    }

    pub fn group_title(&self) -> String {
        translate::title(&self.group())
    }
}
