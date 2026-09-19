<p align="center">
  <a href="#mac-sysmon-">
    <img src="./logo.svg" alt="mac-sysmon Logo" width="200" height="200" />
  </a>
</p>

<h1 align="center">mac-sysmon ⚡</h1>

<p align="center">
  <strong>High-Performance macOS System Resource &amp; Process Monitor for Apple Silicon</strong><br>
  <em>Lightweight Rust Backend • Real-Time WebSocket Streaming • 60fps Canvas Sparklines • Apple Dark Mode</em>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/Platform-macOS%20(Apple%20Silicon%20%7C%20Intel)-000000?logo=apple&logoColor=white" alt="Platform" />
  <img src="https://img.shields.io/badge/Rust-1.80%2B-orange?logo=rust&logoColor=white" alt="Rust" />
  <img src="https://img.shields.io/badge/Frontend-Vanilla%20TypeScript-blue?logo=typescript&logoColor=white" alt="TypeScript" />
  <img src="https://img.shields.io/badge/Streaming-WebSocket%20%40%201Hz-00f2fe" alt="WebSocket" />
  <img src="https://img.shields.io/badge/License-Apache%202.0-green.svg" alt="License" />
</p>

---

**mac-sysmon** is a high-performance macOS system resource and process monitoring application built specifically for Apple Silicon (and Intel) MacBook Pros. It features a lightweight **Rust** backend (`sysinfo`, `tokio`, `axum`) and a zero-dependency **Vanilla TypeScript** frontend designed with Apple's dark mode aesthetic, 60fps Canvas sparklines, and real-time WebSocket streaming.

---

## Features

### 1. System Resource Utilization
- **Unified RAM & Memory Pressure**:
  - Total Unified RAM, Used RAM, Free RAM, Available/Cached RAM.
  - Swap and compressed memory monitoring.
  - Real-time 60-second canvas sparkline and memory pressure indicator (Normal / Warning / Critical).
- **SSD & Storage**:
  - Mounted APFS containers, partitions, and external volumes.
  - Capacity usage bars with color-coded warning thresholds (>80% warning, >90% critical).
  - Real-time disk I/O throughput (Read & Write MB/s, total bytes).
- **CPU & Per-Core Grid**:
  - Global CPU utilization percentage.
  - 1m, 5m, and 15m system load averages.
  - **Per-Core Grid**: Dedicated real-time utilization bar, operating frequency (MHz), and mini-sparkline for every Performance and Efficiency core.
- **System Telemetry**:
  - Live network throughput (Upload / Download rates and totals).
  - Uptime, hostname, kernel version, macOS version, and CPU architecture.

### 2. Live Application & Process Consumer Breakdown
- **Process Table**:
  - Live table of all background tasks and user applications with macOS `.app` bundle identification.
  - Displays PID, Application Name, CPU %, Resident Memory (RSS), Memory Share %, Disk Read Rate, Disk Write Rate, User, and Status.
- **Interactive Sorting & Filtering**:
  - Instant sort by CPU %, Memory, Disk Read, Disk Write, Process Name, or PID.
  - Real-time fuzzy/prefix search (`⌘F`) by name or PID.
  - "Apps Only" toggle to focus exclusively on active user applications.
- **Inspection & Signal Control**:
  - Modal inspection showing full command line, parent PID, working directory, and environment variables.
  - Process termination controls: Graceful `SIGTERM` (15) or Force Quit `SIGKILL` (9) with confirmation dialogs.
  - System safety interlocks preventing termination of PID 0, PID 1, launchd, WindowServer, self PID, and parent PID.

---

## Tech Stack

| Layer | Technologies |
|---|---|
| **Backend** | Rust 2021, `sysinfo 0.33`, `tokio 1.40`, `axum 0.7`, `rust-embed 8`, `clap 4.5` |
| **Streaming** | WebSocket broadcast channel (`/ws`) at 1Hz + REST endpoints (`/api/*`) |
| **Frontend** | Vanilla TypeScript (ES2022), zero UI frameworks, modern DOM components |
| **Graphics** | Custom high-performance 60fps HTML5 Canvas charts with gradient fills & Bezier curves |
| **Styling** | Apple Dark Mode design system (SF Pro typography, translucent glassmorphism) |

---

## Quickstart

### Prerequisites
- macOS (Apple Silicon or Intel)
- Node.js & npm (for frontend bundling)
- Rust & Cargo (1.80+)

### Building and Running
```bash
# 1. Clone or navigate to the repository
cd mac-sysmon-rs

# 2. Build the entire application (Frontend + Rust binary)
./build.sh

# 3. Launch the application (automatically opens browser)
./run.sh --open
```

### CLI Options
```bash
Usage: mac-sysmon [OPTIONS]

Options:
  -H, --host <HOST>          Host to bind server to [default: 127.0.0.1]
  -p, --port <PORT>          Port to listen on [default: 3000]
  -i, --interval <INTERVAL>  Metrics refresh interval in milliseconds [default: 1000]
      --open                 Automatically open browser on launch [default: false]
  -h, --help                 Print help
  -V, --version              Print version
```

---

## Keyboard Shortcuts

| Shortcut | Action |
|---|---|
| `⌘1` | Switch to **Dashboard Overview** |
| `⌘2` | Switch to **CPU Core Grid** |
| `⌘3` | Switch to **Memory Inspector** |
| `⌘4` | Switch to **Storage & Disks** |
| `⌘5` | Switch to **Process Manager** |
| `⌘F` | Focus Process Search input |

---

## API Reference

### WebSocket Stream
- `GET /ws`: Upgrades connection to WebSocket. Emits full `SystemMetrics` JSON snapshot every second (or configured interval).

### REST Endpoints
- `GET /api/system`: Returns current system telemetry snapshot.
- `GET /api/processes`: Returns list of active processes and metrics.
- `GET /api/processes/:pid`: Returns detailed process information (parent PID, working directory, environment).
- `POST /api/processes/:pid/kill`: Sends termination signal (`SIGTERM` or `SIGKILL`). Body: `{"signal": "SIGTERM"}`.

---

## License

This project is licensed under the **Apache License, Version 2.0**. You may obtain a copy of the License in the [LICENSE](LICENSE) file or at:

```
http://www.apache.org/licenses/LICENSE-2.0
```

Unless required by applicable law or agreed to in writing, software distributed under the License is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied. See the License for the specific language governing permissions and limitations under the License.

Copyright (c) 2026 Ashton Mozano / mac-sysmon-rs contributors.
