mod database;
mod import;

use std::collections::{BTreeMap, VecDeque};
use std::sync::{Arc, Mutex, mpsc};
use std::thread;

use database::{MachineConnection, QueryOutput};
use eframe::egui;
use import::ImportReport;

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

struct MachineEntry {
    id: u64,
    selected: bool,
    connection: MachineConnection,
}

#[derive(Default)]
struct MachineForm {
    name: String,
    host: String,
    folder: String,
    port: String,
    database: String,
    username: String,
    password: String,
}

impl MachineForm {
    fn new(defaults: &MysqlDefaults) -> Self {
        Self {
            port: "3306".to_owned(),
            username: defaults.username.clone(),
            password: defaults.password.clone(),
            ..Default::default()
        }
    }

    fn build_connection(&self) -> Result<MachineConnection, String> {
        let host = self.host.trim().to_owned();
        let username = self.username.trim().to_owned();
        let port = self
            .port
            .trim()
            .parse::<u16>()
            .map_err(|_| "Informe uma porta entre 1 e 65535.".to_owned())?;
        if host.is_empty() {
            return Err("Informe o endereço IP ou nome da máquina.".to_owned());
        }
        if username.is_empty() {
            return Err("Informe o usuário MySQL ou configure um usuário padrão.".to_owned());
        }
        if port == 0 {
            return Err("Informe uma porta entre 1 e 65535.".to_owned());
        }
        Ok(MachineConnection {
            name: if self.name.trim().is_empty() {
                host.clone()
            } else {
                self.name.trim().to_owned()
            },
            host,
            folder: self.folder.trim().to_owned(),
            port,
            database: self.database.trim().to_owned(),
            username,
            password: self.password.clone(),
        })
    }
}

#[derive(Default)]
struct MysqlDefaults {
    username: String,
    password: String,
}

struct QueryMassApp {
    machines: Vec<MachineEntry>,
    next_machine_id: u64,
    query: String,
    defaults: MysqlDefaults,
    machine_form: MachineForm,
    show_add_machine: bool,
    editing_machine_id: Option<u64>,
    pending_delete: Option<DeleteTarget>,
    editing_folder: Option<String>,
    folder_name: String,
    show_run_confirmation: bool,
    confirmation_text: String,
    import_message: String,
    status_message: String,
    form_error: String,
    outputs: Vec<QueryOutput>,
    terminal_output: String,
    select_all_output: bool,
    running: bool,
    pending_results: usize,
    result_sender: mpsc::Sender<QueryOutput>,
    result_receiver: mpsc::Receiver<QueryOutput>,
}

enum DeleteTarget {
    Machine(u64, String),
    Folder(String, usize),
}

impl Default for QueryMassApp {
    fn default() -> Self {
        let (result_sender, result_receiver) = mpsc::channel();
        Self {
            machines: Vec::new(),
            next_machine_id: 1,
            query: String::new(),
            defaults: MysqlDefaults::default(),
            machine_form: MachineForm::default(),
            show_add_machine: false,
            editing_machine_id: None,
            pending_delete: None,
            editing_folder: None,
            folder_name: String::new(),
            show_run_confirmation: false,
            confirmation_text: String::new(),
            import_message: String::new(),
            status_message: String::new(),
            form_error: String::new(),
            outputs: Vec::new(),
            terminal_output: String::new(),
            select_all_output: false,
            running: false,
            pending_results: 0,
            result_sender,
            result_receiver,
        }
    }
}

impl eframe::App for QueryMassApp {
    fn update(&mut self, context: &egui::Context, _frame: &mut eframe::Frame) {
        self.receive_query_outputs();
        apply_querymass_theme(context);

        egui::TopBottomPanel::top("toolbar").show(context, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                ui.menu_button("Arquivo", |ui| {
                    if ui.button("Nova conexão MySQL").clicked() {
                        self.open_new_machine();
                        ui.close();
                    }
                    if ui.button("Importar CSV / Excel…").clicked() {
                        self.import_connections();
                        ui.close();
                    }
                    if ui.button("Modelo CSV…").clicked() {
                        self.save_csv_template();
                        ui.close();
                    }
                    if ui.button("Sair").clicked() {
                        context.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });
                ui.menu_button("Conexões", |ui| {
                    if ui.button("Selecionar todas").clicked() {
                        for machine in &mut self.machines {
                            machine.selected = true;
                        }
                        ui.close();
                    }
                    if ui.button("Limpar seleção").clicked() {
                        for machine in &mut self.machines {
                            machine.selected = false;
                        }
                        ui.close();
                    }
                });
                ui.menu_button("Consulta", |ui| {
                    if ui.button("Executar SQL").clicked() {
                        self.request_query_execution();
                        ui.close();
                    }
                    if ui.button("Limpar editor").clicked() {
                        self.query.clear();
                        ui.close();
                    }
                });
                ui.menu_button("Ajuda", |ui| {
                    ui.label("QueryMass · cliente MySQL em massa");
                });
            });
            ui.separator();
            ui.horizontal(|ui| {
                ui.add_space(4.0);
                draw_brand_mark(ui);
                ui.label(egui::RichText::new("QueryMass").strong().size(16.0));
                ui.label(egui::RichText::new("MySQL").color(color(0x969EB2)));
                ui.separator();
                if ui.button("＋ Máquina").clicked() {
                    self.open_new_machine();
                }
                if ui.button("Importar planilha").clicked() {
                    self.import_connections();
                }
            });
        });

        egui::SidePanel::left("machines")
            .resizable(true)
            .default_width(270.0)
            .width_range(210.0..=400.0)
            .show(context, |ui| self.show_machine_panel(ui));

        egui::TopBottomPanel::bottom("output")
            .resizable(true)
            .default_height(260.0)
            .height_range(130.0..=480.0)
            .show(context, |ui| self.show_output_panel(ui));

        egui::CentralPanel::default().show(context, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("EDITOR SQL")
                        .size(11.0)
                        .strong()
                        .color(color(0x969EB2)),
                );
                ui.label(
                    egui::RichText::new("·  MySQL")
                        .size(12.0)
                        .color(color(0x697187)),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .add_enabled(
                            !self.running,
                            egui::Button::new("▶ Executar consulta").fill(color(0x4F8CFF)),
                        )
                        .clicked()
                    {
                        self.request_query_execution();
                    }
                    ui.label(format!("{} alvo(s)", self.selected_count()));
                });
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

        self.show_add_machine_window(context);
        self.show_delete_confirmation(context);
        self.show_folder_editor(context);
        self.show_run_confirmation_window(context);
    }
}

impl QueryMassApp {
    fn show_machine_panel(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new("MÁQUINAS")
                    .size(11.0)
                    .strong()
                    .color(color(0x969EB2)),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let all_selected = !self.machines.is_empty()
                    && self.machines.iter().all(|machine| machine.selected);
                let label = if all_selected {
                    "Limpar seleção"
                } else {
                    "Selecionar todas"
                };
                if ui.small_button(label).clicked() {
                    for machine in &mut self.machines {
                        machine.selected = !all_selected;
                    }
                }
            });
        });
        if !self.import_message.is_empty() {
            ui.label(
                egui::RichText::new(&self.import_message)
                    .size(11.0)
                    .color(color(0x969EB2)),
            );
        }
        ui.separator();

        if self.machines.is_empty() {
            ui.vertical_centered(|ui| {
                ui.add_space(24.0);
                ui.label(egui::RichText::new("▣").size(24.0).color(color(0x697187)));
                ui.add_space(8.0);
                ui.label(egui::RichText::new("Nenhuma máquina").strong());
                ui.label(
                    egui::RichText::new("Cadastre uma conexão ou importe uma planilha.")
                        .size(12.0)
                        .color(color(0x969EB2)),
                );
            });
            return;
        }

        let mut root_machines = Vec::new();
        let mut folders: BTreeMap<String, Vec<usize>> = BTreeMap::new();
        for (index, machine) in self.machines.iter().enumerate() {
            if machine.connection.folder.trim().is_empty() {
                root_machines.push(index);
            } else {
                folders
                    .entry(machine.connection.folder.clone())
                    .or_default()
                    .push(index);
            }
        }

        egui::ScrollArea::vertical().show(ui, |ui| {
            for index in root_machines {
                self.show_machine_row(ui, index);
            }
            for (folder, indexes) in folders {
                let all_selected = indexes.iter().all(|index| self.machines[*index].selected);
                let mut select_folder = all_selected;
                ui.horizontal(|ui| {
                    if ui.checkbox(&mut select_folder, "").changed() {
                        for index in &indexes {
                            self.machines[*index].selected = select_folder;
                        }
                    }
                    let folder_header =
                        ui.collapsing(format!("{folder}  ·  {}", indexes.len()), |ui| {
                            for index in &indexes {
                                self.show_machine_row(ui, *index);
                            }
                        });
                    folder_header.header_response.context_menu(|ui| {
                        if ui.button("Renomear pasta…").clicked() {
                            self.folder_name = folder.clone();
                            self.editing_folder = Some(folder.clone());
                            ui.close();
                        }
                        if ui.button("Excluir pasta e máquinas…").clicked() {
                            self.pending_delete =
                                Some(DeleteTarget::Folder(folder.clone(), indexes.len()));
                            ui.close();
                        }
                    });
                });
            }
        });
    }

    fn show_machine_row(&mut self, ui: &mut egui::Ui, index: usize) {
        let machine_id = self.machines[index].id;
        let machine_name = self.machines[index].connection.name.clone();
        ui.horizontal(|ui| {
            let machine = &mut self.machines[index];
            ui.checkbox(&mut machine.selected, "");
            let details = ui.vertical(|ui| {
                ui.label(egui::RichText::new(&machine.connection.name).strong());
                ui.label(
                    egui::RichText::new(format!(
                        "{}:{}  ·  {}",
                        machine.connection.host,
                        machine.connection.port,
                        if machine.connection.database.is_empty() {
                            "sem banco padrão"
                        } else {
                            &machine.connection.database
                        }
                    ))
                    .monospace()
                    .size(11.0)
                    .color(color(0x969EB2)),
                );
            });
            details
                .response
                .interact(egui::Sense::click())
                .context_menu(|ui| {
                    if ui.button("Configurar máquina…").clicked() {
                        self.edit_machine(machine_id);
                        ui.close();
                    }
                    if ui.button("Excluir máquina…").clicked() {
                        self.pending_delete =
                            Some(DeleteTarget::Machine(machine_id, machine_name.clone()));
                        ui.close();
                    }
                });
        });
        ui.add_space(4.0);
    }

    fn show_output_panel(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new("TERMINAL / RESULTADOS")
                    .size(11.0)
                    .strong()
                    .color(color(0x969EB2)),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let status = if self.running {
                    format!("{} respostas pendentes", self.pending_results)
                } else if self.outputs.is_empty() {
                    "Aguardando execução".to_owned()
                } else {
                    format!("{} máquinas", self.outputs.len())
                };
                ui.label(
                    egui::RichText::new(status)
                        .size(12.0)
                        .color(color(0x697187)),
                );
            });
        });
        ui.separator();
        if !self.status_message.is_empty() {
            ui.label(egui::RichText::new(&self.status_message).color(color(0xFF5D70)));
        }
        let output_editor = egui::TextEdit::multiline(&mut self.terminal_output)
            .code_editor()
            .hint_text("Resultados SQL e mensagens por máquina aparecerão aqui. Clique com o botão direito para opções.")
            .desired_rows((ui.available_height() / 18.0).max(8.0) as usize)
            .desired_width(f32::INFINITY)
            .show(ui);
        output_editor.response.context_menu(|ui| {
            if ui.button("Copiar tudo").clicked() {
                ui.ctx().copy_text(self.terminal_output.clone());
                ui.close();
            }
            if ui.button("Selecionar tudo").clicked() {
                self.select_all_output = true;
                ui.close();
            }
            if ui.button("Limpar saída").clicked() {
                self.outputs.clear();
                self.terminal_output.clear();
                ui.close();
            }
        });
        if self.select_all_output {
            let mut state = egui::TextEdit::load_state(ui.ctx(), output_editor.response.id)
                .unwrap_or(output_editor.state);
            state
                .cursor
                .set_char_range(Some(egui::text::CCursorRange::select_all(
                    &output_editor.galley,
                )));
            state.store(ui.ctx(), output_editor.response.id);
            self.select_all_output = false;
        }
    }

    fn show_add_machine_window(&mut self, context: &egui::Context) {
        if !self.show_add_machine {
            return;
        }
        let mut open = self.show_add_machine;
        let mut submit = false;
        let title = if self.editing_machine_id.is_some() {
            "Configurar máquina MySQL"
        } else {
            "Adicionar máquina MySQL"
        };
        egui::Window::new(title)
            .open(&mut open)
            .resizable(false)
            .show(context, |ui| {
                egui::Grid::new("machine_form")
                    .num_columns(2)
                    .show(ui, |ui| {
                        ui.label("Nome");
                        ui.text_edit_singleline(&mut self.machine_form.name);
                        ui.end_row();
                        ui.label("IP ou host");
                        ui.text_edit_singleline(&mut self.machine_form.host);
                        ui.end_row();
                        ui.label("Pasta");
                        ui.text_edit_singleline(&mut self.machine_form.folder);
                        ui.end_row();
                        ui.label("Porta");
                        ui.text_edit_singleline(&mut self.machine_form.port);
                        ui.end_row();
                        ui.label("Banco padrão");
                        ui.text_edit_singleline(&mut self.machine_form.database);
                        ui.end_row();
                        ui.label("Usuário MySQL");
                        ui.text_edit_singleline(&mut self.machine_form.username);
                        ui.end_row();
                        ui.label("Senha");
                        ui.add(
                            egui::TextEdit::singleline(&mut self.machine_form.password)
                                .password(true),
                        );
                        ui.end_row();
                    });
                ui.collapsing("Credenciais padrão para novas conexões", |ui| {
                    ui.label("Usadas quando usuário ou senha ficarem em branco.");
                    ui.horizontal(|ui| {
                        ui.label("Usuário");
                        ui.text_edit_singleline(&mut self.defaults.username);
                    });
                    ui.horizontal(|ui| {
                        ui.label("Senha");
                        ui.add(
                            egui::TextEdit::singleline(&mut self.defaults.password).password(true),
                        );
                    });
                });
                ui.add_space(8.0);
                let submit_label = if self.editing_machine_id.is_some() {
                    "Salvar alterações"
                } else {
                    "Adicionar máquina"
                };
                if ui.button(submit_label).clicked() {
                    submit = true;
                }
                if !self.form_error.is_empty() {
                    ui.label(egui::RichText::new(&self.form_error).color(color(0xFF5D70)));
                }
            });
        self.show_add_machine = open;
        if submit {
            match self.machine_form.build_connection() {
                Ok(connection) => {
                    if let Some(machine_id) = self.editing_machine_id.take() {
                        if let Some(machine) = self
                            .machines
                            .iter_mut()
                            .find(|machine| machine.id == machine_id)
                        {
                            machine.connection = connection;
                        }
                    } else {
                        self.add_machine(connection);
                    }
                    self.machine_form = MachineForm::new(&self.defaults);
                    self.show_add_machine = false;
                    self.form_error.clear();
                }
                Err(error) => self.form_error = error,
            }
        }
    }

    fn open_new_machine(&mut self) {
        self.editing_machine_id = None;
        self.machine_form = MachineForm::new(&self.defaults);
        self.form_error.clear();
        self.show_add_machine = true;
    }

    fn edit_machine(&mut self, machine_id: u64) {
        let Some(machine) = self
            .machines
            .iter()
            .find(|machine| machine.id == machine_id)
        else {
            return;
        };
        self.machine_form = MachineForm {
            name: machine.connection.name.clone(),
            host: machine.connection.host.clone(),
            folder: machine.connection.folder.clone(),
            port: machine.connection.port.to_string(),
            database: machine.connection.database.clone(),
            username: machine.connection.username.clone(),
            password: machine.connection.password.clone(),
        };
        self.editing_machine_id = Some(machine_id);
        self.form_error.clear();
        self.show_add_machine = true;
    }

    fn request_query_execution(&mut self) {
        if self.query.trim().is_empty() {
            self.status_message = "Digite um comando SQL no editor antes de executar.".to_owned();
        } else if self.selected_count() == 0 {
            self.status_message = "Marque uma ou mais máquinas no painel à esquerda.".to_owned();
        } else {
            self.status_message.clear();
            self.confirmation_text.clear();
            self.show_run_confirmation = true;
        }
    }

    fn show_delete_confirmation(&mut self, context: &egui::Context) {
        let Some(target) = &self.pending_delete else {
            return;
        };
        let (title, message) = match target {
            DeleteTarget::Machine(_, name) => {
                ("Excluir máquina", format!("Excluir a conexão {name}?"))
            }
            DeleteTarget::Folder(folder, count) => (
                "Excluir pasta",
                format!("Excluir a pasta {folder} e suas {count} máquinas?"),
            ),
        };
        let mut open = true;
        let mut confirm = false;
        let mut cancel = false;
        egui::Window::new(title)
            .open(&mut open)
            .resizable(false)
            .show(context, |ui| {
                ui.label(message);
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button("Excluir").clicked() {
                        confirm = true;
                    }
                    if ui.button("Cancelar").clicked() {
                        cancel = true;
                    }
                });
            });
        if confirm {
            match self.pending_delete.take().expect("pending deletion exists") {
                DeleteTarget::Machine(id, _) => self.machines.retain(|machine| machine.id != id),
                DeleteTarget::Folder(folder, _) => self
                    .machines
                    .retain(|machine| machine.connection.folder != folder),
            }
        } else if cancel || !open {
            self.pending_delete = None;
        }
    }

    fn show_folder_editor(&mut self, context: &egui::Context) {
        let Some(old_folder) = self.editing_folder.clone() else {
            return;
        };
        let mut open = true;
        let mut save = false;
        egui::Window::new("Renomear pasta")
            .open(&mut open)
            .resizable(false)
            .show(context, |ui| {
                ui.label(format!("Nome atual: {old_folder}"));
                ui.text_edit_singleline(&mut self.folder_name);
                if ui.button("Salvar").clicked() {
                    save = true;
                }
            });
        if save {
            let new_folder = self.folder_name.trim().to_owned();
            for machine in &mut self.machines {
                if machine.connection.folder == old_folder {
                    machine.connection.folder = new_folder.clone();
                }
            }
            self.editing_folder = None;
        } else if !open {
            self.editing_folder = None;
        }
    }

    fn show_run_confirmation_window(&mut self, context: &egui::Context) {
        if !self.show_run_confirmation {
            return;
        }
        let mut open = self.show_run_confirmation;
        let mut execute = false;
        let mut cancel = false;
        egui::Window::new("Confirmar execução")
            .open(&mut open)
            .resizable(false)
            .show(context, |ui| {
                let selected_count = self.selected_count();
                ui.label(format!(
                    "A consulta será enviada a {selected_count} máquinas selecionadas."
                ));
                ui.label("Confira os alvos e o SQL antes de continuar.");
                ui.collapsing("Máquinas selecionadas", |ui| {
                    egui::ScrollArea::vertical()
                        .max_height(120.0)
                        .show(ui, |ui| {
                            for machine in self.machines.iter().filter(|machine| machine.selected) {
                                ui.label(format!(
                                    "{}  ·  {}:{}  ·  {}",
                                    machine.connection.name,
                                    machine.connection.host,
                                    machine.connection.port,
                                    if machine.connection.database.is_empty() {
                                        "sem banco padrão"
                                    } else {
                                        &machine.connection.database
                                    }
                                ));
                            }
                        });
                });
                ui.add_space(8.0);
                ui.label(
                    egui::RichText::new(self.query.trim())
                        .monospace()
                        .color(color(0x969EB2)),
                );
                let needs_phrase = requires_confirmation_phrase(&self.query);
                if needs_phrase {
                    ui.add_space(8.0);
                    ui.label(
                        "Para executar este comando em todas as selecionadas, digite EXECUTAR.",
                    );
                    ui.text_edit_singleline(&mut self.confirmation_text);
                }
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    let phrase_matches = !needs_phrase || self.confirmation_text == "EXECUTAR";
                    if ui
                        .add_enabled(
                            phrase_matches && !self.running,
                            egui::Button::new("Executar consulta"),
                        )
                        .clicked()
                    {
                        execute = true;
                    }
                    if ui.button("Cancelar").clicked() {
                        cancel = true;
                    }
                });
            });
        if cancel {
            open = false;
        }
        self.show_run_confirmation = open;
        if execute {
            self.start_query_execution(context);
            self.show_run_confirmation = false;
        }
    }

    fn import_connections(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Máquinas", &["csv", "xlsx", "xlsm"])
            .pick_file()
        else {
            return;
        };
        match import::load_connections(&path) {
            Ok(report) => self.add_import_report(report),
            Err(error) => self.import_message = error,
        }
    }

    fn add_import_report(&mut self, report: ImportReport) {
        let mut added = 0;
        let mut duplicate_existing = 0;
        for connection in report.machines {
            if self
                .machines
                .iter()
                .any(|entry| same_target(&entry.connection, &connection))
            {
                duplicate_existing += 1;
                continue;
            }
            self.add_machine(connection);
            added += 1;
        }
        let mut notes = vec![format!("{added} máquina(s) importada(s)")];
        let duplicates = report.duplicate_rows.len() + duplicate_existing;
        if duplicates > 0 {
            notes.push(format!("{duplicates} duplicada(s) ignorada(s)"));
            if !report.duplicate_rows.is_empty() {
                notes.push(format!(
                    "linhas {}",
                    format_row_numbers(&report.duplicate_rows)
                ));
            }
        }
        if !report.rows_without_host.is_empty() {
            notes.push(format!(
                "sem IP nas linhas {}",
                format_row_numbers(&report.rows_without_host)
            ));
        }
        self.import_message = notes.join(" · ");
    }

    fn save_csv_template(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("CSV", &["csv"])
            .set_file_name("querymass-maquinas.csv")
            .save_file()
        else {
            return;
        };
        match std::fs::write(
            &path,
            "IP;HOSTNAME;LOCATION;PORT;DATABASE;USER;PASSWORD\n192.0.2.10;mysql-filial-01;Filial 01;3306;pdv;;\n",
        ) {
            Ok(()) => self.import_message = format!("Modelo salvo em {}", path.display()),
            Err(error) => self.import_message = format!("Falha ao salvar modelo: {error}"),
        }
    }

    fn add_machine(&mut self, connection: MachineConnection) {
        self.machines.push(MachineEntry {
            id: self.next_machine_id,
            selected: true,
            connection,
        });
        self.next_machine_id += 1;
    }

    fn selected_count(&self) -> usize {
        self.machines
            .iter()
            .filter(|machine| machine.selected)
            .count()
    }

    fn start_query_execution(&mut self, context: &egui::Context) {
        let selected: Vec<MachineConnection> = self
            .machines
            .iter()
            .filter(|machine| machine.selected)
            .map(|machine| {
                let mut connection = machine.connection.clone();
                if connection.username.trim().is_empty() {
                    connection.username = self.defaults.username.clone();
                }
                if connection.password.is_empty() {
                    connection.password = self.defaults.password.clone();
                }
                connection
            })
            .collect();
        if selected.is_empty() {
            self.status_message = "Selecione uma ou mais máquinas no painel à esquerda.".to_owned();
            return;
        }
        self.status_message.clear();
        self.outputs.clear();
        self.terminal_output.clear();
        self.pending_results = selected.len();
        self.running = true;
        let query = self.query.clone();
        let jobs = Arc::new(Mutex::new(VecDeque::from(
            selected
                .into_iter()
                .map(|machine| (machine, query.clone()))
                .collect::<Vec<_>>(),
        )));
        let worker_count = self.pending_results.min(16);
        for _ in 0..worker_count {
            let jobs = Arc::clone(&jobs);
            let sender = self.result_sender.clone();
            let context = context.clone();
            thread::spawn(move || {
                loop {
                    let job = jobs.lock().expect("query queue poisoned").pop_front();
                    let Some((machine, query)) = job else {
                        break;
                    };
                    if sender.send(database::execute(machine, query)).is_err() {
                        break;
                    }
                    context.request_repaint();
                }
            });
        }
    }

    fn receive_query_outputs(&mut self) {
        while let Ok(output) = self.result_receiver.try_recv() {
            self.terminal_output.push_str(&format_query_output(&output));
            self.outputs.push(output);
            self.pending_results = self.pending_results.saturating_sub(1);
            if self.pending_results == 0 {
                self.running = false;
            }
        }
    }
}

fn format_query_output(output: &QueryOutput) -> String {
    let mut text = format!("{} · {}\n", output.machine_name, output.host);
    if let Some(error) = &output.error {
        text.push_str(&format!("ERRO: {error}\n\n"));
        return text;
    }
    if output.columns.is_empty() {
        text.push_str(&format!("Linhas afetadas: {}\n\n", output.affected_rows));
        return text;
    }
    text.push_str(&output.columns.join("\t"));
    text.push('\n');
    for row in &output.rows {
        text.push_str(&row.join("\t"));
        text.push('\n');
    }
    if output.truncated {
        text.push_str("Exibindo as primeiras 1.000 linhas.\n");
    }
    text.push('\n');
    text
}

fn same_target(first: &MachineConnection, second: &MachineConnection) -> bool {
    first.host.eq_ignore_ascii_case(&second.host) && first.port == second.port
}

fn format_row_numbers(rows: &[usize]) -> String {
    const DISPLAY_LIMIT: usize = 12;
    let displayed = rows
        .iter()
        .take(DISPLAY_LIMIT)
        .map(usize::to_string)
        .collect::<Vec<_>>()
        .join(", ");
    if rows.len() > DISPLAY_LIMIT {
        format!("{displayed} e mais {}", rows.len() - DISPLAY_LIMIT)
    } else {
        displayed
    }
}

fn requires_confirmation_phrase(query: &str) -> bool {
    let first_keyword = first_sql_keyword(query);
    let ends_with_semicolon = query.trim_end().ends_with(';');
    let contains_multiple_statements = query
        .rfind(';')
        .is_some_and(|semicolon| !query[semicolon + 1..].trim().is_empty())
        || (query.matches(';').count() > 1 && ends_with_semicolon);
    !first_keyword.eq_ignore_ascii_case("SELECT")
        || query.to_ascii_uppercase().contains("INTO OUTFILE")
        || query.to_ascii_uppercase().contains("INTO DUMPFILE")
        || contains_multiple_statements
}

fn first_sql_keyword(query: &str) -> String {
    let mut remaining = query.trim_start();
    loop {
        if let Some(after_comment) = remaining.strip_prefix("--") {
            remaining = after_comment
                .split_once('\n')
                .map_or("", |(_, rest)| rest)
                .trim_start();
        } else if let Some(after_comment) = remaining.strip_prefix('#') {
            remaining = after_comment
                .split_once('\n')
                .map_or("", |(_, rest)| rest)
                .trim_start();
        } else if let Some(after_comment) = remaining.strip_prefix("/*") {
            remaining = after_comment
                .split_once("*/")
                .map_or("", |(_, rest)| rest)
                .trim_start();
        } else {
            break;
        }
    }
    remaining
        .split(|character: char| !character.is_ascii_alphabetic())
        .next()
        .unwrap_or_default()
        .to_owned()
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
        [
            egui::pos2(query.x - 1.3, query.y),
            egui::pos2(query.x + 1.3, query.y),
        ],
        egui::Stroke::new(1.0_f32, color(0xF2F4FA)),
    );
    painter.line_segment(
        [
            egui::pos2(query.x, query.y - 1.3),
            egui::pos2(query.x, query.y + 1.3),
        ],
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
