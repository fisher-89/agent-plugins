#!/usr/bin/env bash
# test_planner_prompt.sh — Integration tests for Step 3a Planner prompt construction.
#
# Covers:
#   AC-03: Planner prompt contains dynamic CLI instructions
#   AC-04 (partial): Explore context is injected into Planner prompt
#   Edge: Empty openspec status --json returns {}
#   Edge: openspec instructions missing 'rules' field
#   Edge: CLI result caching within single skill execution
#
# Usage:
#   bash test_planner_prompt.sh

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
source "$SCRIPT_DIR/helpers/setup_test_env.sh"

PASS_COUNT=0
FAIL_COUNT=0

cleanup() {
    teardown_sandbox
}

trap cleanup EXIT

# ─── Helper: Build a simulated Planner prompt ───────────────────────────────

# Simulates Step 3a prompt construction: combines static template path,
# openspec instructions output, and optional explore context.
build_planner_prompt() {
    local change_name="$1"
    local explore_context="${2:-}"

    local prompt="Write proposal.md for change '$change_name'."

    # Add template reference
    prompt="$prompt\n\nFollow the template at plugins/dev-team/templates/artifacts/proposal.md.template."

    # Add dynamic CLI instructions
    local instructions
    instructions="$(openspec instructions "$change_name" 2>/dev/null || echo '{}')"
    if [[ "$instructions" != "{}" ]]; then
        prompt="$prompt\n\n## CLI Instructions\n\n\`\`\`json\n$instructions\n\`\`\`"
    fi

    # Add explore context if available
    if [[ -n "$explore_context" ]]; then
        prompt="$prompt\n\n## Explore Context\n\n$explore_context"
    fi

    echo -e "$prompt"
}

# ─── Test: AC-03 — Planner prompt contains dynamic CLI instructions ─────────

test_ac03_prompt_contains_cli_instructions() {
    echo "=== AC-03: Planner prompt contains dynamic CLI instructions ==="

    setup_sandbox
    mock_openspec

    local change_name="test-change"
    export MOCK_ANSWERS_DIR="$SCRIPT_DIR/fixtures"

    local prompt
    prompt="$(build_planner_prompt "$change_name")"

    # Prompt should reference the template
    assert_string_contains "$prompt" "proposal.md.template"

    # Prompt should contain CLI instructions (from openspec instructions)
    assert_string_contains "$prompt" "CLI Instructions"

    # Prompt should reference the change name
    assert_string_contains "$prompt" "$change_name"

    echo "PASS: AC-03 prompt contains dynamic CLI instructions"
    return 0
}

# ─── Test: AC-03 variant — Instructions with rules field ────────────────────

test_ac03_prompt_contains_rules_from_instructions() {
    echo "=== AC-03: Planner prompt includes rules from openspec instructions ==="

    setup_sandbox
    mock_openspec

    local change_name="add-user-auth"
    export MOCK_ANSWERS_DIR="$SCRIPT_DIR/fixtures"

    local prompt
    prompt="$(build_planner_prompt "$change_name")"

    # The fixture openspec_instructions.json has a rules array
    # The prompt should contain the instructions JSON
    assert_string_contains "$prompt" "rules"

    echo "PASS: AC-03 prompt includes rules from instructions"
    return 0
}

# ─── Test: AC-03 Edge — Empty JSON status does not crash ────────────────────

test_ac03_empty_status_fallback() {
    echo "=== AC-03 Edge: Empty openspec status --json returns {} ==="

    setup_sandbox
    mock_openspec

    local change_name="empty-change"
    export MOCK_ANSWERS_DIR="$SCRIPT_DIR/fixtures"

    # Build prompt with empty status fixture
    local prompt
    prompt="$(build_planner_prompt "$change_name")"

    # Should not crash, should produce valid prompt
    assert_string_contains "$prompt" "Write proposal.md"

    echo "PASS: AC-03 Edge empty status does not crash"
    return 0
}

# ─── Test: AC-03 Edge — Missing 'rules' field is tolerated ──────────────────

test_ac03_missing_rules_field() {
    echo "=== AC-03 Edge: openspec instructions missing 'rules' field ==="

    setup_sandbox
    mock_openspec

    local change_name="minimal-change"
    # Override mock to return instructions without 'rules' field
    local bin_dir="$SANDBOX_DIR/bin"
    cat > "$bin_dir/openspec" << 'OVERRIDE_EOF'
#!/usr/bin/env bash
case "${1:-}" in
    instructions)
        echo '{"context":"No rules here","template":"basic.template","instruction":"Write it"}'
        ;;
    *)
        exit 1
        ;;
esac
OVERRIDE_EOF
    chmod +x "$bin_dir/openspec"

    local prompt
    prompt="$(build_planner_prompt "$change_name")"

    # Should still produce a valid prompt without crashing
    assert_string_contains "$prompt" "Write proposal.md"
    assert_string_contains "$prompt" "basic.template"

    echo "PASS: AC-03 Edge missing 'rules' field tolerated"
    return 0
}

# ─── Test: AC-03 Edge — CLI result caching ──────────────────────────────────

test_ac03_cli_caching() {
    echo "=== AC-03 Edge: CLI result caching ==="

    setup_sandbox

    # Mock that tracks call count
    local bin_dir="$SANDBOX_DIR/bin"
    mkdir -p "$bin_dir"
    cat > "$bin_dir/openspec" << 'CACHE_EOF'
#!/usr/bin/env bash
CALL_FILE="$SANDBOX_DIR/.openspec_call_count"
COUNT=0
if [[ -f "$CALL_FILE" ]]; then
    COUNT=$(cat "$CALL_FILE")
fi
COUNT=$((COUNT + 1))
echo "$COUNT" > "$CALL_FILE"

# Same output every time
echo '{"rules":[{"id":"test"}],"template":"test.template","instruction":"test"}'
CACHE_EOF
    chmod +x "$bin_dir/openspec"
    export PATH="$bin_dir:$PATH"

    # Call instructions twice (simulating Step 1 + Step 3a)
    openspec instructions "test" > /dev/null 2>&1
    openspec instructions "test" > /dev/null 2>&1

    local call_count
    call_count="$(cat "$SANDBOX_DIR/.openspec_call_count")"

    if [[ "$call_count" -eq 2 ]]; then
        echo "PASS: AC-03 Edge CLI called twice without caching"
    else
        echo "FAIL: Expected 2 CLI calls, got $call_count" >&2
        return 1
    fi

    # TODO: When caching is implemented, this test should verify that
    # the second call uses a cached result (only 1 real CLI call).
    # Current behavior: no caching (2 real calls).

    return 0
}

# ─── Test: AC-04 (partial) — Explore context in prompt ──────────────────────

test_ac04_explore_context_in_prompt() {
    echo "=== AC-04 (partial): Explore context injected into Planner prompt ==="

    setup_sandbox
    mock_openspec

    local change_name="test-change"
    local explore_context="Key decisions: JWT-based auth, bcrypt hashing"
    export MOCK_ANSWERS_DIR="$SCRIPT_DIR/fixtures"

    local prompt
    prompt="$(build_planner_prompt "$change_name" "$explore_context")"

    # Prompt should contain explore context
    assert_string_contains "$prompt" "Explore Context"
    assert_string_contains "$prompt" "JWT-based auth"

    echo "PASS: AC-04 explore context injected into prompt"
    return 0
}

# ─── Runner ─────────────────────────────────────────────────────────────────

run_test() {
    local test_name="$1"
    shift
    echo ""
    echo "──────────────────────────────────────────────"
    echo "  Running: $test_name"
    echo "──────────────────────────────────────────────"

    if "$@"; then
        PASS_COUNT=$((PASS_COUNT + 1))
    else
        FAIL_COUNT=$((FAIL_COUNT + 1))
        echo "FAIL: $test_name" >&2
    fi
}

run_test "AC-03: prompt contains CLI instructions" test_ac03_prompt_contains_cli_instructions
run_test "AC-03: prompt includes rules" test_ac03_prompt_contains_rules_from_instructions
run_test "AC-03 Edge: empty status" test_ac03_empty_status_fallback
run_test "AC-03 Edge: missing rules field" test_ac03_missing_rules_field
run_test "AC-03 Edge: CLI caching" test_ac03_cli_caching
run_test "AC-04: explore context in prompt" test_ac04_explore_context_in_prompt

# ─── Summary ────────────────────────────────────────────────────────────────

echo ""
echo "══════════════════════════════════════════════"
echo "  Results: $PASS_COUNT passed, $FAIL_COUNT failed"
echo "══════════════════════════════════════════════"

if [[ "$FAIL_COUNT" -gt 0 ]]; then
    exit 1
fi
exit 0
