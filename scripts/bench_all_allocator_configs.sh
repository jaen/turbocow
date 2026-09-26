#!/bin/bash

# Phase 1 Allocator Performance Benchmark Suite
# Compares turbocow performance across all allocator configurations

echo "=== Phase 1 Allocator Performance Benchmarks ==="
echo "Testing all allocator feature configurations..."
echo ""

# Color codes for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Function to run benchmark and report results
run_benchmark() {
  local features="$1"
  local description="$2"
  local baseline="$3"

  echo -e "${YELLOW}Running: $description${NC}"
  echo "Features: ${features:-none}"
  echo "----------------------------------------"

  if [ -z "$features" ]; then
    # No features (default)
    if cargo bench --bench allocator_performance -- --baseline "$baseline" 2>&1 | tee "bench_results_default.txt"; then
      echo -e "${GREEN}✓ $description completed successfully${NC}"
    else
      echo -e "${RED}✗ $description failed${NC}"
    fi
  else
    if cargo bench --bench allocator_performance --features "$features" -- --baseline "$baseline" 2>&1 | tee "bench_results_${features//,/_}.txt"; then
      echo -e "${GREEN}✓ $description completed successfully${NC}"
    else
      echo -e "${RED}✗ $description failed${NC}"
    fi
  fi
  echo ""
}

# Ensure we're in the right directory
cd "$(dirname "$0")" || exit 1

echo "Step 1: Running baseline benchmark with upstream ecow..."
cargo bench --bench allocator_baseline -- --save-baseline phase1_baseline
echo ""

echo "Step 2: Testing turbocow with different allocator configurations..."

# 1. Default (no allocator features) - establishes turbocow baseline
echo -e "${YELLOW}Configuration 1: Default (no allocator)${NC}"
cargo bench --bench allocator_performance -- --save-baseline turbocow_default
echo ""

# 2. With allocator-api2 v0.2
run_benchmark "alloc,allocator-api2-v02" "Configuration 2: allocator-api2 v0.2" "turbocow_default"

# 3. With allocator-api2 v0.3
run_benchmark "alloc,allocator-api2-v03" "Configuration 3: allocator-api2 v0.3" "turbocow_default"

# 4. With nightly allocator API (requires nightly toolchain)
echo -e "${YELLOW}Configuration 4: Nightly allocator API${NC}"
if rustc +nightly --version >/dev/null 2>&1; then
  if cargo +nightly bench --bench allocator_performance --features "nightly-allocator-api" -- --baseline turbocow_default 2>&1 | tee "bench_results_nightly.txt"; then
    echo -e "${GREEN}✓ Nightly configuration completed successfully${NC}"
  else
    echo -e "${RED}✗ Nightly configuration failed${NC}"
  fi
else
  echo -e "${YELLOW}Nightly toolchain not available, skipping...${NC}"
fi
echo ""

echo "=== Benchmark Summary ==="
echo "Results saved to bench_results_*.txt files"
echo ""

# Parse and display key metrics
echo "Performance Regression Analysis:"
echo "--------------------------------"
for result_file in bench_results_*.txt; do
  if [ -f "$result_file" ]; then
    echo "File: $result_file"
    grep -E "Performance has (improved|regressed)" "$result_file" | head -5
    echo ""
  fi
done

echo -e "${GREEN}Phase 1 benchmark suite completed!${NC}"
