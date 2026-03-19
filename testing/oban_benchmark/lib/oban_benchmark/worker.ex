defmodule ObanBenchmark.Worker do
  use Oban.Worker, queue: :default, max_attempts: 1

  @impl Oban.Worker
  def perform(%Oban.Job{args: %{"work_ms" => work_ms}}) do
    :timer.sleep(work_ms)
    :ok
  end

  def perform(%Oban.Job{args: _}) do
    :timer.sleep(10)
    :ok
  end
end
