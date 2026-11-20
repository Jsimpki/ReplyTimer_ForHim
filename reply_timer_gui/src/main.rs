//! Reply Timer (GUI) — a countdown that tells you when it's okay to text back.

// Hide the console window on Windows release builds.
#![cfg_attr(all(target_os = "windows", not(debug_assertions)), windows_subsystem = "windows")]

mod core;

use core::{chime_path, fmt_clock, parse_duration, play_async, warning_path, Mode};
use eframe::egui::{self, Color32, RichText};
use std::time::{Duration, Instant};

const BG: Color32 = Color32::from_rgb(18, 16, 28);
const PURPLE: Color32 = Color32::from_rgb(200, 130, 255);
const CYAN: Color32 = Color32::from_rgb(90, 220, 255);
const GREEN: Color32 = Color32::from_rgb(46, 160, 100);
const AMBER: Color32 = Color32::from_rgb(200, 140, 30);
const BLUE: Color32 = Color32::from_rgb(60, 110, 200);
const RED: Color32 = Color32::from_rgb(200, 30, 40);

enum Screen {
    Menu,
    SlowInput,
    Counting { mode: Mode, start: Instant, total: Duration },
    Done { since: Instant },
    Cold { since: Instant },
}

struct App {
    screen: Screen,
    slow_text: String,
    error: String,
}

impl App {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        cc.egui_ctx.set_visuals(egui::Visuals::dark());
        Self { screen: Screen::Menu, slow_text: String::new(), error: String::new() }
    }
}

fn start_count(mode: Mode) -> Screen {
    let total = mode.duration();
    Screen::Counting { mode, start: Instant::now(), total }
}

fn big_button(ui: &mut egui::Ui, text: &str, fill: Color32) -> bool {
    let b = egui::Button::new(RichText::new(text).size(20.0).strong().color(Color32::WHITE))
        .fill(fill)
        .min_size(egui::vec2(ui.available_width(), 60.0));
    ui.add(b).clicked()
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let mut screen = std::mem::replace(&mut self.screen, Screen::Menu);
        let mut next: Option<Screen> = None;

        // Flash timer for the alert screens
        let flash = |since: &Instant| (since.elapsed().as_secs_f32() * 2.0) as i64 % 2 == 0;

        let fill = match &screen {
            Screen::Cold { since } => if flash(since) { RED } else { Color32::from_rgb(120, 0, 10) },
            Screen::Done { since } => if flash(since) { Color32::from_rgb(20, 110, 60) } else { Color32::from_rgb(12, 70, 40) },
            _ => BG,
        };

        egui::CentralPanel::default()
            .frame(egui::Frame::default().fill(fill).inner_margin(24.0))
            .show(ctx, |ui| match &mut screen {
                Screen::Menu => {
                    ui.vertical_centered(|ui| {
                        ui.add_space(6.0);
                        ui.label(RichText::new("REPLY TIMER").size(34.0).strong().color(PURPLE));
                        ui.label(RichText::new("don't be the first one to crack").weak());
                    });
                    ui.add_space(22.0);
                    ui.spacing_mut().item_spacing.y = 14.0;
                    if big_button(ui, "Casual / low stakes  -  20 min", GREEN) {
                        next = Some(start_count(Mode::Casual));
                    }
                    if big_button(ui, "She asked something real  -  30 min", AMBER) {
                        next = Some(start_count(Mode::RealQuestion));
                    }
                    if big_button(ui, "She's been slow  -  you set the time", BLUE) {
                        self.slow_text.clear();
                        self.error.clear();
                        next = Some(Screen::SlowInput);
                    }
                    if big_button(ui, "She went cold", RED) {
                        play_async(warning_path(), 1);
                        next = Some(Screen::Cold { since: Instant::now() });
                    }
                }

                Screen::SlowInput => {
                    ui.vertical_centered(|ui| {
                        ui.add_space(20.0);
                        ui.label(RichText::new("How long did she take to reply?").size(24.0).strong().color(CYAN));
                        ui.add_space(6.0);
                        ui.label(RichText::new("e.g.  45   2h   1h30m   1:30   90s   (a bare number = minutes)").weak());
                        ui.add_space(18.0);
                        let resp = ui.add(
                            egui::TextEdit::singleline(&mut self.slow_text)
                                .hint_text("her response time")
                                .font(egui::TextStyle::Heading)
                                .desired_width(280.0),
                        );
                        resp.request_focus();
                        let enter = resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                        ui.add_space(14.0);
                        if !self.error.is_empty() {
                            ui.label(RichText::new(&self.error).color(Color32::from_rgb(255, 110, 110)));
                            ui.add_space(8.0);
                        }
                        let go = ui
                            .add(egui::Button::new(RichText::new("Start timer").size(20.0).strong().color(Color32::WHITE))
                                .fill(BLUE)
                                .min_size(egui::vec2(220.0, 48.0)))
                            .clicked();
                        if go || enter {
                            match parse_duration(&self.slow_text) {
                                Some(d) => next = Some(start_count(Mode::Slow(d))),
                                None => self.error = format!("Couldn't read \"{}\". Try 45, 2h, or 1h30m.", self.slow_text.trim()),
                            }
                        }
                        ui.add_space(8.0);
                        if ui.button("Back").clicked() {
                            next = Some(Screen::Menu);
                        }
                    });
                }

                Screen::Counting { mode, start, total } => {
                    let elapsed = start.elapsed();
                    if elapsed >= *total {
                        play_async(chime_path(), 3);
                        next = Some(Screen::Done { since: Instant::now() });
                    } else {
                        let remaining = *total - elapsed;
                        let frac = (elapsed.as_secs_f32() / total.as_secs_f32()).clamp(0.0, 1.0);
                        let color = if remaining.as_secs() <= 60 { Color32::from_rgb(255, 210, 70) } else { CYAN };
                        ui.vertical_centered(|ui| {
                            ui.add_space(10.0);
                            ui.label(RichText::new("REPLY TIMER").size(20.0).strong().color(PURPLE));
                            ui.label(RichText::new(mode.label()).weak());
                            ui.add_space(30.0);
                            ui.label(RichText::new(fmt_clock(remaining)).size(104.0).monospace().strong().color(color));
                            ui.add_space(24.0);
                            ui.add(egui::ProgressBar::new(frac).desired_width(380.0).fill(color));
                            ui.add_space(4.0);
                            ui.label(RichText::new(format!("{:.0}% of the wait done", frac * 100.0)).weak());
                            ui.add_space(26.0);
                            ui.label(RichText::new(mode.tip()).size(18.0).color(Color32::from_rgb(120, 230, 150)));
                            ui.add_space(30.0);
                            if ui.button("Cancel").clicked() {
                                next = Some(Screen::Menu);
                            }
                        });
                        ctx.request_repaint_after(Duration::from_millis(100));
                    }
                }

                Screen::Done { .. } => {
                    ui.vertical_centered(|ui| {
                        ui.add_space(80.0);
                        ui.label(RichText::new("TIME'S UP").size(56.0).strong().color(Color32::WHITE));
                        ui.add_space(10.0);
                        ui.label(RichText::new("YOU MAY REPLY").size(36.0).strong().color(Color32::WHITE));
                        ui.add_space(24.0);
                        ui.label(RichText::new("send it. then put the phone down.").size(18.0).color(Color32::from_rgb(210, 255, 225)));
                        ui.add_space(50.0);
                        if ui.button("Back to menu").clicked() {
                            next = Some(Screen::Menu);
                        }
                    });
                    ctx.request_repaint_after(Duration::from_millis(100));
                }

                Screen::Cold { .. } => {
                    ui.vertical_centered(|ui| {
                        ui.add_space(50.0);
                        for line in ["DO NOT RESPOND", "OR TEXT HER", "UNTIL SHE COMES", "TO YOU"] {
                            ui.label(RichText::new(line).size(52.0).strong().color(Color32::WHITE));
                        }
                        ui.add_space(50.0);
                        if ui.button("Back to menu").clicked() {
                            next = Some(Screen::Menu);
                        }
                    });
                    ctx.request_repaint_after(Duration::from_millis(100));
                }
            });

        self.screen = next.unwrap_or(screen);
    }
}

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([560.0, 640.0])
            .with_min_inner_size([480.0, 560.0])
            .with_title("Reply Timer"),
        ..Default::default()
    };
    eframe::run_native("Reply Timer", options, Box::new(|cc| Ok(Box::new(App::new(cc)))))
}
