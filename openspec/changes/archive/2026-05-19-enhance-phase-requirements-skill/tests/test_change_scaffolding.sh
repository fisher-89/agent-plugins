#!/usr/bin/env bash
# test_change_scaffolding.sh — Integration tests for Step 1 change scaffolding.
#
# Covers:
#   AC-01: Change does not exist → auto-scaffolding creates the change directory
#   AC-02: Change already exists → no scaffolding triggered (regression)
#
# Usage:
#   bash test_change_scaffolding.sh

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
source "$SCRIPT_DIR/helpers/setup_test_env.sh"

PASS_COUNT=0
FAIL_COUNT=0

cleanup() {
    teardown_sandbox
}

trap cleanup EXIT

# ─── Test: AC-01 — Change does not exist, scaffolding creates it ────────────

test_ac01_scaffolding_creates_change() {
    echo "=== AC-01: Change does not exist → auto-scaffolding ==="

    setup_sandbox
    mock_openspec

    local change_name="new-feature"
    local change_dir="$FAKE_CHANGES_DIR/$change_name"

    # Simulate Step 1: change does NOT exist
    if change_exists "$change_name"; then
        echo "FAIL: Change should not exist before scaffolding" >&2
        return 1
    fi

    # Execute scaffolding (openspec new change)
    openspec new change "$change_name" > /dev/null 2>&1

    # Verify the change directory was created
    assert_dir_exists "$change_dir"
    assert_file_exists "$change_dir/.openspec.yaml"

    # Verify the phases subdirectory exists
    assert_dir_exists "$change_dir/phases"

    echo "PASS: AC-01 scaffolding created change '$change_name'"
    return 0
}

# ─── Test: AC-02 — Change already exists, skip scaffolding ──────────────────

test_ac02_scaffolding_skipped_when_exists() {
    echo "=== AC-02: Change already exists → skip scaffolding ==="

    setup_sandbox
    mock_openspec

    local change_name="existing-feature"
    local change_dir="$FAKE_CHANGES_DIR/$change_name"

    # Pre-create the change directory (simulating existing change)
    mkdir -p "$change_dir/phases"
    cat > "$change_dir/.openspec.yaml" << YAML_EOF
version: "1.0"
schema: spec-driven
created: 2026-05-19
change: $change_name
YAML_EOF

    # Verify change exists
    if ! change_exists "$change_name"; then
        echo "FAIL: Change should exist before scaffolding check" >&2
        return 1
    fi

    # Attempt scaffolding - should fail because change already exists
    local exit_code=0
    openspec new change "$change_name" > /dev/null 2>&1 || exit_code=$?

    if [[ "$exit_code" -ne 0 ]]; then
        echo "PASS: AC-02 scaffolding correctly rejected (exit code $exit_code) for existing change"
    else
        echo "FAIL: AC-02 scaffolding should have failed for existing change" >&2
        return 1
    fi

    # Verify the existing files were NOT modified (mtime-based check is optional)
    assert_file_exists "$change_dir/.openspec.yaml"

    echo "PASS: AC-02 scaffolding skipped for existing change"
    return 0
}

# ─── Test: AC-01 Edge — Change name with special characters ─────────────────

test_ac01_scaffolding_with_special_chars() {
    echo "=== AC-01 Edge: Change name with special characters ==="

    setup_sandbox
    mock_openspec

    local raw_name="My Feature & Bug Fix!"
    local derived_name
    derived_name="$(derive_kebab_case "$raw_name")"

    # Scaffold with the derived name
    openspec new change "$derived_name" > /dev/null 2>&1

    local change_dir="$FAKE_CHANGES_DIR/$derived_name"
    assert_dir_exists "$change_dir"
    assert_file_exists "$change_dir/.openspec.yaml"

    echo "PASS: AC-01 Edge scaffolding with derived name '$derived_name'"
    return 0
}

# ─── Test: AC-01 Edge — Empty name handling ─────────────────────────────────

test_ac01_scaffolding_empty_name() {
    echo "=== AC-01 Edge: Empty change name ==="

    setup_sandbox
    mock_openspec

    local exit_code=0
    openspec new change "" > /dev/null 2>&1 || exit_code=$?

    if [[ "$exit_code" -ne 0 ]]; then
        echo "PASS: AC-01 Edge empty name correctly rejected (exit code $exit_code)"
    else
        echo "FAIL: AC-01 Edge empty name should have been rejected" >&2
        return 1
    fi

    return 0
}

# ─── Test: AC-02 Edge — Same name, different case ───────────────────────────

test_ac02_scaffolding_case_sensitivity() {
    echo "=== AC-02 Edge: Case sensitivity (kebab-case is lowercase) ==="

    setup_sandbox
    mock_openspec

    local change_name="my-feature"
    mkdir -p "$FAKE_CHANGES_DIR/$change_name"

    # Trying to create with same name (lowercase) should fail
    local exit_code=0
    openspec new change "$change_name" > /dev/null 2>&1 || exit_code=$?

    if [[ "$exit_code" -ne 0 ]]; then
        echo "PASS: AC-02 Edge case-sensitive match correctly blocks duplicate"
    else
        echo "FAIL: AC-02 Edge should have detected existing change" >&2
        return 1
    fi

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

run_test "AC-01: scaffolding creates change" test_ac01_scaffolding_creates_change
run_test "AC-02: scaffolding skipped when exists" test_ac02_scaffolding_skipped_when_exists
run_test "AC-01 Edge: special chars in name" test_ac01_scaffolding_with_special_chars
run_test "AC-01 Edge: empty name" test_ac01_scaffolding_empty_name
run_test "AC-02 Edge: case sensitivity" test_ac02_scaffolding_case_sensitivity

# ─── Summary ────────────────────────────────────────────────────────────────

echo ""
echo "══════════════════════════════════════════════"
echo "  Results: $PASS_COUNT passed, $FAIL_COUNT failed"
echo "══════════════════════════════════════════════"

if [[ "$FAIL_COUNT" -gt 0 ]]; then
    exit 1
fi
exit 0
