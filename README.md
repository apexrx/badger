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
    <img src="https://img.shields.io/github/license/apexrx/badger.svg?style=flat-square" alt="License">
  </a>
</p>
</div>

## Table of Contents

- [Overview](#overview)
- [Features](#features)
- [Prerequisites](#prerequisites)
- [Quick Start](#quick-start)
- [Running Badger](#running-badger)
- [Makefile Commands](#makefile-commands)
- [Observability](#observability)
- [Contributing](#contributing)
- [License](#license)

## Overview

Badger is designed to handle long-running or unreliable HTTP work safely outside of your main application's request lifecycle. It pairs a persistent database-backed queue with a high-performance Rust asynchronous worker pool, giving you full visibility into latency, queue lag, and throughput.

## Features

-   **Persistent Queue**: Backed by PostgreSQL or SQLite to ensure zero data loss.
-   **Asynchronous Pool**: High-performance worker pool with bounded concurrency.
-   **Smart Retries**: Exponential backoff with retry and jitter to prevent thundering herds.
-   **Crash Recovery**: Built-in heartbeats ensure jobs survive unexpected worker crashes.
-   **Rate Limiting**: Built-in per-host rate limiting to respect downstream API constraints.
-   **First-Class Observability**: Pre-configured with Prometheus and Grafana for out-of-the-box queue depth, latency, and lag metrics.

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
