#!/usr/bin/env bats
# test_eval_log_create.bats — Integration tests for eval-log create and append (AC-3, AC-4)
#
# Covers:
#   AC-3: eval.json created from scratch when phases directory exists but eval.json does not
#   AC-4: Running eval-log again appends, does not overwrite existing entries
#   Boundary: --report with 500 characters (maximum allowed)
#
# Usage:
#   bats test_eval_log_create.bats

setup() {
    load "helpers/setup_test_env.sh"
    setup_sandbox
    setup_dev_team_cli
}

teardown() {
    teardown_sandbox
}

# ─── AC-3: Create eval.json from scratch ────────────────────────────────────

@test "AC-3: eval-log creates eval.json when it does not exist" {
    # Ensure eval.json does not exist
    rm -f "$TEST_PHASES_DIR/eval.json"

    run node "$DEV_TEAM_BUNDLE" eval-log \
        --change "$TEST_CHANGE" \
        --phase 01-requirements \
        --verdict pass \
        --report "All 6 required items pass. Verdict: pass." \
        --items '[{"item_id":"R1","pass":true,"evidence":"test","notes":""}]'

    [[ "$status" -eq 0 ]]
    assert_file_exists "$TEST_PHASES_DIR/eval.json"
}

@test "AC-3: eval-log creates phases directory if missing" {
    # Remove the phases directory entirely
    rm -rf "$TEST_PHASES_DIR"

    run node "$DEV_TEAM_BUNDLE" eval-log \
        --change "$TEST_CHANGE" \
        --phase 01-requirements \
        --verdict pass \
        --report "All pass." \
        --items '[]'

    [[ "$status" -eq 0 ]]
    assert_dir_exists "$TEST_PHASES_DIR"
    assert_file_exists "$TEST_PHASES_DIR/eval.json"
}

@test "AC-3: eval-log outputs success JSON on creation" {
    rm -f "$TEST_PHASES_DIR/eval.json"

    run node "$DEV_TEAM_BUNDLE" eval-log \
        --change "$TEST_CHANGE" \
        --phase 01-requirements \
        --verdict pass \
        --report "All pass." \
        --items '[]'

    [[ "$status" -eq 0 ]]
    assert_string_contains "$output" '"written":true'
    assert_string_contains "$output" '"phase":"01-requirements"'
}

@test "AC-3: eval-log writes entry with correct phase field" {
    rm -f "$TEST_PHASES_DIR/eval.json"

    run node "$DEV_TEAM_BUNDLE" eval-log \
        --change "$TEST_CHANGE" \
        --phase 01-requirements \
        --verdict pass \
        --report "Verification complete." \
        --items '[]'

    [[ "$status" -eq 0 ]]
    assert_json_array_field "$TEST_PHASES_DIR/eval.json" 0 "phase" "01-requirements"
}

@test "AC-3: eval-log writes entry with correct verdict field" {
    rm -f "$TEST_PHASES_DIR/eval.json"

    run node "$DEV_TEAM_BUNDLE" eval-log \
        --change "$TEST_CHANGE" \
        --phase 01-requirements \
        --verdict pass \
        --report "Verification complete." \
        --items '[]'

    [[ "$status" -eq 0 ]]
    assert_json_array_field "$TEST_PHASES_DIR/eval.json" 0 "verdict" "pass"
}

@test "AC-3: eval-log writes entry with schema_version 1.0" {
    rm -f "$TEST_PHASES_DIR/eval.json"

    run node "$DEV_TEAM_BUNDLE" eval-log \
        --change "$TEST_CHANGE" \
        --phase 01-requirements \
        --verdict pass \
        --report "Verification complete." \
        --items '[]'

    [[ "$status" -eq 0 ]]
    assert_json_array_field "$TEST_PHASES_DIR/eval.json" 0 "schema_version" "1.0"
}

# ─── AC-4: Append without overwrite ─────────────────────────────────────────

@test "AC-4: eval-log appends entry to existing eval.json" {
    # Create initial entry
    node "$DEV_TEAM_BUNDLE" eval-log \
        --change "$TEST_CHANGE" \
        --phase 01-requirements \
        --verdict fail \
        --report "First attempt failed." \
        --items '[]'

    # Append second entry
    run node "$DEV_TEAM_BUNDLE" eval-log \
        --change "$TEST_CHANGE" \
        --phase 01-requirements \
        --verdict pass \
        --report "Second attempt passed." \
        --items '[]'

    [[ "$status" -eq 0 ]]
    assert_json_array_length "$TEST_PHASES_DIR/eval.json" 2
}

@test "AC-4: first entry is preserved after append" {
    node "$DEV_TEAM_BUNDLE" eval-log \
        --change "$TEST_CHANGE" \
        --phase 01-requirements \
        --verdict fail \
        --report "First attempt." \
        --items '[]'

    node "$DEV_TEAM_BUNDLE" eval-log \
        --change "$TEST_CHANGE" \
        --phase 01-requirements \
        --verdict pass \
        --report "Second attempt." \
        --items '[]'

    # First entry should still have the original verdict
    assert_json_array_field "$TEST_PHASES_DIR/eval.json" 0 "verdict" "fail"
    assert_json_array_field "$TEST_PHASES_DIR/eval.json" 0 "report" "First attempt."
}

@test "AC-4: appended entry has correct verdict" {
    node "$DEV_TEAM_BUNDLE" eval-log \
        --change "$TEST_CHANGE" \
        --phase 01-requirements \
        --verdict fail \
        --report "First attempt." \
        --items '[]'

    node "$DEV_TEAM_BUNDLE" eval-log \
        --change "$TEST_CHANGE" \
        --phase 01-requirements \
        --verdict pass \
        --report "Second attempt." \
        --items '[]'

    # Second entry (index 1) should have pass verdict
    assert_json_array_field "$TEST_PHASES_DIR/eval.json" 1 "verdict" "pass"
}

@test "AC-4: eval-log outputs success JSON on append" {
    node "$DEV_TEAM_BUNDLE" eval-log \
        --change "$TEST_CHANGE" \
        --phase 01-requirements \
        --verdict fail \
        --report "First." \
        --items '[]'

    run node "$DEV_TEAM_BUNDLE" eval-log \
        --change "$TEST_CHANGE" \
        --phase 01-requirements \
        --verdict pass \
        --report "Second." \
        --items '[]'

    [[ "$status" -eq 0 ]]
    assert_string_contains "$output" '"written":true'
}

# ─── Boundary: report with 500 characters ───────────────────────────────────

@test "AC-3: eval-log accepts report with exactly 500 characters" {
    rm -f "$TEST_PHASES_DIR/eval.json"

    local long_report
    long_report="$(python3 -c "print('A' * 500)")"

    run node "$DEV_TEAM_BUNDLE" eval-log \
        --change "$TEST_CHANGE" \
        --phase 01-requirements \
        --verdict pass \
        --report "$long_report" \
        --items '[]'

    [[ "$status" -eq 0 ]]
    assert_file_exists "$TEST_PHASES_DIR/eval.json"
}

@test "AC-3: eval-log rejects report with more than 500 characters" {
    rm -f "$TEST_PHASES_DIR/eval.json"

    local too_long_report
    too_long_report="$(python3 -c "print('A' * 501)")"

    run node "$DEV_TEAM_BUNDLE" eval-log \
        --change "$TEST_CHANGE" \
        --phase 01-requirements \
        --verdict pass \
        --report "$too_long_report" \
        --items '[]'

    [[ "$status" -eq 1 ]]
    assert_string_contains "$output" "500" || assert_string_contains "$output" "exceed"
}
