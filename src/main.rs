#![windows_subsystem = "windows"]

use eframe::egui;
use proc_maps::get_process_maps;
use process_memory::{DataMember, Memory, Pid, ProcessHandle, TryIntoProcessHandle};
use sysinfo::System;
use anyhow::{Context, Result};
use egui::Color32;

fn main() {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([200.0, 100.0])
            .with_always_on_top()
            .with_transparent(true),
        ..Default::default()
    };

    let _ = eframe::run_native(
        "Penalty Timer",
        options,
        Box::new(|_cc| Ok(Box::new(PenaltyTimer::new()))),
    );
}

struct PenaltyTimer {
    pid: Option<Pid>,
    handle: Option<ProcessHandle>,
}

impl PenaltyTimer {
    fn new() -> Self {
        Self {
            pid: None,
            handle: None,
        }
    }
    fn attempt_attach(&mut self) {
        if let Ok((pid, handle)) = get_process_pid_and_handle() {
            self.pid = Some(pid);
            self.handle = Some(handle);
        }
    }
}

fn draw_text(ctx: &egui::Context, text: &str, color: Option<Color32>) {
    egui::CentralPanel::default().show(ctx, |ui| {
        let available = ui.available_size();
        let char_count = text.chars().count() as f32;
        let char_aspect_ratio = 0.6;
        let size_based_on_width = (available.x * 0.9) / (char_count * char_aspect_ratio);
        let size_based_on_height = available.y * 0.8;
        let font_size = size_based_on_width.min(size_based_on_height);
        ui.centered_and_justified(|ui| {
            ui.label(egui::RichText::new(text)
                .size(font_size)
                .monospace()
                .color(color.unwrap_or(Color32::LIGHT_GRAY)));
        });
    });
    ctx.request_repaint();
}

impl eframe::App for PenaltyTimer {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.pid.is_none() || self.handle.is_none() {
            self.attempt_attach();
            draw_text(ctx, "Unattached", None);
            return;
        }
        let pid = match self.pid {
            Some(pid) => pid,
            None => {
                draw_text(ctx, "Unattached", None);
                return
            },
        };
        let game_assembly = match get_module_base(pid) {
            Some(module) => module,
            None => {
                self.attempt_attach();
                draw_text(ctx, "Unattached", None);
                return
            },
        };
        let handle = match self.handle {
            Some(handle) => handle,
            None => {
                draw_text(ctx, "Unattached", None);
                return
            },
        };

        let (penalty_start_frame, round_time) =
            unsafe {(
                get_penalty_start_frame(handle, game_assembly),
                get_round_time(handle, game_assembly)
            )};

        let Some(diff) = penalty_start_frame.zip(round_time).map(|(p, r)| p-r) else {
            draw_text(ctx, "N/A", None);
            return;
        };

        let color = match diff {
            0..=180 => Color32::RED,
            181..=360 => Color32::ORANGE,
            _ => Color32::DARK_GREEN,
        };

        if diff >= 0 {
            draw_text(ctx, &diff.to_string(), Some(color));
        } else {
            draw_text(ctx, "??", None);
        }
    }
}

fn get_process_pid_and_handle() -> Result<(Pid, ProcessHandle)> {
    let sys = System::new_all();
    let process = sys.processes_by_exact_name("BloonsTD6.exe".as_ref()).next().context("Couldn't find BloonsTD6.exe")?;
    let pid = process.pid().as_u32() as Pid;
    let handle = pid.try_into_process_handle()?;
    Ok((pid, handle))
}

unsafe fn get_penalty_start_frame(handle: ProcessHandle, game_assembly: usize) -> Option<i32> {
    let mut penalty_start_frame = DataMember::<i32>::new(handle);
    penalty_start_frame.set_offset(vec![
        game_assembly + 0x48D2988,
        0xB8,
        0x0,
        0xD0,
        0x28,
        0x270,
        0x98,
        0xB0,
        0x44
    ]);
    penalty_start_frame.read().ok()
}

unsafe fn get_round_time(handle: ProcessHandle, game_assembly: usize) -> Option<i32> {
    let mut round_time = DataMember::<i32>::new(handle);
    round_time.set_offset(vec![
        game_assembly + 0x48D2988,
        0xB8,
        0x0,
        0xD0,
        0x28,
        0x30,
        0x10
    ]);
    round_time.read().ok()
}

fn get_module_base(pid: Pid) -> Option<usize> {
    let maps = get_process_maps(pid).ok()?;
    for map in maps {
        if let Some(path) = map.filename() {
            if path.to_string_lossy().contains("GameAssembly.dll") {
                return Some(map.start());
            }
        }
    }
    None
}
