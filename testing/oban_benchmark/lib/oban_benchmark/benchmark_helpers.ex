defmodule ObanBenchmark.BenchmarkHelpers do
  @moduledoc """
  Helper functions for benchmarking Oban
  """

  alias ObanBenchmark.Repo
  alias ObanBenchmark.Worker
  import Ecto.Query

  @doc """
  Truncate the jobs table for clean benchmarks
  """
  def truncate_jobs do
    Repo.query!("TRUNCATE TABLE oban_jobs RESTART IDENTITY CASCADE;")
  end

  @doc """
  Insert a single job
  """
  def insert_single_job(work_ms \\ 10) do
    %{args: %{"work_ms" => work_ms}}
    |> Worker.new()
    |> Oban.insert()
  end

  @doc """
  Insert multiple jobs
  """
  def insert_jobs(count, work_ms \\ 10) do
    jobs =
      for _ <- 1..count do
        %{args: %{"work_ms" => work_ms}}
        |> Worker.new()
      end

    Oban.insert_all(jobs)
  end

  @doc """
  Get pending jobs count
  """
  def pending_jobs_count do
    from(j in "oban_jobs", where: j.state == "available")
    |> Repo.aggregate(:count, :id)
  end

  @doc """
  Get completed jobs count
  """
  def completed_jobs_count do
    from(j in "oban_jobs", where: j.state == "completed")
    |> Repo.aggregate(:count, :id)
  end
end
