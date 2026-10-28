mod ui;
fn main() -> eframe::Result {
    let path = querymass::model::config_path();
    let workspace_lock = match querymass::model::lock_workspace(&path) {
        Ok(lock) => lock,
        Err(error) => {
            rfd::MessageDialog::new()
                .set_title("QueryMass")
                .set_description(error)
                .set_level(rfd::MessageLevel::Warning)
                .show();
            return Ok(());
        }
    };
    eframe::run_native(
        "QueryMass",
        eframe::NativeOptions {
            viewport: eframe::egui::ViewportBuilder::default()
                .with_inner_size([1440.0, 940.0])
                .with_min_inner_size([1080.0, 720.0]),
            ..Default::default()
        },
        Box::new(|cc| Ok(Box::new(ui::App::new(cc, path, workspace_lock)))),
    )
}
