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

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemMetrics {
    pub timestamp: u64,
    pub system_info: SystemInfo,
    pub cpu: CpuMetrics,
    pub memory: MemoryMetrics,
    pub disks: Vec<DiskMetrics>,
    pub disk_io: DiskIoSummary,
    pub network: NetworkMetrics,
    pub processes: Vec<ProcessMetrics>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemInfo {
    pub hostname: String,
    pub os_name: String,
    pub os_version: String,
    pub kernel_version: String,
    pub uptime_secs: u64,
    pub cpu_arch: String,
    pub cpu_brand: String,
    pub physical_core_count: usize,
    pub total_core_count: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub perf_core_count: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub eff_core_count: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CpuMetrics {
    pub global_usage: f32,
    pub load_average: [f64; 3], // 1m, 5m, 15m
    pub cores: Vec<CpuCoreMetrics>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CpuCoreMetrics {
    pub id: usize,
    pub name: String,
    pub usage: f32,
    pub frequency_mhz: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryMetrics {
    pub total_bytes: u64,
    pub used_bytes: u64,
    pub free_bytes: u64,
    pub available_bytes: u64,
    pub swap_total_bytes: u64,
    pub swap_used_bytes: u64,
    pub swap_free_bytes: u64,
    pub usage_percent: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskMetrics {
    pub name: String,
    pub mount_point: String,
    pub total_bytes: u64,
    pub available_bytes: u64,
    pub used_bytes: u64,
    pub usage_percent: f32,
    pub file_system: String,
    pub is_removable: bool,
    pub kind: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskIoSummary {
    pub read_bytes_per_sec: u64,
    pub written_bytes_per_sec: u64,
    pub total_read_bytes: u64,
    pub total_written_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkMetrics {
    pub received_bytes_per_sec: u64,
    pub transmitted_bytes_per_sec: u64,
    pub total_received_bytes: u64,
    pub total_transmitted_bytes: u64,
    pub interfaces: Vec<NetworkInterfaceMetrics>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkInterfaceMetrics {
    pub name: String,
    pub received_bytes_per_sec: u64,
    pub transmitted_bytes_per_sec: u64,
    pub total_received_bytes: u64,
    pub total_transmitted_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessMetrics {
    pub pid: u32,
    pub name: String,
    pub app_name: Option<String>,
    pub exe_path: Option<String>,
    pub cmd: Vec<String>,
    pub cpu_usage: f32,
    pub memory_bytes: u64,
    pub memory_percent: f32,
    pub virtual_memory_bytes: u64,
    pub disk_read_bytes_per_sec: u64,
    pub disk_written_bytes_per_sec: u64,
    pub total_read_bytes: u64,
    pub total_written_bytes: u64,
    pub status: String,
    pub user_id: Option<String>,
    pub user_name: Option<String>,
    pub start_time: u64,
    pub run_time: u64,
    pub is_app: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessDetail {
    pub process: ProcessMetrics,
    pub parent_pid: Option<u32>,
    pub cwd: Option<String>,
    pub root: Option<String>,
    pub environ: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct KillProcessRequest {
    pub signal: Option<String>, // "SIGTERM" or "SIGKILL"
}

#[derive(Debug, Serialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub message: String,
    pub data: Option<T>,
}
