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

#[cfg(test)]
mod tests {
    use mac_sysmon::models::*;
    use mac_sysmon::process_ops::kill_process;

    #[test]
    fn test_kill_process_safety_guards() {
        // Must reject PID 0
        let res_pid0 = kill_process(0, Some("SIGTERM"));
        assert!(res_pid0.is_err());
        assert!(res_pid0.unwrap_err().contains("Cannot terminate protected system PID 0"));

        // Must reject PID 1
        let res_pid1 = kill_process(1, Some("SIGTERM"));
        assert!(res_pid1.is_err());
        assert!(res_pid1.unwrap_err().contains("Cannot terminate protected system PID 1"));

        // Must reject self PID
        let self_pid = std::process::id();
        let res_self = kill_process(self_pid, Some("SIGTERM"));
        assert!(res_self.is_err());
        assert!(res_self.unwrap_err().contains("Cannot terminate self"));

        // Must reject parent PID
        let parent_pid = unsafe { libc::getppid() as u32 };
        if parent_pid > 1 {
            let res_parent = kill_process(parent_pid, Some("SIGTERM"));
            assert!(res_parent.is_err());
            assert!(res_parent.unwrap_err().contains("Cannot terminate parent process"));
        }

        // Must reject invalid signals
        let res_invalid = kill_process(999999, Some("INVALID_SIG"));
        assert!(res_invalid.is_err());
        assert!(res_invalid.unwrap_err().contains("Unsupported signal"));
    }

    #[test]
    fn test_system_metrics_json_serialization() {
        let metrics = SystemMetrics {
            timestamp: 1726358400000,
            system_info: SystemInfo {
                hostname: "test-macbook".to_string(),
                os_name: "macOS".to_string(),
                os_version: "15.0".to_string(),
                kernel_version: "24.0.0".to_string(),
                uptime_secs: 3600,
                cpu_arch: "aarch64".to_string(),
                cpu_brand: "Apple M4 Pro".to_string(),
                physical_core_count: 12,
                total_core_count: 12,
                perf_core_count: Some(8),
                eff_core_count: Some(4),
            },
            cpu: CpuMetrics {
                global_usage: 24.5,
                load_average: [1.2, 1.5, 1.8],
                cores: vec![
                    CpuCoreMetrics {
                        id: 0,
                        name: "CPU 0 (P-Core)".to_string(),
                        usage: 20.0,
                        frequency_mhz: 4050,
                    },
                    CpuCoreMetrics {
                        id: 1,
                        name: "CPU 1 (E-Core)".to_string(),
                        usage: 29.0,
                        frequency_mhz: 2750,
                    },
                ],
            },
            memory: MemoryMetrics {
                total_bytes: 36 * 1024 * 1024 * 1024,
                used_bytes: 18 * 1024 * 1024 * 1024,
                free_bytes: 18 * 1024 * 1024 * 1024,
                available_bytes: 20 * 1024 * 1024 * 1024,
                swap_total_bytes: 2 * 1024 * 1024 * 1024,
                swap_used_bytes: 512 * 1024 * 1024,
                swap_free_bytes: 1536 * 1024 * 1024,
                usage_percent: 50.0,
            },
            disks: vec![DiskMetrics {
                name: "Macintosh HD".to_string(),
                mount_point: "/".to_string(),
                total_bytes: 1_000_000_000_000,
                available_bytes: 500_000_000_000,
                used_bytes: 500_000_000_000,
                usage_percent: 50.0,
                file_system: "apfs".to_string(),
                is_removable: false,
                kind: "SSD".to_string(),
            }],
            disk_io: DiskIoSummary {
                read_bytes_per_sec: 1048576,
                written_bytes_per_sec: 2097152,
                total_read_bytes: 10485760,
                total_written_bytes: 20971520,
            },
            network: NetworkMetrics {
                received_bytes_per_sec: 512000,
                transmitted_bytes_per_sec: 128000,
                total_received_bytes: 5120000,
                total_transmitted_bytes: 1280000,
                interfaces: vec![NetworkInterfaceMetrics {
                    name: "en0".to_string(),
                    received_bytes_per_sec: 512000,
                    transmitted_bytes_per_sec: 128000,
                    total_received_bytes: 5120000,
                    total_transmitted_bytes: 1280000,
                }],
            },
            processes: vec![ProcessMetrics {
                pid: 12345,
                name: "Google Chrome".to_string(),
                app_name: Some("Google Chrome".to_string()),
                exe_path: Some("/Applications/Google Chrome.app/Contents/MacOS/Google Chrome".to_string()),
                cmd: vec!["Google Chrome".to_string()],
                cpu_usage: 12.5,
                memory_bytes: 1024 * 1024 * 800,
                memory_percent: 2.2,
                virtual_memory_bytes: 1024 * 1024 * 2000,
                disk_read_bytes_per_sec: 10240,
                disk_written_bytes_per_sec: 4096,
                total_read_bytes: 102400,
                total_written_bytes: 40960,
                status: "Run".to_string(),
                user_id: Some("501".to_string()),
                user_name: Some("mozano".to_string()),
                start_time: 1726350000,
                run_time: 8400,
                is_app: true,
            }],
        };

        let json = serde_json::to_string(&metrics);
        assert!(json.is_ok());
        let serialized = json.unwrap();
        assert!(serialized.contains("Macintosh HD"));
        assert!(serialized.contains("Google Chrome"));
        assert!(serialized.contains("P-Core"));

        let deserialized: Result<SystemMetrics, _> = serde_json::from_str(&serialized);
        assert!(deserialized.is_ok());
        let roundtrip = deserialized.unwrap();
        assert_eq!(roundtrip.system_info.hostname, "test-macbook");
        assert_eq!(roundtrip.system_info.perf_core_count, Some(8));
        assert_eq!(roundtrip.system_info.eff_core_count, Some(4));
        assert_eq!(roundtrip.cpu.cores.len(), 2);
        assert_eq!(roundtrip.processes.len(), 1);
        assert_eq!(roundtrip.processes[0].pid, 12345);
    }
}
