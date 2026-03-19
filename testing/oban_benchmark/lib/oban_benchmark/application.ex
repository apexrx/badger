defmodule ObanBenchmark.Application do
  @moduledoc false

  use Application

  @impl true
  def start(_type, _args) do
    database_url = System.get_env("DATABASE_URL") || "postgresql://apex@localhost:5432/badger_db"

    children = [
      {Postgrex,
       name: :pg,
       hostname: "localhost",
       database: "badger_db",
       username: "apex",
       password: "",
       port: 5432},
      {Oban,
       engine: Oban.Engines.Basic,
       queues: [default: 10],
       repo: ObanBenchmark.Repo,
       plugins: false}
    ]

    opts = [strategy: :one_for_one, name: ObanBenchmark.Supervisor]
    Supervisor.start_link(children, opts)
  end
end
