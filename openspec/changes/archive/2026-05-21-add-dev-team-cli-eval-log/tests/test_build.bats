#!/usr/bin/env bats
# test_build.bats — Integration tests for esbuild build process (AC-15)
#
# Covers:
#   AC-15: esbuild build script produces dev-team-bundle.js
#   AC-15: Built bundle has correct shebang line
#   AC-15: Built bundle is executable (Node.js can parse it)
#
# Usage:
#   bats test_build.bats

setup() {
    load "helpers/setup_test_env.sh"
    setup_sandbox

    # Locate the build directory
    export BUILD_DIR="$(cd "$(dirname "$BATS_TEST_FILENAME")" && realpath "../../../../plugins/dev-team/bin" 2>/dev/null || echo "")"
}

teardown() {
    teardown_sandbox
}

# ─── Helper: check build prerequisites ──────────────────────────────────────

check_build_prereqs() {
    if [[ -z "$BUILD_DIR" || ! -d "$BUILD_DIR" ]]; then
        skip "Build directory not found"
    fi

    if [[ ! -f "$BUILD_DIR/package.json" ]]; then
        skip "package.json not found in $BUILD_DIR"
    fi
}

# ─── AC-15: esbuild build ───────────────────────────────────────────────────

@test "AC-15: npm run build exits with code 0" {
    check_build_prereqs

    run npm run build --prefix "$BUILD_DIR" 2>&1

    echo "Build output: $output" >&2
    [[ "$status" -eq 0 ]]
}

@test "AC-15: build produces dev-team-bundle.js" {
    check_build_prereqs

    run npm run build --prefix "$BUILD_DIR" 2>&1

    [[ "$status" -eq 0 ]]
    assert_file_exists "$BUILD_DIR/dev-team-bundle.js"
}

@test "AC-15: built bundle has correct shebang" {
    check_build_prereqs

    run npm run build --prefix "$BUILD_DIR" 2>&1

    [[ "$status" -eq 0 ]]

    if [[ -f "$BUILD_DIR/dev-team-bundle.js" ]]; then
        local first_line
        first_line="$(head -1 "$BUILD_DIR/dev-team-bundle.js")"
        [[ "$first_line" == "#!/usr/bin/env node" ]]
    else
        skip "Bundle file not found after build"
    fi
}

@test "AC-15: built bundle is valid Node.js (can parse without error)" {
    check_build_prereqs

    run npm run build --prefix "$BUILD_DIR" 2>&1

    [[ "$status" -eq 0 ]]

    if [[ -f "$BUILD_DIR/dev-team-bundle.js" ]]; then
        # Check syntax is valid
        run node --check "$BUILD_DIR/dev-team-bundle.js"
        [[ "$status" -eq 0 ]]
    else
        skip "Bundle file not found after build"
    fi
}

@test "AC-15: built bundle --help shows usage" {
    check_build_prereqs

    run npm run build --prefix "$BUILD_DIR" 2>&1

    [[ "$status" -eq 0 ]]

    if [[ -f "$BUILD_DIR/dev-team-bundle.js" ]]; then
        run node "$BUILD_DIR/dev-team-bundle.js" --help
        [[ "$status" -eq 0 ]]
        assert_string_contains "$output" "Usage:"
        assert_string_contains "$output" "eval-log"
    else
        skip "Bundle file not found after build"
    fi
}

@test "AC-15: built bundle has non-zero file size" {
    check_build_prereqs

    run npm run build --prefix "$BUILD_DIR" 2>&1

    [[ "$status" -eq 0 ]]

    if [[ -f "$BUILD_DIR/dev-team-bundle.js" ]]; then
        local size
        size="$(stat -c%s "$BUILD_DIR/dev-team-bundle.js" 2>/dev/null || wc -c < "$BUILD_DIR/dev-team-bundle.js" 2>/dev/null || echo "0")"
        [[ "$size" -gt 100 ]]
    else
        skip "Bundle file not found after build"
    fi
}
