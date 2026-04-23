#!/usr/bin/env bash
# Run tests repeatedly to detect flakes.
# Usage: scripts/test-runner.sh [iterations] [filter]
#   iterations  - number of repetitions (default: 20)
#   filter      - cargo test filter (default: run all)

set -euo pipefail

ITERATIONS="${1:-20}"
FILTER="${2:-}"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
NC='\033[0m'

echo "Running tests ${ITERATIONS}x${FILTER:+ (filter: $FILTER)}"
echo "================================================================"

FAILURES=0
TOTAL=0

for i in $(seq 1 "$ITERATIONS"); do
    TOTAL=$((TOTAL + 1))
    OUTPUT=$(cargo test $FILTER 2>&1 || true)

    # Check the last "test result" line (integration tests)
    LAST_RESULT=$(echo "$OUTPUT" | grep "test result:" | tail -1)

    if echo "$LAST_RESULT" | grep -q "0 failed"; then
        STATUS="PASS"
        COLOR="$GREEN"
    else
        FAILURES=$((FAILURES + 1))
        STATUS="FAIL"
        COLOR="$RED"
        echo -e "  ${COLOR}${STATUS}${NC} #${i}: ${LAST_RESULT}"
        echo "$OUTPUT" | grep -E "FAILED|panicked" | head -3
        continue
    fi

    if [ $((i % 5)) -eq 0 ] || [ "$i" -eq "$ITERATIONS" ]; then
        echo -e "  ${COLOR}${STATUS}${NC} #${i}"
    fi
done

echo "================================================================"
if [ "$FAILURES" -gt 0 ]; then
    echo -e "${RED}${FAILURES}/${TOTAL} runs failed${NC}"
    exit 1
else
    echo -e "${GREEN}${FAILURES}/${TOTAL} runs failed (${ITERATIONS} iterations)${NC}"
fi
