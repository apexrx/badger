defmodule ObanBenchmark.HighLoadBenchmarksTest do
  @moduledoc """
  High load benchmarks for Oban
  Stress testing under heavy load conditions
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
  test "high load benchmark - 1000 jobs" do
    total_jobs = 1000
    concurrency = 10
    work_ms = 10

    IO.puts("\n=== High Load Benchmark (#{total_jobs} jobs) ===")
    IO.puts("Concurrency: #{concurrency}")
    IO.puts("Work per job: #{work_ms}ms")

    # Pre-populate jobs
    {insert_time, _} =
      :timer.tc(fn ->
        for _ <- 1..total_jobs do
          BenchmarkHelpers.insert_single_job(work_ms)
        end
      end)

    IO.puts("Insert duration: #{Float.round(insert_time / 1_000_000, 3)}s")
    IO.puts("Insert throughput: #{Float.round(total_jobs / (insert_time / 1_000_000), 2)} jobs/sec")

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

    # Process jobs
    {process_time, _} =
      :timer.tc(fn ->
        wait_until_completed(total_jobs, 120_000)
      end)

    elapsed_sec = process_time / 1_000_000
    jobs_per_sec = total_jobs / elapsed_sec

    IO.puts("Process duration: #{Float.round(elapsed_sec, 3)}s")
    IO.puts("Process throughput: #{Float.round(jobs_per_sec, 2)} jobs/sec")
    IO.puts("Per-worker throughput: #{Float.round(jobs_per_sec / concurrency, 2)} jobs/sec/worker")
  end

  @tag :benchmark
  test "high load benchmark - 5000 jobs" do
    total_jobs = 5000
    concurrency = 10
    work_ms = 10

    IO.puts("\n=== High Load Benchmark (#{total_jobs} jobs) ===")
    IO.puts("Concurrency: #{concurrency}")
    IO.puts("Work per job: #{work_ms}ms")

    # Pre-populate jobs
    {insert_time, _} =
      :timer.tc(fn ->
        for _ <- 1..total_jobs do
          BenchmarkHelpers.insert_single_job(work_ms)
        end
      end)

    IO.puts("Insert duration: #{Float.round(insert_time / 1_000_000, 3)}s")
    IO.puts("Insert throughput: #{Float.round(total_jobs / (insert_time / 1_000_000), 2)} jobs/sec")

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

    # Process jobs
    {process_time, _} =
      :timer.tc(fn ->
        wait_until_completed(total_jobs, 300_000)
      end)

    elapsed_sec = process_time / 1_000_000
    jobs_per_sec = total_jobs / elapsed_sec

    IO.puts("Process duration: #{Float.round(elapsed_sec, 3)}s")
    IO.puts("Process throughput: #{Float.round(jobs_per_sec, 2)} jobs/sec")
    IO.puts("Per-worker throughput: #{Float.round(jobs_per_sec / concurrency, 2)} jobs/sec/worker")
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
