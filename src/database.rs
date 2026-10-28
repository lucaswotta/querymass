use mysql::{Conn, OptsBuilder, SslOpts, Value, prelude::Queryable};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant};

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MachineConnection {
    pub name: String,
    pub host: String,
    pub folder: String,
    pub port: u16,
    pub database: String,
    pub username: String,
    #[serde(skip)]
    pub password: String,
    #[serde(default)]
    pub tls: bool,
    #[serde(default)]
    pub ca_path: Option<PathBuf>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Cell {
    pub text: String,
    pub is_null: bool,
}

#[derive(Clone, Debug)]
pub struct ResultSet {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<Cell>>,
    pub affected_rows: u64,
    pub last_insert_id: Option<u64>,
    pub warnings: u16,
    pub truncated: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Outcome {
    Success,
    Failed,
    Partial,
    Unknown,
    Cancelled,
    TimedOut,
}

#[derive(Clone, Debug)]
pub struct QueryOutput {
    pub machine_id: u64,
    pub machine_name: String,
    pub host: String,
    pub database: String,
    pub sets: Vec<ResultSet>,
    pub outcome: Outcome,
    pub error: Option<String>,
    pub elapsed: Duration,
}

#[derive(Clone)]
pub struct ExecutionOptions {
    pub read_only: bool,
    pub row_limit: usize,
    pub byte_limit: usize,
    pub timeout: Duration,
    pub deadline: Duration,
}
impl Default for ExecutionOptions {
    fn default() -> Self {
        Self {
            read_only: true,
            row_limit: 1000,
            byte_limit: 4 * 1024 * 1024,
            timeout: Duration::from_secs(30),
            deadline: Duration::from_secs(30),
        }
    }
}

#[derive(Clone)]
pub struct ActiveQuery {
    pub machine: MachineConnection,
    pub connection_id: u32,
    pub live: Arc<AtomicBool>,
}
pub type ActiveQueries = Arc<Mutex<Vec<ActiveQuery>>>;

pub fn options(machine: &MachineConnection, timeout: Duration) -> OptsBuilder {
    let mut options = OptsBuilder::default()
        .ip_or_hostname(Some(machine.host.clone()))
        .tcp_port(machine.port)
        .user(Some(machine.username.clone()))
        .pass(Some(machine.password.clone()))
        .tcp_connect_timeout(Some(Duration::from_secs(5)))
        .read_timeout(Some(timeout))
        .write_timeout(Some(timeout));
    if !machine.database.is_empty() {
        options = options.db_name(Some(machine.database.clone()));
    }
    if machine.tls {
        let tls = SslOpts::default().with_root_cert_path(machine.ca_path.clone());
        options = options.ssl_opts(Some(tls));
    }
    options
}

pub fn test_connection(machine: &MachineConnection) -> Result<String, String> {
    let mut conn =
        Conn::new(options(machine, Duration::from_secs(5))).map_err(|e| e.to_string())?;
    let version: Option<String> = conn
        .query_first("SELECT VERSION()")
        .map_err(|e| e.to_string())?;
    Ok(format!("Conectado · MySQL {}", version.unwrap_or_default()))
}

pub fn cancel_active(active: &ActiveQueries) -> Vec<String> {
    let queries = match active.lock() {
        Ok(queries) => queries.clone(),
        Err(_) => return vec!["Não foi possível acessar as consultas ativas.".into()],
    };
    let mut errors = Vec::new();
    for query in queries {
        if !query.live.load(Ordering::SeqCst) {
            continue;
        }
        if let Err(error) = kill_active(&query) {
            errors.push(format!("{}: {error}", query.machine.name));
        }
    }
    errors
}

fn kill_active(query: &ActiveQuery) -> Result<(), String> {
    if !query.live.load(Ordering::SeqCst) {
        return Ok(());
    }
    let mut conn = Conn::new(options(&query.machine, Duration::from_secs(5)))
        .map_err(|error| error.to_string())?;
    if query.live.load(Ordering::SeqCst) {
        conn.query_drop(format!("KILL QUERY {}", query.connection_id))
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

pub fn execute(
    machine_id: u64,
    machine: MachineConnection,
    sql: &str,
    settings: &ExecutionOptions,
    cancelled: &AtomicBool,
    active: &ActiveQueries,
    executing: impl FnOnce(),
) -> QueryOutput {
    let start = Instant::now();
    let mut output = QueryOutput {
        machine_id,
        machine_name: machine.name.clone(),
        host: machine.host.clone(),
        database: machine.database.clone(),
        sets: Vec::new(),
        outcome: Outcome::Success,
        error: None,
        elapsed: Duration::ZERO,
    };
    if cancelled.load(Ordering::SeqCst) {
        output.outcome = Outcome::Cancelled;
        return output;
    }
    let plan = match crate::sql::analyze(sql) {
        Ok(plan) => plan,
        Err(error) => {
            output.outcome = Outcome::Failed;
            output.error = Some(error);
            return output;
        }
    };
    if settings.read_only && !plan.read_only_candidate {
        output.outcome = Outcome::Failed;
        output.error = Some("Comando bloqueado pelo modo somente leitura.".into());
        return output;
    }
    let mut conn = match Conn::new(options(&machine, settings.timeout)) {
        Ok(conn) => conn,
        Err(error) => {
            output.outcome = Outcome::Failed;
            output.error = Some(error.to_string());
            output.elapsed = start.elapsed();
            return output;
        }
    };
    if let Err(error) = conn.query_drop(format!(
        "SET SESSION SQL_SELECT_LIMIT = {}",
        settings.row_limit.saturating_add(1)
    )) {
        output.outcome = Outcome::Failed;
        output.error = Some(error.to_string());
        output.elapsed = start.elapsed();
        return output;
    }
    if settings.read_only
        && let Err(error) = conn.query_drop("START TRANSACTION READ ONLY")
    {
        output.outcome = Outcome::Failed;
        output.error = Some(format!(
            "O servidor não iniciou uma transação de leitura: {error}"
        ));
        output.elapsed = start.elapsed();
        return output;
    }
    let live = Arc::new(AtomicBool::new(true));
    let connection_id = conn.connection_id();
    let active_query = ActiveQuery {
        machine,
        connection_id,
        live: live.clone(),
    };
    if let Ok(mut queries) = active.lock() {
        queries.push(active_query.clone());
    }
    let timed_out = Arc::new(AtomicBool::new(false));
    let deadline_error = Arc::new(Mutex::new(None));
    let (finish_sender, finish_receiver) = std::sync::mpsc::channel();
    let timer_flag = timed_out.clone();
    let timer_error = deadline_error.clone();
    let deadline = settings.deadline;
    std::thread::spawn(move || {
        if matches!(
            finish_receiver.recv_timeout(deadline),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout)
        ) && active_query.live.load(Ordering::SeqCst)
        {
            timer_flag.store(true, Ordering::SeqCst);
            if let Err(error) = kill_active(&active_query)
                && let Ok(mut message) = timer_error.lock()
            {
                *message = Some(error);
            }
        }
    });
    executing();
    if cancelled.load(Ordering::SeqCst) {
        output.outcome = Outcome::Cancelled;
    } else {
        let mut bytes = 0;
        if settings.read_only {
            for statement in plan.statements {
                if cancelled.load(Ordering::SeqCst) {
                    break;
                }
                match conn.exec_iter(statement, ()) {
                    Ok(result) => {
                        collect_results(result, &mut output, settings, cancelled, &mut bytes)
                    }
                    Err(error) => {
                        record_error(&mut output, error, true, cancelled.load(Ordering::SeqCst))
                    }
                }
                if output.error.is_some() {
                    break;
                }
            }
        } else {
            match conn.query_iter(sql) {
                Ok(result) => collect_results(result, &mut output, settings, cancelled, &mut bytes),
                Err(error) => {
                    record_error(&mut output, error, false, cancelled.load(Ordering::SeqCst))
                }
            }
        }
    }
    if cancelled.load(Ordering::SeqCst) && output.outcome == Outcome::Success {
        output.outcome = if settings.read_only {
            Outcome::Cancelled
        } else {
            Outcome::Unknown
        };
        output.error = Some("A execução foi interrompida ou terminou durante o pedido de cancelamento. Confira os resultados recebidos.".into());
    }
    live.store(false, Ordering::SeqCst);
    let _ = finish_sender.send(());
    if timed_out.load(Ordering::SeqCst) {
        output.outcome = if settings.read_only {
            Outcome::TimedOut
        } else {
            Outcome::Unknown
        };
        output.error = Some(format!(
            "Prazo de {}s esgotado.{}",
            settings.deadline.as_secs(),
            deadline_error
                .lock()
                .ok()
                .and_then(|message| message.clone())
                .map(|message| format!(" Não foi possível interromper no servidor: {message}"))
                .unwrap_or_default()
        ));
    }
    if let Ok(mut queries) = active.lock() {
        queries.retain(|query| !Arc::ptr_eq(&query.live, &live));
    }
    if settings.read_only
        && let Err(error) = conn.query_drop("ROLLBACK")
        && output.error.is_none()
    {
        output.outcome = Outcome::Partial;
        output.error = Some(format!(
            "Não foi possível encerrar a transação de leitura: {error}"
        ));
    }
    output.elapsed = start.elapsed();
    output
}

fn collect_results<T: mysql::prelude::Protocol>(
    mut result: mysql::QueryResult<'_, '_, '_, T>,
    output: &mut QueryOutput,
    settings: &ExecutionOptions,
    cancelled: &AtomicBool,
    bytes: &mut usize,
) {
    while let Some(mut set) = result.iter() {
        let mut captured = ResultSet {
            columns: set
                .columns()
                .as_ref()
                .iter()
                .map(|column| column.name_str().into_owned())
                .collect(),
            rows: Vec::new(),
            affected_rows: set.affected_rows(),
            last_insert_id: set.last_insert_id(),
            warnings: set.warnings(),
            truncated: false,
        };
        *bytes += captured
            .columns
            .iter()
            .map(|column| column.len() + std::mem::size_of::<String>())
            .sum::<usize>();
        for row in &mut set {
            match row {
                Ok(row) => {
                    if captured.rows.len() >= settings.row_limit || *bytes >= settings.byte_limit {
                        captured.truncated = true;
                        continue;
                    }
                    let cells: Vec<Cell> = (0..row.len())
                        .map(|index| format_value(row.as_ref(index).unwrap_or(&Value::NULL)))
                        .collect();
                    let row_bytes = cells
                        .iter()
                        .map(|cell| cell.text.len() + std::mem::size_of::<Cell>())
                        .sum::<usize>()
                        + std::mem::size_of::<Vec<Cell>>();
                    if bytes.saturating_add(row_bytes) > settings.byte_limit {
                        captured.truncated = true;
                        continue;
                    }
                    *bytes += row_bytes;
                    captured.rows.push(cells);
                }
                Err(error) => record_error(
                    output,
                    error,
                    settings.read_only,
                    cancelled.load(Ordering::SeqCst),
                ),
            }
        }
        if output.error.is_none() || !captured.columns.is_empty() || captured.affected_rows > 0 {
            output.sets.push(captured);
        }
    }
}

fn record_error(output: &mut QueryOutput, error: mysql::Error, read_only: bool, cancelled: bool) {
    let network = matches!(
        error,
        mysql::Error::IoError(_) | mysql::Error::DriverError(_)
    );
    output.outcome = if !read_only && (network || cancelled) {
        Outcome::Unknown
    } else if cancelled {
        Outcome::Cancelled
    } else if !output.sets.is_empty() {
        Outcome::Partial
    } else {
        Outcome::Failed
    };
    output.error = Some(error.to_string());
}

fn format_value(value: &Value) -> Cell {
    let text = match value {
        Value::NULL => {
            return Cell {
                text: "NULL".into(),
                is_null: true,
            };
        }
        Value::Bytes(bytes) => match std::str::from_utf8(bytes) {
            Ok(text) => text.to_owned(),
            Err(_) => format!(
                "0x{}",
                bytes
                    .iter()
                    .map(|byte| format!("{byte:02X}"))
                    .collect::<String>()
            ),
        },
        Value::Int(value) => value.to_string(),
        Value::UInt(value) => value.to_string(),
        Value::Float(value) => value.to_string(),
        Value::Double(value) => value.to_string(),
        Value::Date(y, m, d, h, min, s, micros) => {
            format!("{y:04}-{m:02}-{d:02} {h:02}:{min:02}:{s:02}.{micros:06}")
        }
        Value::Time(negative, days, h, m, s, micros) => format!(
            "{}{:02}:{m:02}:{s:02}.{micros:06}",
            if *negative { "-" } else { "" },
            days * 24 + u32::from(*h)
        ),
    };
    Cell {
        text,
        is_null: false,
    }
}

impl QueryOutput {
    pub fn memory_bytes(&self) -> usize {
        std::mem::size_of::<Self>()
            + self.machine_name.capacity()
            + self.host.capacity()
            + self.database.capacity()
            + self
                .sets
                .iter()
                .map(|set| {
                    set.columns.capacity() * std::mem::size_of::<String>()
                        + set.columns.iter().map(String::capacity).sum::<usize>()
                        + set.rows.capacity() * std::mem::size_of::<Vec<Cell>>()
                        + set
                            .rows
                            .iter()
                            .map(|row| {
                                row.capacity() * std::mem::size_of::<Cell>()
                                    + row.iter().map(|cell| cell.text.capacity()).sum::<usize>()
                            })
                            .sum::<usize>()
                })
                .sum::<usize>()
    }
}
