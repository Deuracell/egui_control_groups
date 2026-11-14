//! `cargo run --example demo`
//!
//! Joined toggle rows and multi-edit fields. Try:
//!  * pressing a toggle and dragging across its neighbours,
//!  * pressing in one field and dragging across others, then typing a value.

mod app;

fn main() -> eframe::Result<()> {
    let mut demo = app::Demo::default();
    eframe::run_ui_native(
        "egui-control-groups",
        eframe::NativeOptions::default(),
        move |ui, _frame| {
            egui::CentralPanel::default().show(ui, |ui| demo.ui(ui));
        },
    )
}
