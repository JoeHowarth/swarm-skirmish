use std::{
    fs::File,
    io::{self, BufRead, BufReader, Seek, SeekFrom},
    path::PathBuf,
    sync::Arc,
    time::Duration,
};

use async_channel::Sender;
use dashmap::DashMap;
use serde_json::from_str;
use swarm_lib::JournalEntry;
use tracing::{debug, error, info, trace, warn};

pub fn journal_streamer() -> (
    async_channel::Receiver<JournalEntry>,
    Arc<DashMap<u32, Vec<JournalEntry>>>,
) {
    let journal_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("journal.json");

    info!("Starting journal streamer with file: {:?}", journal_dir);
    let (tx, rx) = async_channel::unbounded();
    let bot_id_to_journals = Arc::new(DashMap::new());

    tokio::spawn(streamer(journal_dir, tx, bot_id_to_journals.clone()));
    (rx, bot_id_to_journals)
}

async fn streamer(
    journal_path: PathBuf,
    tx: Sender<JournalEntry>,
    bot_id_to_journals: Arc<DashMap<u32, Vec<JournalEntry>>>,
) {
    // Track file position
    let mut position: u64 = 0;

    // Polling interval
    let interval = Duration::from_millis(100);

    info!("Journal streamer started, watching: {:?}", journal_path);
    loop {
        // Check if file exists
        if !journal_path.exists() {
            debug!("Journal file not found at {:?}, waiting...", journal_path);
            tokio::time::sleep(interval).await;
            continue;
        }

        // Process any new entries
        match process_new_entries(
            &journal_path,
            &tx,
            &mut position,
            &bot_id_to_journals,
        )
        .await
        {
            Ok(_) => {
                trace!(
                    "Successfully processed journal entries, position: {}",
                    position
                );
            }
            Err(e) => {
                error!("Error processing journal: {:?}", e);
            }
        }

        // Wait before checking again
        tokio::time::sleep(interval).await;
    }
}

async fn process_new_entries(
    journal_path: &PathBuf,
    tx: &Sender<JournalEntry>,
    position: &mut u64,
    bot_id_to_journals: &DashMap<u32, Vec<JournalEntry>>,
) -> io::Result<()> {
    // Open the file and seek to the last position
    let mut file = File::open(journal_path)?;
    let file_size = file.metadata()?.len();

    // Nothing new to read
    if file_size <= *position {
        trace!("No new journal entries to read");
        return Ok(());
    }

    // File was truncated or replaced
    if file_size < *position {
        warn!("Journal file was truncated or replaced, resetting position");
        *position = 0;
    }

    file.seek(SeekFrom::Start(*position))?;
    let mut reader = BufReader::new(file);

    // Read new lines
    let mut line = String::new();
    let mut entries_processed = 0;
    while reader.read_line(&mut line)? > 0 {
        let line_trimmed = line.trim();
        if !line_trimmed.is_empty() {
            match from_str::<JournalEntry>(line_trimmed) {
                Ok(entry) => {
                    if let Some(bot_id) = entry.bot_id {
                        bot_id_to_journals
                            .entry(bot_id)
                            .or_default()
                            .push(entry.clone());
                    }

                    if let Err(e) = tx.try_send(entry) {
                        error!("Failed to send journal entry: {}", e);
                    } else {
                        entries_processed += 1;
                    }
                }
                Err(e) => {
                    error!("Failed to parse journal entry: {}", e);
                }
            }
        }

        // Update position and clear line for next iteration
        *position = reader.stream_position()?;
        line.clear();
    }

    debug!("Processed {} new journal entries", entries_processed);
    Ok(())
}
