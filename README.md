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
|   |  | ; /  /  ,.  ||   | '/  ' `---`-'| |'   |  / ||  , ;    
|   :   / ;  :   .'   \   :    :| .'__/\_: ||   :    | ---'     
|   | ,'  |  ,     .-./\   \  /   |   :    : \   \  /           
`----'     `--`---'     `----'     \   \  /   `----'            
                                    `--`-'                      
```
<div align="center">
<h1>Badger</h1>
<p><strong>A reliable, observable Rust background worker for HTTP jobs.</strong></p>

<p align="center">
  <a href="https://github.com/apexrx/badger/commits/main">
    <img src="https://img.shields.io/github/last-commit/apexrx/badger.svg?style=flat-square" alt="Last Commit">
  </a>
  <a href="https://github.com/apexrx/badger/issues">
    <img src="https://img.shields.io/github/issues/apexrx/badger.svg?style=flat-square" alt="Open Issues">
  </a>
  <a href="https://github.com/apexrx/badger/pulls">
    <img src="https://img.shields.io/github/issues-pr/apexrx/badger.svg?style=flat-square" alt="Open Pull Requests">
  </a>
  <a href="https://github.com/apexrx/badger/blob/main/LICENSE">
    <img src="https://img.shields.io/badge/license-MIT-green.svg?style=flat-square" alt="MIT License">
  </a>
</p>
</div>

## Overview

**Badger** is a durable background job executor for HTTP work. It allows your main application to offload slow, unreliable, or long-running HTTP tasks (webhooks, API calls, notifications) to a separate, fault-tolerant system.

Jobs are persisted, executed asynchronously, retried on failure, and fully observable via metrics and dashboards.

## How It Works

1. Clients submit HTTP jobs to Badger via a REST API  
2. Jobs are stored durably in PostgreSQL or SQLite  
3. A bounded async worker pool pulls and executes jobs  
4. Failures are retried with exponential backoff + jitter  
5. Heartbeats ensure jobs are recovered if a worker crashes  

## Execution Guarantees

Badger provides:

- **At-least-once execution**
- **Durable job persistence before execution**
- **Crash-safe recovery via heartbeats**
- **Exponential backoff retries with jitter**
- **Bounded concurrency and backpressure**


## Features

- **Durable Queue**  
  Jobs are stored in PostgreSQL or SQLite to ensure persistence across restarts.

- **Async Worker Pool**  
  High-performance Tokio-based workers with bounded concurrency.

- **Retry Engine**  
  Exponential backoff with jitter to avoid retry storms.

- **Crash Recovery**  
  Worker heartbeats detect and reclaim stalled jobs automatically.

- **Per-host Rate Limiting**  
  Prevents overwhelming downstream APIs.

- **Prometheus Metrics + Grafana Dashboards**  
  Built-in observability for queue depth, latency, retries, and worker utilization.

## Prerequisites

Before you begin, ensure you have the following installed on your system:

-   Rust & Cargo
-   Docker & Docker Compose
-   `make` (GNU Make)

## Quick Start

Get Badger up and running in seconds.

### 1. Clone the repository

```bash
git clone https://github.com/badger-rs/badger.git
cd badger
```

### 2. Install and configure

```bash
make install
```

What `make install` does behind the scenes:

-   Prompts you for your `DATABASE_URL`.
-   Automatically generates your `.env` configuration file.
-   Spins up the monitoring stack (Prometheus + Grafana) via Docker Compose.
-   Builds and installs the `badger` CLI globally to your system.

## Running Badger

Once installed, you can start the background job executor from anywhere in your terminal:

```bash
badger
```

Badger will immediately connect to your database and begin processing jobs from the queue.

## Makefile Commands

The `Makefile` is primarily used for initial setup and local infrastructure management. Once Badger is installed, you generally won't need to use these unless you are tweaking the infrastructure.

| Command        | Description                                                            |
| :------------- | :--------------------------------------------------------------------- |
| `make setup`   | Prompts for variables and creates the `.env` file if it's missing.     |
| `make up`      | Starts the Prometheus and Grafana containers.                          |
| `make down`    | Stops and removes all associated Docker containers.                    |
| `make run`     | Runs the Badger worker locally (useful for development).               |
| `make install` | Full setup sequence + installs the CLI to your local system.           |

## Observability

Badger is built with production visibility in mind. When you run `make up` or `make install`, a complete observability stack is spun up alongside the worker.

You can view your runtime metrics at:

-   **Grafana Dashboards**: http://localhost:3000 (Default credentials usually `admin`/`admin`)
-   **Prometheus Targets**: http://localhost:9090
-   **Raw Metrics Endpoint**: http://localhost:<BADGER_PORT>/metrics

Note: The included Grafana instance comes pre-loaded with dashboards specifically tailored for Badger, allowing you to instantly monitor queue depth, worker saturation, and job success/failure rates.

## License

Distributed under the MIT License. See `LICENSE` for more information.
