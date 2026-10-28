mod connections;
#[cfg(feature = "ui-qa")]
mod qa;
mod results;
mod workspace;
use eframe::egui::{self, Color32, RichText};
use querymass::{
    database::{self, ExecutionOptions, MachineConnection, Outcome, QueryOutput},
    engine::{Event, Run},
    import::ImportReport,
    model::{self, HistoryEntry, Profile, Workspace},
    sql,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
    sync::mpsc,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

pub const BG: Color32 = Color32::from_rgb(14, 18, 27);
pub const PANEL: Color32 = Color32::from_rgb(21, 27, 40);
pub const SURFACE: Color32 = Color32::from_rgb(28, 36, 52);
pub const BORDER: Color32 = Color32::from_rgb(43, 55, 77);
pub const TEXT: Color32 = Color32::from_rgb(229, 235, 245);
pub const MUTED: Color32 = Color32::from_rgb(150, 166, 190);
pub const BLUE: Color32 = Color32::from_rgb(83, 146, 255);
pub const RED: Color32 = Color32::from_rgb(255, 105, 124);
pub const GREEN: Color32 = Color32::from_rgb(83, 211, 173);
pub const AMBER: Color32 = Color32::from_rgb(245, 190, 100);

#[derive(Clone, Copy, PartialEq)]
pub enum ManagerTab {
    Machine,
    Import,
    Shared,
    Folders,
}
#[derive(Clone)]
pub struct Draft {
    pub profile: Profile,
    pub port: String,
}
impl Draft {
    fn new(id: u64) -> Self {
        Self {
            profile: Profile {
                id,
                connection: MachineConnection {
                    name: String::new(),
                    host: String::new(),
                    folder: String::new(),
                    port: 3306,
                    database: String::new(),
                    username: String::new(),
                    password: String::new(),
                    tls: false,
                    ca_path: None,
                },
                use_shared_credentials: true,
                remember_password: false,
                credential_unavailable: false,
                selected: false,
            },
            port: "3306".into(),
        }
    }
}
struct Confirmation {
    sql: String,
    targets: Vec<(u64, MachineConnection)>,
    phrase: String,
    statements: usize,
}
enum Delete {
    Machine(u64, String),
    Folder(String),
}
pub enum Message {
    ConnectionTest(MachineConnection, Result<String, String>),
    Imported(Result<ImportReport, String>),
}
pub struct App {
    #[cfg(feature = "ui-qa")]
    qa_events: std::collections::VecDeque<Vec<egui::Event>>,
    workspace: Workspace,
    _workspace_lock: std::fs::File,
    config_path: PathBuf,
    dirty: bool,
    last_edit: Instant,
    saving_error: Option<String>,
    recovery: bool,
    search: String,
    sql_selection: Option<String>,
    manager: bool,
    manager_tab: ManagerTab,
    draft: Draft,
    editing: bool,
    shared_draft: model::SharedCredentials,
    new_folder: String,
    folder_rename: Option<(String, String)>,
    form_error: Option<String>,
    form_success: Option<String>,
    testing: bool,
    tested_connection: Option<MachineConnection>,
    importing: bool,
    import_report: Option<ImportReport>,
    import_folder: String,
    import_remember: bool,
    sender: mpsc::Sender<Message>,
    receiver: mpsc::Receiver<Message>,
    notice: Option<String>,
    logs: Vec<String>,
    run: Option<Run>,
    jobs: BTreeMap<u64, (String, String)>,
    outputs: Vec<QueryOutput>,
    run_sql: String,
    run_started: Option<Instant>,
    run_total: usize,
    completed: usize,
    history_written: bool,
    confirmation: Option<Confirmation>,
    delete: Option<Delete>,
    selected_output: usize,
    selected_set: usize,
    messages_tab: bool,
    row_selection: BTreeSet<usize>,
    result_search: String,
    history_open: bool,
    result_bytes: usize,
    cancel_requested: bool,
}
impl App {
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        path: PathBuf,
        workspace_lock: std::fs::File,
    ) -> Self {
        theme(&cc.egui_ctx);

        let (workspace, warnings, recovery) = match model::load(&path) {
            Ok((workspace, warnings)) => (workspace, warnings, false),
            Err(error) => (Workspace::default(), vec![error], true),
        };
        let (sender, receiver) = mpsc::channel();
        let shared_draft = workspace.shared.clone();
        let draft = Draft::new(workspace.next_id());
        Self {
            #[cfg(feature = "ui-qa")]
            qa_events: std::collections::VecDeque::new(),
            workspace,
            _workspace_lock: workspace_lock,
            config_path: path,
            dirty: false,
            last_edit: Instant::now(),
            saving_error: None,
            recovery,
            search: String::new(),
            sql_selection: None,
            manager: false,
            manager_tab: ManagerTab::Machine,
            draft,
            editing: false,
            shared_draft,
            new_folder: String::new(),
            folder_rename: None,
            form_error: None,
            form_success: None,
            testing: false,
            tested_connection: None,
            importing: false,
            import_report: None,
            import_folder: String::new(),
            import_remember: false,
            sender,
            receiver,
            notice: warnings.first().cloned(),
            logs: warnings,
            run: None,
            jobs: BTreeMap::new(),
            outputs: Vec::new(),
            run_sql: String::new(),
            run_started: None,
            run_total: 0,
            completed: 0,
            history_written: false,
            confirmation: None,
            delete: None,
            selected_output: 0,
            selected_set: 0,
            messages_tab: false,
            row_selection: BTreeSet::new(),
            result_search: String::new(),
            history_open: false,
            result_bytes: 0,
            cancel_requested: false,
        }
    }
    fn changed(&mut self) {
        self.dirty = true;
        self.last_edit = Instant::now();
    }
    fn save(&mut self) {
        if self.recovery {
            return;
        }
        match model::save(&self.config_path, &self.workspace) {
            Ok(()) => {
                self.dirty = false;
                self.saving_error = None;
            }
            Err(error) => self.saving_error = Some(error),
        }
    }
    fn open_manager(&mut self, tab: ManagerTab) {
        if self.recovery {
            self.notice = Some(
                "Recupere o arquivo de configuração pelo aviso acima antes de cadastrar conexões."
                    .into(),
            );
            return;
        }
        self.manager = true;
        self.manager_tab = tab;
        self.form_error = None;
        self.form_success = None;
        self.shared_draft = self.workspace.shared.clone();
        if tab == ManagerTab::Machine {
            self.draft = Draft::new(self.workspace.next_id());
            self.editing = false;
        }
    }
    fn edit(&mut self, id: u64) {
        if let Some(profile) = self
            .workspace
            .profiles
            .iter()
            .find(|profile| profile.id == id)
        {
            self.draft = Draft {
                profile: profile.clone(),
                port: profile.connection.port.to_string(),
            };
            self.editing = true;
            self.manager = true;
            self.manager_tab = ManagerTab::Machine;
            self.form_error = None;
        }
    }
    fn request_run(&mut self) {
        if self.run.is_some() {
            return;
        }
        let execution_sql = self
            .sql_selection
            .clone()
            .unwrap_or_else(|| self.workspace.sql.clone());
        let plan = match sql::analyze(&execution_sql) {
            Ok(plan) => plan,
            Err(error) => {
                self.notice = Some(error);
                return;
            }
        };
        if self.workspace.read_only && !plan.read_only_candidate {
            self.notice=Some("Este SQL pode alterar dados. Desative ‘Somente leitura’ para revisar e confirmar a execução.".into());
            return;
        }
        if self
            .workspace
            .profiles
            .iter()
            .filter(|profile| profile.selected)
            .any(|profile| {
                if profile.use_shared_credentials {
                    self.workspace.shared.credential_unavailable
                } else {
                    profile.credential_unavailable
                }
            })
        {
            self.notice=Some("A senha de uma conexão não foi recuperada do cofre. Abra Conexões e informe-a novamente.".into());
            return;
        }
        let targets: Vec<_> = self
            .workspace
            .profiles
            .iter()
            .filter(|p| p.selected)
            .map(|p| (p.id, self.workspace.resolved(p)))
            .collect();
        if targets.is_empty() {
            self.notice = Some("Marque pelo menos uma máquina no painel à esquerda.".into());
            return;
        }
        if let Some((_, machine)) = targets
            .iter()
            .find(|(_, machine)| machine.username.trim().is_empty())
        {
            self.notice = Some(format!(
                "{} está sem usuário MySQL. Abra Conexões para configurar as credenciais.",
                machine.name
            ));
            return;
        }
        if self.workspace.read_only {
            self.start(targets, execution_sql.clone());
        } else {
            self.confirmation = Some(Confirmation {
                sql: execution_sql,
                targets,
                phrase: String::new(),
                statements: plan.statements.len(),
            });
        }
    }
    fn start(&mut self, targets: Vec<(u64, MachineConnection)>, sql: String) {
        self.jobs = targets
            .iter()
            .map(|(id, machine)| (*id, (machine.name.clone(), "Na fila".into())))
            .collect();
        self.run_total = targets.len();
        self.completed = 0;
        self.history_written = false;
        self.outputs.clear();
        self.logs.clear();
        self.selected_output = 0;
        self.selected_set = 0;
        self.row_selection.clear();
        self.result_search.clear();
        self.result_bytes = 0;
        self.cancel_requested = false;
        self.logs.push(format!(
            "Execução iniciada em {} máquina(s). {}",
            targets.len(),
            if self.workspace.read_only {
                "Transação de leitura exigida no servidor."
            } else {
                "Alterações independentes por servidor; sem repetição automática."
            }
        ));
        self.run_sql = sql.clone();
        self.run_started = Some(Instant::now());
        self.notice = None;
        self.run = Some(Run::start(
            targets,
            sql,
            ExecutionOptions {
                read_only: self.workspace.read_only,
                row_limit: self.workspace.row_limit,
                timeout: Duration::from_secs(self.workspace.timeout_seconds),
                deadline: Duration::from_secs(self.workspace.timeout_seconds),
                ..Default::default()
            },
            self.workspace.concurrency,
        ));
    }
    fn poll(&mut self) {
        while let Ok(message) = self.receiver.try_recv() {
            match message {
                Message::ConnectionTest(snapshot, outcome) => {
                    let mut current = self.workspace.resolved(&self.draft.profile);
                    current.port = self.draft.port.parse().unwrap_or(0);
                    if current != snapshot {
                        self.testing = false;
                        self.form_success = None;
                        continue;
                    }
                    self.tested_connection = Some(snapshot);
                    self.testing = false;
                    self.form_error = None;
                    match outcome {
                        Ok(message) => self.form_success = Some(message),
                        Err(error) => self.form_error = Some(error),
                    }
                }
                Message::Imported(report) => {
                    self.importing = false;
                    match report {
                        Ok(report) => self.import_report = Some(report),
                        Err(error) => self.form_error = Some(error),
                    }
                }
            }
        }
        let events: Vec<_> = self
            .run
            .as_ref()
            .map(|run| run.receiver.try_iter().collect())
            .unwrap_or_default();
        for event in events {
            match event {
                Event::Connecting(id) => {
                    if let Some(job) = self.jobs.get_mut(&id) {
                        job.1 = "Conectando".into();
                    }
                }
                Event::Executing(id) => {
                    if let Some(job) = self.jobs.get_mut(&id) {
                        job.1 = "Executando".into();
                    }
                }
                Event::Skipped(id) => {
                    self.completed += 1;
                    if let Some(job) = self.jobs.get_mut(&id) {
                        job.1 = "Cancelado".into();
                    }
                }
                Event::CancellationErrors(errors) => self.logs.extend(errors),
                Event::Finished(mut output) => {
                    self.completed += 1;
                    if let Some(job) = self.jobs.get_mut(&output.machine_id) {
                        job.1 = outcome_label(&output.outcome).into();
                    }
                    let bytes = output.memory_bytes();
                    if self.result_bytes.saturating_add(bytes) > 64 * 1024 * 1024 {
                        for set in &mut output.sets {
                            set.rows.clear();
                            set.truncated = true;
                        }
                        self.logs.push("Limite de 64 MiB da execução atingido; resultados seguintes têm apenas metadados.".into());
                    } else {
                        self.result_bytes += bytes;
                    }
                    self.logs.push(format!(
                        "{} · {} · {:.2}s{}",
                        output.machine_name,
                        outcome_label(&output.outcome),
                        output.elapsed.as_secs_f32(),
                        output
                            .error
                            .as_ref()
                            .map(|e| format!(" · {e}"))
                            .unwrap_or_default()
                    ));
                    self.outputs.push(output);
                }
            }
        }
        if self.run.is_some() && self.completed == self.run_total && !self.history_written {
            self.history_written = true;
            self.run = None;
            if !sql::may_contain_credentials(&self.run_sql) {
                self.workspace.history.insert(
                    0,
                    HistoryEntry {
                        when: SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs(),
                        sql: self.run_sql.clone(),
                        targets: self.run_total,
                        success: self
                            .outputs
                            .iter()
                            .filter(|o| o.outcome == Outcome::Success)
                            .count(),
                        failed: self
                            .outputs
                            .iter()
                            .filter(|o| o.outcome != Outcome::Success)
                            .count(),
                    },
                );
            }
            self.workspace.history.truncate(30);
            self.changed();
            self.logs.push(
                "Execução encerrada. Os resultados ficam disponíveis até a próxima execução."
                    .into(),
            );
        }
    }
}
impl eframe::App for App {
    #[cfg(feature = "ui-qa")]
    fn raw_input_hook(&mut self, ctx: &egui::Context, raw: &mut egui::RawInput) {
        qa::input(ctx, raw, &mut self.qa_events);
    }
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        #[cfg(feature = "ui-qa")]
        qa::capture(ctx);
        self.poll();
        if self.dirty && self.run.is_none() && ctx.input(|input| input.viewport().close_requested())
        {
            self.save();
            if self.saving_error.is_some() {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            }
        }
        if self.run.is_some() && ctx.input(|input| input.viewport().close_requested()) {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.notice = Some(
                "Há consultas em andamento. Cancele ou aguarde o encerramento antes de fechar."
                    .into(),
            );
        }
        if self.run.is_some() || self.testing || self.importing {
            ctx.request_repaint_after(Duration::from_millis(80));
        }
        if self.dirty && self.last_edit.elapsed() > Duration::from_secs(1) {
            self.save();
        }
        if ctx.input_mut(|input| input.consume_key(egui::Modifiers::CTRL, egui::Key::Enter))
            && !self.manager
            && self.confirmation.is_none()
        {
            self.request_run();
        }
        self.top(ctx);
        egui::TopBottomPanel::bottom("status")
            .frame(
                egui::Frame::new()
                    .fill(BG)
                    .inner_margin(egui::Margin::symmetric(20, 8)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(format!(
                            "{} máquinas  /  {} pastas",
                            self.workspace.profiles.len(),
                            self.workspace.folders.len()
                        ))
                        .size(12.0)
                        .color(MUTED),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let status = if self.recovery {
                            "Configuração precisa de recuperação"
                        } else if self.saving_error.is_some() {
                            "Falha ao salvar"
                        } else if self.dirty {
                            "Salvando alterações…"
                        } else {
                            "Espaço de trabalho salvo"
                        };
                        ui.label(RichText::new(status).size(12.0).color(
                            if self.saving_error.is_some() || self.recovery {
                                AMBER
                            } else {
                                MUTED
                            },
                        ));
                    });
                });
            });
        egui::SidePanel::left("targets")
            .default_width(300.0)
            .width_range(260.0..=480.0)
            .frame(egui::Frame::new().fill(PANEL).inner_margin(16))
            .show(ctx, |ui| self.sidebar(ui));
        egui::TopBottomPanel::bottom("results")
            .resizable(true)
            .default_height(420.0)
            .min_height(220.0)
            .frame(egui::Frame::new().fill(PANEL).inner_margin(16))
            .show(ctx, |ui| self.results(ui));
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(BG).inner_margin(20))
            .show(ctx, |ui| self.editor(ui));
        self.connection_manager(ctx);
        self.confirmation_dialog(ctx);
        self.delete_dialog(ctx);
        self.history_dialog(ctx);
        #[cfg(feature = "ui-qa")]
        qa::state(self, ctx);
        if let Some(error) = self.saving_error.clone() {
            egui::Window::new("Não foi possível salvar")
                .collapsible(false)
                .show(ctx, |ui| {
                    ui.label(error);
                    if ui.button("Tentar novamente").clicked() {
                        self.save();
                    }
                });
        }
    }
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        if let Some(run) = &self.run {
            run.cancel();
        }
        self.save();
    }
}
fn outcome_label(outcome: &Outcome) -> &'static str {
    match outcome {
        Outcome::Success => "Concluído",
        Outcome::Failed => "Falhou",
        Outcome::Partial => "Falha parcial",
        Outcome::Unknown => "Resultado desconhecido",
        Outcome::Cancelled => "Cancelado",
        Outcome::TimedOut => "Prazo esgotado",
    }
}
fn outcome_color(outcome: &Outcome) -> Color32 {
    match outcome {
        Outcome::Success => GREEN,
        Outcome::Cancelled => MUTED,
        Outcome::Unknown | Outcome::Partial | Outcome::TimedOut => AMBER,
        Outcome::Failed => RED,
    }
}
fn primary(label: impl Into<egui::WidgetText>) -> egui::Button<'static> {
    egui::Button::new(label)
        .fill(BLUE)
        .min_size(egui::vec2(0.0, 34.0))
}
fn theme(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();
    style.spacing.item_spacing = egui::vec2(10.0, 10.0);
    style.spacing.button_padding = egui::vec2(12.0, 7.0);
    style.spacing.interact_size.y = 30.0;
    style.spacing.window_margin = egui::Margin::same(16);
    style
        .text_styles
        .insert(egui::TextStyle::Body, egui::FontId::proportional(14.0));
    style
        .text_styles
        .insert(egui::TextStyle::Button, egui::FontId::proportional(14.0));
    style
        .text_styles
        .insert(egui::TextStyle::Monospace, egui::FontId::monospace(14.0));
    let mut visuals = egui::Visuals::dark();
    visuals.widgets.inactive.corner_radius = egui::CornerRadius::same(6);
    visuals.widgets.hovered.corner_radius = egui::CornerRadius::same(6);
    visuals.widgets.active.corner_radius = egui::CornerRadius::same(6);
    visuals.panel_fill = PANEL;
    visuals.window_fill = PANEL;
    visuals.extreme_bg_color = BG;
    visuals.faint_bg_color = SURFACE;
    visuals.override_text_color = Some(TEXT);
    visuals.widgets.inactive.bg_fill = SURFACE;
    visuals.widgets.inactive.bg_stroke = egui::Stroke::new(1.0_f32, BORDER);
    visuals.widgets.hovered.bg_fill = Color32::from_rgb(43, 61, 86);
    visuals.widgets.active.bg_fill = BLUE;
    visuals.selection.bg_fill = Color32::from_rgb(37, 74, 126);
    visuals.hyperlink_color = BLUE;
    visuals.window_corner_radius = egui::CornerRadius::same(12);
    style.visuals = visuals;
    ctx.set_style(style);
}
