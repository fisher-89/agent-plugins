#!/usr/bin/env bats
# test_eval_log_backtrack.bats — Integration tests for --backtrack-to field (AC-12)
#
# Covers:
#   AC-12: --backtrack-to value is written to the entry's backtrack_to field
#   AC-12: Same phase value as current is accepted (no semantic validation)
#   AC-12: Invalid phase name is accepted (no validation)
#
# Usage:
#   bats test_eval_log_backtrack.bats

setup() {
    load "helpers/setup_test_env.sh"
    setup_sandbox
    setup_dev_team_cli
}

teardown() {
    teardown_sandbox
}

# ─── AC-12: backtrack-to field ──────────────────────────────────────────────

@test "AC-12: --backtrack-to value is written to eval.json entry" {
    rm -f "$TEST_PHASES_DIR/eval.json"

    run node "$DEV_TEAM_BUNDLE" eval-log \
        --change "$TEST_CHANGE" \
        --phase 03-dev-proposal \
        --verdict pass \
        --report "Backtrack to test-design." \
        --items '[]' \
        --backtrack-to 02-test-design

    [[ "$status" -eq 0 ]]
    local bt
    bt="$(python3 -c "import json; d=json.load(open('$TEST_PHASES_DIR/eval.json')); print(d[0].get('backtrack_to', 'NULL'))")"
    [[ "$bt" == "02-test-design" ]]
}

@test "AC-12: --backtrack-to same as current phase is accepted" {
    rm -f "$TEST_PHASES_DIR/eval.json"

    # Need prior phases to pass gate for 04-test-gen
    python3 -c "
import json
entries = [
    {'phase': '01-requirements', 'timestamp': '2026-05-20T10:00:00.000Z', 'attempt': 1, 'verdict': 'pass', 'report': 'Pass.', 'items': [], 'backtrack_to': None, 'schema_version': '1.0'},
    {'phase': '02-test-design', 'timestamp': '2026-05-20T11:00:00.000Z', 'attempt': 1, 'verdict': 'pass', 'report': 'Pass.', 'items': [], 'backtrack_to': None, 'schema_version': '1.0'},
    {'phase': '03-dev-proposal', 'timestamp': '2026-05-20T12:00:00.000Z', 'attempt': 1, 'verdict': 'pass', 'report': 'Pass.', 'items': [], 'backtrack_to': None, 'schema_version': '1.0'}
]
with open('$TEST_PHASES_DIR/eval.json', 'w') as f:
    json.dump(entries, f, indent=2)
"

    run node "$DEV_TEAM_BUNDLE" eval-log \
        --change "$TEST_CHANGE" \
        --phase 04-test-gen \
        --verdict pass \
        --report "Backtrack to same phase." \
        --items '[]' \
        --backtrack-to 04-test-gen

    [[ "$status" -eq 0 ]]
    local bt
    bt="$(python3 -c "import json; d=json.load(open('$TEST_PHASES_DIR/eval.json')); print(d[-1].get('backtrack_to', 'NULL'))")"
    [[ "$bt" == "04-test-gen" ]]
}

@test "AC-12: --backtrack-to with invalid phase name is accepted" {
    rm -f "$TEST_PHASES_DIR/eval.json"

    run node "$DEV_TEAM_BUNDLE" eval-log \
        --change "$TEST_CHANGE" \
        --phase 01-requirements \
        --verdict pass \
        --report "Invalid backtrack target." \
        --items '[]' \
        --backtrack-to 99-nonexistent

    [[ "$status" -eq 0 ]]
    local bt
    bt="$(python3 -c "import json; d=json.load(open('$TEST_PHASES_DIR/eval.json')); print(d[0].get('backtrack_to', 'NULL'))")"
    [[ "$bt" == "99-nonexistent" ]]
}

@test "AC-12: --backtrack-to with empty string is written as empty string" {
    rm -f "$TEST_PHASES_DIR/eval.json"

    run node "$DEV_TEAM_BUNDLE" eval-log \
        --change "$TEST_CHANGE" \
        --phase 01-requirements \
        --verdict pass \
        --report "Empty backtrack." \
        --items '[]' \
        --backtrack-to ""

    [[ "$status" -eq 0 ]]
    local bt
    bt="$(python3 -c "import json; d=json.load(open('$TEST_PHASES_DIR/eval.json')); print(d[0].get('backtrack_to', 'NULL'))")"
    # Should write as the empty string or null depending on implementation
    [[ "$bt" == "" || "$bt" == "None" || "$bt" == "null" ]]
}

@test "AC-12: without --backtrack-to, field is null in entry" {
    rm -f "$TEST_PHASES_DIR/eval.json"

    run node "$DEV_TEAM_BUNDLE" eval-log \
        --change "$TEST_CHANGE" \
        --phase 01-requirements \
        --verdict pass \
        --report "No backtrack." \
        --items '[]'

    [[ "$status" -eq 0 ]]
    local bt
    bt="$(python3 -c "import json; d=json.load(open('$TEST_PHASES_DIR/eval.json')); bt = d[0].get('backtrack_to', 'MISSING'); print('null' if bt is None else bt)")"
    [[ "$bt" == "null" || "$bt" == "None" ]]
}
