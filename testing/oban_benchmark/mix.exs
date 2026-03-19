defmodule ObanBenchmark.MixProject do
  use Mix.Project

  def project do
    [
      app: :oban_benchmark,
      version: "0.1.0",
      elixir: "~> 1.19",
      start_permanent: Mix.env() == :prod,
      deps: deps(),
      elixirc_paths: elixirc_paths(Mix.env())
    ]
  end

  defp elixirc_paths(:test), do: ["lib", "test/support"]
  defp elixirc_paths(_), do: ["lib"]

  def application do
    [
      extra_applications: [:logger],
      mod: {ObanBenchmark.Application, []}
    ]
  end

  defp deps do
    [
      {:oban, "~> 2.17"},
      {:postgrex, ">= 0.0.0"},
      {:ecto_sql, ">= 3.0.0"},
      {:jason, "~> 1.4"},
      {:telemetry, "~> 1.2"},
      {:benchee, "~> 1.3", only: :dev},
      {:benchee_markdown, "~> 0.3", only: :dev}
    ]
  end
end
