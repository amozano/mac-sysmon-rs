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

use crate::models::{ProcessDetail, ProcessMetrics};
use std::sync::Arc;
use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System};
use tokio::sync::RwLock;

/// Sends a termination signal to a process with strict macOS system safety interlocks.
pub fn kill_process(pid: u32, signal_str: Option<&str>) -> Result<(), String> {
    // Safety guard 1: Protect PID 0 and PID 1 (launchd)
    if pid == 0 || pid == 1 {
        return Err(format!("Operation not permitted: Cannot terminate protected system PID {}", pid));
    }

    // Safety guard 2: Protect self PID (sysmon server)
    let my_pid = std::process::id();
    if pid == my_pid {
        return Err("Operation rejected: Cannot terminate self (sysmon server)".to_string());
    }

    // Safety guard 3: Protect parent PID
    let parent_pid = unsafe { libc::getppid() as u32 };
    if pid == parent_pid {
        return Err(format!("Operation rejected: Cannot terminate parent process (PID {})", pid));
    }

    // Validate signal
    let sig = match signal_str.unwrap_or("SIGTERM") {
        "SIGKILL" | "kill" | "9" => libc::SIGKILL,
        "SIGTERM" | "term" | "15" => libc::SIGTERM,
        "SIGHUP" | "1" => libc::SIGHUP,
        "SIGINT" | "2" => libc::SIGINT,
        other => return Err(format!("Unsupported signal: {}. Supported signals: SIGTERM, SIGKILL", other)),
    };

    // Safety guard 4: Protect critical macOS system processes (launchd, WindowServer)
    let mut sys = System::new();
    sys.refresh_processes_specifics(
        ProcessesToUpdate::Some(&[Pid::from_u32(pid)]),
        true,
        ProcessRefreshKind::nothing(),
    );
    if let Some(proc) = sys.process(Pid::from_u32(pid)) {
        let name_lower = proc.name().to_string_lossy().to_lowercase();
        if name_lower == "launchd" || name_lower == "windowserver" {
            return Err(format!(
                "Operation not permitted: Cannot terminate protected system process '{}' (PID {})",
                proc.name().to_string_lossy(),
                pid
            ));
        }
    }

    let ret = unsafe { libc::kill(pid as libc::pid_t, sig) };
    if ret == 0 {
        Ok(())
    } else {
        let err = std::io::Error::last_os_error();
        Err(format!("Failed to send signal to PID {}: {}", pid, err))
    }
}

/// Retrieves deep inspection metadata for a process: PPID, cmdline, cwd, environment.
pub async fn get_process_detail(
    pid: u32,
    latest_metrics: Arc<RwLock<crate::models::SystemMetrics>>,
) -> Result<ProcessDetail, String> {
    // 1. Check cached processes first
    let guard = latest_metrics.read().await;
    let cached_metrics = guard
        .processes
        .iter()
        .find(|p| p.pid == pid)
        .cloned();
    drop(guard);

    // 2. Query sysinfo for live process metadata
    let mut sys = System::new();
    sys.refresh_processes_specifics(
        ProcessesToUpdate::Some(&[Pid::from_u32(pid)]),
        true,
        ProcessRefreshKind::everything(),
    );

    let sys_proc = sys.process(Pid::from_u32(pid));

    let process = match (cached_metrics, sys_proc) {
        (Some(m), _) => m,
        (None, Some(sp)) => {
            // Build fallback ProcessMetrics from sysinfo
            let exe = sp.exe().map(|p| p.to_string_lossy().to_string());
            let (app_name, is_app) = crate::collector::detect_macos_app(&exe, sp.name().to_string_lossy().as_ref());
            ProcessMetrics {
                pid,
                name: sp.name().to_string_lossy().to_string(),
                app_name,
                exe_path: exe,
                cmd: sp.cmd().iter().map(|s| s.to_string_lossy().to_string()).collect(),
                cpu_usage: sp.cpu_usage(),
                memory_bytes: sp.memory(),
                memory_percent: 0.0,
                virtual_memory_bytes: sp.virtual_memory(),
                disk_read_bytes_per_sec: 0,
                disk_written_bytes_per_sec: 0,
                total_read_bytes: sp.disk_usage().total_read_bytes,
                total_written_bytes: sp.disk_usage().total_written_bytes,
                status: format!("{:?}", sp.status()),
                user_id: sp.user_id().map(|u| u.to_string()),
                user_name: None,
                start_time: sp.start_time(),
                run_time: sp.run_time(),
                is_app,
            }
        }
        (None, None) => return Err(format!("Process with PID {} not found", pid)),
    };

    if let Some(sp) = sys_proc {
        let parent_pid = sp.parent().map(|p| p.as_u32());
        let cwd = sp.cwd().map(|p| p.to_string_lossy().to_string());
        let root = sp.root().map(|p| p.to_string_lossy().to_string());
        let environ = sp
            .environ()
            .iter()
            .map(|s| s.to_string_lossy().to_string())
            .collect();

        Ok(ProcessDetail {
            process,
            parent_pid,
            cwd,
            root,
            environ,
        })
    } else {
        Ok(ProcessDetail {
            process,
            parent_pid: None,
            cwd: None,
            root: None,
            environ: Vec::new(),
        })
    }
}
