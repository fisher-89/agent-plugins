#!/usr/bin/env bats
# test_eval_log_same_phase.bats — Integration tests for same-phase rewrites (AC-10)
#
# Covers:
#   AC-10: Multiple writes to the same phase create separate entries (fail then pass)
#   Each entry retains its own verdict and report
#
# Usage:
#   bats test_eval_log_same_phase.bats

setup() {
    load "helpers/setup_test_env.sh"
    setup_sandbox
    setup_dev_team_cli
}

teardown() {
    teardown_sandbox
}

# ─── AC-10: Same phase, multiple entries ────────────────────────────────────

@test "AC-10: fail then pass write creates 2 entries in eval.json" {
    rm -f "$TEST_PHASES_DIR/eval.json"

    # First write: fail
    node "$DEV_TEAM_BUNDLE" eval-log \
        --change "$TEST_CHANGE" \
        --phase 01-requirements \
        --verdict fail \
        --report "Item R3 failed: no risk mitigation found." \
        --items '[{"item_id":"R1","pass":true,"evidence":"test","notes":""},{"item_id":"R3","pass":false,"evidence":"missing","notes":"no mitigation"}]'

    # Second write: pass
    run node "$DEV_TEAM_BUNDLE" eval-log \
        --change "$TEST_CHANGE" \
        --phase 01-requirements \
        --verdict pass \
        --report "All items pass after revision." \
        --items '[{"item_id":"R1","pass":true,"evidence":"test","notes":""},{"item_id":"R3","pass":true,"evidence":"mitigation added","notes":""}]'

    [[ "$status" -eq 0 ]]
    assert_json_array_length "$TEST_PHASES_DIR/eval.json" 2
}

@test "AC-10: first entry retains fail verdict" {
    rm -f "$TEST_PHASES_DIR/eval.json"

    node "$DEV_TEAM_BUNDLE" eval-log \
        --change "$TEST_CHANGE" \
        --phase 01-requirements \
        --verdict fail \
        --report "First attempt failed." \
        --items '[]'

    node "$DEV_TEAM_BUNDLE" eval-log \
        --change "$TEST_CHANGE" \
        --phase 01-requirements \
        --verdict pass \
        --report "Second attempt passed." \
        --items '[]'

    assert_json_array_field "$TEST_PHASES_DIR/eval.json" 0 "verdict" "fail"
}

@test "AC-10: second entry has pass verdict" {
    rm -f "$TEST_PHASES_DIR/eval.json"

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

    assert_json_array_field "$TEST_PHASES_DIR/eval.json" 1 "verdict" "pass"
}

@test "AC-10: each entry has separate timestamp" {
    rm -f "$TEST_PHASES_DIR/eval.json"

    node "$DEV_TEAM_BUNDLE" eval-log \
        --change "$TEST_CHANGE" \
        --phase 01-requirements \
        --verdict fail \
        --report "First." \
        --items '[]'

    sleep 1

    node "$DEV_TEAM_BUNDLE" eval-log \
        --change "$TEST_CHANGE" \
        --phase 01-requirements \
        --verdict pass \
        --report "Second." \
        --items '[]'

    # Read timestamps and confirm they differ
    local t0 t1
    t0="$(python3 -c "import json; d=json.load(open('$TEST_PHASES_DIR/eval.json')); print(d[0]['timestamp'])")"
    t1="$(python3 -c "import json; d=json.load(open('$TEST_PHASES_DIR/eval.json')); print(d[1]['timestamp'])")"

    [[ "$t0" != "$t1" ]]
}

@test "AC-10: three writes to same phase create three entries" {
    rm -f "$TEST_PHASES_DIR/eval.json"

    for i in 1 2 3; do
        node "$DEV_TEAM_BUNDLE" eval-log \
            --change "$TEST_CHANGE" \
            --phase 01-requirements \
            --verdict fail \
            --report "Attempt $i." \
            --items '[]'
    done

    assert_json_array_length "$TEST_PHASES_DIR/eval.json" 3
}

@test "AC-10: entries have increasing attempt numbers" {
    rm -f "$TEST_PHASES_DIR/eval.json"

    node "$DEV_TEAM_BUNDLE" eval-log \
        --change "$TEST_CHANGE" \
        --phase 01-requirements \
        --verdict fail \
        --report "Attempt 1." \
        --items '[]'

    node "$DEV_TEAM_BUNDLE" eval-log \
        --change "$TEST_CHANGE" \
        --phase 01-requirements \
        --verdict fail \
        --report "Attempt 2." \
        --items '[]'

    node "$DEV_TEAM_BUNDLE" eval-log \
        --change "$TEST_CHANGE" \
        --phase 01-requirements \
        --verdict pass \
        --report "Attempt 3." \
        --items '[]'

    # Each entry should have attempt = 1, 2, 3 respectively
    local a0 a1 a2
    a0="$(python3 -c "import json; d=json.load(open('$TEST_PHASES_DIR/eval.json')); print(d[0]['attempt'])")"
    a1="$(python3 -c "import json; d=json.load(open('$TEST_PHASES_DIR/eval.json')); print(d[1]['attempt'])")"
    a2="$(python3 -c "import json; d=json.load(open('$TEST_PHASES_DIR/eval.json')); print(d[2]['attempt'])")"

    [[ "$a0" -eq 1 ]]
    [[ "$a1" -eq 2 ]]
    [[ "$a2" -eq 3 ]]
}
