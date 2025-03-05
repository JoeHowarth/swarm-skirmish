use std::{
    collections::HashMap,
    fs::{File, OpenOptions},
    io::{BufReader, BufWriter, Write},
    net::TcpStream,
    path::PathBuf,
    process::exit,
    sync::mpsc::{self, Receiver, Sender},
    thread::sleep,
    time::Duration,
};

use chrono::Local;
use eyre::Result;

use crate::{
    protocol::Protocol,
    BotMsgEnvelope,
    BotResponse,
    CellKind,
    ClientMsg,
    RadarData,
    ServerMsg,
    ServerUpdate,
    ServerUpdateEnvelope,
    Team,
};

/// Log level for bot logs
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
#[derive(Debug, Clone)]
pub struct LogEntry {
    pub bot_id: u32,
    pub tick: u32,
    pub level: LogLevel,
    pub message: String,
    pub attrs: Option<HashMap<String, String>>,
}

/// Handles logging for a specific bot
pub struct BotLogger {
    bot_id: u32,
    current_tick: u32,
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
            let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
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
    fn flush_buffer_to_stdout(&mut self) {
        if self.buffer.is_empty() {
            return;
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
        self.buffer.clear();
    }
}

pub struct Ctx {
    pub bot_id: u32,
    pub last_received_tick: u32,
    pub resp_rx: Receiver<ServerUpdateEnvelope>,
    pub bot_msg_tx: Sender<BotMsgEnvelope>,
    pub logger: BotLogger,
}

impl Ctx {
    pub fn new(
        bot_id: u32,
        resp_rx: Receiver<ServerUpdateEnvelope>,
        bot_msg_tx: Sender<BotMsgEnvelope>,
    ) -> Self {
        Ctx {
            bot_id,
            last_received_tick: 0,
            resp_rx,
            bot_msg_tx,
            logger: BotLogger::new(bot_id),
        }
    }

    pub fn wait_for_latest_update(&mut self) -> ServerUpdate {
        let mut update = self
            .resp_rx
            .recv()
            .expect("Failed to receive server update");

        // drain the channel
        while let Ok(new_update) = self.resp_rx.try_recv() {
            update = new_update;
        }

        // Update the logger's tick
        self.logger.set_tick(update.response.tick);
        self.last_received_tick = update.response.tick;

        update.response
    }

    pub fn wait_for_update(&mut self) -> ServerUpdate {
        let update = self
            .resp_rx
            .recv()
            .expect("Failed to receive server update");

        // Update the logger's tick
        self.logger.set_tick(update.response.tick);
        self.last_received_tick = update.response.tick;

        update.response
    }

    pub fn send_msg(&mut self, bot_msg: BotResponse) {
        self.debug(format!("Actions: {:?}", &bot_msg.actions));

        let envelope = BotMsgEnvelope {
            bot_id: self.bot_id,
            tick: self.last_received_tick,
            msg: bot_msg,
        };

        self.bot_msg_tx
            .send(envelope)
            .expect("Failed to send bot message");
    }

    /// Log a message at info level
    pub fn logln(&mut self, message: impl Into<String>) {
        self.logger.info(message);
    }

    /// Log a message with custom attributes
    pub fn log_with_attrs(
        &mut self,
        message: impl Into<String>,
        attrs: HashMap<String, String>,
    ) {
        self.logger.log_with_attrs(LogLevel::Info, message, attrs);
    }

    /// Log a debug message
    pub fn debug(&mut self, message: impl Into<String>) {
        self.logger.debug(message);
    }

    /// Log an info message
    pub fn info(&mut self, message: impl Into<String>) {
        self.logger.info(message);
    }

    /// Log a warning message
    pub fn warn(&mut self, message: impl Into<String>) {
        self.logger.warn(message);
    }

    /// Log an error message
    pub fn error(&mut self, message: impl Into<String>) {
        self.logger.error(message);
    }

    /// Prints the radar data to the terminal
    pub fn print_radar(&mut self, update: &ServerUpdate) {
        self.logger.info(format!(
            "Radar for Bot {} (Tick {}):\n{}",
            self.bot_id,
            update.tick,
            format_radar(&update.radar)
        ));
    }
}

pub trait Bot: Send + 'static {
    fn new(ctx: Ctx) -> Self
    where
        Self: Sized;
    fn run(&mut self) -> Result<()>;
}

type BotFactory = Box<dyn Fn(Ctx) -> Box<dyn Bot> + Send>;

pub struct Harness {
    factories: HashMap<String, BotFactory>,
}

impl Harness {
    pub fn new() -> Self {
        Self {
            factories: HashMap::new(),
        }
    }

    pub fn register<B: Bot>(&mut self, name: impl Into<String>) -> &mut Self {
        self.factories
            .insert(name.into(), Box::new(move |ctx| Box::new(B::new(ctx))));

        self
    }

    pub fn run_bots(self) -> Result<()> {
        let writer;
        loop {
            if let Ok(writer_ok) = TcpStream::connect("127.0.0.1:1234") {
                writer = writer_ok;
                break;
            }
            sleep(Duration::from_millis(100));
        }

        let mut reader = BufReader::new(writer.try_clone()?);
        let mut writer = BufWriter::new(writer);

        let (bot_msg_tx, bot_msg_rx) = mpsc::channel();
        let factories = self.factories;

        std::thread::spawn(move || {
            let mut response_channel_map =
                HashMap::<u32, Sender<ServerUpdateEnvelope>>::new();

            loop {
                let msg: ServerMsg =
                    Protocol::read_message(&mut reader).unwrap();
                match msg {
                    ServerMsg::ConnectAck => println!("ConnectAck"),
                    ServerMsg::AssignBot(bot_id, bot_type) => {
                        println!(
                            "Got AssignBot msg: {bot_id} (type: {bot_type}), \
                             spawning bot..."
                        );

                        // Set up channels and Ctx
                        let (resp_tx, resp_rx) = mpsc::channel();
                        let bot_msg_tx = bot_msg_tx.clone();
                        let ctx = Ctx::new(bot_id, resp_rx, bot_msg_tx);

                        response_channel_map.insert(bot_id, resp_tx);

                        // Use factory to create bot
                        let factory = factories.get(&bot_type).unwrap();
                        let mut bot = factory(ctx);

                        // Spawn bot
                        std::thread::spawn(move || {
                            if let Err(e) = bot.run() {
                                eprintln!(
                                    "[Error] Bot {} error: {:?}",
                                    bot_id, e
                                );
                            }
                        });
                        println!("Bot Spawned: {bot_id}");
                    }
                    ServerMsg::ServerUpdate(server_update_envelope) => {
                        // Find the correct response channel for this bot
                        let resp_tx = response_channel_map
                            .get(&server_update_envelope.bot_id)
                            .unwrap();

                        // Forward the response to the bot
                        resp_tx
                            .send(server_update_envelope)
                            .expect("Failed to send server update on channel");
                    }
                    ServerMsg::Close => {
                        println!("Received close message, exiting...");
                        exit(0);
                    }
                }
            }
        });

        Protocol::write_message(&mut writer, &ClientMsg::Connect)
            .expect("Failed to send Connect message");
        println!("Sent Connect msg to server");

        // Send bot messages to the server
        loop {
            let bot_msg: BotMsgEnvelope =
                bot_msg_rx.recv().expect("Failed to receive bot message");
            println!(
                "Received bot message on channel, sending to server...: msg: \
                 {bot_msg:?}"
            );

            Protocol::write_message(&mut writer, &ClientMsg::BotMsg(bot_msg))
                .expect("Failed to send bot message");

            println!("Bot msg sent to server");
        }
    }
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
                    let bot = &radar.bots[pawn_idx];
                    match bot.team {
                        Team::Player => ['P', ' '],
                        Team::Enemy => ['E', ' '],
                    }
                } else if let Some(item) = cell.item {
                    match item {
                        crate::Item::Crumb => ['C', ' '], // Crumb
                        crate::Item::Fent => ['F', ' '],  // Fent
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
    if !radar.bots.is_empty() {
        output.push_str("\nBots detected:\n");
        for (i, bot) in radar.bots.iter().enumerate() {
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
