use crate::{
    config::{KBConfig, KeyUsage},
    threading::HIDThreadState,
};
use crossbeam::channel::Receiver;
use eframe::epaint::{RectShape, TextShape};
use egui::{
    color::Hsva, text::LayoutJob, Color32, FontFamily, FontId, Painter, Pos2, Rect, Rounding,
    Stroke, Ui, Vec2,
};
use std::sync::Arc;

pub struct App {
    rx: Receiver<HIDThreadState>,
    kb_config: Arc<KBConfig>,
    curr_state: HIDThreadState,
}

const REMOVED_COLOR: Color32 = Color32::from_rgb(20, 20, 20);

// unpressed, pressed, foreground
fn get_key_colors(usage: &KeyUsage) -> (Color32, Color32, Color32) {
    match usage {
        KeyUsage::Removed => (REMOVED_COLOR, REMOVED_COLOR, Color32::TRANSPARENT),
        KeyUsage::Modtap | KeyUsage::Modifier => (
            Color32::from_rgb(68, 51, 127),
            Color32::from_rgb(27, 20, 51),
            Color32::WHITE,
        ),
        KeyUsage::Layertap | KeyUsage::Layer => (
            Color32::from_rgb(127, 51, 51),
            Color32::from_rgb(51, 20, 20),
            Color32::WHITE,
        ),
        KeyUsage::Function => (
            Color32::from_rgb(51, 57, 127),
            Color32::from_rgb(20, 22, 51),
            Color32::WHITE,
        ),
        KeyUsage::Mouse => (
            Color32::from_rgb(51, 127, 100),
            Color32::from_rgb(20, 51, 40),
            Color32::WHITE,
        ),
        KeyUsage::Passthrough => unreachable!(),
        _ => (
            Color32::from_rgb(90, 90, 90),
            Color32::from_rgb(50, 50, 50),
            Color32::WHITE,
        ),
    }
}

impl App {
    pub fn new(rx: Receiver<HIDThreadState>, kb_config: Arc<KBConfig>) -> App {
        App {
            rx,
            kb_config,
            curr_state: Default::default(),
        }
    }

    fn info_window(&self, ctx: &egui::Context) {
        egui::Window::new("Information").show(ctx, |ui| {
            egui::Grid::new("info_grid")
                .num_columns(2)
                .spacing([40.0, 4.0])
                .striped(true)
                .show(ui, |ui| {
                    let qmk_info = &self.kb_config.qmk_info;

                    ui.label("Keyboard");
                    ui.label(&qmk_info.keyboard_name);
                    ui.end_row();

                    ui.label("Vendor");
                    ui.label(&qmk_info.manufacturer);
                    ui.end_row();

                    ui.label("Vendor ID");
                    ui.label(format!("{:#06X}", qmk_info.usb.vid));
                    ui.end_row();

                    ui.label("Product ID");
                    ui.label(format!("{:#06X}", qmk_info.usb.pid));
                    ui.end_row();

                    ui.label("LED count");
                    ui.label(format!("{}", self.kb_config.led_count()));
                    ui.end_row();

                    ui.label("HID delta update");
                    ui.label(format!("{:.2}", self.curr_state.delta_update * 1000.0));
                    ui.end_row();

                    ui.label("HID delta frame");
                    ui.label(format!("{:.2}", self.curr_state.delta_frame * 1000.0));
                    ui.end_row();

                    ui.label("HID FPS");
                    ui.label(format!("{:.2}", 1.0 / self.curr_state.delta_frame));
                    ui.end_row();
                });
        });
    }

    fn keyboard_render(&self, ui: &mut Ui, scale: f32) {
        let layout = self.kb_config.layout();

        let min = ui.next_widget_position();
        let max = Pos2 {
            x: min.x + self.kb_config.width() * scale,
            y: min.y + self.kb_config.height() * scale,
        };

        let clip_rect = Rect { min, max };
        let painter = Painter::new(ui.ctx().clone(), ui.layer_id(), clip_rect);

        for key in &layout.layout {
            let key_def = self
                .kb_config
                .legends
                .get_key(self.curr_state.layer_state, key.matrix.0, key.matrix.1)
                .expect("could not find key definition");

            let (bg_norm, bg_pressed, fg) = get_key_colors(&key_def.usage);
            let bg = if self.curr_state.matrix[key.matrix.0 as usize][key.matrix.1 as usize]
                .is_pressed
            {
                bg_pressed
            } else {
                bg_norm
            };

            // bounds
            let key_border = 0.03 * scale;
            let key_shrink = Vec2::new(key_border, key_border);

            let key_min = Vec2::new(key.x * scale, key.y * scale) + key_shrink;
            let key_max =
                Vec2::new(key_min.x + key.w * scale, key_min.y + key.h * scale) - key_shrink;

            let key_rect = Rect {
                min: key_min.to_pos2(),
                max: key_max.to_pos2(),
            };

            // led
            let led_index = self.kb_config.matrix[key.matrix.0 as usize][key.matrix.1 as usize];
            let border_color = if led_index >= 0 {
                let color = self.curr_state.led_state[led_index as usize];
                Hsva::new(
                    color.hue.to_degrees() / 360.0,
                    color.saturation,
                    color.value * color.alpha,
                    1.0,
                )
            } else {
                Hsva::new(0.0, 0.0, 0.0, 0.0)
            };

            let translate = clip_rect.left_top().to_vec2();

            painter.add(RectShape {
                rect: key_rect.translate(translate),
                rounding: Rounding::same(0.1 * scale),
                fill: bg,
                stroke: Stroke::new(key_border, border_color),
            });

            // legend
            if let Some(legend) = key_def.label.as_ref() {
                let text_margin = 0.1 * scale;

                let job = LayoutJob::simple(
                    legend.to_string(),
                    FontId::new(14.0, FontFamily::Proportional),
                    fg,
                    key_rect.width() - 2.0 * text_margin,
                );

                let galley = ui.fonts().layout_job(job);

                painter.add(TextShape {
                    pos: (key_min + Vec2::new(text_margin, text_margin) + translate).to_pos2(),
                    galley,
                    underline: Stroke::none(),
                    override_text_color: None,
                    angle: 0.0,
                });
            }
        }

        ui.expand_to_include_rect(painter.clip_rect());
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if let Some(last) = self.rx.try_iter().last() {
            self.curr_state = last;
        }

        self.info_window(ctx);

        egui::Window::new("Keyboard").show(ctx, |ui| {
            self.keyboard_render(ui, 45.0);
        });

        egui::CentralPanel::default().show(ctx, |_ui| {});

        ctx.request_repaint();
    }
}
