#!/usr/bin/env bats
# test_cli_wrapper.bats — Unit tests for openspec-cli.sh utility functions.
#
# These tests cover the CLI wrapper functions defined in the test design:
#   openspec_new_change, openspec_status_json, openspec_instructions,
#   derive_kebab_case, change_exists, validate_change_name
#
# Usage:
#   bats test_cli_wrapper.bats

setup() {
    load "helpers/setup_test_env.sh"
    setup_sandbox
    mock_openspec

    # Source the openspec-cli.sh script if it exists; otherwise use inline stubs
    local cli_script="../../../../plugins/dev-team/utils/openspec-cli.sh"
    if [[ -f "$cli_script" ]]; then
        source "$cli_script"
    fi
}

teardown() {
    teardown_sandbox
}

# ─── openspec_new_change ────────────────────────────────────────────────────

@test "openspec_new_change: creates change directory and .openspec.yaml" {
    run openspec new change "test-feature"

    [[ "$status" -eq 0 ]]
    assert_dir_exists "$FAKE_CHANGES_DIR/test-feature"
    assert_file_exists "$FAKE_CHANGES_DIR/test-feature/.openspec.yaml"
    assert_file_exists "$FAKE_CHANGES_DIR/test-feature/phases"
}

@test "openspec_new_change: fails on empty name" {
    run openspec new change ""
    [[ "$status" -ne 0 ]]
}

@test "openspec_new_change: fails when change already exists" {
    mkdir -p "$FAKE_CHANGES_DIR/duplicate"
    run openspec new change "duplicate"
    [[ "$status" -ne 0 ]]
    [[ "$output" == *"already exists"* ]]
}

@test "openspec_new_change: creates phases subdirectory" {
    run openspec new change "multi-phase"

    [[ "$status" -eq 0 ]]
    assert_dir_exists "$FAKE_CHANGES_DIR/multi-phase/phases"
}

# ─── openspec_status_json ───────────────────────────────────────────────────

@test "openspec_status_json: returns valid JSON" {
    run openspec status --json "test-change"

    [[ "$status" -eq 0 ]]
    # Should output parseable JSON
    python3 -c "import json; json.loads('$output')" 2>/dev/null
}

@test "openspec_status_json: returns expected fields" {
    local status_file="$(dirname "$0")/fixtures/openspec_status.json"
    export MOCK_ANSWERS_DIR="$(dirname "$0")/fixtures"
    run openspec status --json "add-user-auth"

    [[ "$status" -eq 0 ]]
    local change_name
    change_name="$(echo "$output" | python3 -c "import sys,json; d=json.load(sys.stdin); print(d.get('changeName',''))")"
    [[ "$change_name" == "add-user-auth" ]]
}

@test "openspec_status_json: empty JSON does not crash" {
    local empty_file="$(dirname "$0")/fixtures/openspec_status_empty.json"
    export MOCK_ANSWERS_DIR="$(dirname "$0")/fixtures"
    run openspec status --json "empty-change"

    [[ "$status" -eq 0 ]]
}

# ─── openspec_instructions ──────────────────────────────────────────────────

@test "openspec_instructions: returns valid JSON" {
    run openspec instructions "test-change"

    [[ "$status" -eq 0 ]]
    python3 -c "import json; json.loads('$output')" 2>/dev/null
}

@test "openspec_instructions: contains rules and template fields" {
    export MOCK_ANSWERS_DIR="$(dirname "$0")/fixtures"
    run openspec instructions "add-user-auth"

    [[ "$status" -eq 0 ]]
    local has_rules
    has_rules="$(echo "$output" | python3 -c "import sys,json; d=json.load(sys.stdin); print('rules' in d)")"
    [[ "$has_rules" == "True" ]]

    local has_template
    has_template="$(echo "$output" | python3 -c "import sys,json; d=json.load(sys.stdin); print('template' in d)")"
    [[ "$has_template" == "True" ]]
}

@test "openspec_instructions: tolerates missing fields" {
    # When instructions JSON is just {}, the output should still be parseable
    run openspec instructions "empty-change"
    [[ "$status" -eq 0 ]]
}

# ─── derive_kebab_case ──────────────────────────────────────────────────────

@test "derive_kebab_case: converts simple english phrase" {
    run derive_kebab_case "Add User Authentication"
    [[ "$status" -eq 0 ]]
    [[ "$output" == "add-user-authentication" ]]
}

@test "derive_kebab_case: handles special characters" {
    run derive_kebab_case "My Feature & Bug Fix!"
    [[ "$status" -eq 0 ]]
    # Special chars stripped, spaces → hyphens
    [[ "$output" != *"&"* ]]
    [[ "$output" != *"!"* ]]
    [[ "$output" == "my-feature--bug-fix" ]] || [[ "$output" == "my-feature-bug-fix" ]]
}

@test "derive_kebab_case: returns empty for empty input" {
    run derive_kebab_case ""
    [[ "$status" -eq 0 ]]
    [[ "$output" == "" ]]
}

@test "derive_kebab_case: truncates at 128 characters" {
    local long_input
    long_input="$(python3 -c "print('A ' * 100)")"
    run derive_kebab_case "$long_input"

    [[ "$status" -eq 0 ]]
    local len=${#output}
    [[ "$len" -le 128 ]]
}

@test "derive_kebab_case: collapses multiple hyphens" {
    run derive_kebab_case "Foo   Bar   Baz"
    [[ "$output" != *"--"* ]]
}

@test "derive_kebab_case: strips leading and trailing hyphens" {
    run derive_kebab_case "!Hello World!"
    [[ "$output" != -* ]]
    [[ "$output" != *- ]]
}

@test "derive_kebab_case: handles purely chinese input gracefully" {
    # Purely CJK input like "登录功能" contains no ASCII alphanumeric characters.
    # The function should not crash and must not leak raw Chinese characters
    # into the kebab-case output. Currently returns empty string since all
    # CJK characters are stripped by the sanitizer.
    run derive_kebab_case "登录功能"

    [[ "$status" -eq 0 ]]
    # Output must be safe: either empty or only kebab-case chars
    if [[ -n "$output" ]]; then
        [[ "$output" =~ ^[a-z0-9-]+$ ]]
        [[ "$output" != -* ]]
        [[ "$output" != *- ]]
    fi
}

# ─── change_exists ──────────────────────────────────────────────────────────

@test "change_exists: returns 0 when change directory exists" {
    mkdir -p "$FAKE_CHANGES_DIR/existing-change"
    change_exists "existing-change"
    [[ "$?" -eq 0 ]]
}

@test "change_exists: returns 1 when change directory does not exist" {
    change_exists "nonexistent-change"
    [[ "$?" -eq 1 ]]
}

@test "change_exists: returns 1 for empty name" {
    change_exists ""
    [[ "$?" -eq 1 ]]
}

@test "change_exists: distinguishes between similarly named changes" {
    mkdir -p "$FAKE_CHANGES_DIR/my-change"
    change_exists "my-change"
    [[ "$?" -eq 0 ]]

    change_exists "my-change-extra"
    [[ "$?" -eq 1 ]]
}

# ─── validate_change_name ───────────────────────────────────────────────────

@test "validate_change_name: accepts valid kebab-case name" {
    validate_change_name "add-user-auth"
    [[ "$?" -eq 0 ]]
}

@test "validate_change_name: accepts single word" {
    validate_change_name "auth"
    [[ "$?" -eq 0 ]]
}

@test "validate_change_name: rejects empty name" {
    validate_change_name ""
    [[ "$?" -eq 1 ]]
}

@test "validate_change_name: rejects name over 128 characters" {
    local long_name
    long_name="$(python3 -c "print('a' * 129)")"
    validate_change_name "$long_name"
    [[ "$?" -eq 2 ]]
}

@test "validate_change_name: rejects uppercase characters" {
    validate_change_name "Add-User-Auth"
    [[ "$?" -eq 3 ]]
}

@test "validate_change_name: rejects special characters" {
    validate_change_name "add/user/auth"
    [[ "$?" -eq 3 ]]
}

@test "validate_change_name: rejects leading hyphen" {
    validate_change_name "-add-user-auth"
    [[ "$?" -eq 3 ]]
}

@test "validate_change_name: accepts names with numbers" {
    validate_change_name "feature-v2"
    [[ "$?" -eq 0 ]]
}

# ─── Integration: combined flow ─────────────────────────────────────────────

@test "integration: scaffold then validate creates valid change" {
    # Create a change through the scaffold process
    run openspec new change "my-new-feature"
    [[ "$status" -eq 0 ]]

    # Validate the resulting change
    validate_change_name "my-new-feature"
    [[ "$?" -eq 0 ]]

    change_exists "my-new-feature"
    [[ "$?" -eq 0 ]]
}

@test "integration: change_exists after openspec new change" {
    run openspec new change "integration-test"
    [[ "$status" -eq 0 ]]

    change_exists "integration-test"
    [[ "$?" -eq 0 ]]
}
