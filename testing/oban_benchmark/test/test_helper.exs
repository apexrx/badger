ExUnit.start()

# Configure test timeout for benchmarks
ExUnit.configure(timeout: 120_000)

# Start the Repo for tests
{:ok, _} = ObanBenchmark.Repo.start_link()
