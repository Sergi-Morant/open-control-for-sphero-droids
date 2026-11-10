//! Aspecto de la app: colores, tamaños y las piezas que se repiten (tarjetas, botones, medidores).

use egui::{Color32, CornerRadius, FontId, Margin, Rect, RichText, Sense, Stroke, StrokeKind, TextStyle, Vec2};

pub const BG: Color32 = Color32::from_rgb(15, 18, 24);
pub const PANEL: Color32 = Color32::from_rgb(20, 24, 32);
pub const CARD: Color32 = Color32::from_rgb(28, 33, 44);
pub const BORDER: Color32 = Color32::from_rgb(44, 52, 68);
pub const WIDGET: Color32 = Color32::from_rgb(42, 50, 66);
pub const WIDGET_HOVER: Color32 = Color32::from_rgb(54, 64, 84);
pub const TEXT: Color32 = Color32::from_rgb(228, 232, 240);
pub const MUTED: Color32 = Color32::from_rgb(140, 150, 170);
pub const ACCENT: Color32 = Color32::from_rgb(64, 140, 240);
pub const ACCENT_SOFT: Color32 = Color32::from_rgb(36, 68, 118);
pub const GREEN: Color32 = Color32::from_rgb(76, 195, 138);
pub const AMBER: Color32 = Color32::from_rgb(238, 176, 72);
pub const RED: Color32 = Color32::from_rgb(214, 72, 66);

/// Se aplica una vez al arrancar. Tamaños pensados para ratón y también para pantalla táctil (Steam Deck).
pub fn apply(ctx: &egui::Context) {
    ctx.set_theme(egui::Theme::Dark);
    ctx.all_styles_mut(|s| {
        s.text_styles = [
            (TextStyle::Heading, FontId::proportional(22.0)),
            (TextStyle::Body, FontId::proportional(15.0)),
            (TextStyle::Button, FontId::proportional(15.0)),
            (TextStyle::Small, FontId::proportional(12.5)),
            (TextStyle::Monospace, FontId::monospace(13.0)),
        ]
        .into();
        s.spacing.item_spacing = Vec2::new(10.0, 8.0);
        s.spacing.button_padding = Vec2::new(14.0, 7.0);
        s.spacing.interact_size = Vec2::new(44.0, 34.0);
        s.spacing.slider_width = 240.0;
        s.spacing.slider_rail_height = 8.0;
        s.spacing.icon_width = 20.0;
        s.spacing.combo_height = 320.0;
        s.spacing.scroll.bar_width = 10.0;

        let v = &mut s.visuals;
        v.panel_fill = BG;
        v.window_fill = CARD;
        v.extreme_bg_color = Color32::from_rgb(10, 12, 16);
        v.faint_bg_color = PANEL;
        v.override_text_color = Some(TEXT);
        v.weak_text_color = Some(MUTED);
        v.warn_fg_color = AMBER;
        v.error_fg_color = RED;
        v.hyperlink_color = ACCENT;
        v.selection.bg_fill = ACCENT_SOFT;
        v.selection.stroke = Stroke::new(1.0, ACCENT);
        v.slider_trailing_fill = true;
        v.window_stroke = Stroke::new(1.0, BORDER);
        let r = CornerRadius::same(8);
        v.window_corner_radius = CornerRadius::same(12);
        v.menu_corner_radius = r;
        for w in [&mut v.widgets.noninteractive, &mut v.widgets.inactive, &mut v.widgets.hovered, &mut v.widgets.active, &mut v.widgets.open] {
            w.corner_radius = r;
        }
        v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, BORDER);
        v.widgets.inactive.bg_fill = WIDGET;
        v.widgets.inactive.weak_bg_fill = WIDGET;
        v.widgets.inactive.bg_stroke = Stroke::NONE;
        v.widgets.hovered.bg_fill = WIDGET_HOVER;
        v.widgets.hovered.weak_bg_fill = WIDGET_HOVER;
        v.widgets.hovered.bg_stroke = Stroke::new(1.0, ACCENT);
        v.widgets.active.bg_fill = ACCENT;
        v.widgets.active.weak_bg_fill = ACCENT;
        v.widgets.active.bg_stroke = Stroke::new(1.0, ACCENT);
        v.widgets.open.bg_fill = WIDGET_HOVER;
        v.widgets.open.weak_bg_fill = WIDGET_HOVER;
    });
}

/// Tarjeta con título que ocupa todo el ancho disponible.
pub fn card<R>(ui: &mut egui::Ui, title: &str, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    egui::Frame::new()
        .fill(CARD)
        .stroke(Stroke::new(1.0, BORDER))
        .corner_radius(CornerRadius::same(12))
        .inner_margin(Margin::same(14))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            // Los deslizadores se adaptan al ancho de la tarjeta para no desbordarla.
            ui.spacing_mut().slider_width = (ui.available_width() - 190.0).clamp(90.0, 260.0);
            // Sin justificar: en columnas, egui estiraría cada botón a todo el ancho.
            ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                if !title.is_empty() {
                    ui.label(RichText::new(title).strong().size(16.0));
                    ui.add_space(2.0);
                }
                add(ui)
            })
            .inner
        })
        .inner
}

/// Rótulo pequeño para separar grupos.
pub fn section(ui: &mut egui::Ui, text: &str) {
    ui.add_space(2.0);
    ui.label(RichText::new(text).size(13.0).color(MUTED));
}

pub fn primary(text: &str) -> egui::Button<'_> {
    egui::Button::new(RichText::new(text).strong().color(Color32::WHITE)).fill(ACCENT)
}

pub fn danger(text: &str) -> egui::Button<'_> {
    egui::Button::new(RichText::new(text).strong().color(Color32::WHITE)).fill(RED)
}

/// Botón grande de arrancar/parar un modo. Devuelve true si se ha pulsado.
pub fn start_stop(ui: &mut egui::Ui, on: bool, start: &str, stop: &str) -> bool {
    let b = if on { danger(stop) } else { primary(start) };
    ui.add(b.min_size(Vec2::new(200.0, 42.0))).clicked()
}

/// Etiqueta redondeada con un punto de color: «● texto».
pub fn pill(ui: &mut egui::Ui, color: Color32, text: &str) {
    egui::Frame::new().fill(PANEL).stroke(Stroke::new(1.0, BORDER)).corner_radius(CornerRadius::same(255)).inner_margin(Margin::symmetric(10, 4)).show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            dot(ui, color);
            ui.label(RichText::new(text).size(13.0));
        });
    });
}

pub fn dot(ui: &mut egui::Ui, color: Color32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(10.0), Sense::hover());
    ui.painter().circle_filled(rect.center(), 4.5, color);
}

/// Color orientativo para una intensidad de señal en dBm.
pub fn signal_color(dbm: f32) -> Color32 {
    if dbm >= -60.0 {
        GREEN
    } else if dbm >= -75.0 {
        AMBER
    } else {
        RED
    }
}

/// Cuatro barras de cobertura.
pub fn signal_bars(ui: &mut egui::Ui, dbm: Option<f32>) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(26.0, 18.0), Sense::hover());
    let lit = dbm.map_or(0, |d| ((d + 92.0) / 9.0).clamp(0.0, 4.0).ceil() as usize);
    let color = dbm.map_or(BORDER, signal_color);
    for i in 0..4 {
        let h = 5.0 + i as f32 * 4.0;
        let x = rect.min.x + i as f32 * 7.0;
        let bar = Rect::from_min_max(egui::pos2(x, rect.max.y - h), egui::pos2(x + 5.0, rect.max.y));
        ui.painter().rect_filled(bar, 1.5, if i < lit { color } else { BORDER });
    }
}

/// Gráfica sencilla de una serie en dBm con líneas de referencia (valor, color, rótulo).
pub fn signal_chart(ui: &mut egui::Ui, values: impl ExactSizeIterator<Item = f32>, capacity: usize, marks: &[(f32, Color32, &str)]) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 150.0), Sense::hover());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 8.0, ui.visuals().extreme_bg_color);
    p.rect_stroke(rect, 8.0, Stroke::new(1.0, BORDER), StrokeKind::Inside);
    let (lo, hi) = (-95.0_f32, -30.0_f32);
    let y_of = |v: f32| rect.max.y - 8.0 - (v.clamp(lo, hi) - lo) / (hi - lo) * (rect.height() - 16.0);
    for (v, color, label) in marks {
        let y = y_of(*v);
        p.hline(rect.x_range().shrink(8.0), y, Stroke::new(1.0, color.gamma_multiply(0.7)));
        p.text(egui::pos2(rect.max.x - 10.0, y - 2.0), egui::Align2::RIGHT_BOTTOM, label, FontId::proportional(11.5), *color);
    }
    let n = values.len();
    if n < 2 {
        p.text(rect.center(), egui::Align2::CENTER_CENTER, "Sin medidas todavía", FontId::proportional(14.0), MUTED);
        return;
    }
    let step = (rect.width() - 16.0) / (capacity.max(2) - 1) as f32;
    let x0 = rect.max.x - 8.0 - step * (n - 1) as f32;
    let points: Vec<egui::Pos2> = values.enumerate().map(|(i, v)| egui::pos2(x0 + step * i as f32, y_of(v))).collect();
    let last = *points.last().unwrap();
    p.line(points, Stroke::new(2.0, ACCENT));
    p.circle_filled(last, 4.0, ACCENT);
}
