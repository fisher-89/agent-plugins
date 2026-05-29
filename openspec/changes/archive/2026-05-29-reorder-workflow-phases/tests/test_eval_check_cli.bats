#!/usr/bin/env bats
# test_eval_check_cli.bats — Integration tests for eval-check phase gating logic (AC-9, AC-10)
#
# Covers:
#   AC-9:  eval-check --phase 02-dev-design correctly gates on 01-requirements
#   AC-10: eval-check --phase 03-test-design correctly gates on 01-requirements + 02-dev-design
#   Boundary: old phase identifiers, missing eval.json, nonexistent change, full chain
#
# Usage:
#   bats test_eval_check_cli.bats
#
# Design principle: Tests invoke the standalone test_eval_check_helper.cjs which
# replicates the core phase-gating logic from workflow.ts + eval-check.ts.
# This avoids calling the MCP server binary (dev-team-mcp.cjs) which is a
# JSON-RPC server that ignores CLI arguments.

setup() {
    load "helpers/setup_test_env.sh"
    setup_sandbox

    # Locate the helper script relative to this test file
    local test_dir
    test_dir="$(cd "$(dirname "${BATS_TEST_FILENAME}")" && pwd)"
    export HELPER_SCRIPT="$test_dir/test_eval_check_helper.cjs"
}

teardown() {
    teardown_sandbox
}

# ─── Helper: run the eval-check helper ──────────────────────────────────────

run_eval_check() {
    local phase="$1"
    run node "$HELPER_SCRIPT" \
        --change "$TEST_CHANGE" \
        --phase "$phase" \
        --project-root "$SANDBOX_DIR"
}

# ─── AC-9: eval-check for 02-dev-design ──────────────────────────────────────

@test "AC-9: eval-check 02-dev-design passes when 01-requirements has pass" {
    setup_eval_fixture "eval_requirements_pass.json"
    run_eval_check "02-dev-design"
    [[ "$status" -eq 0 ]]
}

@test "AC-9: eval-check 02-dev-design fails when 01-requirements has no pass" {
    setup_eval_fixture "eval_requirements_fail.json"
    run_eval_check "02-dev-design"
    [[ "$status" -eq 1 ]]
}

@test "AC-9: eval-check 02-dev-design fails when eval.json is empty" {
    setup_eval_fixture "eval_empty.json"
    run_eval_check "02-dev-design"
    [[ "$status" -eq 1 ]]
}

@test "AC-9: eval-check 02-dev-design error mentions 01-requirements as missing" {
    setup_eval_fixture "eval_empty.json"
    run_eval_check "02-dev-design"
    [[ "$status" -eq 1 ]]
    assert_string_contains "$output" "01-requirements"
}

@test "AC-9: eval-check 02-dev-design passes when requirements has both fail and pass entries" {
    # Custom eval.json with a fail entry followed by a pass entry for 01-requirements
    cat > "$TEST_CHANGE_DIR/eval.json" << 'EOF'
[
  {
    "phase": "01-requirements",
    "timestamp": "2026-05-28T10:00:00.000Z",
    "attempt": 1,
    "verdict": "fail",
    "report": "First attempt failed.",
    "items": [],
    "backtrack_to": null,
    "schema_version": "1.0"
  },
  {
    "phase": "01-requirements",
    "timestamp": "2026-05-28T11:00:00.000Z",
    "attempt": 2,
    "verdict": "pass",
    "report": "Second attempt passed.",
    "items": [],
    "backtrack_to": null,
    "schema_version": "1.0"
  }
]
EOF
    run_eval_check "02-dev-design"
    [[ "$status" -eq 0 ]]
}

# ─── AC-10: eval-check for 03-test-design ─────────────────────────────────────

@test "AC-10: eval-check 03-test-design passes when 01-requirements and 02-dev-design both pass" {
    setup_eval_fixture "eval_dev_design_pass.json"
    run_eval_check "03-test-design"
    [[ "$status" -eq 0 ]]
}

@test "AC-10: eval-check 03-test-design fails when only 01-requirements passes" {
    setup_eval_fixture "eval_requirements_pass.json"
    run_eval_check "03-test-design"
    [[ "$status" -eq 1 ]]
}

@test "AC-10: eval-check 03-test-design error mentions 02-dev-design as missing" {
    # Fixture eval_requirements_pass.json has 01-requirements with pass verdict.
    # Only 02-dev-design should be reported as missing since 01-requirements is present.
    setup_eval_fixture "eval_requirements_pass.json"
    run_eval_check "03-test-design"
    [[ "$status" -eq 1 ]]
    assert_string_contains "$output" "02-dev-design"
}

@test "AC-10: eval-check 03-test-design fails when eval.json is empty" {
    setup_eval_fixture "eval_empty.json"
    run_eval_check "03-test-design"
    [[ "$status" -eq 1 ]]
}

# ─── AC-10: eval-check for 09-acceptance (full chain validation) ──────────────

@test "AC-10: eval-check 09-acceptance passes when all 8 prior phases pass" {
    # Build a full eval.json with all 8 prior phases passing
    cat > "$TEST_CHANGE_DIR/eval.json" << 'EOF'
[
  {"phase":"01-requirements","timestamp":"2026-05-28T10:00:00.000Z","attempt":1,"verdict":"pass","report":"","items":[],"backtrack_to":null,"schema_version":"1.0"},
  {"phase":"02-dev-design","timestamp":"2026-05-28T11:00:00.000Z","attempt":1,"verdict":"pass","report":"","items":[],"backtrack_to":null,"schema_version":"1.0"},
  {"phase":"03-test-design","timestamp":"2026-05-28T12:00:00.000Z","attempt":1,"verdict":"pass","report":"","items":[],"backtrack_to":null,"schema_version":"1.0"},
  {"phase":"04-test-gen","timestamp":"2026-05-28T13:00:00.000Z","attempt":1,"verdict":"pass","report":"","items":[],"backtrack_to":null,"schema_version":"1.0"},
  {"phase":"05-implement","timestamp":"2026-05-28T14:00:00.000Z","attempt":1,"verdict":"pass","report":"","items":[],"backtrack_to":null,"schema_version":"1.0"},
  {"phase":"06-unit-test","timestamp":"2026-05-28T15:00:00.000Z","attempt":1,"verdict":"pass","report":"","items":[],"backtrack_to":null,"schema_version":"1.0"},
  {"phase":"07-code-review","timestamp":"2026-05-28T16:00:00.000Z","attempt":1,"verdict":"pass","report":"","items":[],"backtrack_to":null,"schema_version":"1.0"},
  {"phase":"08-integration-test","timestamp":"2026-05-28T17:00:00.000Z","attempt":1,"verdict":"pass","report":"","items":[],"backtrack_to":null,"schema_version":"1.0"}
]
EOF
    run_eval_check "09-acceptance"
    [[ "$status" -eq 0 ]]
}

# ─── Boundary: old phase identifiers ─────────────────────────────────────────

@test "Boundary: eval-check with old 03-dev-proposal identifier returns empty prior phases" {
    # getPriorPhases("03-dev-proposal") returns [] (fault-tolerant, unknown phase).
    # No prior phases means the gate trivially passes.
    setup_eval_fixture "eval_empty.json"
    run node "$HELPER_SCRIPT" \
        --change "$TEST_CHANGE" \
        --phase "03-dev-proposal" \
        --project-root "$SANDBOX_DIR"

    [[ "$status" -eq 0 ]]
}

@test "Boundary: eval-check with old 02-test-design identifier returns empty prior phases" {
    setup_eval_fixture "eval_empty.json"
    run node "$HELPER_SCRIPT" \
        --change "$TEST_CHANGE" \
        --phase "02-test-design" \
        --project-root "$SANDBOX_DIR"

    [[ "$status" -eq 0 ]]
}

# ─── Boundary: eval.json with old identifiers (backward compatibility) ────────

@test "Boundary: eval.json with old phase identifiers still allows gate check on new phases" {
    # Fixture eval_old_identifiers.json has entries with old identifiers
    # ("02-test-design" instead of "02-dev-design"). The gate for 02-dev-design
    # checks only 01-requirements (the sole prior phase), which IS present with
    # verdict "pass" in the fixture. So the gate trivially passes.
    setup_eval_fixture "eval_old_identifiers.json"

    run node "$HELPER_SCRIPT" \
        --change "$TEST_CHANGE" \
        --phase "02-dev-design" \
        --project-root "$SANDBOX_DIR"

    [[ "$status" -eq 0 ]]
}

# ─── Boundary: eval.json does not exist ──────────────────────────────────────

@test "Boundary: eval-check fails gracefully when eval.json does not exist" {
    rm -f "$TEST_CHANGE_DIR/eval.json"

    run_eval_check "02-dev-design"
    [[ "$status" -eq 1 ]]
}

# ─── Boundary: nonexistent change directory ──────────────────────────────────

@test "Boundary: eval-check fails gracefully for nonexistent change" {
    run node "$HELPER_SCRIPT" \
        --change "nonexistent-change" \
        --phase "02-dev-design" \
        --project-root "$SANDBOX_DIR"

    [[ "$status" -eq 1 ]]
}
