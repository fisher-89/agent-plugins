#!/usr/bin/env bash
#
# static-check.sh — subagentStop hook: run static analysis before generator ends
#
# Calls dev-team-cli.cjs run_static_analysis and returns {} on success or
# followup_message on failure so the agent can fix lint/type errors.
#
# Input:  stdin  — subagentStop event JSON (ignored)
# Output: stdout — {} or { "followup_message": "..." }

set -o pipefail

readonly CLI_PATH="${CLAUDE_PLUGIN_ROOT}/bin/dev-team-cli.cjs"
readonly FOLLOWUP_PREFIX=$'静态检查未通过，请修复以下错误后重新提交：\n\n'

# Escape a string for use as a JSON string value
json_escape() {
  local value="$1"
  value="${value//\\/\\\\}"
  value="${value//\"/\\\"}"
  value="${value//$'\n'/\\n}"
  value="${value//$'\r'/\\r}"
  value="${value//$'\t'/\\t}"
  printf '%s' "$value"
}

output_pass() {
  printf '{}\n'
  exit 0
}

output_followup() {
  local cli_output="$1"
  local message="${FOLLOWUP_PREFIX}${cli_output}"
  local escaped
  escaped=$(json_escape "$message")
  printf '{"followup_message":"%s"}\n' "$escaped"
  exit 0
}

main() {
  if [ ! -f "$CLI_PATH" ]; then
    output_followup "dev-team CLI not found at ${CLI_PATH}"
  fi

  local cli_stdout cli_stderr cli_output cli_exit
  cli_stdout=$(mktemp)
  cli_stderr=$(mktemp)

  node "$CLI_PATH" run_static_analysis >"$cli_stdout" 2>"$cli_stderr"
  cli_exit=$?

  cli_output=$(cat "$cli_stdout" "$cli_stderr")
  rm -f "$cli_stdout" "$cli_stderr"

  if [ "$cli_exit" -eq 0 ]; then
    output_pass
  fi

  output_followup "$cli_output"
}

main "$@"
