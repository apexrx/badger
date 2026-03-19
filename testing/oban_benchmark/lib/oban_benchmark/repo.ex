defmodule ObanBenchmark.Repo do
  use Ecto.Repo,
    otp_app: :oban_benchmark,
    adapter: Ecto.Adapters.Postgres

  def init(_type, config) do
    database_url = System.get_env("DATABASE_URL") || "postgresql://apex@localhost:5432/badger_db"
    {:ok, Keyword.put(config, :url, database_url)}
  end
end
