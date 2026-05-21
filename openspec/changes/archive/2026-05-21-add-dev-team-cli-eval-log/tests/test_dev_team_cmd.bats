#!/usr/bin/env bats
# test_dev_team_cmd.bats — Integration tests for dev-team.cmd Windows wrapper (AC-2)
#
# Covers:
#   AC-2: dev-team.cmd --help exits with code 0 on Windows
#
# This test is Windows-only. On non-Windows platforms all tests are skipped.
#
# Usage:
#   bats test_dev_team_cmd.bats

setup() {
    load "helpers/setup_test_env.sh"
    setup_sandbox

    export DEV_TEAM_CMD="$(cd "$(dirname "$BATS_TEST_FILENAME")" && realpath "../../../../plugins/dev-team/bin/dev-team.cmd" 2>/dev/null || echo "")"
}

teardown() {
    teardown_sandbox
}

# ─── Platform check ─────────────────────────────────────────────────────────

@test "AC-2: skip on non-Windows platforms" {
    if [[ "$OSTYPE" != "msys" && "$OSTYPE" != "cygwin" && "$OSTYPE" != "win32" ]]; then
        skip "dev-team.cmd is a Windows-only wrapper; skipping on $OSTYPE"
    fi
}

# ─── AC-2: dev-team.cmd --help ─────────────────────────────────────────────

@test "AC-2: dev-team.cmd --help exits with code 0 on Windows" {
    if [[ "$OSTYPE" != "msys" && "$OSTYPE" != "cygwin" && "$OSTYPE" != "win32" ]]; then
        skip "Windows-only test"
    fi

    if [[ -z "$DEV_TEAM_CMD" || ! -f "$DEV_TEAM_CMD" ]]; then
        skip "dev-team.cmd not found"
    fi

    run cmd /c "$DEV_TEAM_CMD" --help

    [[ "$status" -eq 0 ]]
}

@test "AC-2: dev-team.cmd --help contains usage header" {
    if [[ "$OSTYPE" != "msys" && "$OSTYPE" != "cygwin" && "$OSTYPE" != "win32" ]]; then
        skip "Windows-only test"
    fi

    if [[ -z "$DEV_TEAM_CMD" || ! -f "$DEV_TEAM_CMD" ]]; then
        skip "dev-team.cmd not found"
    fi

    run cmd /c "$DEV_TEAM_CMD" --help

    [[ "$status" -eq 0 ]]
    assert_string_contains "$output" "Usage:"
}

@test "AC-2: dev-team.cmd --help lists eval-log subcommand" {
    if [[ "$OSTYPE" != "msys" && "$OSTYPE" != "cygwin" && "$OSTYPE" != "win32" ]]; then
        skip "Windows-only test"
    fi

    if [[ -z "$DEV_TEAM_CMD" || ! -f "$DEV_TEAM_CMD" ]]; then
        skip "dev-team.cmd not found"
    fi

    run cmd /c "$DEV_TEAM_CMD" --help

    [[ "$status" -eq 0 ]]
    assert_string_contains "$output" "eval-log"
}

@test "AC-2: dev-team.cmd eval-log --help shows detailed help" {
    if [[ "$OSTYPE" != "msys" && "$OSTYPE" != "cygwin" && "$OSTYPE" != "win32" ]]; then
        skip "Windows-only test"
    fi

    if [[ -z "$DEV_TEAM_CMD" || ! -f "$DEV_TEAM_CMD" ]]; then
        skip "dev-team.cmd not found"
    fi

    run cmd /c "$DEV_TEAM_CMD" eval-log --help

    [[ "$status" -eq 0 ]]
    assert_string_contains "$output" "--change"
    assert_string_contains "$output" "--phase"
    assert_string_contains "$output" "--verdict"
}
