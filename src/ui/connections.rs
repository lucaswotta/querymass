use super::*;
use egui_extras::{Column, TableBuilder};
impl App {
    pub(super) fn connection_manager(&mut self, ctx: &egui::Context) {
        if !self.manager {
            return;
        }
        let mut close = false;
        egui::Modal::new(egui::Id::new("connection_manager")).show(ctx, |ui| {
            ui.set_width(800.0);
            ui.horizontal(|ui| {
                ui.heading("Conexões");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Fechar").clicked() {
                        close = true;
                    }
                });
            });
            ui.label(
                RichText::new("Cadastre, importe e organize suas máquinas em um único lugar.")
                    .color(MUTED),
            );
            ui.horizontal(|ui| {
                for (tab, label) in [
                    (ManagerTab::Machine, "Máquina"),
                    (ManagerTab::Import, "Importar planilha"),
                    (ManagerTab::Shared, "Credenciais compartilhadas"),
                    (ManagerTab::Folders, "Pastas"),
                ] {
                    if ui
                        .selectable_label(self.manager_tab == tab, label)
                        .clicked()
                    {
                        self.manager_tab = tab;
                        self.form_error = None;
                        self.form_success = None;
                    }
                }
            });
            ui.separator();
            egui::ScrollArea::vertical()
                .id_salt("manager_scroll")
                .max_height(470.0)
                .min_scrolled_height(470.0)
                .auto_shrink([false, false])
                .show(ui, |ui| match self.manager_tab {
                    ManagerTab::Machine => self.machine_form(ui),
                    ManagerTab::Import => self.import_form(ui),
                    ManagerTab::Shared => self.shared_form(ui),
                    ManagerTab::Folders => self.folder_form(ui),
                });
            if let Some(tested) = &self.tested_connection {
                let mut current = self.workspace.resolved(&self.draft.profile);
                current.port = self.draft.port.parse().unwrap_or(0);
                if current != *tested {
                    self.form_success = None;
                }
            }
            if self.manager_tab == ManagerTab::Machine {
                ui.separator();
                self.machine_actions(ui);
            }
            if let Some(message) = &self.form_success {
                ui.label(RichText::new(message).color(GREEN));
            }
            if let Some(error) = &self.form_error {
                ui.separator();
                ui.label(RichText::new(error).color(RED));
            }
        });
        if close {
            self.manager = false;
            self.form_error = None;
        }
    }
    fn validate_draft(&self) -> Result<Profile, String> {
        let mut profile = self.draft.profile.clone();
        profile.connection.host = profile.connection.host.trim().to_owned();
        profile.connection.database = profile.connection.database.trim().to_owned();
        profile.connection.username = profile.connection.username.trim().to_owned();
        profile.connection.port = self
            .draft
            .port
            .parse::<u16>()
            .ok()
            .filter(|port| *port > 0)
            .ok_or("Informe uma porta entre 1 e 65535.")?;
        if profile.connection.host.is_empty() {
            return Err("Informe o IP ou nome DNS da máquina.".into());
        }
        if profile.connection.host.contains(char::is_whitespace) {
            return Err("O IP ou nome DNS não deve conter espaços.".into());
        }
        if profile.connection.name.trim().is_empty() {
            profile.connection.name = profile.connection.host.clone();
        } else {
            profile.connection.name = profile.connection.name.trim().into();
        }
        let resolved = self.workspace.resolved(&profile);
        if resolved.username.trim().is_empty() {
            return Err(
                "Informe o usuário MySQL ou configure as credenciais compartilhadas.".into(),
            );
        }
        if self.workspace.same_target(
            &resolved,
            if self.editing { Some(profile.id) } else { None },
        ) {
            return Err("Já existe uma conexão com esse host, porta, banco e usuário.".into());
        }
        Ok(profile)
    }
    fn machine_form(&mut self, ui: &mut egui::Ui) {
        let mut selected = None;
        ui.horizontal(|ui| {
            egui::ComboBox::from_id_salt("existing_connection")
                .selected_text(if self.editing {
                    self.draft.profile.connection.name.clone()
                } else {
                    "Nova máquina".into()
                })
                .width(300.0)
                .show_ui(ui, |ui| {
                    if ui.selectable_label(!self.editing, "Nova máquina").clicked() {
                        selected = Some(None);
                    }
                    for profile in &self.workspace.profiles {
                        if ui
                            .selectable_label(
                                self.editing && self.draft.profile.id == profile.id,
                                &profile.connection.name,
                            )
                            .clicked()
                        {
                            selected = Some(Some(profile.id));
                        }
                    }
                });
            if ui.button("Nova máquina").clicked() {
                selected = Some(None);
            }
        });
        if let Some(selected) = selected {
            match selected {
                Some(id) => self.edit(id),
                None => {
                    self.draft = Draft::new(self.workspace.next_id());
                    self.editing = false;
                    self.form_error = None;
                }
            }
        }
        ui.add_space(6.0);
        ui.columns(2, |columns| {
            field(
                &mut columns[0],
                "Nome da máquina",
                &mut self.draft.profile.connection.name,
                "Ex.: PDV 01 · Loja Centro",
            );
            field(
                &mut columns[1],
                "Banco de dados",
                &mut self.draft.profile.connection.database,
                "Ex.: pdv (opcional para SQL qualificado)",
            );
        });
        ui.columns(2, |columns| {
            field(
                &mut columns[0],
                "IP ou nome DNS",
                &mut self.draft.profile.connection.host,
                "192.0.2.10",
            );
            field(&mut columns[1], "Porta MySQL", &mut self.draft.port, "3306");
        });
        ui.label(RichText::new("Pasta").color(MUTED));
        egui::ComboBox::from_id_salt("machine_folder")
            .selected_text(if self.draft.profile.connection.folder.is_empty() {
                "Raiz"
            } else {
                &self.draft.profile.connection.folder
            })
            .width(360.0)
            .show_ui(ui, |ui| {
                ui.selectable_value(
                    &mut self.draft.profile.connection.folder,
                    String::new(),
                    "Raiz",
                );
                for folder in &self.workspace.folders {
                    ui.selectable_value(
                        &mut self.draft.profile.connection.folder,
                        folder.clone(),
                        folder,
                    );
                }
            });
        ui.separator();
        ui.label(RichText::new("Acesso ao MySQL").strong());
        ui.horizontal(|ui| {
            ui.radio_value(
                &mut self.draft.profile.use_shared_credentials,
                true,
                "Usar credenciais compartilhadas",
            );
            ui.radio_value(
                &mut self.draft.profile.use_shared_credentials,
                false,
                "Credenciais desta máquina",
            );
        });
        if self.draft.profile.use_shared_credentials {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(if self.workspace.shared.username.is_empty() {
                        "Nenhuma credencial compartilhada configurada.".into()
                    } else {
                        format!(
                            "Usuário: {} · senha {}",
                            self.workspace.shared.username,
                            if self.workspace.shared.password.is_empty() {
                                "vazia"
                            } else {
                                "definida"
                            }
                        )
                    })
                    .color(MUTED),
                );
                if ui.link("Configurar").clicked() {
                    self.manager_tab = ManagerTab::Shared;
                }
            });
        } else {
            ui.columns(2, |columns| {
                field(
                    &mut columns[0],
                    "Usuário",
                    &mut self.draft.profile.connection.username,
                    "Usuário MySQL",
                );
                columns[1].label(RichText::new("Senha").color(MUTED));
                columns[1].add(
                    egui::TextEdit::singleline(&mut self.draft.profile.connection.password)
                        .password(true)
                        .margin(egui::Margin::symmetric(10, 7))
                        .hint_text("Vazia significa senha vazia")
                        .desired_width(f32::INFINITY),
                );
            });
            ui.checkbox(
                &mut self.draft.profile.remember_password,
                "Guardar senha no cofre do sistema",
            );
            ui.label(
                RichText::new("Sem essa opção, a senha dura apenas nesta sessão.")
                    .size(12.0)
                    .color(MUTED),
            );
        }
        ui.collapsing("Conexão segura e opções de execução", |ui| {
            ui.checkbox(
                &mut self.draft.profile.connection.tls,
                "Exigir TLS e validar o certificado do servidor",
            );
            if self.draft.profile.connection.tls {
                ui.horizontal(|ui| {
                    ui.label(
                        self.draft
                            .profile
                            .connection
                            .ca_path
                            .as_ref()
                            .map(|p| p.display().to_string())
                            .unwrap_or_else(|| "Autoridades certificadoras do sistema".into()),
                    );
                    if ui.button("Escolher CA").clicked() {
                        self.draft.profile.connection.ca_path = rfd::FileDialog::new()
                            .add_filter("Certificado PEM", &["pem", "crt"])
                            .pick_file();
                    }
                    if ui.button("Usar CA do sistema").clicked() {
                        self.draft.profile.connection.ca_path = None;
                    }
                });
            } else {
                ui.label(
                    RichText::new("TLS desativado nesta conexão.")
                        .size(12.0)
                        .color(AMBER),
                );
            }
        });
    }
    fn machine_actions(&mut self, ui: &mut egui::Ui) {
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            if ui
                .add_enabled(
                    !self.testing,
                    egui::Button::new(if self.testing {
                        "Testando…"
                    } else {
                        "Testar conexão"
                    }),
                )
                .clicked()
            {
                match self.validate_draft() {
                    Ok(profile) => {
                        self.testing = true;
                        self.form_error = None;
                        let sender = self.sender.clone();
                        let connection = self.workspace.resolved(&profile);
                        std::thread::spawn(move || {
                            let _ = sender.send(Message::ConnectionTest(
                                connection.clone(),
                                database::test_connection(&connection),
                            ));
                        });
                    }
                    Err(error) => self.form_error = Some(error),
                }
            }
            if ui
                .add(primary(if self.editing {
                    "Salvar alterações"
                } else {
                    "Adicionar máquina"
                }))
                .clicked()
            {
                self.save_profile();
            }
        });
    }
    fn save_profile(&mut self) {
        let mut profile = match self.validate_draft() {
            Ok(profile) => profile,
            Err(error) => {
                self.form_error = Some(error);
                return;
            }
        };
        profile.credential_unavailable = false;
        let key = model::credential_key(&self.config_path, &format!("profile-{}", profile.id));
        if !profile.use_shared_credentials && profile.remember_password {
            if let Err(error) = model::write_password(&key, &profile.connection.password) {
                profile.remember_password = false;
                self.notice = Some(error);
            }
        } else if self
            .workspace
            .profiles
            .iter()
            .any(|p| p.id == profile.id && p.remember_password)
            && let Err(error) = model::delete_password(&key)
        {
            self.form_error = Some(error);
            return;
        }
        if !profile.connection.folder.is_empty() {
            self.workspace
                .folders
                .insert(profile.connection.folder.clone());
        }
        if let Some(existing) = self
            .workspace
            .profiles
            .iter_mut()
            .find(|p| p.id == profile.id)
        {
            *existing = profile;
        } else {
            self.workspace.profiles.push(profile);
        }
        self.changed();
        self.save();
        self.manager = false;
        self.form_error = None;
    }
    fn shared_form(&mut self, ui: &mut egui::Ui) {
        ui.label(
            RichText::new("Um acesso para várias máquinas")
                .size(18.0)
                .strong(),
        );
        ui.label(RichText::new("Usado somente nas conexões marcadas como ‘Usar credenciais compartilhadas’. Credenciais próprias nunca são substituídas.").color(MUTED));
        field(
            ui,
            "Usuário MySQL",
            &mut self.shared_draft.username,
            "Usuário compartilhado",
        );
        ui.label("Senha");
        ui.add(
            egui::TextEdit::singleline(&mut self.shared_draft.password)
                .password(true)
                .margin(egui::Margin::symmetric(10, 7))
                .desired_width(400.0),
        );
        ui.checkbox(
            &mut self.shared_draft.remember_password,
            "Guardar senha no cofre do sistema",
        );
        ui.label(RichText::new("As configurações salvam o usuário e a referência ao cofre. A senha não é escrita no arquivo de configuração.").size(12.0).color(MUTED));
        if ui
            .add(primary("Salvar credenciais compartilhadas"))
            .clicked()
        {
            if self.shared_draft.username.trim().is_empty() {
                self.form_error = Some("Informe o usuário MySQL.".into());
                return;
            }
            if self.shared_draft.remember_password {
                if let Err(error) = model::write_password(
                    &model::credential_key(&self.config_path, "shared"),
                    &self.shared_draft.password,
                ) {
                    self.shared_draft.remember_password = false;
                    self.notice = Some(error);
                }
            } else if self.workspace.shared.remember_password
                && let Err(error) =
                    model::delete_password(&model::credential_key(&self.config_path, "shared"))
            {
                self.form_error = Some(error);
                return;
            }
            self.shared_draft.credential_unavailable = false;
            self.workspace.shared = self.shared_draft.clone();
            self.changed();
            self.save();
            self.form_error = None;
            self.manager_tab = ManagerTab::Machine;
        }
        ui.separator();
        ui.label(RichText::new("Limites de execução").strong());
        let mut changed = false;
        ui.horizontal(|ui| {
            ui.label("Máquinas em paralelo");
            changed |= ui
                .add(egui::DragValue::new(&mut self.workspace.concurrency).range(1..=16))
                .changed();
            ui.label("Linhas por resultado");
            changed |= ui
                .add(egui::DragValue::new(&mut self.workspace.row_limit).range(1..=10000))
                .changed();
            ui.label("Prazo por consulta (s)");
            changed |= ui
                .add(egui::DragValue::new(&mut self.workspace.timeout_seconds).range(5..=300))
                .changed();
        });
        ui.label(RichText::new("Ao atingir o prazo, o aplicativo solicita a interrupção no servidor. Falhas de rede em alterações exigem conferir os dados antes de repetir.").size(12.0).color(MUTED));
        if changed {
            self.changed();
        }
    }
    fn import_form(&mut self, ui: &mut egui::Ui) {
        ui.label(RichText::new("Importar máquinas").size(18.0).strong());
        ui.label(RichText::new("Uma linha por conexão. Colunas: IP, HOSTNAME, LOCATION, PORT, DATABASE, USER e PASSWORD. Só o endereço é obrigatório.").color(MUTED));
        ui.horizontal(|ui|{
            if ui.add_enabled(!self.importing,primary(if self.importing{"Lendo arquivo…"}else{"Escolher CSV ou Excel"})).clicked()&& let Some(path)=rfd::FileDialog::new().add_filter("Planilhas",&["csv","xlsx","xlsm"]).pick_file(){self.importing=true;self.import_report=None;self.form_error=None;let sender=self.sender.clone();std::thread::spawn(move||{let _=sender.send(Message::Imported(querymass::import::load_connections(&path)));
});
}
            if ui.button("Baixar modelo CSV").clicked()&& let Some(path)=rfd::FileDialog::new().add_filter("CSV",&["csv"]).set_file_name("querymass-maquinas.csv").save_file()&& let Err(error)=std::fs::write(path,"IP;HOSTNAME;LOCATION;PORT;DATABASE;USER;PASSWORD\n192.0.2.10;PDV 01;Loja Centro;3306;pdv;;\n"){self.form_error=Some(error.to_string());
}
        });
        ui.label(RichText::new("Excel: primeira aba. CSV: separador detectado automaticamente. As máquinas são adicionadas desmarcadas.").size(12.0).color(MUTED));
        let Some(report) = &self.import_report else {
            ui.add_space(25.0);
            ui.label("Escolha um arquivo para conferir a prévia antes de importar.");
            return;
        };
        ui.label(
            RichText::new(format!(
                "{} conexões válidas · {} repetidas no arquivo · {} linhas sem IP · {} erros",
                report.machines.len(),
                report.duplicate_rows.len(),
                report.rows_without_host.len(),
                report.issues.len()
            ))
            .color(BLUE),
        );
        TableBuilder::new(ui)
            .id_salt("import_preview")
            .striped(true)
            .resizable(true)
            .max_scroll_height(230.0)
            .column(Column::initial(170.0))
            .column(Column::initial(150.0))
            .column(Column::initial(100.0))
            .column(Column::remainder())
            .header(26.0, |mut header| {
                for title in ["Nome", "Endereço", "Banco", "Pasta"] {
                    header.col(|ui| {
                        ui.strong(title);
                    });
                }
            })
            .body(|body| {
                body.rows(26.0, report.machines.len(), |mut row| {
                    let machine = &report.machines[row.index()];
                    for text in [
                        &machine.name,
                        &machine.host,
                        &machine.database,
                        &machine.folder,
                    ] {
                        row.col(|ui| {
                            ui.label(text);
                        });
                    }
                });
            });
        if !report.issues.is_empty() {
            ui.collapsing("Ver erros de validação", |ui| {
                for error in &report.issues {
                    ui.label(RichText::new(error).color(RED));
                }
            });
        }
        let count = report.machines.len();
        ui.separator();
        ui.label("Pasta para todas (opcional; vazio preserva as pastas da planilha)");
        ui.text_edit_singleline(&mut self.import_folder);
        ui.checkbox(
            &mut self.import_remember,
            "Guardar senhas importadas no cofre do sistema",
        );
        ui.label(RichText::new("Sem USER, a conexão usa credenciais compartilhadas. Com USER, senha vazia é tratada como senha vazia.").size(12.0).color(MUTED));
        if ui
            .add_enabled(
                count > 0,
                primary(format!("Importar {count} conexões válidas")),
            )
            .clicked()
        {
            self.apply_import();
        }
    }
    fn apply_import(&mut self) {
        let Some(report) = self.import_report.take() else {
            return;
        };
        let mut added = 0;
        let mut duplicates = 0;
        let mut keyring_failure = None;
        for mut connection in report.machines {
            if !self.import_folder.trim().is_empty() {
                connection.folder = self.import_folder.trim().into();
            }
            let use_shared = connection.username.is_empty();
            let id = self.workspace.next_id();
            let mut profile = Profile {
                id,
                connection,
                use_shared_credentials: use_shared,
                remember_password: self.import_remember && !use_shared,
                credential_unavailable: false,
                selected: false,
            };
            if self
                .workspace
                .same_target(&self.workspace.resolved(&profile), None)
            {
                duplicates += 1;
                continue;
            }
            if profile.remember_password
                && let Err(error) = model::write_password(
                    &model::credential_key(&self.config_path, &format!("profile-{id}")),
                    &profile.connection.password,
                )
            {
                profile.remember_password = false;
                keyring_failure = Some(error);
            }
            if !profile.connection.folder.is_empty() {
                self.workspace
                    .folders
                    .insert(profile.connection.folder.clone());
            }
            self.workspace.profiles.push(profile);
            added += 1;
        }
        self.notice = Some(format!(
            "{added} conexões importadas, {duplicates} já existentes ignoradas. Marque os alvos para executar.{}",
            keyring_failure.map(|e| format!(" {e}")).unwrap_or_default()
        ));
        self.changed();
        self.save();
        self.manager = false;
    }
    fn folder_form(&mut self, ui: &mut egui::Ui) {
        ui.label(
            RichText::new("Organize por loja, ambiente ou região")
                .size(18.0)
                .strong(),
        );
        ui.label(RichText::new("Arraste máquinas na barra lateral para movê-las entre pastas. Excluir uma pasta mantém suas máquinas na raiz.").color(MUTED));
        ui.horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut self.new_folder)
                    .hint_text("Nome da nova pasta")
                    .desired_width(480.0),
            );
            if ui.add(primary("Criar pasta")).clicked() {
                let name = self.new_folder.trim();
                if name.is_empty() {
                    self.form_error = Some("Informe um nome para a pasta.".into());
                } else if self.workspace.folders.contains(name) {
                    self.form_error = Some("Essa pasta já existe.".into());
                } else {
                    self.workspace.folders.insert(name.into());
                    self.new_folder.clear();
                    self.form_error = None;
                    self.changed();
                }
            }
        });
        let folders: Vec<_> = self.workspace.folders.iter().cloned().collect();
        for folder in folders {
            ui.horizontal(|ui| {
                let count = self
                    .workspace
                    .profiles
                    .iter()
                    .filter(|p| p.connection.folder == folder)
                    .count();
                ui.label(format!("{folder} · {count} máquinas"));
                if ui.small_button("Renomear").clicked() {
                    self.folder_rename = Some((folder.clone(), folder.clone()));
                }
                if ui.small_button("Excluir pasta").clicked() {
                    self.delete = Some(Delete::Folder(folder.clone()));
                }
            });
        }
        if let Some((original, new_name)) = &mut self.folder_rename {
            let mut save = false;
            ui.separator();
            ui.label(format!("Renomear {original}"));
            ui.text_edit_singleline(new_name);
            if ui.button("Salvar nome").clicked() {
                save = true;
            }
            if save {
                let old = original.clone();
                let new = new_name.trim().to_owned();
                if new.is_empty() || self.workspace.folders.contains(&new) && new != old {
                    self.form_error = Some("Escolha um nome não vazio e que não exista.".into());
                } else {
                    self.workspace.folders.remove(&old);
                    self.workspace.folders.insert(new.clone());
                    for profile in &mut self.workspace.profiles {
                        if profile.connection.folder == old {
                            profile.connection.folder = new.clone();
                        }
                    }
                    self.folder_rename = None;
                    self.changed();
                }
            }
        }
    }
}
fn field(ui: &mut egui::Ui, label: &str, value: &mut String, hint: &str) {
    let label_response = ui.label(RichText::new(label).color(MUTED));
    ui.add(
        egui::TextEdit::singleline(value)
            .hint_text(hint)
            .margin(egui::Margin::symmetric(10, 7))
            .desired_width(f32::INFINITY),
    )
    .labelled_by(label_response.id);
}
