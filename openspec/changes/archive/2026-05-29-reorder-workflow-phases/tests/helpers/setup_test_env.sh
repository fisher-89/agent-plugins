#!/usr/bin/env bash
# setup_test_env.sh — Shared test helpers for reorder-workflow-phases tests.
#
# Provides:
#   setup_sandbox()            — Create a temporary sandbox directory
#   teardown_sandbox()         — Remove the sandbox directory
#   setup_dev_team_cli()       — Locate dev-team CLI bundle for testing
#   setup_eval_fixture()       — Copy a fixture eval.json into the sandbox change dir
#   assert_dir_exists()        — Assert a directory exists, fail otherwise
#   assert_file_exists()       — Assert a file exists, fail otherwise
#   assert_exit_code()         — Assert the last command's exit code matches expected
#   assert_string_contains()   — Assert a string contains a substring
#   assert_string_not_contains() — Assert a string does NOT contain a substring
#
# Usage:
#   load "helpers/setup_test_env.sh"
#   setup_sandbox
#   # ... run tests ...
#   teardown_sandbox

set -euo pipefail

# ─── Sandbox management ──────────────────────────────────────────────────────

SANDBOX_DIR=""

setup_sandbox() {
    SANDBOX_DIR="$(mktemp -d -t reorder_phases_test_XXXXXX)"
    export SANDBOX_DIR

    # Set up a fake changes directory
    export FAKE_CHANGES_DIR="$SANDBOX_DIR/openspec/changes"
    mkdir -p "$FAKE_CHANGES_DIR"

    # Create a test change directory
    export TEST_CHANGE="test-reorder-fixture"
    export TEST_CHANGE_DIR="$FAKE_CHANGES_DIR/$TEST_CHANGE"
    mkdir -p "$TEST_CHANGE_DIR"

    # Store last exit code for assert_exit_code
    export __LAST_EXIT_CODE=0
}

teardown_sandbox() {
    if [[ -n "$SANDBOX_DIR" && -d "$SANDBOX_DIR" ]]; then
        rm -rf "$SANDBOX_DIR"
    fi
    SANDBOX_DIR=""
}

# ─── Dev-team CLI setup ──────────────────────────────────────────────────────

setup_dev_team_cli() {
    local bundle_path

    # Look for the built dev-team-bundle.js in the project
    local project_root
    project_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../../../../plugins/dev-team/bin" && pwd 2>/dev/null || true)"

    if [[ -f "$project_root/dev-team-bundle.js" ]]; then
        bundle_path="$project_root/dev-team-bundle.js"
    elif [[ -f "$project_root/dev-team-bundle.cjs" ]]; then
        bundle_path="$project_root/dev-team-bundle.cjs"
    else
        # Try alternate path
        project_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../../../plugins/dev-team/bin" && pwd 2>/dev/null || true)"
        if [[ -f "$project_root/dev-team-bundle.js" ]]; then
            bundle_path="$project_root/dev-team-bundle.js"
        else
            echo "FAIL: dev-team-bundle.js not found. Build the project first with 'cd plugins/dev-team/bin && npm run build'." >&2
            exit 1
        fi
    fi

    export DEV_TEAM_BUNDLE="$bundle_path"
    echo "PASS: Using dev-team bundle at $bundle_path"
}

# ─── Fixture helpers ─────────────────────────────────────────────────────────

setup_eval_fixture() {
    local fixture_name="$1"

    local fixtures_dir
    fixtures_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/../fixtures" && pwd 2>/dev/null || true)"

    local fixture_file="$fixtures_dir/$fixture_name"
    if [[ ! -f "$fixture_file" ]]; then
        echo "FAIL: Fixture not found: $fixture_file" >&2
        exit 1
    fi

    cp "$fixture_file" "$TEST_CHANGE_DIR/eval.json"
    echo "PASS: Loaded fixture '$fixture_name' into $TEST_CHANGE_DIR/eval.json"
}

# ─── Assertions ──────────────────────────────────────────────────────────────

assert_dir_exists() {
    local path="$1"
    if [[ ! -d "$path" ]]; then
        echo "FAIL: Directory does not exist: $path" >&2
        exit 1
    fi
    echo "PASS: Directory exists: $path"
}

assert_file_exists() {
    local path="$1"
    if [[ ! -f "$path" ]]; then
        echo "FAIL: File does not exist: $path" >&2
        exit 1
    fi
    echo "PASS: File exists: $path"
}

assert_exit_code() {
    local expected="$1"
    local actual="${__LAST_EXIT_CODE:-$?}"

    if [[ "$actual" -eq "$expected" ]]; then
        echo "PASS: Exit code $expected as expected"
    else
        echo "FAIL: Expected exit code $expected, got $actual" >&2
        exit 1
    fi
}

assert_string_contains() {
    local haystack="$1"
    local needle="$2"

    if [[ "$haystack" == *"$needle"* ]]; then
        echo "PASS: String contains '$needle'"
    else
        echo "FAIL: String does not contain '$needle'" >&2
        echo "  String: $haystack" >&2
        exit 1
    fi
}

assert_string_not_contains() {
    local haystack="$1"
    local needle="$2"

    if [[ "$haystack" != *"$needle"* ]]; then
        echo "PASS: String does not contain '$needle'"
    else
        echo "FAIL: String contains '$needle' (should not)" >&2
        exit 1
    fi
}
