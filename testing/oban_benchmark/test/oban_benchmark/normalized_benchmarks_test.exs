defmodule ObanBenchmark.NormalizedBenchmarksTest do
  @moduledoc """
  Normalized benchmarks for Oban
  Adjusts for batch size, concurrency, and work per job
  """

  use ExUnit.Case
  alias ObanBenchmark.Repo
  alias ObanBenchmark.Worker
  alias ObanBenchmark.BenchmarkHelpers

  setup do
    BenchmarkHelpers.truncate_jobs()
    :ok
  end

  @tag :benchmark
  test "run normalized benchmark suite" do
    IO.puts("\n")
    IO.puts("╔══════════════════════════════════════════════════════════╗")
    IO.puts("║      OBAN NORMALIZED BENCHMARK SUITE                     ║")
    IO.puts("╠══════════════════════════════════════════════════════════╣")
    IO.puts("║  Database: PostgreSQL (localhost)                        ║")
    IO.puts("║  Normalized for: concurrency, batch size, work load      ║")
    IO.puts("╚══════════════════════════════════════════════════════════╝")
    IO.puts("")

    # Test 1: Single worker, no work (pure overhead)
    iterations = 100
    {elapsed, _} =
      :timer.tc(fn ->
        for _ <- 1..iterations do
          BenchmarkHelpers.insert_single_job()
        end
      end)

    elapsed_sec = elapsed / 1_000_000
    jobs_per_sec = iterations / elapsed_sec
    latency_ms = elapsed / 1000 / iterations

    IO.puts("=== Single Worker, No Work (Pure Overhead) ===")
    IO.puts("  Jobs: #{iterations} | Workers: 1 | Work: 0ms")
    IO.puts("  Throughput: #{Float.round(jobs_per_sec, 1)} jobs/sec")
    IO.puts("  Latency: #{Float.round(latency_ms, 2)} ms/job")
    IO.puts("  Normalized: #{Float.round(jobs_per_sec, 1)} jobs/sec/worker")
    IO.puts("")

    # Test 2: Single worker, 10ms work (sample)
    work_ms = 10
    {sample_elapsed, _} =
      :timer.tc(fn ->
        BenchmarkHelpers.insert_single_job(work_ms)
        :timer.sleep(work_ms)
      end)

    _estimated_total_ms = sample_elapsed / 1000 * iterations
    estimated_jobs_per_sec = 1000 / (sample_elapsed / 1000)

    IO.puts("=== Single Worker, 10ms Work ===")
    IO.puts("  Jobs: #{iterations} | Workers: 1 | Work: #{work_ms}ms")
    IO.puts("  Sample Duration: #{Float.round(sample_elapsed / 1_000_000, 3)}s")
    IO.puts("  Estimated Throughput: #{Float.round(estimated_jobs_per_sec, 1)} jobs/sec")
    IO.puts("  Estimated Latency: #{Float.round(sample_elapsed / 1000, 2)} ms/job")
    IO.puts("")

    # Test 3: 10 workers, 10ms work
    total_jobs = 100
    concurrency = 10

    # Pre-populate
    for _ <- 1..total_jobs do
      BenchmarkHelpers.insert_single_job(work_ms)
    end

    # Start Oban
    case Oban.start_link(
           engine: Oban.Engines.Basic,
           queues: [default: concurrency],
           repo: ObanBenchmark.Repo,
           plugins: false
         ) do
      {:ok, _pid} -> :ok
      {:error, {:already_started, _pid}} -> :ok
    end

    {elapsed, _} =
      :timer.tc(fn ->
        wait_until_completed(total_jobs, 30_000)
      end)

    elapsed_sec = elapsed / 1_000_000
    jobs_per_sec = total_jobs / elapsed_sec
    per_worker = jobs_per_sec / concurrency
    latency_ms = (elapsed / 1000) / total_jobs

    IO.puts("=== 10 Workers, 10ms Work ===")
    IO.puts("  Jobs: #{total_jobs} | Workers: #{concurrency} | Work: 10ms")
    IO.puts("  Duration: #{Float.round(elapsed_sec, 3)}s")
    IO.puts("  Throughput: #{Float.round(jobs_per_sec, 1)} jobs/sec (total)")
    IO.puts("  Throughput: #{Float.round(per_worker, 1)} jobs/sec/worker")
    IO.puts("  Latency: #{Float.round(latency_ms, 2)} ms/job (avg)")
    IO.puts("")

    # Test 4: Bulk insert
    batch_size = 1000
    {elapsed, _} =
      :timer.tc(fn ->
        jobs =
          for i <- 0..(batch_size - 1) do
            %{args: %{"job_num" => i}} |> Worker.new()
          end

        Oban.insert_all(jobs)
      end)

    elapsed_sec = elapsed / 1_000_000
    jobs_per_sec = batch_size / elapsed_sec
    latency_us = elapsed / batch_size

    IO.puts("=== Bulk Insert (1000 jobs, single transaction) ===")
    IO.puts("  Jobs: #{batch_size} | Batch: 1 transaction")
    IO.puts("  Duration: #{Float.round(elapsed_sec, 3)}s")
    IO.puts("  Throughput: #{Float.round(jobs_per_sec, 0)} jobs/sec")
    IO.puts("  Latency: #{Float.round(latency_us, 1)} µs/job (marginal cost)")
    IO.puts("")

    # Summary
    IO.puts("╔══════════════════════════════════════════════════════════╗")
    IO.puts("║                    SUMMARY                               ║")
    IO.puts("╠══════════════════════════════════════════════════════════╣")
    IO.puts("║  Metric                          │ Value                 ║")
    IO.puts("╠══════════════════════════════════╪═══════════════════════╣")
    IO.puts("║  Single insert (no work)         │ #{Float.round(jobs_per_sec, 0)} jobs/sec        ║")
    IO.puts("║  Single worker (10ms work)       │ ~#{Float.round(estimated_jobs_per_sec, 0)} jobs/sec        ║")
    IO.puts("║  Per-worker throughput           │ #{Float.round(per_worker, 1)} jobs/sec/worker  ║")
    IO.puts("║  Bulk insert marginal cost       │ #{Float.round(latency_us, 1)} µs/job          ║")
    IO.puts("╚══════════════════════════════════════════════════════════╝")
  end

  defp wait_until_completed(target_count, timeout, start_time \\ nil) do
    start_time = start_time || System.monotonic_time(:millisecond)
    elapsed = System.monotonic_time(:millisecond) - start_time

    cond do
      elapsed >= timeout ->
        :timeout

      BenchmarkHelpers.completed_jobs_count() >= target_count ->
        :ok

      true ->
        :timer.sleep(10)
        wait_until_completed(target_count, timeout, start_time)
    end
  end
end
