use crate::config::{RoutingConfig, RoutingMode};
use crate::{log_debug, log_error, log_info, log_warn};
use std::io::Write;
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

#[derive(Default)]
pub struct NftablesService;

impl NftablesService {
    pub fn new() -> Self {
        Self
    }

    fn run_cmd(&self, cmd: &str, args: &[&str]) -> Result<String, String> {
        let output = Command::new(cmd)
            .args(args)
            .output()
            .map_err(|e| format!("failed to execute {}: {}", cmd, e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("{} {:?} failed: {}", cmd, args, stderr.trim()));
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    fn apply_nft_script(&self, script: &str) -> Result<(), String> {
        let mut child = Command::new("nft")
            .arg("-f")
            .arg("-")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("failed to spawn nft: {}", e))?;

        if let Some(mut stdin) = child.stdin.take() {
            stdin
                .write_all(script.as_bytes())
                .map_err(|e| format!("failed to write to nft stdin: {}", e))?;
        }

        let output = child
            .wait_with_output()
            .map_err(|e| format!("failed to wait for nft: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("nft failed: {}", stderr.trim()));
        }

        Ok(())
    }

    pub fn is_openwrt_fw4(&self) -> bool {
        self.run_cmd("nft", &["list", "table", "inet", "fw4"]).is_ok()
    }

    pub fn setup_routing(&self, routing: &RoutingConfig) -> Result<(), String> {
        log_debug!("Starting SetupRouting");
        log_debug!("Routing config - TCP: {:?}, UDP: {:?}", routing.tcp, routing.udp);

        log_debug!("Step 1: Cleanup existing rules");
        let _ = self.cleanup_all_routing();

        let tcp_tproxy = routing.tcp == RoutingMode::TProxy;
        let udp_tproxy = routing.udp == RoutingMode::TProxy;

        if tcp_tproxy || udp_tproxy {
            log_debug!("Step 2: Setting up TPROXY");
            self.setup_tproxy(tcp_tproxy, udp_tproxy)?;
            log_info!("TPROXY routing setup completed");
        }

        let tcp_tun = routing.tcp == RoutingMode::Tun;
        let udp_tun = routing.udp == RoutingMode::Tun;

        if tcp_tun || udp_tun {
            log_debug!("Setting up TUN");
            self.setup_tun(routing, tcp_tun, udp_tun)?;
            log_info!("TUN routing setup completed");
        }

        if routing.tcp == RoutingMode::Redirect {
            log_debug!("Setting up REDIRECT");
            self.setup_redirect()?;
            log_info!("REDIRECT routing setup completed");
        }

        log_debug!("SetupRouting completed successfully");
        Ok(())
    }

    fn setup_tproxy(&self, tcp: bool, udp: bool) -> Result<(), String> {
        let mut prerouting_rules = String::new();
        let mut output_rules = String::new();

        if tcp {
            prerouting_rules.push_str("        iifname \"lo\" meta l4proto tcp meta mark & 0x000000ff == 0x00000080 tproxy to :7894 counter accept\n");
            output_rules.push_str("        meta l4proto tcp counter meta mark set 0x00000080 accept\n");
        }
        if udp {
            prerouting_rules.push_str("        iifname \"lo\" meta l4proto udp meta mark & 0x000000ff == 0x00000080 tproxy to :7894 counter accept\n");
            output_rules.push_str("        meta l4proto udp counter meta mark set 0x00000080 accept\n");
        }

        let mut prerouting_mark_rules = String::new();
        if tcp {
            prerouting_mark_rules.push_str("        meta l4proto tcp counter meta mark set 0x00000080 tproxy to :7894 accept\n");
        }
        if udp {
            prerouting_mark_rules.push_str("        meta l4proto udp counter meta mark set 0x00000080 tproxy to :7894 accept\n");
        }

        let nft_script = format!(
            r#"table inet fusiontunx_tproxy {{
    set reserved_ip {{
        type ipv4_addr
        flags interval
        elements = {{ 0.0.0.0/8, 10.0.0.0/8, 100.64.0.0/10, 127.0.0.0/8, 169.254.0.0/16, 172.16.0.0/12, 192.0.0.0/24, 192.0.2.0/24, 192.88.99.0/24, 192.168.0.0/16, 198.18.0.0/15, 198.51.100.0/24, 203.0.113.0/24, 224.0.0.0/3 }}
    }}
    set reserved_ip6 {{
        type ipv6_addr
        flags interval
        elements = {{ ::/128, ::1/128, ::ffff:0:0/96, 64:ff9b::/96, 64:ff9b:1::/48, 100::/64, 2001::/32, 2001:20::/28, 2001:db8::/32, 2002::/16, 5f00::/16, fc00::/7, fe80::/10, ff00::/8 }}
    }}
    chain mangle_prerouting {{
        type filter hook prerouting priority mangle; policy accept;
        meta l4proto udp udp dport 443 reject
        meta mark 0x00000100 return
{prerouting_rules}        ct direction reply counter return
        fib daddr type {{ local, broadcast, anycast, multicast }} counter return
        ip daddr @reserved_ip counter return
        ip6 daddr @reserved_ip6 counter return
{prerouting_mark_rules}    }}
    chain mangle_output {{
        type route hook output priority mangle; policy accept;
        meta l4proto udp udp dport 443 reject
        meta mark 0x00000100 return
        ct direction reply counter return
        fib daddr type {{ local, broadcast, anycast, multicast }} counter return
        ip daddr @reserved_ip counter return
        ip6 daddr @reserved_ip6 counter return
{output_rules}    }}
}}
"#
        );

        self.apply_nft_script(&nft_script)?;

        log_debug!("Setting up policy routing for TPROXY");
        let _ = self.run_cmd("ip", &["-4", "route", "replace", "local", "default", "dev", "lo", "table", "80"]);
        let _ = self.run_cmd("ip", &["-4", "rule", "del", "fwmark", "0x80/0xff", "table", "80", "pref", "1024"]);
        let _ = self.run_cmd("ip", &["-4", "rule", "add", "fwmark", "0x80/0xff", "table", "80", "pref", "1024"]);

        let _ = self.run_cmd("ip", &["-6", "route", "replace", "local", "default", "dev", "lo", "table", "80"]);
        let _ = self.run_cmd("ip", &["-6", "rule", "del", "fwmark", "0x80/0xff", "table", "80", "pref", "1024"]);
        let _ = self.run_cmd("ip", &["-6", "rule", "add", "fwmark", "0x80/0xff", "table", "80", "pref", "1024"]);

        Ok(())
    }

    fn setup_tun(&self, routing: &RoutingConfig, tcp: bool, udp: bool) -> Result<(), String> {
        let dev = if routing.tun_device.is_empty() { "Meta" } else { &routing.tun_device };

        // Wait for device to exist
        let mut ready = false;
        for _ in 0..20 {
            if self.run_cmd("ip", &["link", "show", dev]).is_ok() {
                ready = true;
                break;
            }
            thread::sleep(Duration::from_millis(500));
        }

        if !ready {
            log_warn!("TUN device {} not found after waiting", dev);
        }

        let is_fw4 = self.is_openwrt_fw4();
        if is_fw4 {
            log_info!("Detected OpenWrt fw4, using fw4 chains for TUN routing");
            let fw4_script = format!(
                r#"insert rule inet fw4 forward ip protocol tcp oifname "{dev}" counter accept comment "FusionTunX TUN Forward Out"
insert rule inet fw4 forward ip protocol udp oifname "{dev}" counter accept comment "FusionTunX TUN Forward Out"
insert rule inet fw4 forward ip protocol tcp iifname "{dev}" counter accept comment "FusionTunX TUN Forward In"
insert rule inet fw4 forward ip protocol udp iifname "{dev}" counter accept comment "FusionTunX TUN Forward In"
insert rule inet fw4 input ip protocol tcp iifname "{dev}" counter accept comment "FusionTunX TUN Input"
insert rule inet fw4 input ip protocol udp iifname "{dev}" counter accept comment "FusionTunX TUN Input"
insert rule inet fw4 srcnat meta nfproto ipv4 oifname "{dev}" counter return comment "FusionTunX TUN Postrouting"
"#
            );
            let _ = self.apply_nft_script(&fw4_script);
        } else {
            log_info!("Using standalone nftables table for TUN routing");
        }

        let mut tcp_rule = String::new();
        if tcp {
            tcp_rule.push_str("        meta mark 0 ip protocol tcp meta mark set 200 accept\n");
        }
        let mut udp_rule = String::new();
        if udp {
            udp_rule.push_str("        meta mark 0 ip protocol udp meta mark set 200 accept\n");
        }

        let nft_script = format!(
            r#"table ip fusiontunx_tun {{
    chain prerouting {{
        type filter hook prerouting priority mangle; policy accept;
        iifname "lo" accept
        iifname "{dev}" accept
        ip daddr {{ 127.0.0.0/8, 10.0.0.0/8, 172.16.0.0/12, 192.168.0.0/16 }} accept
{tcp_rule}{udp_rule}    }}
    chain output {{
        type route hook output priority mangle; policy accept;
        oifname "lo" accept
        oifname "{dev}" accept
        ip daddr {{ 127.0.0.0/8, 10.0.0.0/8, 172.16.0.0/12, 192.168.0.0/16 }} accept
{tcp_rule}{udp_rule}    }}
}}
"#
        );

        self.apply_nft_script(&nft_script)?;

        log_debug!("Setting up policy routing for TUN");
        let _ = self.run_cmd("ip", &["rule", "del", "fwmark", "200/0xffffffff", "table", "200", "pref", "100"]);
        let _ = self.run_cmd("ip", &["rule", "add", "fwmark", "200/0xffffffff", "table", "200", "pref", "100"]);
        let _ = self.run_cmd("ip", &["route", "replace", "default", "dev", dev, "table", "200"]);

        Ok(())
    }

    fn setup_redirect(&self) -> Result<(), String> {
        let nft_script = r#"table inet fusiontunx_redirect {
    chain nat_output {
        type nat hook output priority -100; policy accept;
        oifname "lo" accept
        meta mark 0x00000100 counter return
        ip daddr { 127.0.0.0/8, 10.0.0.0/8, 172.16.0.0/12, 192.168.0.0/16 } accept
        meta l4proto tcp counter redirect to :7891
    }
}
"#;
        self.apply_nft_script(nft_script)
    }

    pub fn cleanup_all_routing(&self) -> Result<(), String> {
        let _ = self.run_cmd("nft", &["delete", "table", "inet", "fusiontunx_tproxy"]);
        let _ = self.run_cmd("nft", &["delete", "table", "ip", "fusiontunx_tun"]);
        let _ = self.run_cmd("nft", &["delete", "table", "inet", "fusiontunx_redirect"]);

        // Clean TPROXY policy routing
        let _ = self.run_cmd("ip", &["-4", "rule", "del", "fwmark", "0x80/0xff", "table", "80", "pref", "1024"]);
        let _ = self.run_cmd("ip", &["-6", "rule", "del", "fwmark", "0x80/0xff", "table", "80", "pref", "1024"]);
        let _ = self.run_cmd("ip", &["-4", "route", "flush", "table", "80"]);
        let _ = self.run_cmd("ip", &["-6", "route", "flush", "table", "80"]);

        // Clean TUN policy routing
        let _ = self.run_cmd("ip", &["rule", "del", "fwmark", "200/0xffffffff", "table", "200", "pref", "100"]);
        let _ = self.run_cmd("ip", &["route", "flush", "table", "200"]);

        // Clean fw4 rules if any
        if self.is_openwrt_fw4() {
            if let Ok(rules) = self.run_cmd("nft", &["-a", "list", "table", "inet", "fw4"]) {
                for line in rules.lines() {
                    if line.contains("FusionTunX TUN") {
                        if let Some(handle_pos) = line.rfind("# handle ") {
                            let handle_str = &line[handle_pos + 9..].trim();
                            if let Ok(handle) = handle_str.parse::<u64>() {
                                let chain = if line.contains("forward") {
                                    "forward"
                                } else if line.contains("input") {
                                    "input"
                                } else if line.contains("srcnat") {
                                    "srcnat"
                                } else {
                                    ""
                                };
                                if !chain.is_empty() {
                                    let _ = self.run_cmd(
                                        "nft",
                                        &["delete", "rule", "inet", "fw4", chain, "handle", &handle.to_string()],
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }

    pub fn cleanup_tun_routing(&self) -> Result<(), String> {
        self.cleanup_all_routing()
    }

    pub fn is_tun_routing_active(&self) -> bool {
        self.run_cmd("ip", &["route", "show", "table", "200"]).is_ok()
    }
}
