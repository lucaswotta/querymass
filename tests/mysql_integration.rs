use querymass::{
    database::{self, ExecutionOptions, MachineConnection, Outcome},
    engine::{Event, Run},
};
use std::{
    sync::{Arc, Mutex, atomic::AtomicBool},
    time::Duration,
};
fn machine() -> MachineConnection {
    MachineConnection {
        name: "integration-test".into(),
        host: std::env::var("QM_TEST_HOST").expect("QM_TEST_HOST"),
        port: 3306,
        database: std::env::var("QM_TEST_DATABASE").expect("QM_TEST_DATABASE"),
        username: std::env::var("QM_TEST_USER").expect("QM_TEST_USER"),
        password: std::env::var("QM_TEST_PASSWORD").expect("QM_TEST_PASSWORD"),
        folder: String::new(),
        tls: false,
        ca_path: None,
    }
}
fn execute(sql: &str, read_only: bool) -> database::QueryOutput {
    database::execute(
        1,
        machine(),
        sql,
        &ExecutionOptions {
            read_only,
            ..Default::default()
        },
        &AtomicBool::new(false),
        &Arc::new(Mutex::new(Vec::new())),
        || {},
    )
}
#[test]
#[ignore = "requires an explicitly authorized MySQL test host"]
fn mysql_multi_results_affected_rows_and_temporary_writes() {
    println!("{}", database::test_connection(&machine()).unwrap());
    let output = execute(
        "CREATE TEMPORARY TABLE querymass_integration (id INT PRIMARY KEY, label VARCHAR(40)); INSERT INTO querymass_integration VALUES (1,'one'),(2,'two'); UPDATE querymass_integration SET label='changed' WHERE id=1; SELECT * FROM querymass_integration ORDER BY id; DELETE FROM querymass_integration WHERE id=2; SELECT COUNT(*) AS remaining FROM querymass_integration",
        false,
    );
    assert_eq!(output.outcome, Outcome::Success, "{:?}", output.error);
    assert_eq!(output.sets.len(), 6);
    assert_eq!(output.sets[1].affected_rows, 2);
    assert_eq!(output.sets[2].affected_rows, 1);
    assert_eq!(output.sets[3].rows.len(), 2);
    assert_eq!(output.sets[4].affected_rows, 1);
    assert_eq!(output.sets[5].rows[0][0].text, "1");
}
#[test]
#[ignore = "requires an explicitly authorized MySQL test host"]
fn mysql_reports_error_after_successful_result() {
    let output = execute(
        "SELECT 42 AS answer; SELECT * FROM querymass_nonexistent_integration_table",
        true,
    );
    assert_eq!(output.outcome, Outcome::Partial);
    assert!(output.error.is_some());
    assert_eq!(output.sets[0].rows[0][0].text, "42");
}
#[test]
#[ignore = "requires an explicitly authorized MySQL test host"]
fn mysql_readonly_values_and_row_budget() {
    let output = execute(
        "SELECT NULL AS null_value, 'NULL' AS text_value, UNHEX('FF00') AS binary_value; SELECT 2 AS b",
        true,
    );
    assert_eq!(output.outcome, Outcome::Success, "{:?}", output.error);
    assert_eq!(output.sets.len(), 2);
    assert!(output.sets[0].rows[0][0].is_null);
    assert!(!output.sets[0].rows[0][1].is_null);
    assert_eq!(output.sets[0].rows[0][2].text, "0xFF00");
    let settings = ExecutionOptions {
        row_limit: 1,
        ..Default::default()
    };
    let output = database::execute(
        1,
        machine(),
        "SELECT 1 UNION ALL SELECT 2",
        &settings,
        &AtomicBool::new(false),
        &Arc::new(Mutex::new(Vec::new())),
        || {},
    );
    assert_eq!(output.sets[0].rows.len(), 1);
    assert!(output.sets[0].truncated);
}
#[test]
#[ignore = "requires an explicitly authorized MySQL test host"]
fn mysql_cancel_active_and_pending_jobs() {
    let run = Run::start(
        vec![(1, machine()), (2, machine())],
        "SELECT SLEEP(8)".into(),
        ExecutionOptions::default(),
        1,
    );
    loop {
        if matches!(
            run.receiver.recv_timeout(Duration::from_secs(10)).unwrap(),
            Event::Executing(1)
        ) {
            break;
        }
    }
    std::thread::sleep(Duration::from_millis(150));
    run.cancel();
    let mut cancelled = false;
    let mut skipped = false;
    for _ in 0..8 {
        match run.receiver.recv_timeout(Duration::from_secs(4)).unwrap() {
            Event::Finished(output) => {
                assert_eq!(output.outcome, Outcome::Cancelled, "{:?}", output.error);
                cancelled = true;
            }
            Event::Skipped(2) => skipped = true,
            _ => (),
        }
        if cancelled && skipped {
            break;
        }
    }
    assert!(cancelled && skipped);
}

#[test]
#[ignore = "requires an explicitly authorized MySQL test host"]
fn mysql_deadline_interrupts_read_and_reports_unknown_write() {
    for read_only in [true, false] {
        let settings = ExecutionOptions {
            read_only,
            deadline: Duration::from_millis(400),
            ..Default::default()
        };
        let output = database::execute(
            1,
            machine(),
            "SELECT SLEEP(8)",
            &settings,
            &AtomicBool::new(false),
            &Arc::new(Mutex::new(Vec::new())),
            || {},
        );
        assert_eq!(
            output.outcome,
            if read_only {
                Outcome::TimedOut
            } else {
                Outcome::Unknown
            },
            "{:?}",
            output.error
        );
        assert!(output.elapsed < Duration::from_secs(5));
    }
}
#[test]
#[ignore = "requires an explicitly authorized MySQL test host"]
fn mysql_rejects_untrusted_tls_before_sending_sql() {
    let mut connection = machine();
    connection.tls = true;
    connection.ca_path = Some(std::path::PathBuf::from(
        "/querymass/nonexistent-certificate.pem",
    ));
    let output = database::execute(
        1,
        connection,
        "SELECT 1",
        &ExecutionOptions::default(),
        &AtomicBool::new(false),
        &Arc::new(Mutex::new(Vec::new())),
        || {},
    );
    assert_eq!(output.outcome, Outcome::Failed);
    assert!(output.sets.is_empty());
    assert!(output.error.is_some());
}
