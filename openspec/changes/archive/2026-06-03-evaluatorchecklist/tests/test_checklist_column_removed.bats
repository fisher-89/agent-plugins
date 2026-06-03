#!/usr/bin/env bats
# test_checklist_column_removed.bats — Content verification for AC-1 and table format.
#
# Covers:
#   AC-1: All 7 evaluator files have NO `必须` column in their `## Static Checklist` table
#   Table Format: Each Static Checklist table has exactly 3 columns (ID, 检查项, 判断依据)
#
# The `必须` column (formerly the 4th column indicating required/optional) must
# be removed from all 7 evaluator Static Checklist tables. Unit-test-evaluator
# and integration-test-evaluator use a diagnostic decision-tree format and are
# excluded from this rule.
#
# Usage:
#   bats test_checklist_column_removed.bats

setup() {
    load "helpers/setup_test_env.sh"
    setup_test_env
}

teardown() {
    teardown_test_env
}

# ─── AC-1: No `必须` column in any evaluator ─────────────────────────────────
# NOTE: We check for '必须' ONLY as a pipe-delimited table column (the old format
# had | 必须 | in the header). The Chinese word 必须 also appears naturally in
# evidence text (e.g., "问题部分必须包含"), so we restrict the check to table
# structure to avoid false positives.

@test "AC-1: proposal-evaluator has no 必须 column in Static Checklist" {
    local table
    table="$(sed -n '/^## Static Checklist/,/^## /p' "$AGENTS_DIR/proposal-evaluator.md" 2>/dev/null | grep -E '^\|' || true)"
    if echo "$table" | grep -q '| 必须 |'; then
        echo "FAIL: Static Checklist table in proposal-evaluator.md still has '必须' column" >&2
        echo "$table" | grep '| 必须 |' >&2
        return 1
    fi
}

@test "AC-1: test-design-evaluator has no 必须 column in Static Checklist" {
    local table
    table="$(sed -n '/^## Static Checklist/,/^## /p' "$AGENTS_DIR/test-design-evaluator.md" 2>/dev/null | grep -E '^\|' || true)"
    if echo "$table" | grep -q '| 必须 |'; then
        echo "FAIL: Static Checklist table in test-design-evaluator.md still has '必须' column" >&2
        echo "$table" | grep '| 必须 |' >&2
        return 1
    fi
}

@test "AC-1: dev-design-evaluator has no 必须 column in Static Checklist" {
    local table
    table="$(sed -n '/^## Static Checklist/,/^## /p' "$AGENTS_DIR/dev-design-evaluator.md" 2>/dev/null | grep -E '^\|' || true)"
    if echo "$table" | grep -q '| 必须 |'; then
        echo "FAIL: Static Checklist table in dev-design-evaluator.md still has '必须' column" >&2
        echo "$table" | grep '| 必须 |' >&2
        return 1
    fi
}

@test "AC-1: code-review-evaluator has no 必须 column in Static Checklist" {
    local table
    table="$(sed -n '/^## Static Checklist/,/^## /p' "$AGENTS_DIR/code-review-evaluator.md" 2>/dev/null | grep -E '^\|' || true)"
    if echo "$table" | grep -q '| 必须 |'; then
        echo "FAIL: Static Checklist table in code-review-evaluator.md still has '必须' column" >&2
        echo "$table" | grep '| 必须 |' >&2
        return 1
    fi
}

@test "AC-1: test-gen-evaluator has no 必须 column in Static Checklist" {
    local table
    table="$(sed -n '/^## Static Checklist/,/^## /p' "$AGENTS_DIR/test-gen-evaluator.md" 2>/dev/null | grep -E '^\|' || true)"
    if echo "$table" | grep -q '| 必须 |'; then
        echo "FAIL: Static Checklist table in test-gen-evaluator.md still has '必须' column" >&2
        echo "$table" | grep '| 必须 |' >&2
        return 1
    fi
}

@test "AC-1: implementation-evaluator has no 必须 column in Static Checklist" {
    local table
    table="$(sed -n '/^## Static Checklist/,/^## /p' "$AGENTS_DIR/implementation-evaluator.md" 2>/dev/null | grep -E '^\|' || true)"
    if echo "$table" | grep -q '| 必须 |'; then
        echo "FAIL: Static Checklist table in implementation-evaluator.md still has '必须' column" >&2
        echo "$table" | grep '| 必须 |' >&2
        return 1
    fi
}

@test "AC-1: acceptance-evaluator has no 必须 column in Static Checklist" {
    local table
    table="$(sed -n '/^## Static Checklist/,/^## /p' "$AGENTS_DIR/acceptance-evaluator.md" 2>/dev/null | grep -E '^\|' || true)"
    if echo "$table" | grep -q '| 必须 |'; then
        echo "FAIL: Static Checklist table in acceptance-evaluator.md still has '必须' column" >&2
        echo "$table" | grep '| 必须 |' >&2
        return 1
    fi
}

# ─── Table Format: Each Static Checklist has exactly 3 columns ────────────────

@test "AC-1/Table: Static Checklist column headers match expected 3-column format (ID | 检查项 | 判断依据)" {
    # Parse each evaluator's Static Checklist table header row
    local evaluators=(
        proposal-evaluator
        test-design-evaluator
        dev-design-evaluator
        code-review-evaluator
        test-gen-evaluator
        implementation-evaluator
        acceptance-evaluator
    )
    local all_pass=true
    for ev in "${evaluators[@]}"; do
        local file="$AGENTS_DIR/$ev.md"

        # Extract the Static Checklist section and find the table header row
        # Expected header: | ID | 检查项 | 判断依据 |
        local header
        header="$(sed -n '/^## Static Checklist/,/^## /p' "$file" 2>/dev/null | grep -E '^\|.*\|.*\|.*\|' | head -1 || true)"

        if [[ -z "$header" ]]; then
            echo "FAIL: $ev.md: No table header found in Static Checklist section" >&2
            all_pass=false
            continue
        fi

        # Count columns: split by unescaped pipe
        # Strip leading/trailing pipes then count columns
        local trimmed
        trimmed="$(echo "$header" | sed 's/^[[:space:]]*|//; s/|[[:space:]]*$//')"
        local col_count
        col_count="$(echo "$trimmed" | awk -F'|' '{print NF}')"

        if [[ "$col_count" -ne 3 ]]; then
            echo "FAIL: $ev.md: Expected 3 columns, got $col_count" >&2
            echo "  Header: $header" >&2
            all_pass=false
            continue
        fi

        # Verify column header text (strip whitespace for comparison)
        local col1 col2 col3
        col1="$(echo "$trimmed" | awk -F'|' '{gsub(/^[[:space:]]+|[[:space:]]+$/, "", $1); print $1}')"
        col2="$(echo "$trimmed" | awk -F'|' '{gsub(/^[[:space:]]+|[[:space:]]+$/, "", $2); print $2}')"
        col3="$(echo "$trimmed" | awk -F'|' '{gsub(/^[[:space:]]+|[[:space:]]+$/, "", $3); print $3}')"

        if [[ "$col1" != "ID" ]]; then
            echo "FAIL: $ev.md: Column 1 expected 'ID', got '$col1'" >&2
            all_pass=false
        fi
        if [[ "$col2" != "检查项" ]]; then
            echo "FAIL: $ev.md: Column 2 expected '检查项', got '$col2'" >&2
            all_pass=false
        fi
        if [[ "$col3" != "判断依据" ]]; then
            echo "FAIL: $ev.md: Column 3 expected '判断依据', got '$col3'" >&2
            all_pass=false
        fi
    done
    [[ "$all_pass" == "true" ]]
}

@test "AC-1/Table: Static Checklist table rows have consistent column count (no orphaned 4th column)" {
    local evaluators=(
        proposal-evaluator
        test-design-evaluator
        dev-design-evaluator
        code-review-evaluator
        test-gen-evaluator
        implementation-evaluator
        acceptance-evaluator
    )
    local all_pass=true
    for ev in "${evaluators[@]}"; do
        local file="$AGENTS_DIR/$ev.md"

        # Extract all table rows in the Static Checklist section
        # Skip the separator line (|---|...|)
        local rows
        rows="$(sed -n '/^## Static Checklist/,/^## /p' "$file" 2>/dev/null | grep -E '^\|' | grep -v '|---' || true)"

        if [[ -z "$rows" ]]; then
            echo "FAIL: $ev.md: No table data rows found in Static Checklist" >&2
            all_pass=false
            continue
        fi

        local line_num=0
        while IFS= read -r row; do
            line_num=$((line_num + 1))
            local trimmed
            trimmed="$(echo "$row" | sed 's/^[[:space:]]*|//; s/|[[:space:]]*$//')"
            local col_count
            col_count="$(echo "$trimmed" | awk -F'|' '{print NF}')"

            if [[ "$col_count" -ne 3 ]]; then
                echo "FAIL: $ev.md: Row $line_num has $col_count columns (expected 3)" >&2
                echo "  Row content: $row" >&2
                all_pass=false
            fi
        done <<< "$rows"
    done
    [[ "$all_pass" == "true" ]]
}
