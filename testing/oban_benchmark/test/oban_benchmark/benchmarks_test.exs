defmodule ObanBenchmark.BenchmarksTest do
  @moduledoc """
  Comprehensive benchmarks for Oban
  Aligned with Badger benchmark methodology for fair comparison
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
  test "benchmark single job insertion" do
    iterations = 1000

    {elapsed, _} =
      :timer.tc(fn ->
        for _ <- 1..iterations do
          BenchmarkHelpers.insert_single_job()
        end
      end)

    elapsed_sec = elapsed / 1_000_000
    jobs_per_sec = iterations / elapsed_sec
    latency_us = elapsed / iterations

    IO.puts("\n=== Single Job Insertion Benchmark ===")
    IO.puts("Iterations: #{iterations}")
    IO.puts("Duration: #{Float.round(elapsed_sec, 3)}s")
    IO.puts("Throughput: #{Float.round(jobs_per_sec, 2)} jobs/sec")
    IO.puts("Latency (avg): #{Float.round(latency_us, 2)} µs")
  end

  @tag :benchmark
  test "benchmark concurrent single job insertion" do
    iterations = 1000
    concurrency = 10
    jobs_per_worker = div(iterations, concurrency)

    {elapsed, _} =
      :timer.tc(fn ->
        tasks =
          for _ <- 1..concurrency do
            Task.async(fn ->
              for _ <- 1..jobs_per_worker do
                BenchmarkHelpers.insert_single_job()
              end
            end)
          end

        Task.await_many(tasks, 30_000)
      end)

    elapsed_sec = elapsed / 1_000_000
    jobs_per_sec = iterations / elapsed_sec

    IO.puts("\n=== Concurrent Single Job Insertion Benchmark ===")
    IO.puts("Iterations: #{iterations}")
    IO.puts("Concurrency: #{concurrency}")
    IO.puts("Duration: #{Float.round(elapsed_sec, 3)}s")
    IO.puts("Throughput: #{Float.round(jobs_per_sec, 2)} jobs/sec")
  end

  @tag :benchmark
  test "benchmark bulk job insertion" do
    total_jobs = 10_000
    batch_size = 1000

    {elapsed, _} =
      :timer.tc(fn ->
        for batch <- 0..(div(total_jobs, batch_size) - 1) do
          jobs =
            for i <- 0..(batch_size - 1) do
              job_num = batch * batch_size + i
              %{args: %{"job_num" => job_num}} |> Worker.new()
            end

          Oban.insert_all(jobs)
        end
      end)

    elapsed_sec = elapsed / 1_000_000
    jobs_per_sec = total_jobs / elapsed_sec

    IO.puts("\n=== Bulk Job Insertion Benchmark ===")
    IO.puts("Total Jobs: #{total_jobs}")
    IO.puts("Batch Size: #{batch_size}")
    IO.puts("Duration: #{Float.round(elapsed_sec, 3)}s")
    IO.puts("Throughput: #{Float.round(jobs_per_sec, 2)} jobs/sec")
  end

  @tag :benchmark
  test "benchmark concurrent bulk insertion" do
    total_jobs = 10_000
    concurrency = 10
    jobs_per_inserter = div(total_jobs, concurrency)
    batch_size = 100

    {elapsed, _} =
      :timer.tc(fn ->
        tasks =
          for batch <- 0..(concurrency - 1) do
            Task.async(fn ->
              for batch_num <- 0..(div(jobs_per_inserter, batch_size) - 1) do
                jobs =
                  for i <- 0..(batch_size - 1) do
                    job_num = batch * jobs_per_inserter + batch_num * batch_size + i
                    %{args: %{"job_num" => job_num}} |> Worker.new()
                  end

                Oban.insert_all(jobs)
              end
            end)
          end

        Task.await_many(tasks, 30_000)
      end)

    elapsed_sec = elapsed / 1_000_000
    jobs_per_sec = total_jobs / elapsed_sec

    IO.puts("\n=== Concurrent Bulk Insertion Benchmark ===")
    IO.puts("Total Jobs: #{total_jobs}")
    IO.puts("Concurrency: #{concurrency}")
    IO.puts("Batch Size: #{batch_size}")
    IO.puts("Duration: #{Float.round(elapsed_sec, 3)}s")
    IO.puts("Throughput: #{Float.round(jobs_per_sec, 2)} jobs/sec")
  end

  @tag :benchmark
  test "benchmark job processing 10ms work" do
    total_jobs = 100
    concurrency = 10
    work_ms = 10

    # Pre-populate jobs
    for _ <- 1..total_jobs do
      BenchmarkHelpers.insert_single_job(work_ms)
    end

    # Start Oban processing
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

    IO.puts("\n=== Job Processing Benchmark (10ms work) ===")
    IO.puts("Total Jobs: #{total_jobs}")
    IO.puts("Concurrency: #{concurrency}")
    IO.puts("Work per job: #{work_ms}ms")
    IO.puts("Duration: #{Float.round(elapsed_sec, 3)}s")
    IO.puts("Throughput: #{Float.round(jobs_per_sec, 2)} jobs/sec")
  end

  @tag :benchmark
  test "benchmark pure queue overhead" do
    total_jobs = 500
    concurrency = 10

    # Pre-populate jobs (no work)
    for _ <- 1..total_jobs do
      BenchmarkHelpers.insert_single_job(0)
    end

    # Start Oban processing
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

    IO.puts("\n=== Pure Queue Overhead Benchmark ===")
    IO.puts("Total Jobs: #{total_jobs}")
    IO.puts("Concurrency: #{concurrency}")
    IO.puts("Duration: #{Float.round(elapsed_sec, 3)}s")
    IO.puts("Throughput: #{Float.round(jobs_per_sec, 2)} jobs/sec")
  end

  @tag :benchmark
  test "benchmark CPU-bound processing" do
    total_jobs = 200
    concurrency = 10

    # Pre-populate jobs
    for _ <- 1..total_jobs do
      BenchmarkHelpers.insert_single_job(1)
    end

    # Start Oban processing
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

    IO.puts("\n=== CPU-Bound Processing Benchmark (~1ms CPU) ===")
    IO.puts("Total Jobs: #{total_jobs}")
    IO.puts("Concurrency: #{concurrency}")
    IO.puts("Duration: #{Float.round(elapsed_sec, 3)}s")
    IO.puts("Throughput: #{Float.round(jobs_per_sec, 2)} jobs/sec")
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
