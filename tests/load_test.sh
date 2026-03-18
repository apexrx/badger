#!/usr/bin/env bash
# Load testing script for Badger
# This script stress-tests the Badger HTTP API

set -e

BADGER_URL="${BADGER_URL:-http://localhost:3000}"
NUM_JOBS="${NUM_JOBS:-100}"
CONCURRENT="${CONCURRENT:-10}"

echo "========================================"
echo "  Badger Load Test"
echo "========================================"
echo "Target URL: $BADGER_URL"
echo "Jobs to submit: $NUM_JOBS"
echo "Concurrency: $CONCURRENT"
echo "========================================"

# Check if Badger is running
echo ""
echo "[1/4] Checking Badger availability..."
if ! curl -s "$BADGER_URL/metrics" > /dev/null 2>&1; then
    echo "ERROR: Badger is not running at $BADGER_URL"
    echo "Start it with: cargo run"
    exit 1
fi
echo "✓ Badger is running"

# Submit jobs concurrently
echo ""
echo "[2/4] Submitting $NUM_JOBS jobs (concurrency: $CONCURRENT)..."
START_TIME=$(date +%s.%N)

# Create a temporary file for job IDs
JOB_IDS=$(mktemp)

# Submit jobs in batches
submit_jobs() {
    local batch_size=$1
    local batch_num=$2
    
    for i in $(seq 1 $batch_size); do
        local job_num=$((batch_num * batch_size + i))
        local response=$(curl -s -X POST "$BADGER_URL/jobs" \
            -H "Content-Type: application/json" \
            -d "{
                \"url\": \"https://httpbin.org/status/200\",
                \"method\": \"GET\",
                \"headers\": {\"X-Job-ID\": \"$job_num\"},
                \"body\": {\"test\": \"load_test\", \"iteration\": $job_num}
            }")
        echo "$response" >> "$JOB_IDS"
    done
}

# Run concurrent batches
for batch in $(seq 0 $(( (NUM_JOBS / CONCURRENT) - 1 ))); do
    submit_jobs $CONCURRENT $batch &
done

# Wait for all background jobs
wait

END_TIME=$(date +%s.%N)
DURATION=$(echo "$END_TIME - $START_TIME" | bc)
JOBS_PER_SEC=$(echo "scale=2; $NUM_JOBS / $DURATION" | bc)

echo "✓ Submitted $NUM_JOBS jobs in ${DURATION}s (${JOBS_PER_SEC} jobs/sec)"

# Check job status
echo ""
echo "[3/4] Checking job statuses..."
SUCCESS_COUNT=0
PENDING_COUNT=0
RUNNING_COUNT=0
FAILURE_COUNT=0

# Sample a few jobs
SAMPLE_SIZE=10
if [ "$NUM_JOBS" -lt "$SAMPLE_SIZE" ]; then
    SAMPLE_SIZE=$NUM_JOBS
fi

for i in $(seq 1 $SAMPLE_SIZE); do
    JOB_ID=$(sed -n "${i}p" "$JOB_IDS" | tr -d '\n"')
    if [ -n "$JOB_ID" ]; then
        STATUS=$(curl -s "$BADGER_URL/jobs/$JOB_ID" | grep -o '"status":"[^"]*"' | cut -d'"' -f4 || echo "UNKNOWN")
        case "$STATUS" in
            "Success") ((SUCCESS_COUNT++)) ;;
            "Pending") ((PENDING_COUNT++)) ;;
            "Running") ((RUNNING_COUNT++)) ;;
            "Failure") ((FAILURE_COUNT++)) ;;
            *) ;;
        esac
    fi
done

echo "  Sample results (first $SAMPLE_SIZE jobs):"
echo "    - Success: $SUCCESS_COUNT"
echo "    - Pending: $PENDING_COUNT"
echo "    - Running: $RUNNING_COUNT"
echo "    - Failure: $FAILURE_COUNT"

# Get metrics
echo ""
echo "[4/4] Fetching Prometheus metrics..."
METRICS=$(curl -s "$BADGER_URL/metrics")

JOB_QUEUE_DEPTH=$(echo "$METRICS" | grep "job_queue_depth" | grep -v "#" | awk '{print $2}' || echo "N/A")
echo "  - Queue depth: $JOB_QUEUE_DEPTH"

EXECUTION_COUNT=$(echo "$METRICS" | grep "job_execution_result" | grep -v "#" | awk '{sum += $2} END {print sum}' || echo "N/A")
echo "  - Total executions: $EXECUTION_COUNT"

# Cleanup
rm -f "$JOB_IDS"

echo ""
echo "========================================"
echo "  Load Test Complete"
echo "========================================"
echo "Summary:"
echo "  - Total jobs submitted: $NUM_JOBS"
echo "  - Duration: ${DURATION}s"
echo "  - Throughput: ${JOBS_PER_SEC} jobs/sec"
echo "========================================"
