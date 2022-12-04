use crate::{config::KBConfig, threading::HIDThreadState};
use crossbeam::channel::Receiver;
use eframe::epaint::{RectShape, TextShape};
use egui::{
    text::LayoutJob, Align, Color32, FontFamily, FontId, Painter, Pos2, Rect, Rounding, Stroke,
    TextFormat, Ui, Vec2,
};
use std::sync::Arc;

pub struct App {
    rx: Receiver<HIDThreadState>,
    kb_config: Arc<KBConfig>,
    curr_state: HIDThreadState,
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
            let (fg, bg) =
                if self.curr_state.matrix[key.matrix.0 as usize][key.matrix.1 as usize].is_down {
                    (Color32::BLACK, Color32::GRAY)
                } else {
                    (Color32::BLACK, Color32::WHITE)
                };

            let key_min = Pos2 {
                x: key.x * scale,
                y: key.y * scale,
            };

            let key_max = Pos2 {
                x: key_min.x + key.w * scale,
                y: key_min.y + key.h * scale,
            };

            let key_rect = Rect {
                min: key_min,
                max: key_max,
            };

            let translate = clip_rect.left_top().to_vec2();

            painter.add(RectShape {
                rect: key_rect.translate(translate),
                rounding: Rounding::same(0.1 * scale),
                fill: bg,
                stroke: Stroke::new(0.025 * scale, Color32::GRAY),
            });

            let mut job = LayoutJob::default();
            job.append(
                &key.label,
                0.0,
                TextFormat {
                    font_id: FontId::new(14.0, FontFamily::Proportional),
                    color: fg,
                    valign: Align::Center,
                    ..Default::default()
                },
            );

            let galley = ui.fonts().layout_job(job);

            let margin = 0.1 * scale;

            painter.add(TextShape {
                pos: key_min + Vec2::new(margin, margin) + translate,
                galley,
                underline: Stroke::none(),
                override_text_color: None,
                angle: 0.0,
            });
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
