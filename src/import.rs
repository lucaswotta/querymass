use std::collections::HashSet;
use std::io::{BufReader, Read};
use std::path::Path;

use calamine::{Reader, open_workbook_auto};

use crate::database::MachineConnection;

pub struct ImportReport {
    pub machines: Vec<MachineConnection>,
    pub rows_without_host: Vec<usize>,
    pub duplicate_rows: Vec<usize>,
    pub issues: Vec<String>,
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
    let mut sample = Vec::new();
    BufReader::new(file)
        .take(65_536)
        .read_to_end(&mut sample)
        .map_err(|error| format!("Não foi possível ler o CSV: {error}"))?;
    let sample = String::from_utf8_lossy(&sample);
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
    let mut issues = Vec::new();

    for (index, row) in rows.into_iter().enumerate() {
        let row_number = index + 2;
        let mut row = match row {
            Ok(row) => row,
            Err(error) => {
                issues.push(format!("Linha {row_number}: {error}"));
                continue;
            }
        };
        while row.last().is_some_and(|value| value.trim().is_empty()) {
            row.pop();
        }
        if row.is_empty() {
            continue;
        }
        if row.len() > columns.width {
            issues.push(format!(
                "A linha {row_number} tem mais valores do que o cabeçalho."
            ));
            continue;
        }
        row.resize(columns.width, String::new());
        let connection = match columns.machine(&row, row_number) {
            Ok(connection) => connection,
            Err(error) => {
                issues.push(error);
                continue;
            }
        };
        let Some(connection) = connection else {
            rows_without_host.push(row_number);
            continue;
        };
        let key = (
            connection.host.to_ascii_lowercase(),
            connection.port,
            connection.database.clone(),
            connection.username.clone(),
        );
        if !seen.insert(key) {
            duplicate_rows.push(row_number);
            continue;
        }
        machines.push(connection);
    }

    Ok(ImportReport {
        machines,
        rows_without_host,
        duplicate_rows,
        issues,
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
            username: find(&["user", "username", "login", "usuario"]),
            password: find(&["password", "pass", "senha"]),
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
            tls: false,
            ca_path: None,
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn collects_invalid_rows_without_discarding_valid_connections() {
        let report = parse_rows(
            vec!["IP".into(), "PORT".into(), "DATABASE".into()],
            vec![
                Ok(vec!["192.0.2.1".into(), "3306".into(), "one".into()]),
                Ok(vec!["192.0.2.2".into(), "bad".into()]),
                Ok(vec!["192.0.2.1".into(), "3306".into(), "two".into()]),
                Ok(vec!["192.0.2.1".into(), "3306".into(), "one".into()]),
            ],
        )
        .unwrap();
        assert_eq!(report.machines.len(), 2);
        assert_eq!(report.issues.len(), 1);
        assert_eq!(report.duplicate_rows, vec![5]);
    }
    #[test]
    fn preserves_explicit_empty_password_and_normalizes_headers() {
        let report = parse_rows(
            vec![
                "\u{feff}IP".into(),
                "Usuário".into(),
                "Senha".into(),
                "Pasta".into(),
            ],
            vec![Ok(vec![
                "192.0.2.1".into(),
                "own".into(),
                "".into(),
                "Centro".into(),
            ])],
        )
        .unwrap();
        assert_eq!(report.machines[0].username, "own");
        assert!(report.machines[0].password.is_empty());
        assert_eq!(report.machines[0].folder, "Centro");
    }
    #[test]
    fn delimiter_sample_can_end_inside_utf8_character() {
        let path = std::env::temp_dir().join(format!("querymass-csv-{}.csv", std::process::id()));
        let mut contents = "IP;HOSTNAME\n192.0.2.1;".to_owned();
        contents.push_str(&"é".repeat(40000));
        std::fs::write(&path, contents).unwrap();
        let report = read_csv(&path).unwrap();
        assert_eq!(report.machines.len(), 1);
        std::fs::remove_file(path).unwrap();
    }
}
