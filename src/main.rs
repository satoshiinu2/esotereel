mod app;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 720.0])
            .with_title("MusclEdit"),
        ..Default::default()
    };

    eframe::run_native(
        "MusclEdit",
        options,
        Box::new(|_cc| Ok(Box::new(app::App::default()))),
    )
}
