#!/usr/bin/env bash
# test_explore_context.sh — Integration tests for explore context handling.
#
# Covers:
#   AC-04: Explore context is extracted and passed to Planner
#   Edge: Empty explore context
#   Edge: Explore context exceeds 20KB (truncation)
#
# Usage:
#   bash test_explore_context.sh

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
source "$SCRIPT_DIR/helpers/setup_test_env.sh"

PASS_COUNT=0
FAIL_COUNT=0

cleanup() {
    teardown_sandbox
}

trap cleanup EXIT

# ─── Helper: Simulate explore context extraction ────────────────────────────

# Simulates what SKILL.md Step 1 does: read explore output and extract key insights.
extract_explore_context() {
    local explore_file="$1"
    local max_bytes="${2:-10240}"

    if [[ ! -f "$explore_file" ]]; then
        echo ""
        return
    fi

    local content
    content="$(cat "$explore_file")"

    if [[ -z "$content" ]]; then
        echo ""
        return
    fi

    # Truncate if exceeds max_bytes
    if [[ ${#content} -gt "$max_bytes" ]]; then
        content="${content:0:$max_bytes}"
        content="$content\n\n（以下内容已截断，共 $(wc -l < "$explore_file" 2>/dev/null || echo 0) 行）"
    fi

    echo "$content"
}

# ─── Test: AC-04 — Extract explore context ──────────────────────────────────

test_ac04_extract_explore_context() {
    echo "=== AC-04: Extract explore context from file ==="

    setup_sandbox

    local explore_file="$SCRIPT_DIR/fixtures/explore_output.md"
    local context
    context="$(extract_explore_context "$explore_file")"

    # Should contain key markers from the explore output
    assert_string_contains "$context" "What We Figured Out"
    assert_string_contains "$context" "JWT"
    assert_string_contains "$context" "Design Choices"

    echo "PASS: AC-04 extracted explore context"
    return 0
}

# ─── Test: AC-04 — Context contains key decisions ───────────────────────────

test_ac04_context_contains_decisions() {
    echo "=== AC-04: Explore context contains key decisions ==="

    setup_sandbox

    local explore_file="$SCRIPT_DIR/fixtures/explore_output.md"
    local context
    context="$(extract_explore_context "$explore_file")"

    # Should contain decision and analysis content
    assert_string_contains "$context" "Decision 1"
    assert_string_contains "$context" "Decision 2"
    assert_string_contains "$context" "Options Considered"

    echo "PASS: AC-04 context contains key decisions"
    return 0
}

# ─── Test: AC-04 Edge — Empty explore context ───────────────────────────────

test_ac04_empty_context() {
    echo "=== AC-04 Edge: Empty explore context ==="

    setup_sandbox

    local empty_file="$SANDBOX_DIR/empty_explore.md"
    touch "$empty_file"

    local context
    context="$(extract_explore_context "$empty_file")"

    if [[ -z "$context" ]]; then
        echo "PASS: AC-04 Edge empty context returns empty string"
    else
        echo "FAIL: Empty explore file should produce empty context" >&2
        return 1
    fi

    return 0
}

# ─── Test: AC-04 Edge — Missing explore file ────────────────────────────────

test_ac04_missing_explore_file() {
    echo "=== AC-04 Edge: Missing explore file ==="

    setup_sandbox

    local context
    context="$(extract_explore_context "$SANDBOX_DIR/nonexistent.md")"

    if [[ -z "$context" ]]; then
        echo "PASS: AC-04 Edge missing file returns empty string"
    else
        echo "FAIL: Missing explore file should produce empty context" >&2
        return 1
    fi

    return 0
}

# ─── Test: AC-04 Edge — Large context truncation ────────────────────────────

test_ac04_large_context_truncation() {
    echo "=== AC-04 Edge: Large explore context truncated at 10KB ==="

    setup_sandbox

    # Create a large explore output (>20KB)
    local large_file="$SANDBOX_DIR/large_explore.md"
    local line
    for i in $(seq 1 500); do
        echo "Line $i: This is a simulated explore output line containing some decision or analysis text for testing truncation behavior."
    done > "$large_file"

    local file_size
    file_size="$(wc -c < "$large_file")"

    if [[ "$file_size" -le 10240 ]]; then
        echo "SKIP: Test file too small for truncation test ($file_size bytes)"
        return 0
    fi

    local context
    context="$(extract_explore_context "$large_file" 10240)"

    local context_size=${#context}
    if [[ "$context_size" -le 10240 ]]; then
        echo "PASS: AC-04 Edge context truncated to $context_size bytes (max 10240)"
    else
        echo "FAIL: Context size $context_size exceeds 10240 byte limit" >&2
        return 1
    fi

    # Should contain truncation notice
    assert_string_contains "$context" "已截断"

    echo "PASS: AC-04 Edge truncation marker present"
    return 0
}

# ─── Test: AC-04 Edge — Context just under limit not truncated ──────────────

test_ac04_context_under_limit() {
    echo "=== AC-04 Edge: Context just under limit is not truncated ==="

    setup_sandbox

    local small_file="$SANDBOX_DIR/small_explore.md"
    echo "Small decision: use JWT." > "$small_file"

    local context
    context="$(extract_explore_context "$small_file" 10240)"

    if [[ "$context" == *"已截断"* ]]; then
        echo "FAIL: Context under limit should not contain truncation marker" >&2
        return 1
    fi

    echo "PASS: AC-04 Edge context under limit not truncated"
    return 0
}

# ─── Runner ─────────────────────────────────────────────────────────────────

run_test() {
    local test_name="$1"
    shift
    echo ""
    echo "──────────────────────────────────────────────"
    echo "  Running: $test_name"
    echo "──────────────────────────────────────────────"

    if "$@"; then
        PASS_COUNT=$((PASS_COUNT + 1))
    else
        FAIL_COUNT=$((FAIL_COUNT + 1))
        echo "FAIL: $test_name" >&2
    fi
}

run_test "AC-04: extract explore context" test_ac04_extract_explore_context
run_test "AC-04: context contains decisions" test_ac04_context_contains_decisions
run_test "AC-04 Edge: empty context" test_ac04_empty_context
run_test "AC-04 Edge: missing file" test_ac04_missing_explore_file
run_test "AC-04 Edge: large context truncation" test_ac04_large_context_truncation
run_test "AC-04 Edge: context under limit" test_ac04_context_under_limit

# ─── Summary ────────────────────────────────────────────────────────────────

echo ""
echo "══════════════════════════════════════════════"
echo "  Results: $PASS_COUNT passed, $FAIL_COUNT failed"
echo "══════════════════════════════════════════════"

if [[ "$FAIL_COUNT" -gt 0 ]]; then
    exit 1
fi
exit 0
