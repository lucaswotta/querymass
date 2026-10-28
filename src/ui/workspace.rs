use super::*;
impl App {
    pub(super) fn top(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("header")
            .frame(
                egui::Frame::new()
                    .fill(PANEL)
                    .inner_margin(egui::Margin::symmetric(20, 14)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    let (rect, _) =
                        ui.allocate_exact_size(egui::vec2(34.0, 34.0), egui::Sense::hover());
                    let p = ui.painter();
                    let target = rect.center() + egui::vec2(10.0, 0.0);
                    for offset in [-10.0, 0.0, 10.0] {
                        let source = rect.center() + egui::vec2(-10.0, offset);
                        p.line_segment([source, target], egui::Stroke::new(2.0_f32, RED));
                        p.circle_filled(source, 3.0, RED);
                    }
                    p.circle_filled(target, 5.0, BLUE);
                    ui.vertical(|ui| {
                        ui.label(RichText::new("QueryMass").size(23.0).strong());
                        ui.label(
                            RichText::new("Um SQL. Todas as suas máquinas.")
                                .size(12.0)
                                .color(MUTED),
                        );
                    });
                    ui.add_space(28.0);
                    ui.label(RichText::new("ESPAÇO DE TRABALHO").size(11.0).color(MUTED));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let connection_button = ui.button("Conexões").on_hover_text(
                            "Adicionar, importar e configurar máquinas e credenciais",
                        );
                        if connection_button.clicked() {
                            self.open_manager(ManagerTab::Machine);
                        }
                        if ui.button("Histórico").clicked() {
                            self.history_open = true;
                        }
                    });
                });
            });
        if self.notice.is_some() || self.recovery {
            egui::TopBottomPanel::top("notice")
                .frame(
                    egui::Frame::new()
                        .fill(Color32::from_rgb(49, 39, 30))
                        .inner_margin(egui::Margin::symmetric(20, 10)),
                )
                .show(ctx, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(
                            RichText::new(self.notice.clone().unwrap_or_default()).color(AMBER),
                        );
                        if self.recovery {
                            if ui.button("Preservar original e iniciar novo").clicked() {
                                let backup = self.config_path.with_extension(format!(
                                    "backup-{}",
                                    SystemTime::now()
                                        .duration_since(UNIX_EPOCH)
                                        .unwrap_or_default()
                                        .as_secs()
                                ));
                                match std::fs::copy(&self.config_path, &backup) {
                                    Ok(_) => {
                                        self.recovery = false;
                                        self.notice = Some(format!(
                                            "Original preservado em {}",
                                            backup.display()
                                        ));
                                        self.changed();
                                    }
                                    Err(error) => self.notice = Some(error.to_string()),
                                }
                            }
                        } else if ui.small_button("Fechar aviso").clicked() {
                            self.notice = None;
                        }
                    });
                });
        }
    }
    pub(super) fn sidebar(&mut self, ui: &mut egui::Ui) {
        let selected = self
            .workspace
            .profiles
            .iter()
            .filter(|p| p.selected)
            .count();
        ui.horizontal(|ui| {
            ui.label(RichText::new("Máquinas").size(19.0).strong());
            ui.label(
                RichText::new(counted(selected, "selecionada", "selecionadas"))
                    .size(12.0)
                    .color(BLUE),
            );
        });
        ui.add(
            egui::TextEdit::singleline(&mut self.search)
                .hint_text("Buscar nome, IP, banco ou pasta")
                .desired_width(f32::INFINITY),
        );
        ui.horizontal(|ui| {
            if ui.small_button("Selecionar visíveis").clicked() {
                let search = self.search.to_lowercase();
                for profile in &mut self.workspace.profiles {
                    if matches_search(profile, &search) {
                        profile.selected = true;
                    }
                }
            }
            if ui.small_button("Limpar").clicked() {
                for profile in &mut self.workspace.profiles {
                    profile.selected = false;
                }
            }
            ui.menu_button("+", |ui| {
                if ui.button("Adicionar máquina").clicked() {
                    self.open_manager(ManagerTab::Machine);
                    ui.close();
                }
                if ui.button("Importar planilha").clicked() {
                    self.open_manager(ManagerTab::Import);
                    ui.close();
                }
                if ui.button("Criar pasta").clicked() {
                    self.open_manager(ManagerTab::Folders);
                    ui.close();
                }
            });
        });
        ui.separator();
        if self.workspace.profiles.is_empty() && self.workspace.folders.is_empty() {
            ui.add_space(30.0);
            ui.label(
                RichText::new("01  Adicione suas máquinas")
                    .size(17.0)
                    .strong(),
            );
            ui.label(
                RichText::new("Cadastre uma conexão MySQL ou importe várias de uma planilha.")
                    .color(MUTED),
            );
            ui.add_space(8.0);
            if ui
                .add_sized([ui.available_width(), 36.0], primary("Adicionar máquinas"))
                .clicked()
            {
                self.open_manager(ManagerTab::Machine);
            }
            if ui.link("Importar CSV ou Excel").clicked() {
                self.open_manager(ManagerTab::Import);
            }
            ui.add_space(25.0);
            ui.label(RichText::new("02  Selecione os alvos").strong());
            ui.label(RichText::new("Marque máquinas ou pastas antes de executar.").color(MUTED));
            ui.add_space(15.0);
            ui.label(RichText::new("03  Consulte e confira").strong());
            ui.label(
                RichText::new("Escreva SQL à direita. Os resultados serão separados por máquina.")
                    .color(MUTED),
            );
            return;
        }
        let search = self.search.to_lowercase();
        let mut groups: BTreeMap<String, Vec<u64>> = BTreeMap::new();
        groups.insert(String::new(), Vec::new());
        for folder in &self.workspace.folders {
            if search.is_empty() || folder.to_lowercase().contains(&search) {
                groups.entry(folder.clone()).or_default();
            }
        }
        for profile in &self.workspace.profiles {
            if matches_search(profile, &search) {
                groups
                    .entry(profile.connection.folder.clone())
                    .or_default()
                    .push(profile.id);
            }
        }
        egui::ScrollArea::vertical()
            .id_salt("machines_scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for (folder, ids) in groups {
                    let (_, payload) =
                        ui.dnd_drop_zone::<u64, _>(egui::Frame::NONE.inner_margin(4), |ui| {
                            ui.set_min_width(ui.available_width());
                            if folder.is_empty() {
                                if !self.workspace.folders.is_empty() {
                                    ui.label(
                                        RichText::new("RAIZ · solte aqui para remover da pasta")
                                            .size(10.0)
                                            .color(MUTED),
                                    );
                                }
                                for id in &ids {
                                    self.machine_row(ui, *id);
                                }
                            } else {
                                ui.horizontal(|ui| {
                                    let mut checked = !ids.is_empty()
                                        && ids.iter().all(|id| {
                                            self.workspace
                                                .profiles
                                                .iter()
                                                .any(|p| p.id == *id && p.selected)
                                        });
                                    if ui.checkbox(&mut checked, "").changed() {
                                        for profile in &mut self.workspace.profiles {
                                            if ids.contains(&profile.id) {
                                                profile.selected = checked;
                                            }
                                        }
                                    }
                                    let response = egui::CollapsingHeader::new(
                                        RichText::new(format!("{folder}  {}", ids.len())).strong(),
                                    )
                                    .id_salt(("folder", &folder))
                                    .default_open(true)
                                    .show(ui, |ui| {
                                        for id in &ids {
                                            self.machine_row(ui, *id);
                                        }
                                        if ids.is_empty() {
                                            ui.label(
                                                RichText::new("Arraste máquinas para cá")
                                                    .size(12.0)
                                                    .color(MUTED),
                                            );
                                        }
                                    });
                                    response.header_response.context_menu(|ui| {
                                        if ui
                                            .button("Excluir pasta e mover máquinas para a raiz")
                                            .clicked()
                                        {
                                            self.delete = Some(Delete::Folder(folder.clone()));
                                            ui.close();
                                        }
                                        if ui.button("Gerenciar pastas").clicked() {
                                            self.open_manager(ManagerTab::Folders);
                                            ui.close();
                                        }
                                    });
                                });
                            }
                        });
                    if let Some(id) = payload {
                        if let Some(profile) =
                            self.workspace.profiles.iter_mut().find(|p| p.id == *id)
                        {
                            profile.connection.folder = folder.clone();
                        }
                        self.changed();
                    }
                }
            });
    }
    fn machine_row(&mut self, ui: &mut egui::Ui, id: u64) {
        let Some(profile) = self.workspace.profiles.iter().find(|p| p.id == id).cloned() else {
            return;
        };
        egui::Frame::new()
            .fill(if profile.selected {
                Color32::from_rgb(29, 46, 72)
            } else {
                Color32::TRANSPARENT
            })
            .corner_radius(6)
            .inner_margin(6)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    if let Some(profile) = self.workspace.profiles.iter_mut().find(|p| p.id == id) {
                        ui.checkbox(&mut profile.selected, "");
                    }
                    let source =
                        ui.dnd_drag_source(egui::Id::new(("drag-machine", id)), id, |ui| {
                            ui.vertical(|ui| {
                                ui.label(RichText::new(&profile.connection.name).strong());
                                ui.label(
                                    RichText::new(format!(
                                        "{}:{}  /  {}",
                                        profile.connection.host,
                                        profile.connection.port,
                                        if profile.connection.database.is_empty() {
                                            "sem banco"
                                        } else {
                                            &profile.connection.database
                                        }
                                    ))
                                    .size(11.0)
                                    .color(MUTED),
                                );
                            });
                        });
                    source
                        .response
                        .interact(egui::Sense::click_and_drag())
                        .context_menu(|ui| {
                            if ui.button("Configurar conexão").clicked() {
                                self.edit(id);
                                ui.close();
                            }
                            if ui.button("Mover para a raiz").clicked() {
                                if let Some(profile) =
                                    self.workspace.profiles.iter_mut().find(|p| p.id == id)
                                {
                                    profile.connection.folder.clear();
                                }
                                self.changed();
                                ui.close();
                            }
                            if ui.button("Excluir conexão").clicked() {
                                self.delete =
                                    Some(Delete::Machine(id, profile.connection.name.clone()));
                                ui.close();
                            }
                        });
                });
                if let Some((_, stage)) = self.jobs.get(&id) {
                    ui.label(RichText::new(stage).size(11.0).color(match stage.as_str() {
                        "Concluído" => GREEN,
                        "Falhou" => RED,
                        "Resultado desconhecido" | "Falha parcial" => AMBER,
                        _ => MUTED,
                    }));
                }
            });
    }
    pub(super) fn editor(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(RichText::new("Consulta SQL").size(20.0).strong());
            ui.label(RichText::new("MySQL").size(12.0).color(BLUE));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.small_button("Salvar .sql").clicked()
                    && let Some(path) = rfd::FileDialog::new()
                        .add_filter("SQL", &["sql"])
                        .set_file_name("consulta.sql")
                        .save_file()
                    && let Err(error) = std::fs::write(path, &self.workspace.sql)
                {
                    self.notice = Some(error.to_string());
                }
                if ui.small_button("Abrir .sql").clicked()
                    && let Some(path) = rfd::FileDialog::new()
                        .add_filter("SQL", &["sql"])
                        .pick_file()
                {
                    match std::fs::read_to_string(path) {
                        Ok(sql) => {
                            self.workspace.sql = sql;
                            self.changed();
                        }
                        Err(error) => self.notice = Some(error.to_string()),
                    }
                }
            });
        });
        ui.label(
            RichText::new("Escreva uma consulta para as máquinas marcadas à esquerda.")
                .size(12.0)
                .color(MUTED),
        );
        ui.add_space(3.0);
        egui::TopBottomPanel::bottom("editor_actions")
            .frame(
                egui::Frame::new()
                    .fill(BG)
                    .inner_margin(egui::Margin::symmetric(0, 12)),
            )
            .show_inside(ui, |ui| self.editor_actions(ui));
        let response = sql_editor(ui, &mut self.workspace.sql);
        if let Some(state) = egui::TextEdit::load_state(ui.ctx(), response.id) {
            self.sql_selection = state
                .cursor
                .char_range()
                .filter(|range| !range.is_empty())
                .map(|range| {
                    let range = range.as_sorted_char_range();
                    self.workspace
                        .sql
                        .chars()
                        .skip(range.start)
                        .take(range.len())
                        .collect()
                });
        }
        if response.changed() {
            self.changed();
        }
    }
    fn editor_actions(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if ui.checkbox(&mut self.workspace.read_only,"Somente leitura")
                .on_hover_text("O servidor deve aceitar uma transação READ ONLY. Alterações são bloqueadas no servidor.")
                .changed() { self.changed(); }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if let Some(run) = &self.run {
                    let label=if self.cancel_requested {"Cancelando…"} else {"Cancelar execução"};
                    if ui.add_enabled(!self.cancel_requested,egui::Button::new(label).fill(Color32::from_rgb(105,39,51))).clicked() {
                        self.cancel_requested=true;
                        run.cancel();
                    }
                } else {
                    let count=self.workspace.profiles.iter().filter(|profile|profile.selected).count();
                    let enabled=count>0&&!self.workspace.sql.trim().is_empty();
                    let part=if self.sql_selection.is_some() {"seleção"} else {"SQL"};
                    let label=format!("Executar {part} em {}", counted(count,"máquina","máquinas"));
                    if ui.add_enabled(enabled,primary(label)).on_hover_text("Ctrl+Enter. Com texto selecionado, executa apenas a seleção.").clicked() { self.request_run(); }
                }
            });
        });
        if !self.workspace.read_only {
            ui.label(
                RichText::new(
                    "Alterações habilitadas. Você revisará os alvos antes de enviar o SQL.",
                )
                .size(12.0)
                .color(AMBER),
            );
        } else if self
            .workspace
            .profiles
            .iter()
            .all(|profile| !profile.selected)
        {
            ui.label(
                RichText::new("Marque uma máquina para habilitar a execução. Ctrl+Enter executa.")
                    .size(12.0)
                    .color(MUTED),
            );
        }
    }
    pub(super) fn confirmation_dialog(&mut self, ctx: &egui::Context) {
        let Some(snapshot) = &mut self.confirmation else {
            return;
        };
        let mut execute = false;
        let mut cancel =
            ctx.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
        egui::Modal::new(egui::Id::new("confirm_run")).show(ctx, |ui| {
            ui.set_width(650.0);
            ui.heading("Revisar alterações");
            ui.label(RichText::new(format!("{} em {}",counted(snapshot.statements,"instrução","instruções"),counted(snapshot.targets.len(),"máquina","máquinas"))).color(AMBER));
            ui.label("Cada servidor executa de forma independente. Uma falha não desfaz alterações já feitas em outros servidores.");
            egui::ScrollArea::vertical().id_salt("confirm_targets").max_height(180.0).show(ui, |ui| {
                for (_,machine) in &snapshot.targets {
                    let database=if machine.database.is_empty() {"sem banco padrão"} else {&machine.database};
                    ui.label(format!("{} · {}:{} · {database}",machine.name,machine.host,machine.port));
                }
            });
            egui::ScrollArea::vertical().id_salt("confirm_sql").max_height(140.0).show(ui, |ui| {ui.monospace(&snapshot.sql);});
            let expected=format!("EXECUTAR {}",snapshot.targets.len());
            ui.label(format!("Para confirmar, digite {expected}"));
            ui.text_edit_singleline(&mut snapshot.phrase);
            ui.horizontal(|ui| {
                if ui.button("Voltar ao editor").clicked() { cancel=true; }
                if ui.add_enabled(snapshot.phrase==expected,egui::Button::new("Confirmar e executar").fill(Color32::from_rgb(140,48,65))).clicked() { execute=true; }
            });
        });
        if execute && let Some(snapshot) = self.confirmation.take() {
            self.start(snapshot.targets, snapshot.sql);
        } else if cancel {
            self.confirmation = None;
        }
    }
    pub(super) fn delete_dialog(&mut self, ctx: &egui::Context) {
        let Some(target) = &self.delete else {
            return;
        };
        let mut confirm = false;
        let mut cancel =
            ctx.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
        egui::Modal::new(egui::Id::new("delete")).show(ctx, |ui| {
            ui.set_width(430.0);
            ui.heading(match target {
                Delete::Machine(_, _) => "Excluir conexão?",
                Delete::Folder(_) => "Excluir pasta?",
            });
            ui.label(match target {
                Delete::Machine(_, name) => {
                    format!("A conexão {name} será removida deste espaço de trabalho.")
                }
                Delete::Folder(name) => {
                    format!("A pasta {name} será removida. As máquinas vão para a raiz.")
                }
            });
            ui.horizontal(|ui| {
                if ui.button("Cancelar").clicked() {
                    cancel = true;
                }
                if ui.button("Excluir").clicked() {
                    confirm = true;
                }
            });
        });
        if confirm && let Some(target) = self.delete.take() {
            match target {
                Delete::Machine(id, _) => {
                    if self
                        .workspace
                        .profiles
                        .iter()
                        .any(|p| p.id == id && p.remember_password)
                        && let Err(error) = model::delete_password(&model::credential_key(
                            &self.config_path,
                            &format!("profile-{id}"),
                        ))
                    {
                        self.notice = Some(error);
                        return;
                    }
                    self.workspace.profiles.retain(|p| p.id != id);
                }
                Delete::Folder(folder) => {
                    self.workspace.folders.remove(&folder);
                    for profile in &mut self.workspace.profiles {
                        if profile.connection.folder == folder {
                            profile.connection.folder.clear();
                        }
                    }
                }
            }
            self.changed();
        }
        if cancel {
            self.delete = None;
        }
    }
    pub(super) fn history_dialog(&mut self, ctx: &egui::Context) {
        if !self.history_open {
            return;
        }
        let mut open = true;
        let mut load = None;
        egui::Window::new("Histórico de consultas").open(&mut open).default_size([700.0,500.0]).show(ctx, |ui| {
            ui.label(RichText::new("Carregar uma consulta não a executa. Resultados e senhas não são gravados no histórico.").color(MUTED));
            if self.workspace.history.is_empty() {ui.label("As consultas executadas aparecerão aqui.");}
            egui::ScrollArea::vertical().show(ui, |ui| {
                for (index,entry) in self.workspace.history.iter().enumerate() {
                    egui::Frame::group(ui.style()).show(ui, |ui| {
                        ui.label(format!("{} · {} alvos · {} sucesso · {} falhas · {} canceladas · {} desconhecidas",relative_time(entry.when),entry.targets,entry.success,entry.failed,entry.cancelled,entry.unknown));
                        ui.monospace(entry.sql.chars().take(240).collect::<String>());
                        if ui.button("Carregar no editor").clicked() {load=Some(index);}
                    });
                }
            });
        });
        self.history_open = open;
        if let Some(index) = load {
            self.workspace.sql = self.workspace.history[index].sql.clone();
            self.sql_selection = None;
            self.history_open = false;
            self.changed();
        }
    }
}
fn matches_search(profile: &Profile, search: &str) -> bool {
    format!(
        "{} {} {} {}",
        profile.connection.name,
        profile.connection.host,
        profile.connection.database,
        profile.connection.folder
    )
    .to_lowercase()
    .contains(search)
}
fn highlight(text: &str) -> egui::text::LayoutJob {
    let mut job = egui::text::LayoutJob::default();
    let chars: Vec<char> = text.chars().collect();
    let mut index = 0;
    while index < chars.len() {
        let start = index;
        let ch = chars[index];
        let mut color = TEXT;
        if ch == '\'' || ch == '"' {
            color = GREEN;
            index += 1;
            while index < chars.len() {
                if chars[index] == '\\' {
                    index = (index + 2).min(chars.len());
                    continue;
                }
                let end = chars[index] == ch;
                index += 1;
                if end {
                    break;
                }
            }
        } else if ch == '#' || (ch == '-' && chars.get(index + 1) == Some(&'-')) {
            color = MUTED;
            while index < chars.len() && chars[index] != '\n' {
                index += 1;
            }
        } else if ch.is_alphanumeric() || ch == '_' {
            index += 1;
            while index < chars.len() && (chars[index].is_alphanumeric() || chars[index] == '_') {
                index += 1;
            }
            let word: String = chars[start..index].iter().collect();
            if [
                "SELECT", "FROM", "WHERE", "LIMIT", "ORDER", "BY", "GROUP", "JOIN", "LEFT",
                "RIGHT", "INNER", "ON", "AS", "AND", "OR", "NOT", "NULL", "UPDATE", "DELETE",
                "INSERT", "INTO", "VALUES", "SET", "CREATE", "TABLE", "DROP", "ALTER", "SHOW",
                "DISTINCT", "HAVING", "UNION", "ALL", "CASE", "WHEN", "THEN", "END", "DESC", "ASC",
                "IS", "LIKE", "IN",
            ]
            .contains(&word.to_ascii_uppercase().as_str())
            {
                color = BLUE;
            } else if ch.is_ascii_digit() {
                color = AMBER;
            }
        } else {
            index += 1;
        }
        job.append(
            &chars[start..index].iter().collect::<String>(),
            0.0,
            egui::TextFormat {
                font_id: egui::FontId::monospace(14.0),
                color,
                ..Default::default()
            },
        );
    }
    job
}

fn relative_time(timestamp: u64) -> String {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .saturating_sub(timestamp);
    if elapsed < 60 {
        "agora".into()
    } else if elapsed < 3600 {
        format!("há {} min", elapsed / 60)
    } else if elapsed < 86400 {
        format!("há {} h", elapsed / 3600)
    } else {
        format!("há {} dias", elapsed / 86400)
    }
}

fn sql_editor(ui: &mut egui::Ui, sql: &mut String) -> egui::Response {
    let available = ui.available_size();
    egui::Frame::new()
        .fill(Color32::from_rgb(11, 16, 25))
        .stroke(egui::Stroke::new(1.0_f32, BORDER))
        .corner_radius(8)
        .inner_margin(12)
        .show(ui, |ui| {
            ui.set_min_size((available - egui::vec2(26.0, 26.0)).max(egui::Vec2::ZERO));
            let line_count = sql.lines().count().max(1);
            egui::ScrollArea::both()
                .id_salt("sql_scroll")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.horizontal_top(|ui| {
                        let numbers = (1..=line_count)
                            .map(|line| line.to_string())
                            .collect::<Vec<_>>()
                            .join("\n");
                        ui.label(
                            RichText::new(numbers)
                                .monospace()
                                .color(Color32::from_rgb(74, 90, 114)),
                        );
                        let mut layouter =
                            |ui: &egui::Ui, text: &dyn egui::TextBuffer, _width: f32| {
                                ui.fonts_mut(|fonts| fonts.layout_job(highlight(text.as_str())))
                            };
                        ui.add(
                            egui::TextEdit::multiline(sql)
                                .code_editor()
                                .id_salt("sql-editor")
                                .frame(false)
                                .hint_text("SELECT *\nFROM sua_tabela\nLIMIT 100;")
                                .desired_width(ui.available_width().max(400.0))
                                .layouter(&mut layouter),
                        )
                    })
                    .inner
                })
                .inner
        })
        .inner
}

#[cfg(test)]
mod layout_tests {
    use super::*;
    #[test]
    fn editor_renders_when_other_panels_leave_almost_no_height() {
        let context = egui::Context::default();
        for height in [0.0, 11.0, 25.0, 80.0] {
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(300.0, height),
                )),
                ..Default::default()
            };
            let mut sql = "SELECT 1".to_owned();
            let _ = context.run(input, |context| {
                egui::CentralPanel::default().show(context, |ui| {
                    sql_editor(ui, &mut sql);
                });
            });
            assert_eq!(sql, "SELECT 1");
        }
    }
}
