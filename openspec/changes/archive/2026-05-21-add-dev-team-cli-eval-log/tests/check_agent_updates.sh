#!/usr/bin/env bash
# check_agent_updates.sh — Scan Evaluator Agent files for old eval.json patterns.
#
# This script checks all 7 Evaluator Agent prompt files to identify any that
# still contain old patterns (direct eval.json reads/writes) instead of using
# the new dev-team eval-log CLI.
#
# Old patterns scanned:
#   - "Compute attempt" (old manual attempt calculation)
#   - "Append to eval.json" (old direct file write)
#   - "fs.readFileSync" (direct Node.js file read)
#   - "writeFileSync" (direct Node.js file write)
#   - "eval.json" combined with "fs." or "write" or "read" patterns
#
# Usage:
#   bash check_agent_updates.sh [--verbose]
#
# Exit code: 0 if all agents are updated, 1 if any still contain old patterns.

set -euo pipefail

# ─── Configuration ──────────────────────────────────────────────────────────
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
# From tests dir (openspec/changes/<change>/tests), go up 4 levels to project root
AGENTS_DIR="$(cd "$SCRIPT_DIR/../../../../plugins/dev-team/agents" && pwd 2>/dev/null || echo "")"

# Keywords that indicate old eval.json direct manipulation patterns
OLD_PATTERNS=(
    "Compute attempt"
    "Append to eval.json"
    "fs.readFileSync"
    "writeFileSync"
)

# Desired patterns that should be present (CLI invocation)
NEW_PATTERNS=(
    "dev-team eval-log"
)

# Colors
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m'

PASS_COUNT=0
FAIL_COUNT=0
WARN_COUNT=0

# ─── Agent files ────────────────────────────────────────────────────────────
AGENT_FILES=(
    "requirements-evaluator.md"
    "test-design-evaluator.md"
    "dev-proposal-evaluator.md"
    "test-gen-evaluator.md"
    "implementation-evaluator.md"
    "code-review-evaluator.md"
    "acceptance-evaluator.md"
)

# ─── Helpers ────────────────────────────────────────────────────────────────

check_agent_file() {
    local file="$1"
    local verbose="${2:-false}"
    local basename
    basename="$(basename "$file")"

    if [[ ! -f "$file" ]]; then
        echo -e "${YELLOW}WARN${NC}: Agent file not found: $file"
        WARN_COUNT=$((WARN_COUNT + 1))
        return 2
    fi

    local has_old_pattern=false
    local found_patterns=()

    # Check for old patterns
    for pattern in "${OLD_PATTERNS[@]}"; do
        if grep -qi "$pattern" "$file" 2>/dev/null; then
            has_old_pattern=true
            found_patterns+=("$pattern")
        fi
    done

    # Check for new patterns (CLI invocation)
    local has_new_pattern=false
    for pattern in "${NEW_PATTERNS[@]}"; do
        if grep -q "$pattern" "$file" 2>/dev/null; then
            has_new_pattern=true
            break
        fi
    done

    # Report
    if $has_old_pattern; then
        echo -e "${RED}FAIL${NC}: $basename — still contains old patterns:"
        for p in "${found_patterns[@]}"; do
            echo "       - \"$p\""
        done
        if $has_new_pattern; then
            echo "       (also has new CLI pattern, but old patterns remain)"
        fi
        FAIL_COUNT=$((FAIL_COUNT + 1))
        return 1
    elif $has_new_pattern; then
        echo -e "${GREEN}PASS${NC}: $basename — fully updated to CLI pattern"
        PASS_COUNT=$((PASS_COUNT + 1))
        return 0
    else
        echo -e "${YELLOW}WARN${NC}: $basename — has neither old nor new patterns (may not be an evaluator)"
        WARN_COUNT=$((WARN_COUNT + 1))
        return 2
    fi
}

# ─── Main ───────────────────────────────────────────────────────────────────

main() {
    local verbose=false
    if [[ "${1:-}" == "--verbose" ]]; then
        verbose=true
    fi

    echo "=================================================="
    echo "  Agent Update Check: eval.json -> CLI migration"
    echo "=================================================="
    echo ""

    if [[ -z "$AGENTS_DIR" || ! -d "$AGENTS_DIR" ]]; then
        echo -e "${RED}ERROR${NC}: Agents directory not found at $AGENTS_DIR" >&2
        echo "Make sure this script is run from the tests directory."
        exit 1
    fi

    echo "Scanning agents in: $AGENTS_DIR"
    echo ""

    for agent_file in "${AGENT_FILES[@]}"; do
        check_agent_file "$AGENTS_DIR/$agent_file" "$verbose"
    done

    # Check for any additional evaluator files we might have missed
    local extra_files=0
    while IFS= read -r -d '' extra; do
        local basename
        basename="$(basename "$extra")"
        # Skip if already in our list
        local already_checked=false
        for known in "${AGENT_FILES[@]}"; do
            if [[ "$basename" == "$known" ]]; then
                already_checked=true
                break
            fi
        done
        if ! $already_checked && [[ "$basename" == *"evaluator"* ]]; then
            echo -e "${YELLOW}EXTRA${NC}: Unchecked evaluator file found: $basename"
            extra_files=$((extra_files + 1))
        fi
    done < <(find "$AGENTS_DIR" -name "*evaluator*.md" -print0 2>/dev/null || true)

    echo ""
    echo "=================================================="
    echo "  Summary: $PASS_COUNT updated, $FAIL_COUNT outdated, $WARN_COUNT warnings"
    if [[ "$extra_files" -gt 0 ]]; then
        echo "  Extra evaluator files not in checklist: $extra_files"
    fi
    echo "=================================================="

    if $verbose; then
        echo ""
        echo "--- Verbose: old pattern occurrences ---"
        for pattern in "${OLD_PATTERNS[@]}"; do
            echo "Pattern: \"$pattern\""
            grep -rl "$pattern" "$AGENTS_DIR" 2>/dev/null | sed 's/^/  /' || echo "  (none)"
        done
        echo ""
        echo "--- Verbose: new pattern occurrences ---"
        for pattern in "${NEW_PATTERNS[@]}"; do
            echo "Pattern: \"$pattern\""
            grep -rl "$pattern" "$AGENTS_DIR" 2>/dev/null | sed 's/^/  /' || echo "  (none)"
        done
    fi

    if [[ "$FAIL_COUNT" -gt 0 ]]; then
        echo ""
        echo -e "${RED}Some agents still need updating. Review each FAIL above.${NC}"
        exit 1
    fi

    if [[ "$WARN_COUNT" -eq "${#AGENT_FILES[@]}" ]]; then
        echo ""
        echo -e "${YELLOW}Warning: No evaluator files were found or recognized.${NC}"
        exit 1
    fi

    echo ""
    echo -e "${GREEN}All agents updated.${NC}"
    exit 0
}

main "$@"
