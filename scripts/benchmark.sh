#!/bin/bash
# CRT Terminal Benchmark Script
#
# Usage:
#   ./scripts/benchmark.sh              # Run all benchmarks
#   ./scripts/benchmark.sh quick        # Quick CPU-only benchmark
#   ./scripts/benchmark.sh gpu          # Full GPU benchmark (opens window)
#   ./scripts/benchmark.sh memory       # Memory monitoring

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"

cd "$PROJECT_DIR"

echo "CRT Terminal Benchmark Suite"
echo "============================"
echo ""

case "${1:-all}" in
    quick)
        echo "Running quick CPU-side benchmark..."
        cargo run --release --bin benchmark
        ;;

    gpu)
        echo "Running GPU benchmark with frame timing..."
        echo "Will open terminal window. Use Cmd+Q to exit."
        echo ""
        CRT_BENCHMARK=1 cargo run --release 2>&1 | tee benchmark_results.txt &
        PID=$!

        echo "Terminal running with PID $PID"
        echo "Run test commands in the terminal to stress test."
        echo "Frame timings are logged to stderr."
        echo ""
        echo "Suggested stress tests:"
        echo "  yes                      # Continuous scrolling"
        echo "  ls -laR /                # Color output + scrolling"
        echo "  cat /dev/urandom | xxd   # Heavy output"
        echo ""
        wait $PID
        ;;

    memory)
        echo "Running memory monitoring..."
        echo "Starting terminal in background..."

        cargo run --release &
        PID=$!
        sleep 2

        echo "Monitoring memory for PID $PID..."
        echo "Time(s), RSS(MB)"

        START=$(date +%s)
        while kill -0 $PID 2>/dev/null; do
            NOW=$(date +%s)
            ELAPSED=$((NOW - START))
            RSS=$(ps -o rss= -p $PID 2>/dev/null | tr -d ' ')
            if [ -n "$RSS" ]; then
                RSS_MB=$(echo "scale=1; $RSS / 1024" | bc)
                echo "$ELAPSED, $RSS_MB"
            fi
            sleep 1
        done
        ;;

    stress)
        echo "Running automated stress test..."
        echo "Starting terminal with benchmark mode..."

        CRT_BENCHMARK=1 cargo run --release &
        PID=$!
        sleep 3

        echo "Sending stress commands via osascript..."

        # Send test input (macOS only)
        osascript -e '
            tell application "System Events"
                delay 1
                keystroke "yes | head -n 10000"
                keystroke return
                delay 5
                keystroke "q" using command down
            end tell
        ' 2>/dev/null || echo "osascript not available, please run commands manually"

        wait $PID || true
        echo "Stress test complete."
        ;;

    all)
        echo "Running full benchmark suite..."
        echo ""

        echo "=== CPU-side Benchmark ==="
        cargo run --release --bin benchmark
        echo ""

        echo "=== GPU Benchmark ==="
        echo "Starting terminal with benchmark mode for 10 seconds..."
        timeout 10 bash -c 'CRT_BENCHMARK=1 cargo run --release 2>&1' || true
        echo ""

        echo "Benchmark suite complete!"
        ;;

    *)
        echo "Usage: $0 [quick|gpu|memory|stress|all]"
        echo ""
        echo "  quick   - CPU-only benchmark (fast, no window)"
        echo "  gpu     - Full GPU benchmark (opens window)"
        echo "  memory  - Memory usage monitoring"
        echo "  stress  - Automated stress test (macOS)"
        echo "  all     - Run all benchmarks"
        exit 1
        ;;
esac
