//! Web build of the demo, served as a GitHub page. Build with `trunk build`.

#[path = "../../examples/demo/app.rs"]
mod app;

struct App(app::Demo);

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ui, |ui| self.0.ui(ui));
    }
}

#[cfg(target_arch = "wasm32")]
fn main() {
    use eframe::wasm_bindgen::JsCast as _;

    wasm_bindgen_futures::spawn_local(async {
        let canvas = web_sys::window()
            .and_then(|w| w.document())
            .and_then(|d| d.get_element_by_id("the_canvas_id"))
            .and_then(|e| e.dyn_into::<web_sys::HtmlCanvasElement>().ok())
            .expect("canvas #the_canvas_id not found");
        eframe::WebRunner::new()
            .start(
                canvas,
                eframe::WebOptions::default(),
                Box::new(|_cc| Ok(Box::new(App(app::Demo::default())))),
            )
            .await
            .expect("failed to start eframe");
    });
}

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    eprintln!("This crate is the web demo: build it with `trunk build` (wasm32-unknown-unknown).");
}
