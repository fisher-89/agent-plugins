#!/usr/bin/env node
// protect-eval.mjs — PreToolUse hook: protect eval.json from direct writes
//
// Intercepts Write, Edit, and Bash tool calls that try to modify
// openspec/changes/<name>/eval.json. Allowed writing method:
// mcp__plugin_dev-team_dev-team__phase_log MCP tool.
//
// Input:  stdin  — JSON with { tool_name, tool_input: { file_path?, command? } }
// Output: stdout — JSON with { hookSpecificOutput: { permissionDecision, ... } }

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const DENY_REASON_TEMPLATE =
  'eval.json 只能通过 phase_log MCP 工具写入。检测到对变更 "%s" 的 eval.json 的直接写入（通过 %s 工具）。请使用 mcp__plugin_dev-team_dev-team__phase_log 工具替代。直接写入将绕过校验、破坏数据完整性并导致审计追溯失效。';

export function isEvalJsonPath(filePath) {
  if (!filePath) return false;
  const normalized = filePath.replace(/\\/g, '/');
  return /openspec\/changes\/.*eval\.json$/.test(normalized);
}

export function detectBashWrite(cmd) {
  if (!cmd) return false;
  const normalized = cmd.replace(/\\/g, '/');

  if (/^(python|python3|node)\s/.test(normalized)) return false;
  if (!/eval\.json/.test(normalized)) return false;

  // > or >> redirect (not ->)
  if (/(^|[^-])>{1,2}\s+[^\s;|`$&()]*eval\.json/.test(normalized)) return true;
  // tee
  if (/(^|[|&;])\s*tee\s+.*eval\.json/.test(normalized)) return true;
  // heredoc
  if (/<<[^<]/.test(normalized)) return true;
  // >& redirect
  if (/>&\s*[^\s;|`$&()]*eval\.json/.test(normalized)) return true;

  return false;
}

export function extractChangeName(filePath) {
  if (!filePath) return '';
  const normalized = filePath.replace(/\\/g, '/');
  const match = normalized.match(/openspec\/changes\/([^/]+)/);
  return match ? match[1] : '';
}

export function buildDenyReason(changeName, toolName) {
  return DENY_REASON_TEMPLATE.replace('%s', changeName).replace('%s', toolName);
}

export function outputAllow() {
  return JSON.stringify({
    hookSpecificOutput: {
      hookEventName: 'PreToolUse',
      permissionDecision: 'allow',
    },
  });
}

export function outputDeny(reason) {
  return JSON.stringify({
    hookSpecificOutput: {
      hookEventName: 'PreToolUse',
      permissionDecision: 'deny',
      permissionDecisionReason: reason,
    },
  });
}

export function parseInput(raw) {
  if (!raw || !raw.trim()) return { decision: 'allow' };

  let parsed;
  try {
    parsed = JSON.parse(raw);
  } catch {
    return { decision: 'allow' };
  }

  const toolName = parsed.tool_name;
  if (!toolName) return { decision: 'allow' };

  if (toolName === 'Write' || toolName === 'Edit') {
    const filePath = parsed.tool_input?.file_path;
    if (!filePath) return { decision: 'allow' };
    if (isEvalJsonPath(filePath)) {
      const changeName = extractChangeName(filePath) || '未知';
      return { decision: 'deny', reason: buildDenyReason(changeName, toolName) };
    }
    return { decision: 'allow' };
  }

  if (toolName === 'Bash') {
    const command = parsed.tool_input?.command;
    if (!command) return { decision: 'allow' };
    if (detectBashWrite(command)) {
      const changeName = extractChangeName(command) || '未知';
      return { decision: 'deny', reason: buildDenyReason(changeName, toolName) };
    }
    return { decision: 'allow' };
  }

  return { decision: 'allow' };
}

function main() {
  const input = readFileSync(0, 'utf-8');
  const result = parseInput(input);

  if (result.decision === 'deny') {
    process.stdout.write(`${outputDeny(result.reason)}\n`);
    return;
  }

  process.stdout.write(`${outputAllow()}\n`);
}

if (process.argv[1] && fileURLToPath(import.meta.url) === path.resolve(process.argv[1])) {
  main();
}
