use crate::database::{self, ActiveQueries, ExecutionOptions, MachineConnection, QueryOutput};
use std::collections::VecDeque;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
    mpsc,
};
use std::thread;

#[derive(Debug)]
pub enum Event {
    Connecting(u64),
    Executing(u64),
    Finished(QueryOutput),
    Skipped(u64),
    CancellationErrors(Vec<String>),
}
pub struct Run {
    pub receiver: mpsc::Receiver<Event>,
    pub cancelled: Arc<AtomicBool>,
    active: ActiveQueries,
    sender: mpsc::SyncSender<Event>,
}
impl Run {
    pub fn start(
        targets: Vec<(u64, MachineConnection)>,
        sql: String,
        settings: ExecutionOptions,
        concurrency: usize,
    ) -> Self {
        let count = targets.len();
        let jobs = Arc::new(Mutex::new(VecDeque::from(targets)));
        let active: ActiveQueries = Arc::new(Mutex::new(Vec::new()));
        let cancelled = Arc::new(AtomicBool::new(false));
        let (sender, receiver) = mpsc::sync_channel(32);
        for _ in 0..concurrency.clamp(1, 16).min(count) {
            let (jobs, active, cancelled, sender, sql, settings) = (
                jobs.clone(),
                active.clone(),
                cancelled.clone(),
                sender.clone(),
                sql.clone(),
                settings.clone(),
            );
            thread::spawn(move || {
                loop {
                    let job = match jobs.lock() {
                        Ok(mut queue) => queue.pop_front(),
                        Err(_) => None,
                    };
                    let Some((id, machine)) = job else {
                        break;
                    };
                    if cancelled.load(Ordering::SeqCst) {
                        if sender.send(Event::Skipped(id)).is_err() {
                            break;
                        }
                        continue;
                    }
                    if sender.send(Event::Connecting(id)).is_err() {
                        break;
                    }
                    let output = database::execute(
                        id,
                        machine,
                        &sql,
                        &settings,
                        &cancelled,
                        &active,
                        || {
                            let _ = sender.send(Event::Executing(id));
                        },
                    );
                    if sender.send(Event::Finished(output)).is_err() {
                        break;
                    }
                }
            });
        }
        Self {
            receiver,
            cancelled,
            active,
            sender,
        }
    }
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
        let active = self.active.clone();
        let sender = self.sender.clone();
        thread::spawn(move || {
            let errors = database::cancel_active(&active);
            if !errors.is_empty() {
                let _ = sender.send(Event::CancellationErrors(errors));
            }
        });
    }
}
impl Drop for Run {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::SeqCst);
    }
}
