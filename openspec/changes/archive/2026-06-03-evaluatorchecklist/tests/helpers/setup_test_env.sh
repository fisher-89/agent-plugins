#!/usr/bin/env bash
# setup_test_env.sh — Shared test helpers for evaluatorchecklist content verification tests.
#
# This change is a static Markdown prompt file edit (no runtime code).
# Tests verify file content properties using grep/awk/sed/diff.
#
# Provides:
#   setup_test_env()        — Set AGENTS_DIR and verify it exists
#   teardown_test_env()     — Cleanup (placeholder for future use)
#   assert_file_contains()  — Assert a file contains a pattern (grep)
#   assert_file_not_contains() — Assert a file does NOT contain a pattern
#   assert_grep_match()     — Run grep on a file and assert match count > 0
#   assert_grep_no_match()  — Run grep on a file and assert match count = 0
#   get_agent_files()       — Return all 7 evaluator agent file paths
#
# Usage:
#   source "$(dirname "$0")/helpers/setup_test_env.sh"
#   setup_test_env

set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../../../.." && pwd)"
AGENTS_DIR="$PROJECT_ROOT/plugins/dev-team/agents"

# ─── Test Environment Setup ──────────────────────────────────────────────────

setup_test_env() {
    if [[ ! -d "$AGENTS_DIR" ]]; then
        echo "FATAL: Agents directory not found: $AGENTS_DIR" >&2
        exit 1
    fi
    echo "Test env ready: AGENTS_DIR=$AGENTS_DIR"
}

teardown_test_env() {
    # Placeholder for cleanup if needed in the future
    :
}

# ─── Agent file path helpers ─────────────────────────────────────────────────

# Returns the 7 evaluator agent file paths (excludes unit-test and integration-test)
get_agent_files() {
    for f in \
        proposal-evaluator.md \
        test-design-evaluator.md \
        dev-design-evaluator.md \
        code-review-evaluator.md \
        test-gen-evaluator.md \
        implementation-evaluator.md \
        acceptance-evaluator.md; do
        echo "$AGENTS_DIR/$f"
    done
}

# Returns all 9 evaluator agent files (including unit-test and integration-test)
get_all_evaluator_files() {
    for f in \
        proposal-evaluator.md \
        test-design-evaluator.md \
        dev-design-evaluator.md \
        code-review-evaluator.md \
        test-gen-evaluator.md \
        implementation-evaluator.md \
        acceptance-evaluator.md \
        unit-test-evaluator.md \
        integration-test-evaluator.md; do
        echo "$AGENTS_DIR/$f"
    done
}

# ─── Assertions ──────────────────────────────────────────────────────────────

assert_file_contains() {
    local file="$1"
    local pattern="$2"
    local label="${3:-}"

    if [[ ! -f "$file" ]]; then
        echo "FAIL: File not found: $file" >&2
        exit 1
    fi

    if grep -q "$pattern" "$file"; then
        if [[ -n "$label" ]]; then
            echo "PASS [$label]: Pattern found in $(basename "$file"): $pattern"
        else
            echo "PASS: Pattern found in $(basename "$file"): $pattern"
        fi
    else
        if [[ -n "$label" ]]; then
            echo "FAIL [$label]: Pattern NOT found in $(basename "$file"): $pattern" >&2
        else
            echo "FAIL: Pattern NOT found in $(basename "$file"): $pattern" >&2
        fi
        exit 1
    fi
}

assert_file_not_contains() {
    local file="$1"
    local pattern="$2"
    local label="${3:-}"

    if [[ ! -f "$file" ]]; then
        echo "FAIL: File not found: $file" >&2
        exit 1
    fi

    if grep -q "$pattern" "$file"; then
        if [[ -n "$label" ]]; then
            echo "FAIL [$label]: Pattern FOUND (should not) in $(basename "$file"): $pattern" >&2
        else
            echo "FAIL: Pattern FOUND (should not) in $(basename "$file"): $pattern" >&2
        fi
        exit 1
    else
        if [[ -n "$label" ]]; then
            echo "PASS [$label]: Pattern correctly absent from $(basename "$file"): $pattern"
        else
            echo "PASS: Pattern correctly absent from $(basename "$file"): $pattern"
        fi
    fi
}

assert_grep_match() {
    local file="$1"
    local pattern="$2"
    local label="${3:-}"

    if [[ ! -f "$file" ]]; then
        echo "FAIL: File not found: $file" >&2
        exit 1
    fi

    local count
    count="$(grep -c "$pattern" "$file" 2>/dev/null || echo 0)"
    if [[ "$count" -gt 0 ]]; then
        if [[ -n "$label" ]]; then
            echo "PASS [$label]: grep found $count match(es) in $(basename "$file"): $pattern"
        else
            echo "PASS: grep found $count match(es) in $(basename "$file"): $pattern"
        fi
    else
        if [[ -n "$label" ]]; then
            echo "FAIL [$label]: grep found 0 matches in $(basename "$file"): $pattern" >&2
        else
            echo "FAIL: grep found 0 matches in $(basename "$file"): $pattern" >&2
        fi
        exit 1
    fi
}

assert_grep_no_match() {
    local file="$1"
    local pattern="$2"
    local label="${3:-}"

    if [[ ! -f "$file" ]]; then
        echo "FAIL: File not found: $file" >&2
        exit 1
    fi

    local count
    count="$(grep -c "$pattern" "$file" 2>/dev/null || echo 0)"
    if [[ "$count" -eq 0 ]]; then
        if [[ -n "$label" ]]; then
            echo "PASS [$label]: grep correctly found 0 matches in $(basename "$file"): $pattern"
        else
            echo "PASS: grep correctly found 0 matches in $(basename "$file"): $pattern"
        fi
    else
        if [[ -n "$label" ]]; then
            echo "FAIL [$label]: grep found $count match(es) in $(basename "$file") (expected 0): $pattern" >&2
        else
            echo "FAIL: grep found $count match(es) in $(basename "$file") (expected 0): $pattern" >&2
        fi
        exit 1
    fi
}
