#!/usr/bin/env bats
# test_eval_log_attempt.bats — Integration tests for auto attempt calculation (AC-11)
#
# Covers:
#   AC-11: --attempt auto-calculated when omitted
#   AC-11: --attempt starts at 1 for empty eval.json
#   AC-11: --attempt increments based on same-phase entries only (cross-phase isolation)
#   Explicit --attempt overrides auto calculation
#
# Usage:
#   bats test_eval_log_attempt.bats

setup() {
    load "helpers/setup_test_env.sh"
    setup_sandbox
    setup_dev_team_cli
}

teardown() {
    teardown_sandbox
}

# ─── AC-11: Auto attempt calculation ────────────────────────────────────────

@test "AC-11: first entry has attempt=1 when eval.json is empty" {
    rm -f "$TEST_PHASES_DIR/eval.json"

    run node "$DEV_TEAM_BUNDLE" eval-log \
        --change "$TEST_CHANGE" \
        --phase 01-requirements \
        --verdict pass \
        --report "First entry." \
        --items '[]'

    [[ "$status" -eq 0 ]]
    local attempt
    attempt="$(python3 -c "import json; d=json.load(open('$TEST_PHASES_DIR/eval.json')); print(d[0]['attempt'])")"
    [[ "$attempt" -eq 1 ]]
}

@test "AC-11: second entry for same phase has attempt=2" {
    rm -f "$TEST_PHASES_DIR/eval.json"

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
    local attempt
    attempt="$(python3 -c "import json; d=json.load(open('$TEST_PHASES_DIR/eval.json')); print(d[1]['attempt'])")"
    [[ "$attempt" -eq 2 ]]
}

@test "AC-11: third entry for same phase has attempt=3" {
    rm -f "$TEST_PHASES_DIR/eval.json"

    for i in 1 2 3; do
        node "$DEV_TEAM_BUNDLE" eval-log \
            --change "$TEST_CHANGE" \
            --phase 01-requirements \
            --verdict pass \
            --report "Entry $i." \
            --items '[]'
    done

    local attempt
    attempt="$(python3 -c "import json; d=json.load(open('$TEST_PHASES_DIR/eval.json')); print(d[2]['attempt'])")"
    [[ "$attempt" -eq 3 ]]
}

# ─── AC-11: Cross-phase isolation ───────────────────────────────────────────

@test "AC-11: entries in other phases do not affect current phase attempt count" {
    rm -f "$TEST_PHASES_DIR/eval.json"

    # Create 3 entries for 02-test-design (via fixture + appending)
    # First, create 01-requirements so gate allows 02-test-design
    node "$DEV_TEAM_BUNDLE" eval-log \
        --change "$TEST_CHANGE" \
        --phase 01-requirements \
        --verdict pass \
        --report "Requirements pass." \
        --items '[]'

    # Add 3 entries for 02-test-design
    for i in 1 2 3; do
        node "$DEV_TEAM_BUNDLE" eval-log \
            --change "$TEST_CHANGE" \
            --phase 02-test-design \
            --verdict pass \
            --report "Design entry $i." \
            --items '[]'
    done

    # Now add a new entry for 01-requirements — attempt should be 2 (one existing)
    run node "$DEV_TEAM_BUNDLE" eval-log \
        --change "$TEST_CHANGE" \
        --phase 01-requirements \
        --verdict pass \
        --report "Requirements second attempt." \
        --items '[]'

    [[ "$status" -eq 0 ]]
    local attempt
    attempt="$(python3 -c "import json; d=json.load(open('$TEST_PHASES_DIR/eval.json')); print(d[-1]['attempt'])")"
    [[ "$attempt" -eq 2 ]]
}

# ─── Explicit --attempt override ────────────────────────────────────────────

@test "AC-11: explicit --attempt 5 is honored even if only 1 prior entry exists" {
    rm -f "$TEST_PHASES_DIR/eval.json"

    node "$DEV_TEAM_BUNDLE" eval-log \
        --change "$TEST_CHANGE" \
        --phase 01-requirements \
        --verdict pass \
        --report "First." \
        --items '[]'

    run node "$DEV_TEAM_BUNDLE" eval-log \
        --change "$TEST_CHANGE" \
        --phase 01-requirements \
        --verdict pass \
        --report "Explicit attempt." \
        --items '[]' \
        --attempt 5

    [[ "$status" -eq 0 ]]
    local attempt
    attempt="$(python3 -c "import json; d=json.load(open('$TEST_PHASES_DIR/eval.json')); print(d[1]['attempt'])")"
    [[ "$attempt" -eq 5 ]]
}

@test "AC-11: explicit --attempt 1 on second write creates entry with attempt=1" {
    rm -f "$TEST_PHASES_DIR/eval.json"

    node "$DEV_TEAM_BUNDLE" eval-log \
        --change "$TEST_CHANGE" \
        --phase 01-requirements \
        --verdict pass \
        --report "First." \
        --items '[]'

    run node "$DEV_TEAM_BUNDLE" eval-log \
        --change "$TEST_CHANGE" \
        --phase 01-requirements \
        --verdict pass \
        --report "Explicit attempt 1." \
        --items '[]' \
        --attempt 1

    [[ "$status" -eq 0 ]]
    local attempt
    attempt="$(python3 -c "import json; d=json.load(open('$TEST_PHASES_DIR/eval.json')); print(d[1]['attempt'])")"
    [[ "$attempt" -eq 1 ]]
}

# ─── Output confirmation includes attempt number ────────────────────────────

@test "AC-11: eval-log output JSON contains correct attempt number" {
    rm -f "$TEST_PHASES_DIR/eval.json"

    run node "$DEV_TEAM_BUNDLE" eval-log \
        --change "$TEST_CHANGE" \
        --phase 01-requirements \
        --verdict pass \
        --report "Output check." \
        --items '[]'

    [[ "$status" -eq 0 ]]
    assert_string_contains "$output" '"attempt":1'
}
