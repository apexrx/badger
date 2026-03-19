
    ,---,.
  ,'  .'  \                 ,---,
,---.' .' |               ,---.'|                       __  ,-.
|   |  |: |               |   | :  ,----._,.          ,' ,'/ /|
:   :  :  /  ,--.--.      |   | | /   /  ' /   ,---.  '  | |' |
:   |    ;  /       \   ,--.__| ||   :     |  /     \ |  |   ,'
|   :     \.--.  .-. | /   ,'   ||   | .\  . /    /  |'  :  /
|   |   . | \__\/: . ..   '  /  |.   ; ';  |.    ' / ||  | '
'   :  '; | ," .--.; |'   ; |:  |'   .   . |'   ;   /|;  : |
|   |  | ; /  /  ,.  ||   | '/  ' `---`-'| |   |  / ||  , ;
|   :   / ;  :   .'   \   :    :| .'__/\_: ||   :    | ---'
|   | ,'  |  ,     .-./\   \  /   |   :    : \   \  /
`----'     `--`---'     `----'     \   \  /   `----'
                                    `--`-'

<div align="center">

# Badger

**A reliable, observable Rust background worker for HTTP jobs**

[![License](https://img.shields.io/github/license/apexrx/badger.svg?style=flat-square)](LICENSE)
[![Last Commit](https://img.shields.io/github/last-commit/apexrx/badger.svg?style=flat-square)](commits)
[![Issues](https://img.shields.io/github/issues/apexrx/badger.svg?style=flat-square)](issues)
[![Tests](https://img.shields.io/badge/tests-35%20passed-brightgreen?style=flat-square)](TESTING_REPORT.md)

</div>

---

## Overview

**Badger** is a durable background job executor for HTTP work. Offload slow, unreliable, or long-running HTTP tasks (webhooks, API calls, notifications) to a separate, fault-tolerant system.

## Features

| Feature | Description |
|---------|-------------|
| **Durable Queue** | Jobs persist in PostgreSQL/SQLite across restarts |
| **Async Worker Pool** | High-performance Tokio-based workers |
| **Retry Engine** | Exponential backoff with jitter |
| **Crash Recovery** | Heartbeat-based stale job detection |
| **Rate Limiting** | Per-host throttling with Governor |
| **Observability** | Prometheus metrics + Grafana dashboards |

---

## Badger vs Oban

Head-to-head comparison on identical hardware (AMD Ryzen 5 5600H, 16GB RAM, PostgreSQL). Full benchmark methodology and results available in the [Benchmark Report](testing/BENCHMARK_REPORT.md).

### Insertion Performance

```
Single Job Insertion (jobs/sec) - Higher is Better
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
Badger          ████████████████████████████████████  155.67
Oban            ██████████████████████████░░░░░░░░░░  82.46
                Badger is 88.8% faster
```

```
Concurrent Single Insertion (jobs/sec) - Higher is Better
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
Badger          ████████████████████████████████████  1,530.21
Oban            ████████████████░░░░░░░░░░░░░░░░░░░░  545.05
                Badger is 180.7% faster
```

```
Bulk Insertion (jobs/sec) - Higher is Better
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
Badger          ████████████████████████████████████  27,950.63
Oban            ████████████████████░░░░░░░░░░░░░░░░  13,503.60
                Badger is 107.0% faster
```

```
Concurrent Bulk Insertion (jobs/sec) - Higher is Better
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
Oban            ████████████████████████████████████  37,242.01
Badger          ████████████████████████████░░░░░░░░  28,289.87
                Oban is 31.6% faster
```

### Summary Table

| Benchmark | Badger | Oban | Winner |
|-----------|--------|------|--------|
| Single Insert | 155.67 jobs/sec | 82.46 jobs/sec | **Badger** (+88.8%) |
| Concurrent Insert | 1,530.21 jobs/sec | 545.05 jobs/sec | **Badger** (+180.7%) |
| Bulk Insert | 27,950.63 jobs/sec | 13,503.60 jobs/sec | **Badger** (+107.0%) |
| Concurrent Bulk | 28,289.87 jobs/sec | 37,242.01 jobs/sec | **Oban** (+31.6%) |
| Insert Latency | 6,423 us | 12,127 us | **Badger** (-47%) |
| Marginal Cost | 54.1 us/job | 72.7 us/job | **Badger** (-25.6%) |

### Single-Worker Processing (10ms work)

| Metric | Badger | Oban | Winner |
|--------|--------|------|--------|
| Throughput | ~73 jobs/sec | ~41 jobs/sec | **Badger** |

> **Note:** Full processing throughput comparison requires further instrumentation. Oban measures dispatch rate while Badger measures full claim to execute to complete cycle. See [Benchmark Report](testing/BENCHMARK_REPORT.md) for details.

### When to Choose Badger

- Single-job insertion patterns
- Low-latency requirements
- Memory-constrained environments
- Zero-GC requirements
- Rust ecosystem integration

### When to Choose Oban

- Concurrent bulk insertion patterns
- Elixir/Phoenix ecosystem
- Complex job workflows
- Mature plugin ecosystem

See the full [Benchmark Report](testing/BENCHMARK_REPORT.md) for detailed methodology and complete results.

---

## Quick Start

### Prerequisites

- Rust & Cargo
- Docker & Docker Compose
- GNU Make

### Installation

```bash
# 1. Clone the repository
git clone https://github.com/badger-rs/badger.git
cd badger

# 2. Install and configure
make install
```

The `make install` command:
1. Prompts for your `DATABASE_URL`
2. Generates `.env` configuration
3. Starts Prometheus + Grafana via Docker Compose
4. Builds and installs the `badger` CLI

### Running

```bash
badger
```

Badger connects to your database and begins processing jobs immediately.

---

## Makefile Commands

| Command | Description |
|---------|-------------|
| `make setup` | Create `.env` file interactively |
| `make up` | Start Prometheus + Grafana |
| `make down` | Stop all Docker containers |
| `make run` | Run Badger locally (dev mode) |
| `make install` | Full installation + CLI |

---

## Architecture

### How It Works

```
+-------------+     +-------------+     +-------------+
|   Client    | --> |   Badger    | --> |  HTTP Job   |
| Application |     |   Worker    |     |   Target    |
+-------------+     +------+------+     +-------------+
                           |
                           v
                    +-------------+
                    |  PostgreSQL |
                    |   / SQLite  |
                    +-------------+
```

### Job Lifecycle

```
+---------+     +---------+     +---------+
| Pending | --> | Running | --> | Success |
+----+----+     +----+----+     +---------+
     |               |
     |  (retry)      |  (error)
     |               v
     +---------------+-------> Failure
```

---

## Observability

Badger includes built-in monitoring via Prometheus and Grafana.

### Metrics Endpoints

| Endpoint | URL | Description |
|----------|-----|-------------|
| Grafana | http://localhost:3001 | Pre-configured dashboards |
| Prometheus | http://localhost:9091 | Metrics collection |
| Raw Metrics | http://localhost:3000/metrics | Prometheus format |

### Available Metrics

- `job_queue_depth` - Pending jobs count
- `job_execution_duration_seconds` - Execution time histogram
- `job_queue_lag_seconds` - Time between scheduled and actual execution
- `job_execution_result` - Success/failure counter

---

## Testing

Badger includes comprehensive test coverage:

```bash
# Run all tests
cargo test

# Run PostgreSQL benchmarks
DATABASE_URL="postgresql://user:pass@localhost:5432/db" cargo test --test pg_benchmark -- --nocapture
```

**Test Results:** 35 tests, 100% pass rate

See [TESTING_REPORT.md](TESTING_REPORT.md) for detailed results.

---

## Configuration

### Environment Variables

| Variable | Description | Default | Example |
|----------|-------------|---------|---------|
| `DATABASE_URL` | Database connection string | (required) | `postgres://user:pass@localhost/db` |
| `BADGER_PORT` | HTTP API port | `3000` | `3000` |
| `WORKER_COUNT` | Number of worker threads | `10` | `10` |
| `MAX_RETRIES` | Maximum retry attempts | `10` | `10` |

### Database Schema

```sql
CREATE TABLE job (
    unique_id   TEXT PRIMARY KEY,     -- SHA256 fingerprint
    id          UUID NOT NULL,
    url         TEXT NOT NULL,
    method      TEXT NOT NULL,
    headers     JSONB NOT NULL,
    body        JSONB NOT NULL,
    retries     INTEGER NOT NULL,
    attempts    INTEGER NOT NULL,
    status      TEXT NOT NULL,        -- Pending/Running/Success/Failure
    next_run_at TIMESTAMPTZ NOT NULL,
    created_at  TIMESTAMPTZ NOT NULL,
    updated_at  TIMESTAMPTZ NOT NULL,
    check_in    TIMESTAMPTZ,          -- Heartbeat timestamp
    cron        TEXT                  -- Cron expression
);
```

---

## API Reference

### Submit a Job

```bash
curl -X POST http://localhost:3000/jobs \
  -H "Content-Type: application/json" \
  -d '{
    "url": "https://api.example.com/webhook",
    "method": "POST",
    "headers": {"Authorization": "Bearer token"},
    "body": {"event": "user.created", "user_id": 123},
    "run_at": "2026-03-18T12:00:00Z",
    "cron": "0 0 * * * *"
  }'
```

**Response:** Job ID (UUID)

### Get Job Status

```bash
curl http://localhost:3000/jobs/{job_id}
```

**Response:**
```json
{
  "id": "uuid",
  "url": "https://api.example.com/webhook",
  "method": "POST",
  "status": "Pending",
  "retries": 0,
  "attempts": 0,
  "next_run_at": "2026-03-18T12:00:00Z",
  "created_at": "2026-03-18T10:00:00Z"
}
```

---

## License

Distributed under the MIT License. See [LICENSE](LICENSE) for details.

---

<div align="center">

**Badger v1.0.0** | [GitHub](https://github.com/apexrx/badger) | [Benchmark Report](testing/BENCHMARK_REPORT.md)

*Built with Rust | Powered by Tokio | Backed by PostgreSQL*

</div>
