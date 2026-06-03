#!/usr/bin/env bats
# test_excluded_files_unchanged.bats — Regression verification for AC-6.
#
# Covers AC-6: unit-test-evaluator.md and integration-test-evaluator.md MUST NOT
# have been modified by this change. These files use a diagnostic decision-tree
# format (not a static checklist) and are explicitly excluded from the scope.
#
# This test uses `git diff` to check whether these files have any uncommitted
# changes or differ from the base branch. If the change was implemented as a
# series of commits, this test verifies that no commit touched these files.
#
# Usage:
#   bats test_excluded_files_unchanged.bats

setup() {
    load "helpers/setup_test_env.sh"
    setup_test_env
}

teardown() {
    teardown_test_env
}

# ─── Helper: Check if a file was changed in git history ─────────────────────

# Returns 0 (success) if the file was NOT changed, 1 (failure) if it WAS changed.
# Checks both working tree changes and commits reachable from HEAD.
file_unchanged_in_git() {
    local file="$1"
    local rel_path
    local project_root
    project_root="$(cd "$(dirname "$0")/../../../../.." && pwd)"

    # Make the path relative to project root for git commands
    rel_path="$(realpath --relative-to="$project_root" "$file" 2>/dev/null || echo "$file")"

    # Check 1: Working tree changes (unstaged + staged)
    if git -C "$project_root" diff -- "$rel_path" 2>/dev/null | grep -q .; then
        echo "CHANGED: $rel_path has unstaged/staged working tree changes"
        return 1
    fi

    # Check 2: Check if any commit in this branch touches the file
    # Only meaningful if there are commits beyond the base branch
    local base_branch
    base_branch="$(git -C "$project_root" rev-parse --abbrev-ref HEAD 2>/dev/null || echo "master")"

    # Try to find commits that touch this file
    local commits
    commits="$(git -C "$project_root" log --oneline HEAD -- "$rel_path" 2>/dev/null || true)"
    if [[ -n "$commits" ]]; then
        echo "CHANGED: $rel_path was modified in commits on this branch:"
        echo "$commits"
        return 1
    fi

    echo "UNCHANGED: $rel_path has no changes (working tree or history)"
    return 0
}

# ─── AC-6: unit-test-evaluator.md is unchanged ──────────────────────────────

@test "AC-6: unit-test-evaluator.md has no changes" {
    local file="$AGENTS_DIR/unit-test-evaluator.md"

    if [[ ! -f "$file" ]]; then
        echo "FAIL: File not found: $file" >&2
        return 1
    fi

    run file_unchanged_in_git "$file"
    echo "$output"

    if [[ "$status" -eq 0 ]]; then
        return 0
    else
        echo "FAIL: unit-test-evaluator.md was modified (should be excluded)" >&2
        return 1
    fi
}

# ─── AC-6: integration-test-evaluator.md is unchanged ───────────────────────

@test "AC-6: integration-test-evaluator.md has no changes" {
    local file="$AGENTS_DIR/integration-test-evaluator.md"

    if [[ ! -f "$file" ]]; then
        echo "FAIL: File not found: $file" >&2
        return 1
    fi

    run file_unchanged_in_git "$file"
    echo "$output"

    if [[ "$status" -eq 0 ]]; then
        return 0
    else
        echo "FAIL: integration-test-evaluator.md was modified (should be excluded)" >&2
        return 1
    fi
}

# ─── AC-6: Edge case — File existence check ─────────────────────────────────

@test "AC-6: unit-test-evaluator.md exists at expected path" {
    local file="$AGENTS_DIR/unit-test-evaluator.md"
    [[ -f "$file" ]] || {
        echo "FAIL: unit-test-evaluator.md not found at $file" >&2
        return 1
    }
}

@test "AC-6: integration-test-evaluator.md exists at expected path" {
    local file="$AGENTS_DIR/integration-test-evaluator.md"
    [[ -f "$file" ]] || {
        echo "FAIL: integration-test-evaluator.md not found at $file" >&2
        return 1
    }
}
