use chrono::Local;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::sync::{Arc, Mutex, OnceLock};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LogLevel {
    Debug = 0,
    Info = 1,
    Warn = 2,
    Error = 3,
}

impl LogLevel {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "debug" => LogLevel::Debug,
            "info" => LogLevel::Info,
            "warn" | "warning" => LogLevel::Warn,
            "error" => LogLevel::Error,
            _ => LogLevel::Info,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            LogLevel::Debug => "DEBUG",
            LogLevel::Info => "INFO",
            LogLevel::Warn => "WARN",
            LogLevel::Error => "ERROR",
        }
    }
}

pub struct Logger {
    level: LogLevel,
    file: Option<Arc<Mutex<File>>>,
}

static GLOBAL_LOGGER: OnceLock<Logger> = OnceLock::new();

pub fn init(level_str: &str, file_path: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let level = LogLevel::from_str(level_str);
    let file = if !file_path.is_empty() {
        if let Some(parent) = Path::new(file_path).parent() {
            fs::create_dir_all(parent)?;
        }
        let f = OpenOptions::new()
            .create(true)
            .write(true)
            .append(true)
            .open(file_path)?;
        Some(Arc::new(Mutex::new(f)))
    } else {
        None
    };

    let logger = Logger { level, file };
    let _ = GLOBAL_LOGGER.set(logger);

    log(LogLevel::Info, &format!("Logger initialized in {} mode", level.as_str()));
    Ok(())
}

pub fn log(level: LogLevel, message: &str) {
    if let Some(logger) = GLOBAL_LOGGER.get() {
        if level < logger.level {
            return;
        }

        let now = Local::now().format("%Y/%m/%d %H:%M:%S");
        let line = format!("{} [{}] {}\n", now, level.as_str(), message);

        print!("{}", line);

        if let Some(file_mutex) = &logger.file {
            if let Ok(mut f) = file_mutex.lock() {
                let _ = f.write_all(line.as_bytes());
                let _ = f.flush();
            }
        }
    } else {
        let now = Local::now().format("%Y/%m/%d %H:%M:%S");
        println!("{} [{}] {}", now, level.as_str(), message);
    }
}

#[macro_export]
macro_rules! log_debug {
    ($($arg:tt)*) => {
        $crate::logger::log($crate::logger::LogLevel::Debug, &format!($($arg)*))
    };
}

#[macro_export]
macro_rules! log_info {
    ($($arg:tt)*) => {
        $crate::logger::log($crate::logger::LogLevel::Info, &format!($($arg)*))
    };
}

#[macro_export]
macro_rules! log_warn {
    ($($arg:tt)*) => {
        $crate::logger::log($crate::logger::LogLevel::Warn, &format!($($arg)*))
    };
}

#[macro_export]
macro_rules! log_error {
    ($($arg:tt)*) => {
        $crate::logger::log($crate::logger::LogLevel::Error, &format!($($arg)*))
    };
}
