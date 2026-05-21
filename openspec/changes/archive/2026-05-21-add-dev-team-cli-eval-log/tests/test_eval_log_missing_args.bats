#!/usr/bin/env bats
# test_eval_log_missing_args.bats — Integration tests for required arg validation (AC-5, AC-6, AC-13)
#
# Covers:
#   AC-5: --change missing produces error
#   AC-6: --phase missing produces error
#   Items JSON parsing errors
#
# Usage:
#   bats test_eval_log_missing_args.bats

setup() {
    load "helpers/setup_test_env.sh"
    setup_sandbox
    setup_dev_team_cli
}

teardown() {
    teardown_sandbox
}

# ─── AC-5: --change missing ─────────────────────────────────────────────────

@test "AC-5: eval-log exits with code 1 when --change is missing" {
    run node "$DEV_TEAM_BUNDLE" eval-log \
        --phase 01-requirements \
        --verdict pass \
        --report "No change arg." \
        --items '[]'

    [[ "$status" -eq 1 ]]
}

@test "AC-5: error message mentions --change when it is missing" {
    run node "$DEV_TEAM_BUNDLE" eval-log \
        --phase 01-requirements \
        --verdict pass \
        --report "No change arg." \
        --items '[]'

    [[ "$status" -eq 1 ]]
    assert_string_contains "$output" "--change"
}

# ─── AC-6: --phase missing ──────────────────────────────────────────────────

@test "AC-6: eval-log exits with code 1 when --phase is missing" {
    run node "$DEV_TEAM_BUNDLE" eval-log \
        --change "$TEST_CHANGE" \
        --verdict pass \
        --report "No phase arg." \
        --items '[]'

    [[ "$status" -eq 1 ]]
}

@test "AC-6: error message mentions --phase when it is missing" {
    run node "$DEV_TEAM_BUNDLE" eval-log \
        --change "$TEST_CHANGE" \
        --verdict pass \
        --report "No phase arg." \
        --items '[]'

    [[ "$status" -eq 1 ]]
    assert_string_contains "$output" "--phase"
}

# ─── Other required args missing ────────────────────────────────────────────

@test "eval-log exits with code 1 when --verdict is missing" {
    run node "$DEV_TEAM_BUNDLE" eval-log \
        --change "$TEST_CHANGE" \
        --phase 01-requirements \
        --report "No verdict." \
        --items '[]'

    [[ "$status" -eq 1 ]]
    assert_string_contains "$output" "--verdict"
}

@test "eval-log exits with code 1 when --report is missing" {
    run node "$DEV_TEAM_BUNDLE" eval-log \
        --change "$TEST_CHANGE" \
        --phase 01-requirements \
        --verdict pass \
        --items '[]'

    [[ "$status" -eq 1 ]]
    assert_string_contains "$output" "--report"
}

@test "eval-log exits with code 1 when --items is missing" {
    run node "$DEV_TEAM_BUNDLE" eval-log \
        --change "$TEST_CHANGE" \
        --phase 01-requirements \
        --verdict pass \
        --report "No items."

    [[ "$status" -eq 1 ]]
    assert_string_contains "$output" "--items"
}

@test "eval-log exits with code 1 when all required args are missing" {
    run node "$DEV_TEAM_BUNDLE" eval-log

    [[ "$status" -eq 1 ]]
}

# ─── Items JSON parsing errors ──────────────────────────────────────────────

@test "eval-log rejects malformed items JSON" {
    run node "$DEV_TEAM_BUNDLE" eval-log \
        --change "$TEST_CHANGE" \
        --phase 01-requirements \
        --verdict pass \
        --report "Bad items JSON." \
        --items '{bad json}'

    [[ "$status" -eq 1 ]]
}

@test "eval-log error message guides user on items JSON format" {
    run node "$DEV_TEAM_BUNDLE" eval-log \
        --change "$TEST_CHANGE" \
        --phase 01-requirements \
        --verdict pass \
        --report "Bad items JSON." \
        --items '{bad json}'

    [[ "$status" -eq 1 ]]
    assert_string_contains "$output" "JSON" || assert_string_contains "$output" "items"
}

@test "eval-log rejects items with JSON object (non-array)" {
    run node "$DEV_TEAM_BUNDLE" eval-log \
        --change "$TEST_CHANGE" \
        --phase 01-requirements \
        --verdict pass \
        --report "Object not array." \
        --items '{"key":"value"}'

    [[ "$status" -eq 1 ]]
}

# ─── --verdict validation ───────────────────────────────────────────────────

@test "eval-log rejects invalid --verdict value" {
    run node "$DEV_TEAM_BUNDLE" eval-log \
        --change "$TEST_CHANGE" \
        --phase 01-requirements \
        --verdict invalid_value \
        --report "Bad verdict." \
        --items '[]'

    [[ "$status" -eq 1 ]]
    assert_string_contains "$output" "verdict"
}

@test "eval-log rejects empty --verdict value" {
    run node "$DEV_TEAM_BUNDLE" eval-log \
        --change "$TEST_CHANGE" \
        --phase 01-requirements \
        --verdict "" \
        --report "Empty verdict." \
        --items '[]'

    [[ "$status" -eq 1 ]]
}
