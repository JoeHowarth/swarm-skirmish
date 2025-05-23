use std::{
    collections::HashMap,
    fs::{File, OpenOptions},
    io::Write,
    path::PathBuf,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};

use crate::{BotUpdate, CellKind, RadarData, Team};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LogLevel {
    Debug,
    Info,
    Warn,
    Error,
}

impl LogLevel {
    fn as_str(&self) -> &'static str {
        match self {
            LogLevel::Debug => "DEBUG",
            LogLevel::Info => "INFO",
            LogLevel::Warn => "WARN",
            LogLevel::Error => "ERROR",
        }
    }
}

/// A log entry from a bot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub bot_id: u32,
    pub tick: u32,
    pub level: LogLevel,
    pub message: String,
    pub attrs: Option<HashMap<String, String>>,
}

/// Handles logging for a specific bot
pub struct BotLogger {
    pub bot_id: u32,
    pub current_tick: u32,
    log_file: Option<File>,
    buffer: Vec<LogEntry>,
}

impl BotLogger {
    /// Create a new logger for a specific bot
    pub fn new(bot_id: u32) -> Self {
        // Create log directory if it doesn't exist
        std::fs::create_dir_all("logs").unwrap_or_else(|e| {
            eprintln!("Warning: Failed to create logs directory: {}", e);
        });

        // Try to open a log file for this bot
        let log_path = PathBuf::from(format!("logs/bot_{}.log", bot_id));
        let log_file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&log_path)
            .ok();

        if log_file.is_none() {
            eprintln!("Warning: Failed to open log file for bot {}", bot_id);
        }

        BotLogger {
            bot_id,
            current_tick: 0,
            log_file,
            buffer: Vec::new(),
        }
    }

    /// Log a message at the specified level
    pub fn log(&mut self, level: LogLevel, message: impl Into<String>) {
        let entry = LogEntry {
            bot_id: self.bot_id,
            tick: self.current_tick,
            level,
            message: message.into(),
            attrs: None,
        };

        self.buffer.push(entry.clone());
        self.write_to_file(&entry);
    }

    /// Log a message with additional attributes
    pub fn log_with_attrs(
        &mut self,
        level: LogLevel,
        message: impl Into<String>,
        attrs: HashMap<String, String>,
    ) {
        let entry = LogEntry {
            bot_id: self.bot_id,
            tick: self.current_tick,
            level,
            message: message.into(),
            attrs: Some(attrs),
        };

        self.buffer.push(entry.clone());
        self.write_to_file(&entry);
    }

    /// Convenience method for debug-level logging
    pub fn debug(&mut self, message: impl Into<String>) {
        self.log(LogLevel::Debug, message);
    }

    /// Convenience method for info-level logging
    pub fn info(&mut self, message: impl Into<String>) {
        self.log(LogLevel::Info, message);
    }

    /// Convenience method for warning-level logging
    pub fn warn(&mut self, message: impl Into<String>) {
        self.log(LogLevel::Warn, message);
    }

    /// Convenience method for error-level logging
    pub fn error(&mut self, message: impl Into<String>) {
        self.log(LogLevel::Error, message);
    }

    /// Update the current tick number
    pub fn set_tick(&mut self, tick: u32) {
        if tick != self.current_tick {
            // Print buffered logs with a header when moving to a new tick
            if !self.buffer.is_empty() {
                self.flush_buffer_to_stdout();
            }
            self.current_tick = tick;
        }
    }

    /// Write a log entry to the bot's log file
    fn write_to_file(&mut self, entry: &LogEntry) {
        if let Some(file) = &mut self.log_file {
            let timestamp = format_system_time(SystemTime::now());
            let mut line = format!(
                "[{}] [Tick {}] [{}] {}\n",
                timestamp,
                entry.tick,
                entry.level.as_str(),
                entry.message
            );

            // Add attributes if present
            if let Some(attrs) = &entry.attrs {
                for (key, value) in attrs {
                    line.push_str(&format!("  {} = {}\n", key, value));
                }
            }

            if let Err(e) = file.write_all(line.as_bytes()) {
                eprintln!(
                    "Failed to write to log file for bot {}: {}",
                    self.bot_id, e
                );
            }
        }
    }

    /// Flush all buffered logs to stdout with appropriate headers
    pub fn flush_buffer_to_stdout(&mut self) -> Vec<LogEntry> {
        if self.buffer.is_empty() {
            return Vec::new();
        }

        // Build the entire output as a single string to write at once
        let mut output = format!(
            "\n===== Bot {} - Tick {} =====\n",
            self.bot_id, self.current_tick
        );

        // Add all buffered messages
        for entry in &self.buffer {
            let prefix = match entry.level {
                LogLevel::Debug => "\x1b[36mDEBUG\x1b[0m", // Cyan
                LogLevel::Info => "\x1b[32mINFO\x1b[0m",   // Green
                LogLevel::Warn => "\x1b[33mWARN\x1b[0m",   // Yellow
                LogLevel::Error => "\x1b[31mERROR\x1b[0m", // Red
            };

            output.push_str(&format!("[{}] {}\n", prefix, entry.message));

            // Add attributes if present
            if let Some(attrs) = &entry.attrs {
                for (key, value) in attrs {
                    output.push_str(&format!("  {} = {}\n", key, value));
                }
            }
        }

        output.push_str("=========================\n");

        // Use a mutex to lock stdout and prevent interleaved output from
        // multiple bots
        use std::io::{self, Write};
        let stdout = io::stdout();
        let mut handle = stdout.lock();

        // Write the entire output at once while holding the lock
        let _ = handle.write_all(output.as_bytes());
        let _ = handle.flush();

        // Clear the buffer

        std::mem::take(&mut self.buffer)
    }

    pub fn log_debug_info(
        &mut self,
        update: &BotUpdate,
        log_every_x_ticks: u32,
    ) {
        self.debug(format!("Processing tick {}", update.tick));

        if update.tick % log_every_x_ticks == 0 {
            // Format items as a readable list
            let items_str = if update.bot_data.inventory.is_empty() {
                "None".to_string()
            } else {
                update
                    .bot_data
                    .inventory
                    .iter()
                    .map(|(item, count)| format!("{}: {}", item, count))
                    .collect::<Vec<_>>()
                    .join(", ")
            };

            self.debug(format!(
                "Bot Status Report [Tick {}]:\n{}\nEnergy: {}\nItems: \
                 {}\nTeam: {:?}",
                update.tick,
                update.bot_data.pos,
                update.bot_data.energy,
                items_str,
                update.bot_data.team
            ));

            // The print_radar method now logs internally
            // print_radar(&update.radar);
        }
    }
}

/// Format a SystemTime into a string with format "YYYY-MM-DD HH:MM:SS.mmm"
fn format_system_time(time: SystemTime) -> String {
    // Convert to duration since UNIX_EPOCH
    let duration = time
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::from_secs(0));

    // Extract seconds and calculate date/time components
    let secs = duration.as_secs();
    let millis = duration.subsec_millis();

    // Convert to seconds, minutes, hours
    let secs_of_day = secs % 86400;
    let hours = secs_of_day / 3600;
    let minutes = (secs_of_day % 3600) / 60;
    let seconds = secs_of_day % 60;

    // Calculate days since epoch
    let days_since_epoch = secs / 86400;

    // Simple algorithm to calculate year, month, day
    // This is a basic implementation that doesn't account for leap seconds
    // and uses a simplified leap year calculation
    let mut year = 1970;
    let mut days_remaining = days_since_epoch;

    // Advance years
    loop {
        let days_in_year = if is_leap_year(year) { 366 } else { 365 };
        if days_remaining >= days_in_year {
            days_remaining -= days_in_year;
            year += 1;
        } else {
            break;
        }
    }

    // Calculate month and day
    let days_in_month = [
        31,
        if is_leap_year(year) { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];

    let mut month = 0;
    while month < 12 && days_remaining >= days_in_month[month] {
        days_remaining -= days_in_month[month];
        month += 1;
    }

    // days_remaining is now the day of the month (0-based)
    let day = days_remaining + 1;

    // Format the date and time
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02}.{:03}",
        year,
        month + 1, // Convert to 1-based month
        day,
        hours,
        minutes,
        seconds,
        millis
    )
}

/// Check if a year is a leap year
fn is_leap_year(year: u64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}

/// Formats radar data as a string instead of printing directly
pub fn format_radar(radar: &RadarData) -> String {
    let mut output = String::new();

    // Determine the bounds of the radar view for display purposes
    let (min_x, max_x, min_y, max_y) = if !radar.cells.is_empty() {
        let (min_x, min_y) = radar.cells.iter().fold(
            (isize::MAX, isize::MAX),
            |(min_x, min_y), cell| {
                let (world_x, world_y) = cell.pos.as_isize();
                (min_x.min(world_x), min_y.min(world_y))
            },
        );

        let (max_x, max_y) = radar.cells.iter().fold(
            (isize::MIN, isize::MIN),
            |(max_x, max_y), cell| {
                let (world_x, world_y) = cell.pos.as_isize();
                (max_x.max(world_x), max_y.max(world_y))
            },
        );

        (min_x, max_x, min_y, max_y)
    } else {
        // If no cells, just show a small area around the center
        let (center_x, center_y) = radar.center_world_pos.as_isize();
        (center_x - 2, center_x + 2, center_y - 2, center_y + 2)
    };

    let width = (max_x - min_x + 1) as usize;
    let height = (max_y - min_y + 1) as usize;

    // Create a 2D grid for display purposes
    let mut display_grid = vec![vec![['.', ' ']; width]; height];

    // Fill the grid with cell representations
    for cell in &radar.cells {
        let (world_x, world_y) = cell.pos.as_isize();
        let grid_x = (world_x - min_x) as usize;
        let grid_y = (max_y - world_y) as usize; // Flip Y for display

        let cell_repr: [char; 2] = match cell.kind {
            CellKind::Unknown => ['.', ' '],
            CellKind::Empty => {
                if let Some(pawn_idx) = cell.pawn {
                    let bot = &radar.pawns[pawn_idx];
                    match bot.team {
                        Team::Player => ['P', ' '],
                        Team::Enemy => ['E', ' '],
                    }
                } else if let Some(item) = cell.item {
                    match item {
                        crate::Item::Crumb => ['C', ' '],   // Crumb
                        crate::Item::Fent => ['F', ' '],    // Fent
                        crate::Item::Truffle => ['T', ' '], // Truffle
                        crate::Item::Metal => ['M', ' '],   // Metal
                    }
                } else {
                    [' ', ' '] // Empty space
                }
            }
            CellKind::Blocked => ['[', ']'],
        };

        // Write the two characters to the grid
        if grid_y < height && grid_x < width {
            if let Some(row) = display_grid.get_mut(grid_y) {
                let idx = grid_x;
                if idx < row.len() {
                    row[idx] = cell_repr;
                }
            }
        }
    }

    // Mark the bot's position with a special character
    let (center_x, center_y) = radar.center_world_pos.as_isize();
    let grid_center_x = (center_x - min_x) as usize;
    let grid_center_y = (max_y - center_y) as usize;

    if grid_center_y < height && grid_center_x < width {
        if let Some(row) = display_grid.get_mut(grid_center_y) {
            let idx = grid_center_x;
            if idx < row.len() {
                // Always mark the center position with '@'
                row[idx] = ['@', ' '];
            }
        }
    }

    // Now render the grid
    // Top border
    output.push_str(&format!("┌{}┐\n", "─".repeat(width * 2)));

    // Radar grid
    for y in 0..height {
        output.push('│');
        for x in 0..width {
            let cell_chars = display_grid[y][x];
            output.push(cell_chars[0]);
            output.push(cell_chars[1]);
        }
        output.push_str("│\n");
    }

    // Bottom border
    output.push_str(&format!("└{}┘\n", "─".repeat(width * 2)));

    // Add world coordinate labels
    output.push_str(&format!(
        "World coordinates: ({}, {}) to ({}, {})\n",
        min_x, min_y, max_x, max_y
    ));

    // Bot information
    if !radar.pawns.is_empty() {
        output.push_str("\nBots detected:\n");
        for (i, bot) in radar.pawns.iter().enumerate() {
            output
                .push_str(&format!("  {}: {:?} at {}\n", i, bot.team, bot.pos));
        }
    }

    output
}

/// Prints a visual representation of radar data to the terminal
pub fn print_radar(radar: &RadarData) {
    print!("{}", format_radar(radar));
}
