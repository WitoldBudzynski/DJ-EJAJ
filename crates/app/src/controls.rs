use eframe::egui::{
    self, Align2, Color32, FontId, Pos2, Rect, Response, RichText, Sense, Stroke, Vec2,
};
pub const AMBER: Color32 = Color32::from_rgb(234, 177, 77);
pub const CYAN: Color32 = Color32::from_rgb(77, 203, 199);
pub const TEXT: Color32 = Color32::from_rgb(227, 231, 235);
pub const MUTED: Color32 = Color32::from_rgb(153, 163, 175);
pub const PANEL: Color32 = Color32::from_rgb(30, 34, 39);
pub const GREEN: Color32 = Color32::from_rgb(124, 221, 130);

pub struct Spectrum {
    samples: [f32; 1024],
    cursor: usize,
    levels: [f32; 32],
    fft: std::sync::Arc<dyn rustfft::Fft<f32>>,
    data: Vec<rustfft::num_complex::Complex<f32>>,
    scratch: Vec<rustfft::num_complex::Complex<f32>>,
    last: std::time::Instant,
}
impl Spectrum {
    pub fn new() -> Self {
        let fft = rustfft::FftPlanner::new().plan_fft_forward(1024);
        let scratch = vec![Default::default(); fft.get_inplace_scratch_len()];
        Self {
            samples: [0.0; 1024],
            cursor: 0,
            levels: [0.0; 32],
            fft,
            data: vec![Default::default(); 1024],
            scratch,
            last: std::time::Instant::now(),
        }
    }
    pub fn feed(&mut self, block: &[[f32; 2]]) {
        for frame in block {
            self.samples[self.cursor] = (frame[0] + frame[1]) * 0.5;
            self.cursor = (self.cursor + 1) % 1024;
        }
    }
    pub fn draw(&mut self, ui: &mut egui::Ui, sr: f32) {
        if self.last.elapsed().as_millis() >= 50 {
            self.last = std::time::Instant::now();
            for (i, x) in self.data.iter_mut().enumerate() {
                *x = rustfft::num_complex::Complex::new(
                    self.samples[(i + self.cursor) % 1024]
                        * (0.5 - 0.5 * (std::f32::consts::TAU * i as f32 / 1024.0).cos()),
                    0.0,
                );
            }
            self.fft
                .process_with_scratch(&mut self.data, &mut self.scratch);
            for (i, level) in self.levels.iter_mut().enumerate() {
                let start = (40.0 * (400.0_f32).powf(i as f32 / 32.0) * 1024.0 / sr) as usize;
                let end = (40.0 * (400.0_f32).powf((i + 1) as f32 / 32.0) * 1024.0 / sr) as usize;
                let max = self.data[start.clamp(1, 511)..end.max(start + 1).min(512)]
                    .iter()
                    .map(|x| x.norm() / 256.0)
                    .fold(0.0_f32, f32::max);
                let target = ((20.0 * max.max(1e-6).log10() + 72.0) / 72.0).clamp(0.0, 1.0);
                *level = target.max(*level * 0.8);
            }
        }
        ui.vertical(|ui| {
            ui.label(RichText::new("MASTER SPECTRUM").size(9.0).color(MUTED));
            let (rect, _) = ui.allocate_exact_size(
                Vec2::new(ui.available_width().max(80.0), 54.0),
                Sense::hover(),
            );
            ui.painter()
                .rect_filled(rect, 2.0, Color32::from_rgb(17, 21, 24));
            let width = rect.width() / 32.0;
            for (i, level) in self.levels.iter().enumerate() {
                ui.painter().rect_filled(
                    Rect::from_min_max(
                        Pos2::new(
                            rect.left() + i as f32 * width,
                            rect.bottom() - level * rect.height(),
                        ),
                        Pos2::new(rect.left() + (i + 1) as f32 * width - 1.0, rect.bottom()),
                    ),
                    0.0,
                    GREEN.gamma_multiply(0.8),
                );
            }
        });
    }
}

pub fn mini_knob(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut f32,
    keys: &str,
    color: Color32,
) -> Response {
    let (rect, mut response) =
        ui.allocate_exact_size(Vec2::new(43.0, 60.0), Sense::click_and_drag());
    if response.clicked() {
        response.request_focus();
    }
    if response.dragged() {
        *value = (*value - response.drag_delta().y * 0.01).clamp(0.0, 2.0);
        response.mark_changed();
    }
    if response.has_focus() {
        let delta = ui.input(|i| {
            f32::from(i.key_pressed(egui::Key::ArrowUp))
                - f32::from(i.key_pressed(egui::Key::ArrowDown))
        });
        if delta != 0.0 {
            *value = (*value + delta * 0.05).clamp(0.0, 2.0);
            response.mark_changed();
        }
    }
    let p = ui.painter();
    let center = Pos2::new(rect.center().x, rect.top() + 18.0);
    let angle = std::f32::consts::PI * (0.75 + 1.5 * (*value / 2.0));
    p.circle_filled(center, 15.0, Color32::from_rgb(48, 54, 59));
    p.circle_stroke(
        center,
        17.0,
        Stroke::new(1.5_f32, color.gamma_multiply(0.6)),
    );
    p.line_segment(
        [
            center + Vec2::angled(angle) * 6.0,
            center + Vec2::angled(angle) * 13.0,
        ],
        Stroke::new(2.0_f32, TEXT),
    );
    p.text(
        Pos2::new(rect.center().x, rect.top() + 41.0),
        Align2::CENTER_CENTER,
        label,
        FontId::proportional(9.0),
        TEXT,
    );
    p.text(
        Pos2::new(rect.center().x, rect.top() + 55.0),
        Align2::CENTER_CENTER,
        keys,
        FontId::monospace(8.5),
        MUTED,
    );
    response
}

pub struct KnobStyle<'a> {
    pub label: &'a str,
    pub display: &'a str,
    pub keys: &'a str,
    pub color: Color32,
    pub log: bool,
}
pub fn knob(
    ui: &mut egui::Ui,
    value: &mut f32,
    range: std::ops::RangeInclusive<f32>,
    style: KnobStyle<'_>,
) -> Response {
    let KnobStyle {
        label,
        display,
        keys,
        color,
        log,
    } = style;
    let size = Vec2::new(72.0, 86.0);
    let (rect, mut response) = ui.allocate_exact_size(size, Sense::click_and_drag());
    if response.clicked() {
        response.request_focus();
    }
    let (min, max) = (*range.start(), *range.end());
    let normalize = |v: f32| {
        if log {
            (v.max(min) / min).ln() / (max / min).ln()
        } else {
            (v - min) / (max - min)
        }
    };
    let mut t = normalize(*value).clamp(0.0, 1.0);
    if response.dragged() {
        t = (t - response.drag_delta().y * 0.004).clamp(0.0, 1.0);
        *value = if log {
            min * (max / min).powf(t)
        } else {
            min + t * (max - min)
        };
        response.mark_changed();
    }
    if response.has_focus() {
        let step = ui.input(|i| {
            f32::from(i.key_pressed(egui::Key::ArrowUp))
                - f32::from(i.key_pressed(egui::Key::ArrowDown))
        });
        if step != 0.0 {
            t = (t + step * 0.015).clamp(0.0, 1.0);
            *value = if log {
                min * (max / min).powf(t)
            } else {
                min + t * (max - min)
            };
            response.mark_changed();
        }
    }
    if response.double_clicked() {
        *value = if log { max } else { min };
        response.mark_changed();
    }
    let p = ui.painter();
    let center = Pos2::new(rect.center().x, rect.top() + 26.0);
    let r = 19.0;
    p.circle_filled(
        center + Vec2::new(0.0, 2.0),
        r + 2.0,
        Color32::from_rgb(10, 12, 14),
    );
    p.circle_filled(center, r, Color32::from_rgb(52, 58, 63));
    p.circle_stroke(center, r, Stroke::new(1.0_f32, Color32::from_gray(95)));
    p.circle_filled(center, r - 3.0, Color32::from_rgb(38, 43, 47));
    let start = std::f32::consts::PI * 0.75;
    let sweep = std::f32::consts::PI * 1.5;
    for i in 0..=24 {
        let a = start + sweep * i as f32 / 24.0;
        let dir = Vec2::angled(a);
        p.line_segment(
            [center + dir * (r + 5.0), center + dir * (r + 7.0)],
            Stroke::new(
                2.0_f32,
                if i as f32 / 24.0 <= t {
                    color
                } else {
                    Color32::from_gray(65)
                },
            ),
        );
    }
    let dir = Vec2::angled(start + sweep * t);
    p.line_segment(
        [center + dir * 7.0, center + dir * 16.0],
        Stroke::new(2.0_f32, TEXT),
    );
    p.text(
        Pos2::new(rect.center().x, rect.top() + 54.0),
        Align2::CENTER_CENTER,
        label,
        FontId::proportional(11.0),
        TEXT,
    );
    p.text(
        Pos2::new(rect.center().x, rect.top() + 67.0),
        Align2::CENTER_CENTER,
        display,
        FontId::monospace(10.5),
        color,
    );
    p.text(
        Pos2::new(rect.center().x, rect.top() + 80.0),
        Align2::CENTER_CENTER,
        keys,
        FontId::monospace(9.0),
        MUTED,
    );
    if response.has_focus() {
        p.rect_stroke(
            rect,
            4.0,
            Stroke::new(1.0_f32, color),
            egui::StrokeKind::Inside,
        );
    }
    response.on_hover_text(format!(
        "{label}: {display}\n{keys}\nPrzeciągnij pionowo lub użyj klawiszy ↑/↓ po wybraniu Tab"
    ))
}
pub fn waveform(
    ui: &mut egui::Ui,
    peaks: &[[f32; 2]],
    position: f64,
    duration: f64,
    color: Color32,
    bpm: Option<f32>,
    downbeat: f64,
) -> Response {
    let (r, response) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), 48.0), Sense::click());
    let p = ui.painter();
    p.rect_filled(r, 3.0, Color32::from_rgb(17, 21, 24));
    if !peaks.is_empty() {
        for (i, v) in peaks.iter().enumerate() {
            let x = r.left() + i as f32 / peaks.len() as f32 * r.width();
            for (c, peak) in v.iter().enumerate() {
                let mid = r.top() + 12.0 + c as f32 * 23.0;
                let a = peak.min(1.0) * 9.5;
                p.line_segment(
                    [Pos2::new(x, mid - a), Pos2::new(x, mid + a)],
                    Stroke::new(
                        (r.width() / peaks.len() as f32).max(1.0),
                        color.gamma_multiply(0.8),
                    ),
                );
            }
        }
    } else {
        p.text(
            r.center(),
            Align2::CENTER_CENTER,
            "Wybierz utwór · analiza waveform w tle",
            FontId::proportional(11.0),
            MUTED,
        );
    }
    if duration > 0.0 {
        if let Some(bpm) = bpm {
            let beat = 60.0 / bpm as f64;
            let count = (duration / beat).min(2048.0) as usize;
            for i in (0..count).step_by(4) {
                let x = r.left() + ((downbeat + i as f64 * beat) / duration) as f32 * r.width();
                if x <= r.right() {
                    p.line_segment(
                        [Pos2::new(x, r.top()), Pos2::new(x, r.bottom())],
                        Stroke::new(0.5_f32, Color32::from_white_alpha(35)),
                    );
                }
            }
        }
        let x = r.left() + (position / duration).clamp(0.0, 1.0) as f32 * r.width();
        p.line_segment(
            [Pos2::new(x, r.top()), Pos2::new(x, r.bottom())],
            Stroke::new(2.0_f32, TEXT),
        );
    }
    response
}
pub fn meter(ui: &mut egui::Ui, levels: [f32; 2]) {
    let (r, _) = ui.allocate_exact_size(Vec2::new(30.0, 44.0), Sense::hover());
    for (c, level) in levels.iter().enumerate() {
        let db = 20.0 * level.max(1e-6).log10();
        for i in 0..20 {
            let lit = db > -60.0 + i as f32 * 3.0;
            let color = if i > 17 { AMBER } else { GREEN };
            let y = r.bottom() - 14.0 - i as f32 * 1.5;
            ui.painter().rect_filled(
                Rect::from_min_size(
                    Pos2::new(r.left() + c as f32 * 15.0, y),
                    Vec2::new(10.0, 1.0),
                ),
                0.0,
                if lit { color } else { Color32::from_gray(48) },
            );
        }
        ui.painter().text(
            Pos2::new(r.left() + c as f32 * 15.0 + 5.0, r.bottom() - 2.0),
            Align2::CENTER_BOTTOM,
            if c == 0 { "L" } else { "R" },
            FontId::monospace(9.0),
            MUTED,
        );
    }
}
