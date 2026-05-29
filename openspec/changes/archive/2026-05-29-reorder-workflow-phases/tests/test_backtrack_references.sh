#!/usr/bin/env bash
# test_backtrack_references.sh — Full-project grep for old identifier residue (AC-8)
#
# Covers:
#   AC-8: All backtrack_to references use 02-dev-design (not 03-dev-proposal)
#         and 03-test-design (not 02-test-design)
#   Boundary: Whitelisted directories (archive, specs) may contain old identifiers
#   Boundary: src/lib/workflow.ts must have the new phase array (already verified in unit test)
#
# This test performs a full-project grep for old phase identifiers and fails if
# any match is found OUTSIDE the whitelisted directories.
#
# Usage:
#   bash test_backtrack_references.sh
#   (run from any directory; resolves project root from script location)

set -euo pipefail

# ─── Resolve project root ────────────────────────────────────────────────────
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../../../.." && pwd)"
echo "Project root: $PROJECT_ROOT"

# ─── Test counters ────────────────────────────────────────────────────────────
PASS=0
FAIL=0

assert_pass() {
    local test_name="$1"
    local message="$2"
    PASS=$((PASS + 1))
    echo "  PASS [${test_name}]: ${message}"
}

assert_fail() {
    local test_name="$1"
    local message="$2"
    FAIL=$((FAIL + 1))
    echo "  FAIL [${test_name}]: ${message}" >&2
}

# ─── Whitelist path patterns ─────────────────────────────────────────────────
# These directories contain historical records that may reference old identifiers.
# grep --exclude-dir does not support nested path exclusions, so we use post-
# filtering with grep -v on the full file path in each match line.
WHITELIST_PATTERNS=(
    "openspec/changes/archive/"
    "openspec/specs/"
    "openspec/changes/reorder-workflow-phases/"
)

cd "$PROJECT_ROOT"

# ─── Common grep options ─────────────────────────────────────────────────────
# We exclude only generic noise directories (basename match is safe here).
# Whitelist filtering is done via grep -v on match lines.
GREP_OPTS=(
    --recursive
    --exclude-dir .git
    --exclude-dir node_modules
    --exclude-dir ".pnpm"
    --include="*.ts"
    --include="*.md"
    --include="*.sh"
    --include="*.json"
    --include="*.yaml"
    --include="*.yml"
    --include="*.cjs"
    --include="*.mjs"
    --include="plugin.json"
)

filter_whitelisted() {
    local input
    input=$(cat)
    for pattern in "${WHITELIST_PATTERNS[@]}"; do
        input=$(echo "$input" | grep -v "$pattern" || true)
    done
    echo "$input"
}

# =============================================================================
# AC-8: Search for old identifier "03-dev-proposal" in non-whitelist files
# =============================================================================

echo "=== AC-8a: Search for old identifier '03-dev-proposal' ==="

# Search for "03-dev-proposal" and filter out whitelisted paths.
# The grep -v post-filtering handles nested paths that --exclude-dir cannot.
OLD_PROPOSAL_MATCHES=$(grep "${GREP_OPTS[@]}" "03-dev-proposal" . 2>/dev/null | filter_whitelisted || true)

if [[ -z "$OLD_PROPOSAL_MATCHES" ]]; then
    assert_pass "AC-8a" "No references to old identifier '03-dev-proposal' found outside whitelist directories"
else
    echo "  Found references to '03-dev-proposal':"
    echo "$OLD_PROPOSAL_MATCHES"
    assert_fail "AC-8a" "Old identifier '03-dev-proposal' still referenced in non-whitelist files"
fi

echo ""

# =============================================================================
# AC-8: Search for old identifier "02-test-design" in non-whitelist files
# =============================================================================

echo "=== AC-8b: Search for old identifier '02-test-design' ==="

OLD_TEST_DESIGN_MATCHES=$(grep "${GREP_OPTS[@]}" "02-test-design" . 2>/dev/null | filter_whitelisted || true)

if [[ -z "$OLD_TEST_DESIGN_MATCHES" ]]; then
    assert_pass "AC-8b" "No references to old identifier '02-test-design' found outside whitelist directories"
else
    echo "  Found references to '02-test-design':"
    echo "$OLD_TEST_DESIGN_MATCHES"
    assert_fail "AC-8b" "Old identifier '02-test-design' still referenced in non-whitelist files"
fi

echo ""

# =============================================================================
# AC-8: Verify new identifiers exist in key files
# =============================================================================

echo "=== AC-8c: Verify new identifiers in key source files ==="

# Workflow.ts should contain the new phase identifiers
WORKFLOW_FILE="$PROJECT_ROOT/plugins/dev-team/bin/src/lib/workflow.ts"
if [[ -f "$WORKFLOW_FILE" ]]; then
    if grep -q "02-dev-design" "$WORKFLOW_FILE"; then
        assert_pass "AC-8c-i" "workflow.ts contains 02-dev-design"
    else
        assert_fail "AC-8c-i" "workflow.ts does NOT contain 02-dev-design"
    fi

    if grep -q "03-test-design" "$WORKFLOW_FILE"; then
        assert_pass "AC-8c-ii" "workflow.ts contains 03-test-design"
    else
        assert_fail "AC-8c-ii" "workflow.ts does NOT contain 03-test-design"
    fi

    # Old identifiers should not be in the PHASES array
    if grep -q '"02-test-design"' "$WORKFLOW_FILE"; then
        assert_fail "AC-8c-iii" "workflow.ts PHASES still contains '02-test-design'"
    else
        assert_pass "AC-8c-iii" "workflow.ts PHASES does NOT contain '02-test-design'"
    fi

    if grep -q '"03-dev-proposal"' "$WORKFLOW_FILE"; then
        assert_fail "AC-8c-iv" "workflow.ts PHASES still contains '03-dev-proposal'"
    else
        assert_pass "AC-8c-iv" "workflow.ts PHASES does NOT contain '03-dev-proposal'"
    fi
else
    assert_fail "AC-8c" "workflow.ts not found at $WORKFLOW_FILE"
fi

echo ""

# =============================================================================
# Summary
# =============================================================================
echo "=== Summary ==="
echo "Total: $((PASS + FAIL)) | Passed: ${PASS} | Failed: ${FAIL}"

if [[ "$FAIL" -gt 0 ]]; then
    exit 1
fi
exit 0
