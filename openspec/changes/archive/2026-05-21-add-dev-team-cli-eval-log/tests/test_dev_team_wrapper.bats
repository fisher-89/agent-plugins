#!/usr/bin/env bats
# test_dev_team_wrapper.bats — Integration tests for dev-team bash wrapper (AC-1, AC-14)
#
# Covers:
#   AC-1: dev-team --help output contains usage and subcommand list
#   AC-14: Bundle missing produces clear error message on stderr
#
# Usage:
#   bats test_dev_team_wrapper.bats

setup() {
    load "helpers/setup_test_env.sh"
    setup_sandbox

    # Determine dev-team wrapper path relative to test file
    export DEV_TEAM_WRAPPER="$(cd "$(dirname "$BATS_TEST_FILENAME")" && realpath "../../../../plugins/dev-team/bin/dev-team" 2>/dev/null || echo "")"
    export DEV_TEAM_BUNDLE_DIR="$(cd "$(dirname "$BATS_TEST_FILENAME")" && realpath "../../../../plugins/dev-team/bin" 2>/dev/null || echo "")"
}

teardown() {
    teardown_sandbox
}

# ─── AC-1: dev-team --help output ──────────────────────────────────────────

@test "AC-1: dev-team --help exits with code 0" {
    if [[ -z "$DEV_TEAM_WRAPPER" || ! -f "$DEV_TEAM_WRAPPER" ]]; then
        skip "dev-team wrapper not found"
    fi

    run bash "$DEV_TEAM_WRAPPER" --help

    [[ "$status" -eq 0 ]]
}

@test "AC-1: dev-team --help contains usage header" {
    if [[ -z "$DEV_TEAM_WRAPPER" || ! -f "$DEV_TEAM_WRAPPER" ]]; then
        skip "dev-team wrapper not found"
    fi

    run bash "$DEV_TEAM_WRAPPER" --help

    [[ "$status" -eq 0 ]]
    assert_string_contains "$output" "Usage:"
}

@test "AC-1: dev-team --help lists eval-log subcommand" {
    if [[ -z "$DEV_TEAM_WRAPPER" || ! -f "$DEV_TEAM_WRAPPER" ]]; then
        skip "dev-team wrapper not found"
    fi

    run bash "$DEV_TEAM_WRAPPER" --help

    [[ "$status" -eq 0 ]]
    assert_string_contains "$output" "eval-log"
}

@test "AC-1: dev-team eval-log --help shows detailed help" {
    if [[ -z "$DEV_TEAM_WRAPPER" || ! -f "$DEV_TEAM_WRAPPER" ]]; then
        skip "dev-team wrapper not found"
    fi

    run bash "$DEV_TEAM_WRAPPER" eval-log --help

    [[ "$status" -eq 0 ]]
    assert_string_contains "$output" "--change"
    assert_string_contains "$output" "--phase"
    assert_string_contains "$output" "--verdict"
    assert_string_contains "$output" "--report"
    assert_string_contains "$output" "--items"
}

@test "AC-1: dev-team with no arguments shows help" {
    if [[ -z "$DEV_TEAM_WRAPPER" || ! -f "$DEV_TEAM_WRAPPER" ]]; then
        skip "dev-team wrapper not found"
    fi

    run bash "$DEV_TEAM_WRAPPER"

    [[ "$status" -eq 0 ]]
    assert_string_contains "$output" "Usage:"
}

@test "AC-1: dev-team eval-log --help mentions attempt and backtrack-to options" {
    if [[ -z "$DEV_TEAM_WRAPPER" || ! -f "$DEV_TEAM_WRAPPER" ]]; then
        skip "dev-team wrapper not found"
    fi

    run bash "$DEV_TEAM_WRAPPER" eval-log --help

    [[ "$status" -eq 0 ]]
    assert_string_contains "$output" "--attempt"
    assert_string_contains "$output" "--backtrack-to"
}

# ─── AC-14: Bundle missing error ────────────────────────────────────────────

@test "AC-14: dev-team wrapper fails when bundle.js is missing" {
    if [[ -z "$DEV_TEAM_WRAPPER" || ! -f "$DEV_TEAM_WRAPPER" ]]; then
        skip "dev-team wrapper not found"
    fi

    # Temporarily rename the bundle to simulate missing bundle
    local bundle_file="$DEV_TEAM_BUNDLE_DIR/dev-team-bundle.js"
    if [[ -f "$bundle_file" ]]; then
        mv "$bundle_file" "${bundle_file}.bak"
    fi

    run bash "$DEV_TEAM_WRAPPER" --help

    # Restore bundle file
    if [[ -f "${bundle_file}.bak" ]]; then
        mv "${bundle_file}.bak" "$bundle_file"
    fi

    [[ "$status" -eq 1 ]]
    assert_string_contains "$output" "未找到" || assert_string_contains "$output" "not found" || assert_string_contains "$output" "构建"
}

@test "AC-14: dev-team wrapper error message guides user to build" {
    if [[ -z "$DEV_TEAM_WRAPPER" || ! -f "$DEV_TEAM_WRAPPER" ]]; then
        skip "dev-team wrapper not found"
    fi

    local bundle_file="$DEV_TEAM_BUNDLE_DIR/dev-team-bundle.js"
    if [[ -f "$bundle_file" ]]; then
        mv "$bundle_file" "${bundle_file}.bak"
    fi

    run bash "$DEV_TEAM_WRAPPER" --help

    if [[ -f "${bundle_file}.bak" ]]; then
        mv "${bundle_file}.bak" "$bundle_file"
    fi

    # Error message should reference the build step
    assert_string_contains "$output" "npm run build" || assert_string_contains "$output" "build"
}
