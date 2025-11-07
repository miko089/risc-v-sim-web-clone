use std::path::PathBuf;
use std::sync::Arc;

use bytes::Bytes;
use tokio::sync::mpsc::Receiver;
use tracing::{Instrument, debug, info, info_span};
use ulid::{ULID_LEN, Ulid};

pub struct SubmissionTask {
    pub source_code: Bytes,
    pub ticks: u32,
    pub ulid: Ulid,
}

pub struct Config {
    pub as_binary: PathBuf,
    pub ld_binary: PathBuf,
    pub simulator_binary: PathBuf,
    pub submissions_folder: PathBuf,
}

pub async fn run_submission_actor(config: Arc<Config>, mut tasks: Receiver<SubmissionTask>) {
    while let Some(task) = tasks.recv().await {
        let ulid = task.ulid;
        debug!("Received task {ulid}");
        tokio::spawn(
            submission_task(config.clone(), task)
                .instrument(info_span!("submission_task", ulid=%ulid)),
        );
    }
}

async fn submission_task(config: Arc<Config>, task: SubmissionTask) {
    info!("Tasks are not implemented");
}

pub fn submission_dir(config: &Config, ulid: Ulid) -> PathBuf {
    let mut buf = [0u8; ULID_LEN];
    let ulid_str = ulid.array_to_str(&mut buf);
    config.submissions_folder.join(&ulid_str)
}

pub fn submission_file(config: &Config, ulid: Ulid) -> PathBuf {
    let mut buf = [0u8; ULID_LEN];
    let ulid_str = ulid.array_to_str(&mut buf);
    let mut path = config.submissions_folder.clone();
    path.extend([&ulid_str, "simulation.json"]);

    path
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_path_utils() {
        let config = Config {
            as_binary: "dummy".into(),
            ld_binary: "dummy".into(),
            simulator_binary: "dummy".into(),
            submissions_folder: "submissions".into(),
        };
        for _ in 0..10 {
            let ulid = Ulid::new();
            let dir = submission_dir(&config, ulid);
            let file = submission_file(&config, ulid);
            assert!(file.starts_with(dir));
        }
    }
}
