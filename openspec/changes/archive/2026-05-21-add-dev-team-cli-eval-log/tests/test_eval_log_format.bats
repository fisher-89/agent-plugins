#!/usr/bin/env bats
# test_eval_log_format.bats — Integration tests for JSON format and indentation (AC-13)
#
# Covers:
#   AC-13: eval.json is valid JSON with 2-space indentation
#   AC-13: Invalid existing eval.json produces error
#   AC-13: Output confirmation JSON is valid
#
# Usage:
#   bats test_eval_log_format.bats

setup() {
    load "helpers/setup_test_env.sh"
    setup_sandbox
    setup_dev_team_cli
}

teardown() {
    teardown_sandbox
}

# ─── AC-13: JSON format and indentation ─────────────────────────────────────

@test "AC-13: eval.json is valid JSON" {
    rm -f "$TEST_PHASES_DIR/eval.json"

    node "$DEV_TEAM_BUNDLE" eval-log \
        --change "$TEST_CHANGE" \
        --phase 01-requirements \
        --verdict pass \
        --report "Format test." \
        --items '[]'

    run python3 -c "import json; json.load(open('$TEST_PHASES_DIR/eval.json'))"
    [[ "$status" -eq 0 ]]
}

@test "AC-13: eval.json root is a JSON array" {
    rm -f "$TEST_PHASES_DIR/eval.json"

    node "$DEV_TEAM_BUNDLE" eval-log \
        --change "$TEST_CHANGE" \
        --phase 01-requirements \
        --verdict pass \
        --report "Array check." \
        --items '[]'

    run python3 -c "import json; d=json.load(open('$TEST_PHASES_DIR/eval.json')); assert isinstance(d, list), 'Root is not a list'"
    [[ "$status" -eq 0 ]]
}

@test "AC-13: eval.json uses 2-space indentation" {
    rm -f "$TEST_PHASES_DIR/eval.json"

    node "$DEV_TEAM_BUNDLE" eval-log \
        --change "$TEST_CHANGE" \
        --phase 01-requirements \
        --verdict pass \
        --report "Indent check." \
        --items '[]'

    # Check that the file starts with "[" on its own line and each field is indented by 2 spaces
    local first_data_line
    first_data_line="$(grep -v '^\[' "$TEST_PHASES_DIR/eval.json" | grep -v '^\]' | head -1)"
    # The first non-array-bracket line should start with 2 spaces
    [[ "$first_data_line" == "  "* ]]
}

@test "AC-13: eval.json has no tab characters" {
    rm -f "$TEST_PHASES_DIR/eval.json"

    node "$DEV_TEAM_BUNDLE" eval-log \
        --change "$TEST_CHANGE" \
        --phase 01-requirements \
        --verdict pass \
        --report "No tabs." \
        --items '[]'

    run grep -c $'\t' "$TEST_PHASES_DIR/eval.json"
    # Should have 0 tabs
    [[ "$output" -eq 0 ]]
}

@test "AC-13: eval.json ends with newline" {
    rm -f "$TEST_PHASES_DIR/eval.json"

    node "$DEV_TEAM_BUNDLE" eval-log \
        --change "$TEST_CHANGE" \
        --phase 01-requirements \
        --verdict pass \
        --report "Newline check." \
        --items '[]'

    # Last character should be newline
    local last_char
    last_char="$(tail -c 1 "$TEST_PHASES_DIR/eval.json")"
    [[ "$last_char" == $'\n' ]]
}

# ─── AC-13: Invalid eval.json handling ───────────────────────────────────────

@test "AC-13: eval-log fails when existing eval.json contains invalid JSON" {
    echo '{invalid-json' > "$TEST_PHASES_DIR/eval.json"

    run node "$DEV_TEAM_BUNDLE" eval-log \
        --change "$TEST_CHANGE" \
        --phase 01-requirements \
        --verdict pass \
        --report "Invalid JSON test." \
        --items '[]'

    [[ "$status" -eq 1 ]]
}

@test "AC-13: eval-log stderr mentions parse error for invalid JSON" {
    echo '{invalid-json' > "$TEST_PHASES_DIR/eval.json"

    run node "$DEV_TEAM_BUNDLE" eval-log \
        --change "$TEST_CHANGE" \
        --phase 01-requirements \
        --verdict pass \
        --report "Invalid JSON test." \
        --items '[]'

    [[ "$status" -eq 1 ]]
    # stderr should mention JSON parse error
    assert_string_contains "$output" "parse" || assert_string_contains "$output" "JSON" || assert_string_contains "$output" "SyntaxError"
}

# ─── Output format ──────────────────────────────────────────────────────────

@test "AC-13: stdout output is valid JSON" {
    rm -f "$TEST_PHASES_DIR/eval.json"

    run node "$DEV_TEAM_BUNDLE" eval-log \
        --change "$TEST_CHANGE" \
        --phase 01-requirements \
        --verdict pass \
        --report "Output format." \
        --items '[]'

    [[ "$status" -eq 0 ]]
    # The output should contain valid JSON
    run python3 -c "import json; json.loads('''$output''')"
    [[ "$status" -eq 0 ]]
}

@test "AC-13: success JSON contains written=true field" {
    rm -f "$TEST_PHASES_DIR/eval.json"

    run node "$DEV_TEAM_BUNDLE" eval-log \
        --change "$TEST_CHANGE" \
        --phase 01-requirements \
        --verdict pass \
        --report "Output format." \
        --items '[]'

    [[ "$status" -eq 0 ]]
    local written
    written="$(python3 -c "import json; d=json.loads('''$output'''); print(d.get('written', False))")"
    [[ "$written" == "True" ]]
}

@test "AC-13: success JSON contains phase and attempt fields" {
    rm -f "$TEST_PHASES_DIR/eval.json"

    run node "$DEV_TEAM_BUNDLE" eval-log \
        --change "$TEST_CHANGE" \
        --phase 01-requirements \
        --verdict pass \
        --report "Output format." \
        --items '[]'

    [[ "$status" -eq 0 ]]
    python3 -c "
import json
d = json.loads('''$output''')
assert 'phase' in d, 'Missing phase field'
assert 'attempt' in d, 'Missing attempt field'
assert d['phase'] == '01-requirements'
assert d['attempt'] == 1
"
}
