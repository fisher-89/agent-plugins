#!/usr/bin/env bats
# test_eval_log_gate.bats — Integration tests for phase gating logic (AC-7, AC-8, AC-9)
#
# Covers:
#   AC-7: Gate rejects when prior phase is missing pass verdict
#   AC-8: Gate allows when all prior phases have pass
#   AC-9: First phase (01-requirements) has no gate check
#   Boundary: multiple phases with mixed pass/fail, empty eval.json, absent prior phases
#
# Usage:
#   bats test_eval_log_gate.bats

setup() {
    load "helpers/setup_test_env.sh"
    setup_sandbox
    setup_dev_team_cli
}

teardown() {
    teardown_sandbox
}

# ─── AC-9: First phase no gate (01-requirements) ────────────────────────────

@test "AC-9: eval-log for 01-requirements succeeds with empty eval.json" {
    # Empty eval.json — this is the first phase, so no gate check
    echo '[]' > "$TEST_PHASES_DIR/eval.json"

    run node "$DEV_TEAM_BUNDLE" eval-log \
        --change "$TEST_CHANGE" \
        --phase 01-requirements \
        --verdict pass \
        --report "First phase always allowed." \
        --items '[]'

    [[ "$status" -eq 0 ]]
}

@test "AC-9: eval-log for 01-requirements succeeds with no eval.json" {
    rm -f "$TEST_PHASES_DIR/eval.json"

    run node "$DEV_TEAM_BUNDLE" eval-log \
        --change "$TEST_CHANGE" \
        --phase 01-requirements \
        --verdict pass \
        --report "First phase, no prior eval.json." \
        --items '[]'

    [[ "$status" -eq 0 ]]
}

# ─── AC-7: Gate rejects when prior phase missing pass ────────────────────────

@test "AC-7: eval-log for 02-test-design fails when 01-requirements has no pass" {
    # Load fixture where 01-requirements has only fail entries
    setup_eval_log_fixture "eval_single_phase_fail.json"

    run node "$DEV_TEAM_BUNDLE" eval-log \
        --change "$TEST_CHANGE" \
        --phase 02-test-design \
        --verdict pass \
        --report "Should be rejected by gate." \
        --items '[]'

    [[ "$status" -eq 1 ]]
}

@test "AC-7: error message mentions the failing phase name" {
    setup_eval_log_fixture "eval_single_phase_fail.json"

    run node "$DEV_TEAM_BUNDLE" eval-log \
        --change "$TEST_CHANGE" \
        --phase 02-test-design \
        --verdict pass \
        --report "Should be rejected." \
        --items '[]'

    [[ "$status" -eq 1 ]]
    assert_string_contains "$output" "01-requirements"
}

@test "AC-7: eval-log for 03-dev-proposal fails when 02-test-design has only fail" {
    setup_eval_log_fixture "eval_multi_phase_mixed.json"  # 01 pass, 02 fail

    run node "$DEV_TEAM_BUNDLE" eval-log \
        --change "$TEST_CHANGE" \
        --phase 03-dev-proposal \
        --verdict pass \
        --report "Should be rejected." \
        --items '[]'

    [[ "$status" -eq 1 ]]
    assert_string_contains "$output" "02-test-design"
}

@test "AC-7: empty eval.json causes all prior phases to be reported as missing" {
    echo '[]' > "$TEST_PHASES_DIR/eval.json"

    run node "$DEV_TEAM_BUNDLE" eval-log \
        --change "$TEST_CHANGE" \
        --phase 03-dev-proposal \
        --verdict pass \
        --report "Should be rejected." \
        --items '[]'

    [[ "$status" -eq 1 ]]
    # Should mention both prior phases
    assert_string_contains "$output" "01-requirements"
    assert_string_contains "$output" "02-test-design"
}

# ─── AC-8: Gate allows when all prior phases have pass ──────────────────────

@test "AC-8: eval-log for 02-test-design succeeds when 01-requirements has pass" {
    setup_eval_log_fixture "eval_single_phase_pass.json"

    run node "$DEV_TEAM_BUNDLE" eval-log \
        --change "$TEST_CHANGE" \
        --phase 02-test-design \
        --verdict pass \
        --report "Gate allows: prior phase has pass." \
        --items '[]'

    [[ "$status" -eq 0 ]]
}

@test "AC-8: eval-log for 03-dev-proposal succeeds when all prior phases have pass" {
    setup_eval_log_fixture "eval_multi_phase_pass.json"

    run node "$DEV_TEAM_BUNDLE" eval-log \
        --change "$TEST_CHANGE" \
        --phase 03-dev-proposal \
        --verdict pass \
        --report "Gate allows: both prior phases have pass." \
        --items '[]'

    [[ "$status" -eq 0 ]]
}

@test "AC-8: eval-log for 07-acceptance succeeds when all prior phases have pass" {
    # Build a full eval.json with all 6 prior phases passing
    python3 -c "
import json
entries = []
for i, phase in enumerate(['01-requirements', '02-test-design', '03-dev-proposal',
                            '04-test-gen', '05-implementation', '06-code-review']):
    entries.append({
        'phase': phase,
        'timestamp': '2026-05-20T10:00:00.000Z',
        'attempt': 1,
        'verdict': 'pass',
        'report': f'{phase} passed.',
        'items': [],
        'backtrack_to': None,
        'schema_version': '1.0'
    })
with open('$TEST_PHASES_DIR/eval.json', 'w') as f:
    json.dump(entries, f, indent=2)
"

    run node "$DEV_TEAM_BUNDLE" eval-log \
        --change "$TEST_CHANGE" \
        --phase 07-acceptance \
        --verdict pass \
        --report "All prior phases pass." \
        --items '[]'

    [[ "$status" -eq 0 ]]
}

# ─── Boundary: gate with fail + pass in same prior phase ────────────────────

@test "AC-8: gate allows when prior phase has both fail and pass entries" {
    # Create eval.json with a fail entry and later a pass entry for same phase
    python3 -c "
import json
entries = [
    {
        'phase': '01-requirements',
        'timestamp': '2026-05-20T10:00:00.000Z',
        'attempt': 1,
        'verdict': 'fail',
        'report': 'First attempt failed.',
        'items': [],
        'backtrack_to': None,
        'schema_version': '1.0'
    },
    {
        'phase': '01-requirements',
        'timestamp': '2026-05-20T11:00:00.000Z',
        'attempt': 2,
        'verdict': 'pass',
        'report': 'Second attempt passed.',
        'items': [],
        'backtrack_to': None,
        'schema_version': '1.0'
    }
]
with open('$TEST_PHASES_DIR/eval.json', 'w') as f:
    json.dump(entries, f, indent=2)
"

    run node "$DEV_TEAM_BUNDLE" eval-log \
        --change "$TEST_CHANGE" \
        --phase 02-test-design \
        --verdict pass \
        --report "Prior phase eventually passed." \
        --items '[]'

    [[ "$status" -eq 0 ]]
}
