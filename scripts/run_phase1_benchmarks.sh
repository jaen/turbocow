#!/bin/bash

# Phase 1 Allocator Performance Verification
# Tests turbocow with different allocator configurations

echo "=== Phase 1 Allocator Performance Verification ==="
echo "Comparing performance across allocator configurations"
echo ""

# Color codes
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

cd "$(dirname "$0")" || exit 1

# Wait for baseline to complete if still running
echo "Waiting for any running benchmarks to complete..."
while pgrep -f "cargo bench" >/dev/null; do
  sleep 2
done

echo -e "${GREEN}Starting Phase 1 performance benchmarks...${NC}"
echo ""

# 1. Default configuration (no allocator features)
echo -e "${YELLOW}Test 1/3: Default configuration (no allocator)${NC}"
cargo bench --bench allocator_performance 2>&1 | tee bench_results_default.txt
echo ""

# 2. With allocator-api2 v0.2
echo -e "${YELLOW}Test 2/3: allocator-api2 v0.2${NC}"
cargo bench --bench allocator_performance --features "alloc,allocator-api2-v02" 2>&1 | tee bench_results_api2_v02.txt
echo ""

# 3. With allocator-api2 v0.3
echo -e "${YELLOW}Test 3/3: allocator-api2 v0.3${NC}"
cargo bench --bench allocator_performance --features "alloc,allocator-api2-v03" 2>&1 | tee bench_results_api2_v03.txt
echo ""

echo "=== Performance Summary ==="
echo ""

# Check for regressions
echo "Checking for performance regressions..."
for file in bench_results_*.txt; do
  if [ -f "$file" ]; then
    echo "Configuration: $(basename "$file" .txt)"
    if grep -q "Performance has regressed" "$file"; then
      echo -e "  ⚠️  Regressions detected:"
      grep "Performance has regressed" "$file" | head -3
    else
      echo -e "  ${GREEN}✓${NC} No regressions detected"
    fi

    # Show improvements if any
    if grep -q "Performance has improved" "$file"; then
      improvements=$(grep -c "Performance has improved" "$file")
      echo -e "  ${GREEN}↑${NC} $improvements improvements detected"
    fi
    echo ""
  fi
done

echo -e "${GREEN}Phase 1 benchmarks completed!${NC}"
echo "Detailed results saved in bench_results_*.txt files"
