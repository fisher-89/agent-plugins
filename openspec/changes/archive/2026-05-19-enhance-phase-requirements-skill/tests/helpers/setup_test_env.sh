#!/usr/bin/env bash
# setup_test_env.sh — Shared test helpers for phase-requirements skill tests.
#
# Provides:
#   setup_sandbox()       — Create a temporary sandbox directory
#   teardown_sandbox()    — Remove the sandbox directory
#   mock_openspec()       — Override openspec CLI with a mock implementation
#   assert_dir_exists()   — Assert a directory exists, fail otherwise
#   assert_file_exists()  — Assert a file exists, fail otherwise
#   assert_json_field()   — Assert a JSON field equals an expected value
#   assert_exit_code()    — Assert the last command's exit code matches expected
#   derive_kebab_case()   — Inline implementation for testing name derivation
#   validate_change_name()— Inline implementation for testing name validation
#
# Usage:
#   source "$(dirname "$0")/helpers/setup_test_env.sh"
#   setup_sandbox
#   # ... run tests ...
#   teardown_sandbox

set -euo pipefail

# ─── Sandbox management ──────────────────────────────────────────────────────

SANDBOX_DIR=""

setup_sandbox() {
    SANDBOX_DIR="$(mktemp -d -t phase_req_test_XXXXXX)"
    export SANDBOX_DIR

    # Create a clean PATH with no side-effects from the real project
    export TEST_PATH="$SANDBOX_DIR/bin:$PATH"

    # Set up a fake openspec home
    export FAKE_CHANGES_DIR="$SANDBOX_DIR/openspec/changes"
    mkdir -p "$FAKE_CHANGES_DIR"

    # Store last exit code for assert_exit_code
    export __LAST_EXIT_CODE=0
}

teardown_sandbox() {
    if [[ -n "$SANDBOX_DIR" && -d "$SANDBOX_DIR" ]]; then
        rm -rf "$SANDBOX_DIR"
    fi
    SANDBOX_DIR=""
}

# ─── Mock openspec CLI ───────────────────────────────────────────────────────

mock_openspec() {
    local bin_dir="$SANDBOX_DIR/bin"
    mkdir -p "$bin_dir"

    cat > "$bin_dir/openspec" << 'MOCK_EOF'
#!/usr/bin/env bash
# Mock openspec CLI for testing

set -euo pipefail

MOCK_ANSWERS_DIR="${MOCK_ANSWERS_DIR:-}"

case "${1:-}" in
    new)
        if [[ "${2:-}" == "change" ]]; then
            local name="${3:-}"
            if [[ -z "$name" ]]; then
                echo "Error: change name required" >&2
                exit 1
            fi
            local change_dir="${FAKE_CHANGES_DIR:-$SANDBOX_DIR/openspec/changes}/$name"
            if [[ -d "$change_dir" ]]; then
                echo "Error: change '$name' already exists" >&2
                exit 1
            fi
            mkdir -p "$change_dir/phases"
            cat > "$change_dir/.openspec.yaml" << YAML_EOF
version: "1.0"
schema: spec-driven
created: $(date +%Y-%m-%d)
change: $name
YAML_EOF
            echo "Created change '$name'"
        fi
        ;;
    status)
        if [[ "${2:-}" == "--json" ]]; then
            local name="${3:-}"
            if [[ -n "$MOCK_ANSWERS_DIR" && -f "$MOCK_ANSWERS_DIR/openspec_status.json" ]]; then
                cat "$MOCK_ANSWERS_DIR/openspec_status.json"
            else
                echo '{"changeName":"mock-change","artifacts":[],"applyRequires":[]}'
            fi
        fi
        ;;
    instructions)
        if [[ -n "$MOCK_ANSWERS_DIR" && -f "$MOCK_ANSWERS_DIR/openspec_instructions.json" ]]; then
            cat "$MOCK_ANSWERS_DIR/openspec_instructions.json"
        else
            echo '{"rules":[],"context":"","template":"","instruction":"","outputPath":"","dependencies":[]}'
        fi
        ;;
    *)
        echo "Unknown openspec command: $*" >&2
        exit 1
        ;;
esac
MOCK_EOF
    chmod +x "$bin_dir/openspec"
    export PATH="$bin_dir:$PATH"
}

# ─── Assertions ──────────────────────────────────────────────────────────────

assert_dir_exists() {
    local path="$1"
    if [[ ! -d "$path" ]]; then
        echo "FAIL: Directory does not exist: $path" >&2
        exit 1
    fi
    echo "PASS: Directory exists: $path"
}

assert_file_exists() {
    local path="$1"
    if [[ ! -f "$path" ]]; then
        echo "FAIL: File does not exist: $path" >&2
        exit 1
    fi
    echo "PASS: File exists: $path"
}

assert_json_field() {
    local file="$1"
    local field="$2"
    local expected="$3"

    if [[ ! -f "$file" ]]; then
        echo "FAIL: JSON file not found: $file" >&2
        exit 1
    fi

    local actual
    actual="$(python3 -c "import json; d=json.load(open('$file')); print(d.get('$field', '<<MISSING>>'))" 2>/dev/null || echo '<<PARSE_ERROR>>')"

    if [[ "$actual" == "$expected" ]]; then
        echo "PASS: JSON field '$field' = '$expected'"
    else
        echo "FAIL: JSON field '$field' expected '$expected', got '$actual'" >&2
        exit 1
    fi
}

assert_exit_code() {
    local expected="$1"
    local actual="${__LAST_EXIT_CODE:-$?}"

    if [[ "$actual" -eq "$expected" ]]; then
        echo "PASS: Exit code $expected as expected"
    else
        echo "FAIL: Expected exit code $expected, got $actual" >&2
        exit 1
    fi
}

assert_string_contains() {
    local haystack="$1"
    local needle="$2"

    if [[ "$haystack" == *"$needle"* ]]; then
        echo "PASS: String contains '$needle'"
    else
        echo "FAIL: String does not contain '$needle'" >&2
        echo "  String: $haystack" >&2
        exit 1
    fi
}

assert_string_not_contains() {
    local haystack="$1"
    local needle="$2"

    if [[ "$haystack" != *"$needle"* ]]; then
        echo "PASS: String does not contain '$needle'"
    else
        echo "FAIL: String contains '$needle' (should not)" >&2
        exit 1
    fi
}

# ─── Implementation stubs for testing ─────────────────────────────────────────

# derive_kebab_case: Convert a description string to kebab-case.
# This is the reference implementation for testing purposes.
derive_kebab_case() {
    local input="$1"
    if [[ -z "$input" ]]; then
        echo ""
        return
    fi

    # Convert to lowercase
    local result
    result="$(echo "$input" | tr '[:upper:]' '[:lower:]')"

    # Replace non-alphanumeric characters (except Chinese) with hyphens
    # Simplified: strip CJK characters, keep ASCII alphanumeric and hyphens
    result="$(echo "$result" | sed 's/[^a-z0-9 -]//g')"

    # Replace spaces with hyphens
    result="$(echo "$result" | sed 's/ /-/g')"

    # Collapse multiple hyphens
    while [[ "$result" == *--* ]]; do
        result="${result//--/-}"
    done

    # Strip leading/trailing hyphens
    result="$(echo "$result" | sed 's/^-//; s/-$//')"

    # Truncate to 128 characters
    result="${result:0:128}"

    echo "$result"
}

# validate_change_name: Check change name format.
# Returns 0=valid, 1=empty, 2=too long (>128), 3=invalid characters
validate_change_name() {
    local name="$1"
    if [[ -z "$name" ]]; then
        return 1
    fi
    if [[ ${#name} -gt 128 ]]; then
        return 2
    fi
    # Only allow lowercase alphanumeric and hyphens
    if [[ ! "$name" =~ ^[a-z0-9][a-z0-9-]*$ ]]; then
        return 3
    fi
    return 0
}

# change_exists: Check if a change directory exists.
change_exists() {
    local name="$1"
    local changes_dir="${FAKE_CHANGES_DIR:-$SANDBOX_DIR/openspec/changes}"
    [[ -d "$changes_dir/$name" ]]
}
