// Copyright 2026 Ashton Mozano / mac-sysmon-rs contributors
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use crate::models::*;
use std::collections::HashMap;
use std::path::Path;
use std::process::Command;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use sysinfo::{
    CpuRefreshKind, Disks, Networks, ProcessRefreshKind, ProcessesToUpdate, RefreshKind,
    System, Users,
};
use tokio::sync::{broadcast, RwLock};
use tracing::info;

pub struct MetricsCollector {
    system: System,
    disks: Disks,
    networks: Networks,
    users: Users,
    last_tick: Instant,
    last_net_totals: HashMap<String, (u64, u64)>, // iface -> (total_rx, total_tx)
    last_disk_totals: (u64, u64),                  // (total_read, total_write)
    latest_metrics: Arc<RwLock<SystemMetrics>>,
    broadcast_tx: broadcast::Sender<SystemMetrics>,
    perf_core_count: usize,
    eff_core_count: usize,
}

impl MetricsCollector {
    pub fn new() -> (Self, Arc<RwLock<SystemMetrics>>, broadcast::Sender<SystemMetrics>) {
        let refresh_kind = RefreshKind::nothing()
            .with_cpu(CpuRefreshKind::everything())
            .with_memory(sysinfo::MemoryRefreshKind::everything())
            .with_processes(ProcessRefreshKind::everything());

        let mut system = System::new_with_specifics(refresh_kind);
        let disks = Disks::new_with_refreshed_list();
        let networks = Networks::new_with_refreshed_list();
        let users = Users::new_with_refreshed_list();

        // Initial refresh to populate baselines
        system.refresh_cpu_all();
        system.refresh_memory();
        system.refresh_processes(ProcessesToUpdate::All, true);

        let (perf_cores, eff_cores) = Self::detect_perf_eff_cores();

        let initial_metrics = Self::build_snapshot(
            &system,
            &disks,
            &networks,
            &users,
            0.0,
            &HashMap::new(),
            0,
            0,
            perf_cores,
            eff_cores,
        );
        let (broadcast_tx, _) = broadcast::channel(64);
        let latest_metrics = Arc::new(RwLock::new(initial_metrics));

        let collector = Self {
            system,
            disks,
            networks,
            users,
            last_tick: Instant::now(),
            last_net_totals: HashMap::new(),
            last_disk_totals: (0, 0),
            latest_metrics: Arc::clone(&latest_metrics),
            broadcast_tx: broadcast_tx.clone(),
            perf_core_count: perf_cores,
            eff_core_count: eff_cores,
        };

        (collector, latest_metrics, broadcast_tx)
    }

    pub fn subscribe(&self) -> broadcast::Receiver<SystemMetrics> {
        self.broadcast_tx.subscribe()
    }

    pub async fn run_loop(mut self, interval_ms: u64) {
        let mut interval = tokio::time::interval(Duration::from_millis(interval_ms));
        info!("Metrics collector background loop started with interval {}ms", interval_ms);

        loop {
            interval.tick().await;

            let now = Instant::now();
            let elapsed_secs = now.duration_since(self.last_tick).as_secs_f64().max(0.001);
            self.last_tick = now;

            // Refresh system data
            self.system.refresh_cpu_all();
            self.system.refresh_memory();
            self.system.refresh_processes(ProcessesToUpdate::All, true);
            self.disks.refresh(true);
            self.networks.refresh(true);

            // Calculate network rates
            let mut net_rates = HashMap::new();

            for (name, net) in self.networks.iter() {
                // Filter out non-physical or inactive interfaces
                if name.ends_with('*') {
                    continue;
                }

                let rx = net.total_received();
                let tx = net.total_transmitted();

                let (last_rx, last_tx) = self.last_net_totals.get(name).copied().unwrap_or((rx, tx));
                let rx_delta = rx.saturating_sub(last_rx);
                let tx_delta = tx.saturating_sub(last_tx);

                let rx_per_sec = (rx_delta as f64 / elapsed_secs) as u64;
                let tx_per_sec = (tx_delta as f64 / elapsed_secs) as u64;

                net_rates.insert(name.clone(), (rx_per_sec, tx_per_sec));
                self.last_net_totals.insert(name.clone(), (rx, tx));
            }

            // Calculate aggregate process disk I/O rates
            let mut total_disk_read_delta = 0u64;
            let mut total_disk_written_delta = 0u64;
            for process in self.system.processes().values() {
                let du = process.disk_usage();
                total_disk_read_delta += du.read_bytes;
                total_disk_written_delta += du.written_bytes;
            }

            let disk_read_per_sec = (total_disk_read_delta as f64 / elapsed_secs) as u64;
            let disk_written_per_sec = (total_disk_written_delta as f64 / elapsed_secs) as u64;

            self.last_disk_totals = (
                self.last_disk_totals.0 + total_disk_read_delta,
                self.last_disk_totals.1 + total_disk_written_delta,
            );

            let snapshot = Self::build_snapshot(
                &self.system,
                &self.disks,
                &self.networks,
                &self.users,
                elapsed_secs,
                &net_rates,
                disk_read_per_sec,
                disk_written_per_sec,
                self.perf_core_count,
                self.eff_core_count,
            );

            // Update shared cache
            {
                let mut guard = self.latest_metrics.write().await;
                *guard = snapshot.clone();
            }

            // Broadcast to all active WebSocket subscribers
            let _ = self.broadcast_tx.send(snapshot);
        }
    }

    fn detect_perf_eff_cores() -> (usize, usize) {
        let mut perf = 0;
        let mut eff = 0;

        // Query sysctl for Apple Silicon performance level counts
        if let Ok(out) = Command::new("sysctl").args(["-n", "hw.perflevel0.logicalcpu"]).output() {
            if let Ok(s) = std::str::from_utf8(&out.stdout) {
                if let Ok(v) = s.trim().parse::<usize>() {
                    perf = v;
                }
            }
        }

        if let Ok(out) = Command::new("sysctl").args(["-n", "hw.perflevel1.logicalcpu"]).output() {
            if let Ok(s) = std::str::from_utf8(&out.stdout) {
                if let Ok(v) = s.trim().parse::<usize>() {
                    eff = v;
                }
            }
        }

        (perf, eff)
    }

    #[allow(clippy::too_many_arguments)]
    fn build_snapshot(
        system: &System,
        disks: &Disks,
        networks: &Networks,
        users: &Users,
        elapsed_secs: f64,
        net_rates: &HashMap<String, (u64, u64)>,
        disk_read_rate: u64,
        disk_write_rate: u64,
        perf_cores: usize,
        eff_cores: usize,
    ) -> SystemMetrics {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        // System Info
        let sys_info = SystemInfo {
            hostname: System::host_name().unwrap_or_else(|| "macbook".to_string()),
            os_name: System::name().unwrap_or_else(|| "macOS".to_string()),
            os_version: System::os_version().unwrap_or_else(|| "Unknown".to_string()),
            kernel_version: System::kernel_version().unwrap_or_default(),
            uptime_secs: System::uptime(),
            cpu_arch: std::env::consts::ARCH.to_string(),
            cpu_brand: system
                .cpus()
                .first()
                .map(|c| c.brand().to_string())
                .unwrap_or_else(|| "Apple Silicon".to_string()),
            physical_core_count: system.physical_core_count().unwrap_or_else(|| system.cpus().len()),
            total_core_count: system.cpus().len(),
            perf_core_count: if perf_cores > 0 { Some(perf_cores) } else { None },
            eff_core_count: if eff_cores > 0 { Some(eff_cores) } else { None },
        };

        // CPU metrics (Overall + Per-Core breakdown with P-Core / E-Core tagging)
        let load_avg = System::load_average();
        let cores = system
            .cpus()
            .iter()
            .enumerate()
            .map(|(idx, cpu)| {
                let name = if perf_cores > 0 {
                    if idx < perf_cores {
                        format!("CPU {} (P-Core)", idx)
                    } else {
                        format!("CPU {} (E-Core)", idx)
                    }
                } else {
                    cpu.name().to_string()
                };

                let freq = if cpu.frequency() > 0 {
                    cpu.frequency()
                } else if perf_cores > 0 {
                    if idx < perf_cores {
                        4050 // Apple Silicon P-Core nominal MHz
                    } else {
                        2750 // Apple Silicon E-Core nominal MHz
                    }
                } else {
                    3200
                };

                CpuCoreMetrics {
                    id: idx,
                    name,
                    usage: (cpu.cpu_usage() * 10.0).round() / 10.0,
                    frequency_mhz: freq,
                }
            })
            .collect();

        let cpu = CpuMetrics {
            global_usage: (system.global_cpu_usage() * 10.0).round() / 10.0,
            load_average: [load_avg.one, load_avg.five, load_avg.fifteen],
            cores,
        };

        // Memory metrics
        let total_mem = system.total_memory();
        let used_mem = system.used_memory();
        let free_mem = system.free_memory();
        let available_mem = system.available_memory();
        let swap_total = system.total_swap();
        let swap_used = system.used_swap();
        let swap_free = system.free_swap();
        let usage_percent = if total_mem > 0 {
            ((used_mem as f32 / total_mem as f32) * 1000.0).round() / 10.0
        } else {
            0.0
        };

        let memory = MemoryMetrics {
            total_bytes: total_mem,
            used_bytes: used_mem,
            free_bytes: free_mem,
            available_bytes: available_mem,
            swap_total_bytes: swap_total,
            swap_used_bytes: swap_used,
            swap_free_bytes: swap_free,
            usage_percent,
        };

        // Disks (APFS containers, partitions, external volumes)
        let mut disk_metrics = Vec::new();
        for d in disks.iter() {
            let total = d.total_space();
            // Skip tiny system volumes (<500MB), same as Python collector
            if total < 500 * 1024 * 1024 {
                continue;
            }

            let avail = d.available_space();
            let used = total.saturating_sub(avail);
            let pct = if total > 0 {
                ((used as f32 / total as f32) * 1000.0).round() / 10.0
            } else {
                0.0
            };

            let mount_str = d.mount_point().to_string_lossy().to_string();
            let mut name_str = d.name().to_string_lossy().to_string();
            if mount_str == "/" || name_str.is_empty() {
                name_str = "Macintosh HD".to_string();
            }

            let is_removable = mount_str.starts_with("/Volumes/") && mount_str != "/Volumes/Macintosh HD";

            disk_metrics.push(DiskMetrics {
                name: name_str,
                mount_point: mount_str,
                total_bytes: total,
                available_bytes: avail,
                used_bytes: used,
                usage_percent: pct,
                file_system: d.file_system().to_string_lossy().to_string(),
                is_removable,
                kind: format!("{:?}", d.kind()),
            });
        }

        // Aggregate disk I/O
        let disk_io = DiskIoSummary {
            read_bytes_per_sec: disk_read_rate,
            written_bytes_per_sec: disk_write_rate,
            total_read_bytes: 0,
            total_written_bytes: 0,
        };

        // Networks
        let mut ifaces = Vec::new();
        let mut tot_rx_sec = 0u64;
        let mut tot_tx_sec = 0u64;
        let mut tot_rx = 0u64;
        let mut tot_tx = 0u64;

        for (name, net) in networks.iter() {
            if name.ends_with('*') {
                continue;
            }
            let (rx_rate, tx_rate) = net_rates.get(name).copied().unwrap_or((0, 0));
            tot_rx_sec += rx_rate;
            tot_tx_sec += tx_rate;
            tot_rx += net.total_received();
            tot_tx += net.total_transmitted();

            ifaces.push(NetworkInterfaceMetrics {
                name: name.clone(),
                received_bytes_per_sec: rx_rate,
                transmitted_bytes_per_sec: tx_rate,
                total_received_bytes: net.total_received(),
                total_transmitted_bytes: net.total_transmitted(),
            });
        }

        let network = NetworkMetrics {
            received_bytes_per_sec: tot_rx_sec,
            transmitted_bytes_per_sec: tot_tx_sec,
            total_received_bytes: tot_rx,
            total_transmitted_bytes: tot_tx,
            interfaces: ifaces,
        };

        // Processes (with .app bundle detection, CPU %, memory RSS, disk I/O, status)
        let mut processes = Vec::new();
        for (pid, proc) in system.processes() {
            let exe = proc.exe().map(|p| p.to_string_lossy().to_string());
            let (app_name, is_app) = Self::detect_macos_app(&exe, proc.name().to_string_lossy().as_ref());

            let user_name = proc.user_id().and_then(|uid| {
                users.iter().find(|u| u.id() == uid).map(|u| u.name().to_string())
            });

            let disk_usage = proc.disk_usage();
            let p_read_rate = if elapsed_secs > 0.0 {
                (disk_usage.read_bytes as f64 / elapsed_secs) as u64
            } else {
                0
            };
            let p_write_rate = if elapsed_secs > 0.0 {
                (disk_usage.written_bytes as f64 / elapsed_secs) as u64
            } else {
                0
            };

            let mem_bytes = proc.memory();
            let mem_pct = if total_mem > 0 {
                ((mem_bytes as f32 / total_mem as f32) * 1000.0).round() / 10.0
            } else {
                0.0
            };

            processes.push(ProcessMetrics {
                pid: pid.as_u32(),
                name: proc.name().to_string_lossy().to_string(),
                app_name,
                exe_path: exe,
                cmd: proc.cmd().iter().map(|s| s.to_string_lossy().to_string()).collect(),
                cpu_usage: (proc.cpu_usage() * 10.0).round() / 10.0,
                memory_bytes: mem_bytes,
                memory_percent: mem_pct,
                virtual_memory_bytes: proc.virtual_memory(),
                disk_read_bytes_per_sec: p_read_rate,
                disk_written_bytes_per_sec: p_write_rate,
                total_read_bytes: disk_usage.total_read_bytes,
                total_written_bytes: disk_usage.total_written_bytes,
                status: Self::format_status(format!("{:?}", proc.status())),
                user_id: proc.user_id().map(|u| u.to_string()),
                user_name,
                start_time: proc.start_time(),
                run_time: proc.run_time(),
                is_app,
            });
        }

        SystemMetrics {
            timestamp,
            system_info: sys_info,
            cpu,
            memory,
            disks: disk_metrics,
            disk_io,
            network,
            processes,
        }
    }

    pub fn detect_macos_app(exe_path: &Option<String>, proc_name: &str) -> (Option<String>, bool) {
        detect_macos_app(exe_path, proc_name)
    }

    fn format_status(raw: String) -> String {
        let s = raw.to_lowercase();
        if s.contains("run") {
            "Run".to_string()
        } else if s.contains("sleep") || s.contains("idle") {
            "Sleep".to_string()
        } else if s.contains("stop") {
            "Stop".to_string()
        } else if s.contains("zombie") {
            "Zombie".to_string()
        } else {
            "Run".to_string()
        }
    }
}

/// Detects macOS .app bundle name and flag from executable path and process name.
pub fn detect_macos_app(exe_path: &Option<String>, proc_name: &str) -> (Option<String>, bool) {
    if proc_name.ends_with(" Helper") || proc_name.ends_with(" Service") {
        return (None, false);
    }

    if let Some(ref path_str) = exe_path {
        let path = Path::new(path_str);
        let mut current = path.parent();
        while let Some(dir) = current {
            if let Some(file_name) = dir.file_name().and_then(|s| s.to_str()) {
                if file_name.ends_with(".app") {
                    let clean_name = file_name.trim_end_matches(".app").to_string();
                    if clean_name.ends_with(" Helper") || clean_name.ends_with(" Service") {
                        return (None, false);
                    }
                    return (Some(clean_name), true);
                }
            }
            current = dir.parent();
        }

        if path_str.starts_with("/Applications/") || path_str.starts_with("/System/Applications/") {
            let clean = Path::new(path_str)
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or(proc_name)
                .trim_end_matches(".app")
                .to_string();
            if clean.ends_with(" Helper") || clean.ends_with(" Service") {
                return (None, false);
            }
            return (Some(clean), true);
        }
    }

    (None, false)
}
