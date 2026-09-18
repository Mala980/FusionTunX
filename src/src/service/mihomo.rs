use crate::config::{save, Config, RoutingMode};
use crate::service::nftables::NftablesService;
use crate::{log_debug, log_error, log_info, log_warn};
use std::fs::{self, File, OpenOptions};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use tokio::sync::RwLock;

pub struct MihomoService {
    pub config: Arc<RwLock<Config>>,
    pub config_path: String,
    pub nftables: Arc<NftablesService>,
}

impl MihomoService {
    pub fn new(config: Arc<RwLock<Config>>, config_path: String, nftables: Arc<NftablesService>) -> Self {
        Self {
            config,
            config_path,
            nftables,
        }
    }

    fn pid_file(&self, working_dir: &str) -> PathBuf {
        Path::new(working_dir).join("mihomo.pid")
    }

    pub async fn get_status(&self) -> String {
        let cfg = self.config.read().await;
        let pid_file = self.pid_file(&cfg.mihomo.working_dir);
        drop(cfg);

        if let Ok(content) = fs::read_to_string(&pid_file) {
            if let Ok(pid) = content.trim().parse::<i32>() {
                unsafe {
                    if libc::kill(pid, 0) == 0 {
                        return "running".to_string();
                    }
                }
            }
            let _ = fs::remove_file(pid_file);
        }

        "stopped".to_string()
    }

    pub async fn kill_existing_mihomo(&self) -> Result<(), String> {
        let cfg = self.config.read().await;
        let pid_file = self.pid_file(&cfg.mihomo.working_dir);
        drop(cfg);

        if let Ok(content) = fs::read_to_string(&pid_file) {
            if let Ok(pid) = content.trim().parse::<i32>() {
                unsafe {
                    if libc::kill(pid, 0) == 0 {
                        log_info!("Killing existing mihomo process (PID: {})", pid);
                        libc::kill(pid, libc::SIGKILL);
                        thread::sleep(Duration::from_millis(200));
                    }
                }
            }
            let _ = fs::remove_file(pid_file);
            log_info!("Existing mihomo process killed successfully");
        }

        Ok(())
    }

    pub async fn start(&self) -> Result<(), String> {
        log_info!("Starting mihomo service");

        self.kill_existing_mihomo().await?;

        log_debug!("Adjusting mihomo configuration");
        self.adjust_mihomo_config().await?;

        let cfg = self.config.read().await.clone();

        if !cfg.mihomo.log_file.is_empty() {
            if Path::new(&cfg.mihomo.log_file).exists() {
                log_debug!("Clearing old mihomo log file");
                let _ = fs::remove_file(&cfg.mihomo.log_file);
            }
            if let Some(parent) = Path::new(&cfg.mihomo.log_file).parent() {
                let _ = fs::create_dir_all(parent);
            }
        }

        let should_setup_routing = self.should_setup_routing(&cfg);

        log_debug!("Starting mihomo core: {}", cfg.mihomo.core_path);

        let mut cmd = Command::new(&cfg.mihomo.core_path);
        cmd.args(["-d", &cfg.mihomo.working_dir, "-f", &cfg.mihomo.config_path]);

        if !cfg.mihomo.log_file.is_empty() {
            if let Ok(out_file) = OpenOptions::new()
                .create(true)
                .write(true)
                .append(true)
                .open(&cfg.mihomo.log_file)
            {
                if let Ok(err_file) = out_file.try_clone() {
                    cmd.stdout(Stdio::from(out_file));
                    cmd.stderr(Stdio::from(err_file));
                }
            }
        }

        let child = cmd.spawn().map_err(|e| {
            log_error!("Failed to start mihomo: {}", e);
            format!("failed to start mihomo: {}", e)
        })?;

        let pid = child.id() as i32;
        let pid_file = self.pid_file(&cfg.mihomo.working_dir);
        if let Err(e) = fs::write(&pid_file, pid.to_string()) {
            unsafe { libc::kill(pid, libc::SIGKILL) };
            log_error!("Failed to write PID file: {}", e);
            return Err(format!("failed to write pid file: {}", e));
        }

        log_info!("Mihomo process started (PID: {})", pid);

        if should_setup_routing {
            log_debug!("Waiting for mihomo to be ready");
            if let Err(e) = self.wait_for_mihomo_ready(&cfg).await {
                unsafe { libc::kill(pid, libc::SIGKILL) };
                let _ = fs::remove_file(&pid_file);
                log_error!("Mihomo not ready: {}", e);
                return Err(format!("mihomo not ready: {}", e));
            }

            log_debug!("Setting up routing");
            if let Err(e) = self.nftables.setup_routing(&cfg.mihomo.routing) {
                unsafe { libc::kill(pid, libc::SIGKILL) };
                let _ = fs::remove_file(&pid_file);
                log_error!("Failed to setup routing: {}", e);
                return Err(format!("failed to setup routing: {}", e));
            }
        }

        drop(cfg);

        {
            let mut cfg = self.config.write().await;
            cfg.mihomo.auto_start = true;
            let _ = save(&cfg, &self.config_path);
        }

        log_info!("Mihomo service started successfully");
        Ok(())
    }

    pub async fn stop(&self, save_state: bool) -> Result<(), String> {
        log_info!("Stopping mihomo service");

        if self.get_status().await == "stopped" {
            log_warn!("Mihomo is already stopped");
            return Err("mihomo is not running".to_string());
        }

        let cfg = self.config.read().await.clone();
        let pid_file = self.pid_file(&cfg.mihomo.working_dir);

        if let Ok(content) = fs::read_to_string(&pid_file) {
            if let Ok(pid) = content.trim().parse::<i32>() {
                log_debug!("Killing mihomo process (PID: {})", pid);
                unsafe {
                    libc::kill(pid, libc::SIGKILL);
                }
            }
            let _ = fs::remove_file(pid_file);
        }

        if self.should_setup_routing(&cfg) {
            log_debug!("Cleaning up routing");
            let _ = self.nftables.cleanup_tun_routing();
        }

        if save_state {
            log_debug!("Saving auto_start state to config");
            drop(cfg);
            let mut cfg = self.config.write().await;
            cfg.mihomo.auto_start = false;
            let _ = save(&cfg, &self.config_path);
        }

        log_info!("Mihomo service stopped successfully");
        Ok(())
    }

    pub async fn restart(&self) -> Result<(), String> {
        log_info!("Restarting mihomo service");
        let _ = self.stop(false).await;
        self.start().await
    }

    pub async fn restore_state(&self) -> Result<(), String> {
        log_debug!("Checking auto_start state");
        let auto_start = { self.config.read().await.mihomo.auto_start };

        if auto_start {
            log_info!("Auto-start is enabled, checking mihomo status");
            if self.get_status().await == "stopped" {
                log_info!("Mihomo is stopped, starting automatically");
                return self.start().await;
            }
            log_info!("Mihomo is already running");
        } else {
            log_debug!("Auto-start is disabled");
        }
        Ok(())
    }

    fn should_setup_routing(&self, cfg: &Config) -> bool {
        cfg.mihomo.routing.tcp != RoutingMode::Disable || cfg.mihomo.routing.udp != RoutingMode::Disable
    }

    async fn wait_for_mihomo_ready(&self, cfg: &Config) -> Result<(), String> {
        let max_wait = Duration::from_secs(10);
        let interval = Duration::from_millis(500);
        let mut elapsed = Duration::ZERO;

        let need_tun = cfg.mihomo.routing.tcp == RoutingMode::Tun || cfg.mihomo.routing.udp == RoutingMode::Tun;
        let tun_device = if cfg.mihomo.routing.tun_device.is_empty() {
            "Meta"
        } else {
            &cfg.mihomo.routing.tun_device
        };

        let pid_file = self.pid_file(&cfg.mihomo.working_dir);

        while elapsed < max_wait {
            if let Ok(content) = fs::read_to_string(&pid_file) {
                if let Ok(pid) = content.trim().parse::<i32>() {
                    let alive = unsafe { libc::kill(pid, 0) == 0 };
                    if alive {
                        if need_tun {
                            let sys_path = format!("/sys/class/net/{}", tun_device);
                            if Path::new(&sys_path).exists() {
                                log_info!("TUN interface is ready");
                                return Ok(());
                            }
                            log_debug!("TUN interface not ready yet, elapsed: {:?}", elapsed);
                        } else {
                            log_info!("Mihomo process is ready");
                            return Ok(());
                        }
                    } else {
                        log_error!("Mihomo process died unexpectedly");
                        return Err("mihomo process died".to_string());
                    }
                }
            }
            tokio::time::sleep(interval).await;
            elapsed += interval;
        }

        if need_tun {
            Err("timeout waiting for TUN interface".to_string())
        } else {
            Err("timeout waiting for mihomo to be ready".to_string())
        }
    }

    pub async fn adjust_mihomo_config(&self) -> Result<(), String> {
        let cfg = self.config.read().await.clone();
        let need_tun = cfg.mihomo.routing.tcp == RoutingMode::Tun || cfg.mihomo.routing.udp == RoutingMode::Tun;
        let tun_device = if cfg.mihomo.routing.tun_device.is_empty() {
            "Meta"
        } else {
            &cfg.mihomo.routing.tun_device
        };

        let content = fs::read_to_string(&cfg.mihomo.config_path)
            .map_err(|e| format!("failed to read mihomo config: {}", e))?;

        let adjusted = if need_tun {
            ensure_tun_enabled(&content, tun_device)
        } else {
            ensure_tun_disabled(&content)
        };

        fs::write(&cfg.mihomo.config_path, adjusted)
            .map_err(|e| format!("failed to write mihomo config: {}", e))?;

        Ok(())
    }
}

fn ensure_tun_enabled(config: &str, device_name: &str) -> String {
    let lines: Vec<&str> = config.lines().collect();
    let mut new_lines = Vec::new();
    let mut in_tun_section = false;
    let mut has_device_field = false;
    let mut tun_section_indent = "  ".to_string();

    let mut temp_in_tun = false;
    for line in &lines {
        let trimmed = line.trim();
        if trimmed == "tun:" {
            temp_in_tun = true;
            continue;
        }
        if temp_in_tun {
            if !trimmed.is_empty() {
                let is_indented = line.starts_with(' ') || line.starts_with('\t');
                if !is_indented {
                    temp_in_tun = false;
                } else if trimmed.starts_with("device:") {
                    has_device_field = true;
                }
            }
        }
    }

    for line in &lines {
        let trimmed = line.trim();

        if trimmed == "tun:" {
            in_tun_section = true;
            new_lines.push(line.to_string());
            continue;
        }

        if in_tun_section && !trimmed.is_empty() {
            let is_indented = line.starts_with(' ') || line.starts_with('\t');
            if !is_indented {
                in_tun_section = false;
            }
        }

        if in_tun_section {
            if line.starts_with(' ') || line.starts_with('\t') {
                let indent_len = line.len() - line.trim_start().len();
                tun_section_indent = line[..indent_len].to_string();
            }

            if trimmed.starts_with("enable:") {
                if line.contains("false") {
                    new_lines.push(line.replace("enable: false", "enable: true"));
                } else {
                    new_lines.push(line.to_string());
                }

                if !has_device_field {
                    new_lines.push(format!("{}device: {}", tun_section_indent, device_name));
                    has_device_field = true;
                }
                continue;
            }

            if trimmed.starts_with("device:") {
                if let Some(idx) = line.find(':') {
                    let prefix = &line[..idx + 1];
                    new_lines.push(format!("{} {}", prefix, device_name));
                } else {
                    new_lines.push(line.to_string());
                }
                continue;
            }
        }

        new_lines.push(line.to_string());
    }

    new_lines.join("\n")
}

fn ensure_tun_disabled(config: &str) -> String {
    let mut lines: Vec<String> = config.lines().map(|s| s.to_string()).collect();
    let mut in_tun_section = false;

    for line in &mut lines {
        let trimmed = line.trim();

        if trimmed == "tun:" {
            in_tun_section = true;
            continue;
        }

        if in_tun_section && trimmed.starts_with("enable:") {
            *line = line.replace("enable: true", "enable: false");
            in_tun_section = false;
        }

        if in_tun_section && !trimmed.is_empty() && !line.starts_with(' ') && !line.starts_with('\t') {
            in_tun_section = false;
        }
    }

    lines.join("\n")
}
