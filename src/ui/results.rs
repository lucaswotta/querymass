use super::*;
use egui_extras::{Column, TableBuilder};
use querymass::database::ResultSet;
impl App {
    pub(super) fn results(&mut self, ui: &mut egui::Ui) {
        ui.set_min_height(ui.available_height());
        ui.horizontal(|ui| {
            ui.label(RichText::new("Resultados").size(19.0).strong());
            ui.selectable_value(&mut self.messages_tab, false, "Dados");
            ui.selectable_value(&mut self.messages_tab, true, "Mensagens");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add_enabled(
                        self.run.is_none() && !self.outputs.is_empty(),
                        egui::Button::new("Limpar saída"),
                    )
                    .clicked()
                {
                    self.outputs.clear();
                    self.logs.clear();
                    self.run_total = 0;
                    self.completed = 0;
                    self.row_selection.clear();
                }
                let success = self
                    .outputs
                    .iter()
                    .filter(|output| output.outcome == Outcome::Success)
                    .count();
                let errors = self
                    .outputs
                    .iter()
                    .filter(|output| output.outcome != Outcome::Success)
                    .count();
                if self.run_total > 0 {
                    ui.label(
                        RichText::new(format!(
                            "{success} sucesso · {errors} falha · {}/{} encerradas",
                            self.completed, self.run_total
                        ))
                        .size(12.0)
                        .color(MUTED),
                    );
                }
            });
        });
        if self.run.is_some() {
            ui.add(
                egui::ProgressBar::new(self.completed as f32 / self.run_total.max(1) as f32)
                    .text(if self.cancel_requested {
                        "Cancelamento solicitado. Conferindo os resultados das consultas enviadas."
                    } else {
                        "Execução em andamento"
                    })
                    .fill(BLUE),
            );
        }
        ui.separator();
        if self.messages_tab {
            self.message_log(ui);
            return;
        }
        if self.outputs.is_empty() {
            if self.run.is_some() {
                ui.spinner();
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for (name, stage) in self.jobs.values() {
                        ui.horizontal(|ui| {
                            ui.label(name);
                            ui.label(RichText::new(stage).color(MUTED));
                        });
                    }
                });
            } else {
                ui.add_space(28.0);
                ui.vertical_centered(|ui| {
                    ui.label(
                        RichText::new("Os resultados vão aparecer aqui")
                            .size(20.0)
                            .strong(),
                    );
                    ui.label(
                        RichText::new("Cada máquina terá sua própria tabela, tempo e status.")
                            .color(MUTED),
                    );
                    ui.add_space(10.0);
                    ui.label(
                        RichText::new("Selecione os alvos  /  escreva SQL  /  execute").color(BLUE),
                    );
                });
            }
            return;
        }
        self.selected_output = self.selected_output.min(self.outputs.len() - 1);
        let height = ui.available_height();
        ui.horizontal_top(|ui|{
            ui.allocate_ui_with_layout(egui::vec2(190.0,height),egui::Layout::top_down(egui::Align::Min),|ui|{
                ui.set_max_width(190.0);egui::ScrollArea::vertical().id_salt("result_hosts").auto_shrink([false,false]).show(ui,|ui|{
                    for (index,output)in self.outputs.iter().enumerate(){let response=egui::Frame::new().fill(if index==self.selected_output{Color32::from_rgb(31,51,79)}else{Color32::TRANSPARENT}).corner_radius(6).inner_margin(8).show(ui,|ui|{
ui.set_min_width(165.0);
ui.label(RichText::new(&output.machine_name).strong());
ui.label(RichText::new(format!("{} · {:.2}s",outcome_label(&output.outcome),output.elapsed.as_secs_f32())).size(11.0).color(outcome_color(&output.outcome)));
});
if response.response.interact(egui::Sense::click()).clicked(){self.selected_output=index;self.selected_set=0;self.result_search.clear();self.row_selection.clear();
}}
                });
            });
            ui.separator();
            ui.vertical(|ui|{
                ui.set_min_width(ui.available_width());
                let output=&self.outputs[self.selected_output];
                ui.horizontal_wrapped(|ui|{
ui.strong(&output.machine_name);
ui.label(RichText::new(format!("{} / {}",output.host,output.database)).monospace().size(12.0).color(MUTED));
});
                if let Some(error)=&output.error{
ui.label(RichText::new(error).color(outcome_color(&output.outcome)));
if output.outcome==Outcome::Unknown{
ui.label(RichText::new("O servidor pode ter aplicado alterações antes da interrupção. Confira os dados antes de executar novamente.").color(AMBER));
}}
                if output.sets.is_empty(){
ui.label("Nenhum resultado de dados foi retornado.");return;
}
                self.selected_set=self.selected_set.min(output.sets.len()-1);
                if output.sets.len()>1{
ui.horizontal_wrapped(|ui|{for index in 0..output.sets.len(){if ui.selectable_label(index==self.selected_set,format!("Resultado {}",index+1)).clicked(){self.selected_set=index;self.row_selection.clear();
}}});
}
                let set=&output.sets[self.selected_set];
                if set.columns.is_empty(){
ui.add_space(15.0);
ui.label(RichText::new(format!("{} linha(s) afetada(s)",set.affected_rows)).size(24.0).color(GREEN));
if let Some(id)=set.last_insert_id{
ui.label(format!("Último ID inserido: {id}"));
}ui.label(format!("{} aviso(s) do servidor",set.warnings));return;
}
                let mut select_all=false;let mut clear_selection=false;let mut copy_selected=false;let mut copy_all=false;let mut export=false;
                ui.horizontal(|ui|{
                    ui.add(egui::TextEdit::singleline(&mut self.result_search).hint_text("Filtrar linhas deste resultado").desired_width(230.0));
                    ui.menu_button("Copiar",|ui|{if ui.add_enabled(!self.row_selection.is_empty(),egui::Button::new("Linhas selecionadas")).clicked(){copy_selected=true;ui.close();
}
if ui.button("Tabela inteira").clicked(){copy_all=true;ui.close();
}});
                    if ui.button("Exportar CSV").clicked(){export=true;
}
                    ui.label(RichText::new(format!("{} linhas",set.rows.len())).size(12.0).color(MUTED));
                });
                if set.truncated{
ui.label(RichText::new("Resultado limitado por quantidade de linhas ou memória. Restrinja o SQL para consultar outro intervalo.").size(12.0).color(AMBER));
}
                let search=self.result_search.to_lowercase();let visible:Vec<_>=set.rows.iter().enumerate().filter(|(_,row)|search.is_empty()||row.iter().any(|cell|cell.text.to_lowercase().contains(&search))).map(|(index,_)|index).collect();
                let table_height=ui.available_height().max(80.0);
                egui::ScrollArea::horizontal().id_salt("result_horizontal").show(ui,|ui| {
                let mut table=TableBuilder::new(ui).id_salt(("data",output.machine_id,self.selected_set)).striped(true).resizable(true).sense(egui::Sense::click()).max_scroll_height(table_height).min_scrolled_height(80.0);
                for _ in &set.columns{table=table.column(Column::initial(145.0).at_least(70.0).clip(true));
}
                table.header(28.0,|mut header|{for title in &set.columns{header.col(|ui|{
ui.label(RichText::new(title).strong().color(BLUE));
});
}}).body(|body|{body.rows(27.0,visible.len(),|mut row|{
                    let index=visible[row.index()];row.set_selected(self.row_selection.contains(&index));
                    for cell in &set.rows[index]{row.col(|ui|{let label=ui.add(egui::Label::new(RichText::new(&cell.text).color(if cell.is_null{MUTED}else{TEXT})).truncate());label.clone().on_hover_text(&cell.text);label.interact(egui::Sense::click()).context_menu(|ui|{if ui.button("Copiar célula").clicked(){
ui.ctx().copy_text(cell.text.clone());
ui.close();
}
if ui.button("Copiar linha").clicked(){
ui.ctx().copy_text(tsv(set,Some(&BTreeSet::from([index]))));
ui.close();
}
if ui.button("Selecionar todas as linhas").clicked(){select_all=true;ui.close();
}
if ui.button("Limpar seleção").clicked(){clear_selection=true;ui.close();
}});
});
}
                    let response=row.response();
if response.clicked(){if !response.ctx.input(|input|input.modifiers.ctrl){self.row_selection.clear();
}
if !self.row_selection.insert(index){self.row_selection.remove(&index);
}}
                });
});
                });
                if select_all{self.row_selection=visible.into_iter().collect();
}
if clear_selection{self.row_selection.clear();
}
                if copy_selected{
ui.ctx().copy_text(tsv(set,Some(&self.row_selection)));
}
if copy_all{
ui.ctx().copy_text(tsv(set,None));
}
                if export&& let Some(path)=rfd::FileDialog::new().add_filter("CSV",&["csv"]).set_file_name("resultado.csv").save_file(){match write_csv(&path,set){Ok(())=>self.notice=Some(format!("Resultado exportado para {}",path.display())),Err(error)=>self.notice=Some(error)}}
            });
        });
    }
    fn message_log(&mut self, ui: &mut egui::Ui) {
        let text = self.logs.join("\n\n");
        let mut view = text.as_str();
        let mut editor = egui::TextEdit::multiline(&mut view)
            .code_editor()
            .desired_rows((ui.available_height() / 20.0).max(4.0) as usize)
            .desired_width(f32::INFINITY)
            .hint_text("As etapas e os erros da execução aparecerão aqui.")
            .show(ui);
        let mut all = false;
        let mut clear = false;
        editor.response.context_menu(|ui| {
            if ui.button("Copiar mensagens").clicked() {
                ui.ctx().copy_text(text.clone());
                ui.close();
            }
            if ui.button("Selecionar tudo").clicked() {
                all = true;
                ui.close();
            }
            if ui
                .add_enabled(self.run.is_none(), egui::Button::new("Limpar mensagens"))
                .clicked()
            {
                clear = true;
                ui.close();
            }
        });
        if all {
            editor
                .state
                .cursor
                .set_char_range(Some(egui::text::CCursorRange::select_all(&editor.galley)));
            editor.state.store(ui.ctx(), editor.response.id);
            editor.response.request_focus();
        }
        if clear {
            self.logs.clear();
        }
    }
}
fn tsv(set: &ResultSet, selection: Option<&BTreeSet<usize>>) -> String {
    let mut writer = csv::WriterBuilder::new()
        .delimiter(b'\t')
        .from_writer(Vec::new());
    if writer.write_record(&set.columns).is_err() {
        return String::new();
    }
    for (index, row) in set.rows.iter().enumerate() {
        if selection.is_none_or(|selection| selection.contains(&index))
            && writer
                .write_record(row.iter().map(|cell| cell.text.as_str()))
                .is_err()
        {
            return String::new();
        }
    }
    String::from_utf8(writer.into_inner().unwrap_or_default()).unwrap_or_default()
}
fn write_csv(path: &std::path::Path, set: &ResultSet) -> Result<(), String> {
    let mut writer = csv::Writer::from_path(path).map_err(|e| e.to_string())?;
    writer
        .write_record(&set.columns)
        .map_err(|e| e.to_string())?;
    for row in &set.rows {
        writer
            .write_record(row.iter().map(|cell| cell.text.as_str()))
            .map_err(|e| e.to_string())?;
    }
    writer.flush().map_err(|e| e.to_string())
}
