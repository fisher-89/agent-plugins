#!/usr/bin/env bash
#
# protect-eval.sh — PreToolUse hook: protect eval.json from direct writes
#
# Intercepts Write, Edit, and Bash tool calls that try to modify
# openspec/changes/<name>/eval.json. Allowed writing method:
# mcp__plugin_dev-team_dev-team__phase_log MCP tool.
#
# Input:  stdin  — JSON with { tool_name, tool_input: { file_path?, command? } }
# Output: stdout — JSON with { hookSpecificOutput: { permissionDecision, ... } }

set -o pipefail

# ---------------------------------------------------------------------------
# Configuration
# ---------------------------------------------------------------------------
readonly DENY_REASON_TEMPLATE='eval.json 只能通过 phase_log MCP 工具写入。检测到对变更 "%s" 的 eval.json 的直接写入（通过 %s 工具）。请使用 mcp__plugin_dev-team_dev-team__phase_log 工具替代。直接写入将绕过校验、破坏数据完整性并导致审计追溯失效。'

# ---------------------------------------------------------------------------
# Output helpers
# ---------------------------------------------------------------------------

# Print allow decision to stdout
output_allow() {
  cat <<'EOF'
{
  "hookSpecificOutput": {
    "hookEventName": "PreToolUse",
    "permissionDecision": "allow"
  }
}
EOF
  exit 0
}

# Print deny decision with a reason string
# Arguments: $1 — reason text (will be used as JSON string value)
output_deny() {
  local reason="$1"
  # Escape backslashes and double-quotes for JSON
  reason="${reason//\\/\\\\}"
  reason="${reason//\"/\\\"}"
  # Remove newlines
  reason="${reason//$'\n'/\\n}"
  reason="${reason//$'\r'/\\r}"

  printf '{\n  "hookSpecificOutput": {\n    "hookEventName": "PreToolUse",\n    "permissionDecision": "deny",\n    "permissionDecisionReason": "%s"\n  }\n}\n' "$reason"
  exit 0
}

# ---------------------------------------------------------------------------
# JSON field extraction
# ---------------------------------------------------------------------------

# Extract a top-level or nested string field from JSON
# Usage: extract_json_field <json> <keypath>
# Keypath uses dot notation: "tool_name" or "tool_input.file_path"
# Returns empty string if not found
extract_json_field() {
  local json="$1"
  local keypath="$2"

  if command -v jq &>/dev/null; then
    # Use jq for robust parsing
    case "$keypath" in
      tool_name)              echo "$json" | jq -r '.tool_name // empty' 2>/dev/null ;;
      tool_input.file_path)   echo "$json" | jq -r '.tool_input.file_path // empty' 2>/dev/null ;;
      tool_input.command)     echo "$json" | jq -r '.tool_input.command // empty' 2>/dev/null ;;
      *)                      echo "" ;;
    esac
  elif command -v node &>/dev/null; then
    # Fallback: use Node.js for robust JSON parsing (always available in plugin env)
    local node_field
    case "$keypath" in
      tool_name)            node_field="tool_name" ;;
      tool_input.file_path) node_field="tool_input?.file_path" ;;
      tool_input.command)   node_field="tool_input?.command" ;;
      *)                    echo ""; return ;;
    esac
    node -e "
var d = '';
process.stdin.setEncoding('utf8');
process.stdin.on('data', function(c) { d += c; });
process.stdin.on('end', function() {
  try {
    var j = JSON.parse(d);
    var v = eval('j.' + process.argv[1] + '');
    process.stdout.write((v != null ? v : '') + '\n');
  } catch(e) {
    process.stdout.write('\n');
  }
});
" "$node_field" <<< "$json" 2>/dev/null
  else
    # Fallback: extract with grep/sed for simple JSON without nested escaping
    case "$keypath" in
      tool_name)
        echo "$json" | grep -oE '"tool_name"[[:space:]]*:[[:space:]]*"[^"]*"' | head -1 | sed 's/.*"tool_name"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/'
        ;;
      tool_input.file_path)
        echo "$json" | grep -oE '"file_path"[[:space:]]*:[[:space:]]*"[^"]*"' | head -1 | sed 's/.*"file_path"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/'
        ;;
      tool_input.command)
        # Command strings can be long and contain escaped chars.
        # Best-effort extraction: treat as empty
        echo ""
        ;;
      *)
        echo ""
        ;;
    esac
  fi
}

# ---------------------------------------------------------------------------
# Change name extraction
# ---------------------------------------------------------------------------

# Extract change name from a path containing openspec/changes/<name>/
# Returns empty string if no match
extract_change_name() {
  local path="$1"
  local name

  name=$(echo "$path" | grep -oE 'openspec/changes/[^/]+' | sed 's|openspec/changes/||')
  echo "$name"
}

# ---------------------------------------------------------------------------
# Path matching for Write/Edit
# ---------------------------------------------------------------------------

# Check if a path targets an eval.json inside openspec/changes/
# Returns 0 (true) if it matches, 1 (false) otherwise
is_eval_json_path() {
  local path="$1"
  # Normalize backslashes to forward slashes
  path="${path//\\//}"

  # Must contain openspec/changes/ and end with eval.json
  if echo "$path" | grep -qE 'openspec/changes/.*eval\.json$'; then
    return 0
  fi
  return 1
}

# ---------------------------------------------------------------------------
# Bash command write detection
# ---------------------------------------------------------------------------

# Check if a Bash command attempts to write to eval.json
# Returns 0 (write detected), 1 (no write detected)
# Exempts commands starting with python, python3, or node
detect_bash_write() {
  local cmd="$1"

  # Normalize backslashes to forward slashes
  cmd="${cmd//\\//}"

  # Python/Node exemption: if command starts with python/python3/node
  if echo "$cmd" | grep -qE '^(python|python3|node)[[:space:]]'; then
    return 1
  fi

  # Check for eval.json in the command at all — if not present, not a write
  if ! echo "$cmd" | grep -qE 'eval\.json'; then
    return 1
  fi

  # Pattern 1: > or >> redirect to eval.json
  # Matches: "> path/eval.json" or ">> path/eval.json"
  # Does NOT match "->" (dash arrow) or "<<>" (heredoc delimiter artifact)
  if echo "$cmd" | grep -qE '(^|[^-])>{1,2}[[:space:]]+[^[:space:];|`$&()]*eval\.json'; then
    return 0
  fi

  # Pattern 2: tee piped to eval.json
  # Matches: "| tee path/eval.json" or "| tee -a path/eval.json"
  if echo "$cmd" | grep -qE '(^|[|&;])[[:space:]]*tee[[:space:]]+.*eval\.json'; then
    return 0
  fi

  # Pattern 3: heredoc (<<) with eval.json as target
  # A heredoc <<WORD combined with eval.json in the command indicates write intent
  if echo "$cmd" | grep -qE '<<[^<]'; then
    return 0
  fi

  # Pattern 4: file descriptor redirect >& to eval.json
  # Matches: "echo '[]' >& path/eval.json" (redirect with clobber)
  if echo "$cmd" | grep -qE '>&[[:space:]]*[^[:space:];|`$&()]*eval\.json'; then
    return 0
  fi

  return 1
}

# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------

main() {
  # Read all stdin
  local input
  input=$(cat)

  # Exit early on empty input (fail open)
  if [ -z "$input" ]; then
    output_allow
  fi

  # Extract fields
  local tool_name file_path command_str
  tool_name=$(extract_json_field "$input" "tool_name")
  file_path=$(extract_json_field "$input" "tool_input.file_path")
  command_str=$(extract_json_field "$input" "tool_input.command")

  # If we can't determine the tool, allow (fail open)
  if [ -z "$tool_name" ]; then
    output_allow
  fi

  # Normalize paths
  file_path="${file_path//\\//}"

  case "$tool_name" in
    Write|Edit)
      # ---- Write/Edit: check file_path ----
      if [ -z "$file_path" ]; then
        output_allow
      fi

      if is_eval_json_path "$file_path"; then
        local change_name
        change_name=$(extract_change_name "$file_path")
        change_name="${change_name:-未知}"
        local reason
        reason=$(printf "$DENY_REASON_TEMPLATE" "$change_name" "$tool_name")
        output_deny "$reason"
      fi

      output_allow
      ;;

    Bash)
      # ---- Bash: check command ----
      if [ -z "$command_str" ]; then
        output_allow
      fi

      if detect_bash_write "$command_str"; then
        local change_name
        change_name=$(extract_change_name "$command_str")
        change_name="${change_name:-未知}"
        local reason
        reason=$(printf "$DENY_REASON_TEMPLATE" "$change_name" "$tool_name")
        output_deny "$reason"
      fi

      output_allow
      ;;

    *)
      # Unknown tool — allow (fail open)
      output_allow
      ;;
  esac
}

main "$@"
