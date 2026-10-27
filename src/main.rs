use eframe::egui;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([900.0, 600.0]),
        ..Default::default()
    };

    eframe::run_native(
        "QueryMass",
        options,
        Box::new(|_creation_context| Ok(Box::<QueryMassApp>::default())),
    )
}

#[derive(Default)]
struct QueryMassApp {
    query: String,
}

impl eframe::App for QueryMassApp {
    fn update(&mut self, context: &egui::Context, _frame: &mut eframe::Frame) {
        apply_querymass_theme(context);

        egui::TopBottomPanel::top("toolbar").show(context, |ui| {
            ui.horizontal(|ui| {
                ui.add_space(4.0);
                draw_brand_mark(ui);
                ui.label(egui::RichText::new("QueryMass").strong().size(16.0));
                ui.label(egui::RichText::new("MySQL").color(color(0x969EB2)));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_enabled(
                        false,
                        egui::Button::new("Executar consulta").fill(color(0x4F8CFF)),
                    );
                });
            });
        });

        egui::SidePanel::left("machines")
            .resizable(true)
            .default_width(260.0)
            .width_range(200.0..=380.0)
            .show(context, |ui| {
                ui.label(egui::RichText::new("MÁQUINAS").size(11.0).strong().color(color(0x969EB2)));
                ui.add_space(10.0);
                ui.separator();
                ui.vertical_centered(|ui| {
                    ui.add_space(34.0);
                    ui.label(egui::RichText::new("▣").size(24.0).color(color(0x697187)));
                    ui.add_space(8.0);
                    ui.label(egui::RichText::new("Nenhuma máquina").strong());
                    ui.label(egui::RichText::new("Adicione uma conexão MySQL para começar.").size(12.0).color(color(0x969EB2)));
                    ui.add_space(14.0);
                    ui.add_enabled(false, egui::Button::new("＋  Adicionar máquina"));
                });
            });

        egui::TopBottomPanel::bottom("output")
            .resizable(true)
            .default_height(220.0)
            .height_range(120.0..=420.0)
            .show(context, |ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("SAÍDA").size(11.0).strong().color(color(0x969EB2)));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(egui::RichText::new("Aguardando execução").size(12.0).color(color(0x697187)));
                    });
                });
                ui.separator();
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.add_space(4.0);
                    ui.monospace(
                        egui::RichText::new("Os resultados tabulares, linhas afetadas e mensagens por máquina aparecerão aqui.")
                            .color(color(0xA49AAE)),
                    );
                });
            });

        egui::CentralPanel::default().show(context, |ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("EDITOR SQL").size(11.0).strong().color(color(0x969EB2)));
                ui.label(egui::RichText::new("·  MySQL").size(12.0).color(color(0x697187)));
            });
            ui.add_space(8.0);
            ui.add_sized(
                ui.available_size(),
                egui::TextEdit::multiline(&mut self.query)
                    .code_editor()
                    .hint_text("SELECT * FROM sua_tabela LIMIT 100;")
                    .desired_width(f32::INFINITY),
            );
        });
    }
}

fn apply_querymass_theme(context: &egui::Context) {
    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = color(0x171923);
    visuals.window_fill = color(0x202330);
    visuals.extreme_bg_color = color(0x10121A);
    visuals.faint_bg_color = color(0x1D202B);
    visuals.code_bg_color = color(0x10121A);
    visuals.selection.bg_fill = color(0x20365C);
    visuals.selection.stroke.color = color(0x4F8CFF);
    visuals.widgets.noninteractive.bg_fill = color(0x1D202B);
    visuals.widgets.noninteractive.fg_stroke.color = color(0xF2F4FA);
    visuals.widgets.inactive.bg_fill = color(0x2D3241);
    visuals.widgets.inactive.fg_stroke.color = color(0xF2F4FA);
    visuals.widgets.hovered.bg_fill = color(0x39445A);
    visuals.widgets.hovered.fg_stroke.color = color(0xFFF9FF);
    visuals.widgets.active.bg_fill = color(0x4F8CFF);
    visuals.widgets.active.fg_stroke.color = color(0xFFFFFF);
    visuals.hyperlink_color = color(0x75A5FF);
    context.set_visuals(visuals);
}

fn draw_brand_mark(ui: &mut egui::Ui) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(28.0, 28.0), egui::Sense::hover());
    let painter = ui.painter();
    let blue = color(0x4F8CFF);
    let red = color(0xFF5D70);
    let origin = rect.min;
    let nodes = [
        egui::pos2(origin.x + 5.0, origin.y + 6.0),
        egui::pos2(origin.x + 5.0, origin.y + 14.0),
        egui::pos2(origin.x + 5.0, origin.y + 22.0),
    ];
    let junction = egui::pos2(origin.x + 15.0, origin.y + 14.0);
    let query = egui::pos2(origin.x + 23.0, origin.y + 14.0);

    for node in nodes {
        painter.line_segment([node, junction], egui::Stroke::new(1.5_f32, red));
        painter.circle_filled(node, 2.2, red);
    }
    painter.line_segment([junction, query], egui::Stroke::new(1.8_f32, blue));
    painter.circle_filled(query, 3.0, blue);
    painter.circle_stroke(query, 4.7, egui::Stroke::new(1.2_f32, blue));
    painter.line_segment(
        [egui::pos2(query.x - 1.3, query.y), egui::pos2(query.x + 1.3, query.y)],
        egui::Stroke::new(1.0_f32, color(0xF2F4FA)),
    );
    painter.line_segment(
        [egui::pos2(query.x, query.y - 1.3), egui::pos2(query.x, query.y + 1.3)],
        egui::Stroke::new(1.0_f32, color(0xF2F4FA)),
    );
}

fn color(hex: u32) -> egui::Color32 {
    egui::Color32::from_rgb(
        ((hex >> 16) & 0xFF) as u8,
        ((hex >> 8) & 0xFF) as u8,
        (hex & 0xFF) as u8,
    )
}
