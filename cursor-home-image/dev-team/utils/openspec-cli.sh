#!/usr/bin/env bash
# openspec-cli.sh -- CLI wrapper functions for openspec.
#
# Provides shell functions for interacting with the openspec CLI
# during skill execution. This script is meant to be sourced, not
# executed directly.
#
# Usage:
#   source __DEV_TEAM_ROOT__/utils/openspec-cli.sh
#   if change_exists "my-change"; then
#       openspec_status_json "my-change"
#   fi
#
# Environment:
#   OPENSPEC_CHANGES_DIR   Override base directory for changes
#                          (default: openspec/changes)
#
# Functions:
#   change_exists(name)        Check if change directory exists (exit 0/1)
#   validate_change_name(name) Validate change name format (exit 0/1/2/3)
#   derive_kebab_case(input)   Convert description to kebab-case string
#   openspec_new_change(name)  Create a new change scaffold
#   openspec_status_json(name) Get change status as JSON
#   openspec_instructions(name)Get artifact instructions as JSON
#   openspec_spec_list(name)   获取全局能力列表，返回 JSON 数组或 []
#   openspec_cli_cache(cmd,name) Cached wrapper for CLI calls
#
# Exit codes for validate_change_name:
#   0 = valid
#   1 = empty string
#   2 = exceeds 128 characters
#   3 = does not match kebab-case pattern ^[a-z0-9][a-z0-9-]*$

set -euo pipefail

# ─── Configuration ────────────────────────────────────────────────────────────

: "${OPENSPEC_CHANGES_DIR:=openspec/changes}"

# ─── change_exists: Check if a change directory exists ─────────────────────────
# Returns 0 if the directory exists, 1 otherwise.

change_exists() {
    local name="$1"
    if [[ -z "$name" ]]; then
        return 1
    fi
    if [[ -d "$OPENSPEC_CHANGES_DIR/$name" ]]; then
        return 0
    else
        return 1
    fi
}

# ─── validate_change_name: Validate change name format ─────────────────────────
# Returns:
#   0 = valid kebab-case
#   1 = empty
#   2 = longer than 128 characters
#   3 = contains invalid characters or format

validate_change_name() {
    local name="$1"

    if [[ -z "$name" ]]; then
        return 1
    fi

    if [[ ${#name} -gt 128 ]]; then
        return 2
    fi

    # Must start with lowercase alphanumeric, followed by lowercase
    # alphanumeric or hyphens
    if [[ ! "$name" =~ ^[a-z0-9][a-z0-9-]*$ ]]; then
        return 3
    fi

    return 0
}

# ─── derive_kebab_case: Convert description to kebab-case ──────────────────────
# Echoes the derived name to stdout. Returns empty string for empty input.
# Algorithm:
#   1. Convert to lowercase
#   2. Strip CJK characters and non-alphanumeric (keep spaces and digits)
#   3. Replace spaces with hyphens
#   4. Collapse consecutive hyphens
#   5. Strip leading/trailing hyphens
#   6. Truncate to 128 characters

derive_kebab_case() {
    local input="$1"

    if [[ -z "$input" ]]; then
        echo ""
        return
    fi

    local result
    # Step 1: lowercase
    result="$(echo "$input" | tr '[:upper:]' '[:lower:]')"

    # Step 2: strip everything except lowercase letters, digits, spaces, hyphens
    result="$(echo "$result" | sed 's/[^a-z0-9 -]//g')"

    # Step 3: spaces to hyphens
    result="$(echo "$result" | sed 's/ /-/g')"

    # Step 4: collapse consecutive hyphens
    while [[ "$result" == *--* ]]; do
        result="${result//--/-}"
    done

    # Step 5: strip leading/trailing hyphens
    result="$(echo "$result" | sed 's/^-//; s/-$//')"

    # Step 6: truncate to 128 characters
    result="${result:0:128}"

    echo "$result"
}

# ─── openspec_new_change: Create a new change scaffold ─────────────────────────
# Wraps `openspec new change "<name>"`. Exits with the CLI's exit code.
# Outputs CLI stdout; stderr from the CLI is passed through.

openspec_new_change() {
    local name="$1"

    if [[ -z "$name" ]]; then
        echo "Error: change name is required" >&2
        return 1
    fi

    openspec new change "$name"
    local exit_code=$?

    if [[ $exit_code -ne 0 ]]; then
        echo "Error: failed to create change '$name' (exit $exit_code)" >&2
        return $exit_code
    fi

    return 0
}

# ─── openspec_status_json: Get change status as JSON ───────────────────────────
# Wraps `openspec status --json`. Returns "{}" on failure for resilient
# fallback in prompt construction.

openspec_status_json() {
    local name="$1"
    local output

    output="$(openspec status --json "$name" 2>/dev/null)" || {
        echo "{}"
        return 0
    }

    # If output is empty or whitespace-only, return empty JSON object
    if [[ -z "${output// /}" ]]; then
        echo "{}"
        return 0
    fi

    echo "$output"
}

# ─── openspec_instructions: Get artifact instructions as JSON ──────────────────
# Wraps `openspec instructions`. Returns "{}" on failure.
# Downstream consumers should tolerate missing top-level fields.

openspec_instructions() {
    local name="$1"
    local output

    output="$(openspec instructions "$name" 2>/dev/null)" || {
        echo "{}"
        return 0
    }

    if [[ -z "${output// /}" ]]; then
        echo "{}"
        return 0
    fi

    echo "$output"
}

# ─── openspec_spec_list: 获取全局能力列表，返回 JSON 数组 ─────────────────────
# 封装 `openspec spec list --json`。失败时返回 "[]" 以提供弹性回退，
# 供能力分类使用。校验输出是否为 JSON 数组；非数组输出触发警告并返回 "[]"。
# 供 requirements-planner agent 调用，用于区分新增能力和修改的能力。

openspec_spec_list() {
    local name="$1"
    local output

    output="$(openspec spec list --json 2>/dev/null)" || {
        echo "[]"
        return 0
    }

    # If output is empty or whitespace-only, return empty array
    if [[ -z "${output// /}" ]]; then
        echo "[]"
        return 0
    fi

    # Validate output is a valid JSON array
    local validated
    validated="$(echo "$output" | python3 -c "
import json,sys
try:
    data = json.load(sys.stdin)
    assert isinstance(data, list), 'not a list'
    print(json.dumps(data))
except Exception:
    print('INVALID')
" 2>/dev/null)" || {
        echo "[]"
        return 0
    }

    if [[ "$validated" == "INVALID" ]]; then
        echo "Warning: openspec spec list returned non-array output" >&2
        echo "[]"
        return 0
    fi

    echo "$validated"
}

# ─── openspec_cli_cache: Cached wrapper for CLI calls ──────────────────────────
# Avoids redundant `openspec status --json` and `openspec instructions` calls
# within a single skill execution. The cache is invalidated if the change
# directory's modification time changes (detected via ls -ld timestamp).
#
# Usage:
#   result="$(openspec_cli_cache status "my-change")"
#   result="$(openspec_cli_cache instructions "my-change")"
#   result="$(openspec_cli_cache spec_list "my-change")"

declare -A __OPENSPEC_CLI_CACHE=()
declare -A __OPENSPEC_CLI_CACHE_MTIME=()

openspec_cli_cache() {
    local cmd="$1"
    local name="$2"
    local cache_key="${cmd}:${name}"
    local change_dir="$OPENSPEC_CHANGES_DIR/$name"

    # Compute current directory state token for cache invalidation
    local dir_token=""
    if [[ -d "$change_dir" ]]; then
        dir_token="$(ls -ld "$change_dir" 2>/dev/null | md5sum 2>/dev/null | cut -d' ' -f1 || ls -ld "$change_dir" 2>/dev/null)"
    fi
    local full_key="${cache_key}|dir:${dir_token:-none}"

    # Return cached result if available
    if [[ -n "${__OPENSPEC_CLI_CACHE[$full_key]:-}" ]]; then
        echo "${__OPENSPEC_CLI_CACHE[$full_key]}"
        return 0
    fi

    # Run the actual CLI command
    local output=""
    case "$cmd" in
        status)
            output="$(openspec_status_json "$name")"
            ;;
        instructions)
            output="$(openspec_instructions "$name")"
            ;;
        spec_list)
            output="$(openspec_spec_list "$name")"
            ;;
        *)
            echo "Error: unknown cache command '$cmd'" >&2
            return 1
            ;;
    esac

    # Cache the result
    __OPENSPEC_CLI_CACHE[$full_key]="$output"

    echo "$output"
    return 0
}
