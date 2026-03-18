```


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
```

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

## Execution Guarantees

- At-least-once execution
- Durable job persistence before execution
- Crash-safe recovery via heartbeats
- Exponential backoff retries with jitter
- Bounded concurrency and backpressure

---

## Performance Comparison

### Normalized Throughput (jobs/sec/worker, 10ms work)

```
Redis-Backed (In-Memory, No ACID)
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
BullMQ          ████████████████████████████████░░  830*
Sidekiq         ████████████████████████████████░░  800*
Celery          ██████████████████████████████░░░░  700*

PostgreSQL-Backed (Full ACID, Durable)
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
Oban            ████████████████████░░░░░░░░░░░░░░░░  440*
Badger          ██████████████░░░░░░░░░░░░░░░░░░░░░░  289

*Estimated from published benchmarks, normalized to 10ms work
```

### Trade-off: Throughput vs Durability

| Queue | Backend | Throughput | Durability | Use Case |
|-------|---------|------------|------------|----------|
| BullMQ | Redis | 830 jobs/sec | In-memory | High throughput, job loss OK |
| Sidekiq | Redis | 800 jobs/sec | In-memory | High throughput, job loss OK |
| Oban | PostgreSQL | 440 jobs/sec | Full ACID | Durability required |
| **Badger** | **PostgreSQL** | **289 jobs/sec** | **Full ACID** | **Durability required** |

**Badger's niche:** Durability-critical workloads where ~300 jobs/sec is sufficient

See [NORMALIZED_COMPARISON.md](NORMALIZED_COMPARISON.md) for detailed analysis.

### Feature Comparison

| Feature | Badger | BullMQ | Sidekiq | Celery | Oban |
|---------|:------:|:------:|:-------:|:------:|:----:|
| At-least-once | Yes | Yes | Yes | Yes | Yes |
| Durable Persistence | Yes | No | No | No | Yes |
| Crash Recovery | Yes | Partial | Partial | Partial | Yes |
| Rate Limiting | Yes | Yes | Yes* | Partial | Yes |
| Cron Scheduling | Yes | Yes | Yes | Yes | Yes |
| Built-in Metrics | Yes | Partial | Partial | Partial | Yes |
| Memory Safe | Yes | No | No | No | Yes |
| Zero GC | Yes | No | No | No | No |
| **Per-Worker (10ms)** | **29 jobs/sec** | **830 jobs/sec** | **800 jobs/sec** | **700 jobs/sec** | **440 jobs/sec** |

*Enterprise feature

**Note:** Throughput measured on PostgreSQL (localhost). Higher throughput available with bulk insertion (16,772 jobs/sec).

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
┌─────────────┐     ┌─────────────┐     ┌─────────────┐
│   Client    │ ──→ │   Badger    │ ──→ │  HTTP Job   │
│ Application │     │   Worker    │     │   Target    │
└─────────────┘     └──────┬──────┘     └─────────────┘
                           │
                           ↓
                    ┌─────────────┐
                    │  PostgreSQL │
                    │   / SQLite  │
                    └─────────────┘
```

### Job Lifecycle

```
┌─────────┐     ┌─────────┐     ┌─────────┐
│ Pending │ ──→ │ Running │ ──→ │ Success │
└────┬────┘     └────┬────┘     └─────────┘
     │               │
     │  (retry)      │  (error)
     │               ↓
     └───────────────┴─────────→ Failure
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
DATABASE_URL="postgresql://user:pass@localhost:5432/badger_db" cargo test --test pg_benchmark -- --nocapture
```

**Test Results:** 35 tests, 100% pass rate

See [TESTING_REPORT.md](TESTING_REPORT.md) for detailed results.

> *Note: Qwen Code was used for testing purposes only.*

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

**Badger v1.0.0** | [GitHub](https://github.com/apexrx/badger) | [Testing Report](TESTING_REPORT.md)

*Built with Rust | Powered by Tokio | Backed by PostgreSQL*

</div>
