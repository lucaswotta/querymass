use std::time::Duration;

use mysql::{Conn, OptsBuilder, Row, Value, prelude::Queryable};

#[derive(Clone)]
pub struct MachineConnection {
    pub name: String,
    pub host: String,
    pub folder: String,
    pub port: u16,
    pub database: String,
    pub username: String,
    pub password: String,
}

pub struct QueryOutput {
    pub machine_name: String,
    pub host: String,
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub affected_rows: u64,
    pub truncated: bool,
    pub error: Option<String>,
}

struct QueryData {
    columns: Vec<String>,
    rows: Vec<Vec<String>>,
    affected_rows: u64,
    truncated: bool,
}

pub fn execute(machine: MachineConnection, query: String) -> QueryOutput {
    let result = execute_inner(&machine, &query);
    match result {
        Ok(data) => QueryOutput {
            machine_name: machine.name,
            host: machine.host,
            columns: data.columns,
            rows: data.rows,
            affected_rows: data.affected_rows,
            truncated: data.truncated,
            error: None,
        },
        Err(error) => QueryOutput {
            machine_name: machine.name,
            host: machine.host,
            columns: Vec::new(),
            rows: Vec::new(),
            affected_rows: 0,
            truncated: false,
            error: Some(error),
        },
    }
}

fn execute_inner(machine: &MachineConnection, query: &str) -> Result<QueryData, String> {
    if machine.username.trim().is_empty() {
        return Err("Informe o usuário MySQL nas credenciais padrão ou na máquina.".to_owned());
    }
    let mut options = OptsBuilder::default()
        .ip_or_hostname(Some(machine.host.clone()))
        .tcp_port(machine.port)
        .user(Some(machine.username.clone()))
        .pass(Some(machine.password.clone()))
        .tcp_connect_timeout(Some(Duration::from_secs(5)))
        .read_timeout(Some(Duration::from_secs(30)))
        .write_timeout(Some(Duration::from_secs(30)));
    if !machine.database.trim().is_empty() {
        options = options.db_name(Some(machine.database.clone()));
    }
    let mut connection = Conn::new(options).map_err(|error| error.to_string())?;
    let mut result = connection
        .query_iter(query)
        .map_err(|error| error.to_string())?;
    let columns = result
        .columns()
        .as_ref()
        .iter()
        .map(|column| column.name_str().to_string())
        .collect();
    let mut rows = Vec::new();
    let mut truncated = false;
    for row in &mut result {
        let row = row.map_err(|error| error.to_string())?;
        if rows.len() < 1000 {
            rows.push(format_row(row));
        } else {
            truncated = true;
        }
    }
    let affected_rows = result.affected_rows();
    Ok(QueryData {
        columns,
        rows,
        affected_rows,
        truncated,
    })
}

fn format_row(row: Row) -> Vec<String> {
    (0..row.len())
        .map(|index| match row.as_ref(index) {
            Some(value) => format_value(value),
            None => String::new(),
        })
        .collect()
}

fn format_value(value: &Value) -> String {
    match value {
        Value::NULL => "NULL".to_owned(),
        Value::Bytes(bytes) => String::from_utf8_lossy(bytes).into_owned(),
        Value::Int(value) => value.to_string(),
        Value::UInt(value) => value.to_string(),
        Value::Float(value) => value.to_string(),
        Value::Double(value) => value.to_string(),
        Value::Date(year, month, day, hour, minute, second, micros) => {
            format!("{year:04}-{month:02}-{day:02} {hour:02}:{minute:02}:{second:02}.{micros:06}")
        }
        Value::Time(negative, days, hours, minutes, seconds, micros) => {
            let sign = if *negative { "-" } else { "" };
            let total_hours = days * 24 + u32::from(*hours);
            format!("{sign}{total_hours:02}:{minutes:02}:{seconds:02}.{micros:06}")
        }
    }
}
