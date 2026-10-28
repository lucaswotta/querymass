use super::*;
use egui_extras::{Column, TableBuilder};
use querymass::database::ResultSet;

impl App {
    pub(super) fn results(&mut self, ui: &mut egui::Ui) {
        ui.set_min_height(ui.available_height());
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new("Resultados").size(19.0).strong());
            ui.selectable_value(&mut self.messages_tab, false, "Dados");
            ui.selectable_value(&mut self.messages_tab, true, "Mensagens");
            let success = self
                .outputs
                .iter()
                .filter(|output| output.outcome == Outcome::Success)
                .count();
            let attention = self.outputs.len() - success;
            if self.run_total > 0 {
                ui.label(
                    RichText::new(format!(
                        "{success} sucesso · {attention} atenção · {}/{} encerradas",
                        self.completed, self.run_total
                    ))
                    .size(12.0)
                    .color(MUTED),
                );
            }
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
        });
        if self.run.is_some() {
            ui.add(
                egui::ProgressBar::new(self.completed as f32 / self.run_total.max(1) as f32)
                    .text(if self.cancel_requested {
                        "Cancelamento solicitado. Conferindo as consultas já enviadas."
                    } else {
                        "Execução em andamento"
                    })
                    .fill(BLUE),
            );
        }
        ui.separator();
        if self.run_total > 0 {
            ui.collapsing("SQL desta execução", |ui| {
                ui.monospace(&self.run_sql);
            });
        }
        if self.messages_tab {
            self.message_log(ui);
            return;
        }
        if self.outputs.is_empty() {
            self.empty_results(ui);
            return;
        }
        self.selected_output = self.selected_output.min(self.outputs.len() - 1);
        let height = ui.available_height();
        let list_width = if ui.available_width() < 780.0 {
            150.0
        } else {
            190.0
        };
        ui.horizontal_top(|ui| {
            ui.allocate_ui_with_layout(egui::vec2(list_width, height), egui::Layout::top_down(egui::Align::Min), |ui| {
                ui.set_max_width(list_width);
                egui::ScrollArea::vertical().id_salt("result_hosts").auto_shrink([false, false]).show(ui, |ui| {
                    for (index, output) in self.outputs.iter().enumerate() {
                        let card = egui::Frame::new().fill(if index == self.selected_output { Color32::from_rgb(31, 51, 79) } else { Color32::TRANSPARENT }).corner_radius(6).inner_margin(8).show(ui, |ui| {
                            ui.set_min_width(list_width - 25.0);
                            ui.label(RichText::new(&output.machine_name).strong());
                            ui.label(RichText::new(format!("{} · {:.2}s", outcome_label(&output.outcome), output.elapsed.as_secs_f32())).size(11.0).color(outcome_color(&output.outcome)));
                        });
                        if card.response.interact(egui::Sense::click()).clicked() {
                            self.selected_output = index;
                            self.selected_set = 0;
                            self.result_search.clear();
                            self.row_selection.clear();
                            self.sort_column = None;
                        }
                    }
                });
            });
            ui.separator();
            ui.vertical(|ui| {
                let output = &self.outputs[self.selected_output];
                ui.horizontal_wrapped(|ui| {
                    ui.strong(&output.machine_name);
                    ui.label(RichText::new(format!("{} / {}", output.host, output.database)).monospace().size(12.0).color(MUTED));
                });
                if let Some(error) = &output.error {
                    ui.label(RichText::new(error).color(outcome_color(&output.outcome)));
                    if output.outcome == Outcome::Unknown {
                        ui.label(RichText::new("O servidor pode ter aplicado alterações. Confira os dados antes de executar novamente.").color(AMBER));
                    }
                }
                if output.sets.is_empty() {
                    ui.label("Nenhum resultado de dados foi retornado.");
                    return;
                }
                self.selected_set = self.selected_set.min(output.sets.len() - 1);
                if output.sets.len() > 1 {
                    ui.horizontal_wrapped(|ui| {
                        for index in 0..output.sets.len() {
                            if ui.selectable_label(index == self.selected_set, format!("Resultado {}", index + 1)).clicked() {
                                self.selected_set = index;
                                self.row_selection.clear();
                                self.sort_column = None;
                            }
                        }
                    });
                }
                let set = &output.sets[self.selected_set];
                if set.columns.is_empty() {
                    ui.add_space(15.0);
                    ui.label(RichText::new(format!("{} {}", set.affected_rows, if set.affected_rows == 1 { "linha afetada" } else { "linhas afetadas" })).size(24.0).color(GREEN));
                    if let Some(id) = set.last_insert_id.filter(|id| *id > 0) { ui.label(format!("Último ID inserido: {id}")); }
                    if set.warnings > 0 { ui.label(RichText::new(format!("{} avisos do servidor", set.warnings)).color(AMBER)); }
                    return;
                }
                let mut view = DataView {
                    selection: &mut self.row_selection,
                    search: &mut self.result_search,
                    sort_column: &mut self.sort_column,
                    descending: &mut self.sort_descending,
                };
                match data_table(ui, set, (output.machine_id, self.selected_set), &mut view) {
                    Ok(Some(message)) => self.notice = Some(message),
                    Err(error) => self.notice = Some(error),
                    Ok(None) => (),
                }
            });
        });
    }

    fn empty_results(&self, ui: &mut egui::Ui) {
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

struct DataView<'a> {
    selection: &'a mut BTreeSet<usize>,
    search: &'a mut String,
    sort_column: &'a mut Option<usize>,
    descending: &'a mut bool,
}

fn data_table(
    ui: &mut egui::Ui,
    set: &ResultSet,
    id: (u64, usize),
    view: &mut DataView<'_>,
) -> Result<Option<String>, String> {
    let mut select_all = false;
    let mut clear_selection = false;
    let mut copy_selected = false;
    let mut copy_all = false;
    let mut export = false;
    ui.horizontal_wrapped(|ui| {
        ui.add(
            egui::TextEdit::singleline(view.search)
                .hint_text("Filtrar linhas")
                .desired_width(200.0),
        );
        ui.menu_button("Copiar e selecionar", |ui| {
            if ui
                .add_enabled(
                    !view.selection.is_empty(),
                    egui::Button::new("Copiar linhas selecionadas"),
                )
                .clicked()
            {
                copy_selected = true;
                ui.close();
            }
            if ui.button("Copiar tabela inteira").clicked() {
                copy_all = true;
                ui.close();
            }
            ui.separator();
            if ui.button("Selecionar todas as linhas visíveis").clicked() {
                select_all = true;
                ui.close();
            }
            if ui.button("Limpar seleção").clicked() {
                clear_selection = true;
                ui.close();
            }
        });
        if ui.button("Exportar CSV").clicked() {
            export = true;
        }
        ui.label(
            RichText::new(format!(
                "{} linhas · {} selecionadas",
                set.rows.len(),
                view.selection.len()
            ))
            .size(12.0)
            .color(MUTED),
        );
    });
    if set.truncated {
        ui.label(
            RichText::new("Resultado limitado. Restrinja o SQL para consultar outro intervalo.")
                .size(12.0)
                .color(AMBER),
        );
    }
    let search = view.search.to_lowercase();
    let mut visible: Vec<_> = set
        .rows
        .iter()
        .enumerate()
        .filter(|(_, row)| {
            search.is_empty()
                || row
                    .iter()
                    .any(|cell| cell.text.to_lowercase().contains(&search))
        })
        .map(|(index, _)| index)
        .collect();
    if let Some(column) = *view.sort_column {
        sort_rows(set, &mut visible, column, *view.descending);
    }
    let table_height = ui.available_height().max(80.0);
    let column_width = ((ui.available_width() - set.columns.len() as f32 * 10.0)
        / set.columns.len().max(1) as f32)
        .max(140.0);
    let mut clicked_row = None;
    egui::ScrollArea::horizontal()
        .id_salt("result_horizontal")
        .show(ui, |ui| {
            let mut table = TableBuilder::new(ui)
                .id_salt(("data", id))
                .striped(true)
                .resizable(true)
                .sense(egui::Sense::click())
                .max_scroll_height(table_height)
                .min_scrolled_height(80.0);
            for _ in &set.columns {
                table = table.column(Column::initial(column_width).at_least(70.0).clip(true));
            }
            table
                .header(30.0, |mut header| {
                    for (column, title) in set.columns.iter().enumerate() {
                        header.col(|ui| {
                            let sorted = *view.sort_column == Some(column);
                            let label = format!(
                                "{title}{}",
                                if sorted {
                                    if *view.descending { " (Z-A)" } else { " (A-Z)" }
                                } else {
                                    ""
                                }
                            );
                            if ui
                                .selectable_label(sorted, RichText::new(label).strong().color(BLUE))
                                .on_hover_text("Clique para ordenar")
                                .clicked()
                            {
                                *view.descending = sorted && !*view.descending;
                                *view.sort_column = Some(column);
                            }
                        });
                    }
                })
                .body(|body| {
                    body.rows(28.0, visible.len(), |mut row| {
                        let index = visible[row.index()];
                        row.set_selected(view.selection.contains(&index));
                        for cell in &set.rows[index] {
                            row.col(|ui| {
                                let preview: String = cell.text.chars().take(256).collect();
                                let response = ui.add(
                                    egui::Label::new(
                                        RichText::new(preview).color(if cell.is_null {
                                            MUTED
                                        } else {
                                            TEXT
                                        }),
                                    )
                                    .truncate()
                                    .sense(egui::Sense::click()),
                                );
                                response.clone().on_hover_text(
                                    cell.text.chars().take(2048).collect::<String>(),
                                );
                                if response.clicked() {
                                    clicked_row =
                                        Some((index, ui.input(|input| input.modifiers.ctrl)));
                                }
                                response.context_menu(|ui| {
                                    if ui.button("Copiar célula").clicked() {
                                        ui.ctx().copy_text(cell.text.clone());
                                        ui.close();
                                    }
                                    if ui.button("Copiar linha").clicked() {
                                        ui.ctx()
                                            .copy_text(tsv(set, Some(&BTreeSet::from([index]))));
                                        ui.close();
                                    }
                                    if ui.button("Selecionar todas as linhas visíveis").clicked() {
                                        select_all = true;
                                        ui.close();
                                    }
                                    if ui.button("Limpar seleção").clicked() {
                                        clear_selection = true;
                                        ui.close();
                                    }
                                });
                            });
                        }
                        let response = row.response();
                        if response.clicked() && clicked_row.is_none() {
                            clicked_row =
                                Some((index, response.ctx.input(|input| input.modifiers.ctrl)));
                        }
                    });
                });
        });
    if let Some((index, additive)) = clicked_row {
        if !additive {
            view.selection.clear();
        }
        if !view.selection.insert(index) {
            view.selection.remove(&index);
        }
    }
    if select_all {
        *view.selection = visible.into_iter().collect();
    }
    if clear_selection {
        view.selection.clear();
    }
    if copy_selected {
        ui.ctx().copy_text(tsv(set, Some(view.selection)));
    }
    if copy_all {
        ui.ctx().copy_text(tsv(set, None));
    }
    if export
        && let Some(path) = rfd::FileDialog::new()
            .add_filter("CSV", &["csv"])
            .set_file_name("resultado.csv")
            .save_file()
    {
        write_csv(&path, set)?;
        return Ok(Some(format!("Resultado exportado para {}", path.display())));
    }
    Ok(None)
}

fn sort_rows(set: &ResultSet, rows: &mut [usize], column: usize, descending: bool) {
    rows.sort_by(|a, b| {
        let left = &set.rows[*a][column];
        let right = &set.rows[*b][column];
        let order = match (left.text.parse::<f64>(), right.text.parse::<f64>()) {
            (Ok(left), Ok(right))
                if !set.rows[*a][column].is_null && !set.rows[*b][column].is_null =>
            {
                left.total_cmp(&right)
            }
            _ => left.text.cmp(&right.text),
        };
        if descending { order.reverse() } else { order }
    });
}
fn tsv(set: &ResultSet, selection: Option<&BTreeSet<usize>>) -> String {
    let mut writer = csv::WriterBuilder::new()
        .delimiter(b'\t')
        .from_writer(Vec::new());
    writer
        .write_record(&set.columns)
        .expect("writing CSV to memory is infallible");
    for (index, row) in set.rows.iter().enumerate() {
        if selection.is_none_or(|selection| selection.contains(&index)) {
            writer
                .write_record(row.iter().map(|cell| cell.text.as_str()))
                .expect("writing CSV to memory is infallible");
        }
    }
    String::from_utf8(
        writer
            .into_inner()
            .expect("memory writer flush is infallible"),
    )
    .expect("CSV preserves UTF-8")
}
fn write_csv(path: &std::path::Path, set: &ResultSet) -> Result<(), String> {
    let mut writer = csv::Writer::from_path(path).map_err(|error| error.to_string())?;
    writer
        .write_record(&set.columns)
        .map_err(|error| error.to_string())?;
    for row in &set.rows {
        writer
            .write_record(row.iter().map(|cell| cell.text.as_str()))
            .map_err(|error| error.to_string())?;
    }
    writer.flush().map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn sample() -> ResultSet {
        ResultSet {
            columns: vec!["id".into(), "text".into()],
            rows: vec![
                vec![
                    database::Cell {
                        text: "10".into(),
                        is_null: false,
                    },
                    database::Cell {
                        text: "tab\tand\nline".into(),
                        is_null: false,
                    },
                ],
                vec![
                    database::Cell {
                        text: "2".into(),
                        is_null: false,
                    },
                    database::Cell {
                        text: "quoted \"value\"".into(),
                        is_null: false,
                    },
                ],
            ],
            affected_rows: 0,
            last_insert_id: None,
            warnings: 0,
            truncated: false,
        }
    }
    #[test]
    fn clipboard_roundtrips_tabs_newlines_and_selected_rows() {
        let set = sample();
        let text = tsv(&set, Some(&BTreeSet::from([0])));
        let mut reader = csv::ReaderBuilder::new()
            .delimiter(b'\t')
            .from_reader(text.as_bytes());
        let rows: Vec<_> = reader.records().map(Result::unwrap).collect();
        assert_eq!(rows.len(), 1);
        assert_eq!(&rows[0][1], "tab\tand\nline");
    }
    #[test]
    fn sorting_is_numeric_for_numeric_values() {
        let set = sample();
        let mut rows = vec![0, 1];
        sort_rows(&set, &mut rows, 0, false);
        assert_eq!(rows, vec![1, 0]);
        sort_rows(&set, &mut rows, 0, true);
        assert_eq!(rows, vec![0, 1]);
    }
}
