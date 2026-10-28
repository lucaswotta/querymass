use std::collections::HashSet;
use std::io::{BufReader, Read};
use std::path::Path;

use calamine::{Reader, open_workbook_auto};

use crate::database::MachineConnection;

pub struct ImportReport {
    pub machines: Vec<MachineConnection>,
    pub rows_without_host: Vec<usize>,
    pub duplicate_rows: Vec<usize>,
}

pub fn load_connections(path: &Path) -> Result<ImportReport, String> {
    let extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    match extension.as_str() {
        "csv" | "txt" => read_csv(path),
        "xlsx" | "xlsm" => read_excel(path),
        _ => Err("Formato não suportado. Escolha CSV, XLSX ou XLSM.".to_owned()),
    }
}

fn read_csv(path: &Path) -> Result<ImportReport, String> {
    let mut reader = csv::ReaderBuilder::new()
        .delimiter(detect_delimiter(path)?)
        .flexible(true)
        .from_path(path)
        .map_err(|error| format!("Não foi possível abrir o CSV: {error}"))?;
    let headers = reader
        .headers()
        .map_err(|error| format!("Cabeçalho CSV inválido: {error}"))?
        .iter()
        .map(str::to_owned)
        .collect();
    let rows = reader.records().map(|row| {
        row.map(|record| record.iter().map(str::to_owned).collect())
            .map_err(|error| format!("Linha CSV inválida: {error}"))
    });
    parse_rows(headers, rows)
}

fn detect_delimiter(path: &Path) -> Result<u8, String> {
    let file = std::fs::File::open(path)
        .map_err(|error| format!("Não foi possível ler o CSV: {error}"))?;
    let mut sample = String::new();
    BufReader::new(file)
        .take(65_536)
        .read_to_string(&mut sample)
        .map_err(|error| format!("Não foi possível ler o CSV: {error}"))?;
    let first_line = sample.lines().next().unwrap_or_default();
    let mut counts = [(b',', 0usize), (b';', 0), (b'\t', 0), (b'|', 0)];
    let mut quoted = false;
    let mut chars = first_line.chars().peekable();
    while let Some(character) = chars.next() {
        if character == '"' {
            if quoted && chars.peek() == Some(&'"') {
                chars.next();
            } else {
                quoted = !quoted;
            }
        } else if !quoted
            && let Some((_, count)) = counts
                .iter_mut()
                .find(|(delimiter, _)| *delimiter == character as u8)
        {
            *count += 1;
        }
    }
    Ok(counts
        .into_iter()
        .max_by_key(|(_, count)| *count)
        .filter(|(_, count)| *count > 0)
        .map(|(delimiter, _)| delimiter)
        .unwrap_or(b','))
}

fn read_excel(path: &Path) -> Result<ImportReport, String> {
    let mut workbook = open_workbook_auto(path)
        .map_err(|error| format!("Não foi possível abrir a planilha: {error}"))?;
    let range = workbook
        .worksheet_range_at(0)
        .ok_or_else(|| "A planilha não contém abas.".to_owned())?
        .map_err(|error| format!("Não foi possível ler a primeira aba: {error}"))?;
    let mut rows = range.rows();
    let headers = rows
        .next()
        .ok_or_else(|| "A planilha está vazia.".to_owned())?
        .iter()
        .map(ToString::to_string)
        .collect();
    parse_rows(
        headers,
        rows.map(|row| Ok(row.iter().map(ToString::to_string).collect())),
    )
}

fn parse_rows<I>(headers: Vec<String>, rows: I) -> Result<ImportReport, String>
where
    I: IntoIterator<Item = Result<Vec<String>, String>>,
{
    let columns = HeaderMap::from_headers(&headers)?;
    let mut machines = Vec::new();
    let mut seen = HashSet::new();
    let mut rows_without_host = Vec::new();
    let mut duplicate_rows = Vec::new();

    for (index, row) in rows.into_iter().enumerate() {
        let row_number = index + 2;
        let mut row = row?;
        while row.last().is_some_and(|value| value.trim().is_empty()) {
            row.pop();
        }
        if row.is_empty() {
            continue;
        }
        if row.len() > columns.width {
            return Err(format!(
                "A linha {row_number} tem mais valores do que o cabeçalho."
            ));
        }
        row.resize(columns.width, String::new());
        let Some(connection) = columns.machine(&row, row_number)? else {
            rows_without_host.push(row_number);
            continue;
        };
        let key = (connection.host.to_ascii_lowercase(), connection.port);
        if !seen.insert(key) {
            duplicate_rows.push(row_number);
            continue;
        }
        machines.push(connection);
    }

    if machines.is_empty() {
        return Err("Nenhuma máquina com IP válido foi encontrada.".to_owned());
    }
    Ok(ImportReport {
        machines,
        rows_without_host,
        duplicate_rows,
    })
}

struct HeaderMap {
    width: usize,
    host: usize,
    name: Option<usize>,
    folder: Option<usize>,
    port: Option<usize>,
    database: Option<usize>,
    username: Option<usize>,
    password: Option<usize>,
}

impl HeaderMap {
    fn from_headers(headers: &[String]) -> Result<Self, String> {
        let mut normalized: Vec<String> = headers.iter().map(|header| normalize(header)).collect();
        while normalized.last().is_some_and(String::is_empty) {
            normalized.pop();
        }
        if normalized.is_empty() {
            return Err("O arquivo não tem cabeçalho.".to_owned());
        }
        let mut seen = HashSet::new();
        for header in &normalized {
            if !header.is_empty() && !seen.insert(header) {
                return Err(format!("O cabeçalho contém a coluna duplicada {header}."));
            }
        }
        let find = |aliases: &[&str]| {
            aliases
                .iter()
                .find_map(|alias| normalized.iter().position(|header| header == alias))
        };
        Ok(Self {
            width: normalized.len(),
            host: find(&[
                "ip",
                "host",
                "hostname",
                "address",
                "ipaddress",
                "endpoint",
                "target",
            ])
            .ok_or_else(|| "Coluna obrigatória ausente: IP, HOST ou HOSTNAME.".to_owned())?,
            name: find(&["hostname", "name", "nome", "machine"]),
            folder: find(&["location", "local", "folder", "pasta", "group", "grupo"]),
            port: find(&["port", "porta", "mysqlport"]),
            database: find(&["database", "db", "banco", "base"]),
            username: find(&["user", "username", "login", "usuario", "sshuser"]),
            password: find(&["password", "pass", "senha", "sshpassword"]),
        })
    }

    fn machine(
        &self,
        row: &[String],
        row_number: usize,
    ) -> Result<Option<MachineConnection>, String> {
        let value = |index: usize| row.get(index).map(String::as_str).unwrap_or_default();
        let host = value(self.host).trim().to_owned();
        if host.is_empty() || matches!(host.to_ascii_lowercase().as_str(), "nan" | "none") {
            return Ok(None);
        }
        let port = match self
            .port
            .map(value)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            Some(port) => port.parse::<u16>().map_err(|_| {
                format!(
                    "A linha {row_number} tem uma porta inválida; use um valor entre 1 e 65535."
                )
            })?,
            None => 3306,
        };
        if port == 0 {
            return Err(format!(
                "A linha {row_number} tem uma porta inválida; use um valor entre 1 e 65535."
            ));
        }
        let name = self
            .name
            .map(value)
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .unwrap_or(&host)
            .to_owned();
        Ok(Some(MachineConnection {
            name,
            host,
            folder: self.folder.map(value).unwrap_or_default().trim().to_owned(),
            port,
            database: self
                .database
                .map(value)
                .unwrap_or_default()
                .trim()
                .to_owned(),
            username: self
                .username
                .map(value)
                .unwrap_or_default()
                .trim()
                .to_owned(),
            password: self.password.map(value).unwrap_or_default().to_owned(),
        }))
    }
}

fn normalize(value: &str) -> String {
    value
        .trim_matches(['\u{feff}', ' ', '\t'])
        .to_lowercase()
        .replace(['á', 'à', 'ã', 'â'], "a")
        .replace(['é', 'ê'], "e")
        .replace(['í'], "i")
        .replace(['ó', 'ô', 'õ'], "o")
        .replace(['ú'], "u")
        .replace(['ç'], "c")
        .replace([' ', '-', '_'], "")
}
