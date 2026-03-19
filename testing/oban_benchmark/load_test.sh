#!/usr/bin/env bash
# Load testing script for Oban via Badger API comparison
# This script provides a comparable load test methodology

set -e

echo "========================================"
echo "  Oban Load Test"
echo "========================================"
echo "Database: PostgreSQL (localhost)"
echo "Queues: default (10 workers)"
echo "========================================"

# Check if PostgreSQL is running
echo ""
echo "[1/4] Checking PostgreSQL availability..."
if ! pg_isready -h localhost -p 5432 > /dev/null 2>&1; then
    echo "ERROR: PostgreSQL is not running"
    exit 1
fi
echo "✓ PostgreSQL is running"

# Setup database
echo ""
echo "[2/4] Setting up Oban tables..."
cd "$(dirname "$0")/oban_benchmark"
mix ecto.create --quiet 2>/dev/null || true

# Run benchmark
echo ""
echo "[3/4] Running Oban load benchmark..."
NUM_JOBS="${NUM_JOBS:-100}"
CONCURRENCY="${CONCURRENCY:-10}"

echo "  Jobs to process: $NUM_JOBS"
echo "  Concurrency: $CONCURRENCY"

# Run the benchmark via mix
MIX_ENV=test mix run -e "
  ObanBenchmark.Repo.init([])
  ObanBenchmark.BenchmarkHelpers.truncate_jobs()
  
  # Insert jobs
  {insert_time, _} = :timer.tc(fn ->
    for _ <- 1..$NUM_JOBS do
      ObanBenchmark.BenchmarkHelpers.insert_single_job(10)
    end
  end)
  
  IO.puts(\"  Insert duration: \#{Float.round(insert_time / 1_000_000, 3)}s\")
  IO.puts(\"  Insert throughput: \#{Float.round($NUM_JOBS / (insert_time / 1_000_000), 2)} jobs/sec\")
  
  # Start Oban
  {:ok, _pid} = Oban.start_link(
    engine: Oban.Engines.Basic,
    queues: [default: $CONCURRENCY],
    repo: ObanBenchmark.Repo,
    plugins: false
  )
  
  # Process jobs
  {process_time, _} = :timer.tc(fn ->
    receive do
      after
        0 ->
          if ObanBenchmark.BenchmarkHelpers.completed_jobs_count() >= $NUM_JOBS do
            :ok
          else
            receive do
              after
                30_000 -> :timeout
            end
          end
    after
      30_000 -> :timeout
    end
  end)
  
  jobs_per_sec = $NUM_JOBS / (process_time / 1_000_000)
  IO.puts(\"  Process duration: \#{Float.round(process_time / 1_000_000, 3)}s\")
  IO.puts(\"  Process throughput: \#{Float.round(jobs_per_sec, 2)} jobs/sec\")
  IO.puts(\"  Per-worker throughput: \#{Float.round(jobs_per_sec / $CONCURRENCY, 2)} jobs/sec/worker\")
"

# Get stats
echo ""
echo "[4/4] Fetching job statistics..."
mix run -e "
  completed = ObanBenchmark.BenchmarkHelpers.completed_jobs_count()
  pending = ObanBenchmark.BenchmarkHelpers.pending_jobs_count()
  IO.puts(\"  Completed jobs: \#{completed}\")
  IO.puts(\"  Pending jobs: \#{pending}\")
"

echo ""
echo "========================================"
echo "  Load Test Complete"
echo "========================================"
