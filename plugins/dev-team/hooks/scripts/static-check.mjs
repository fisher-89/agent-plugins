#!/usr/bin/env node
// static-check.mjs — SubagentStop hook: run static analysis before generator ends
//
// Calls dev-team-cli.cjs run_static_analysis and returns {} on success or
// followup_message on failure so the agent can fix lint/type errors.
//
// Input:  stdin  — subagentStop event JSON (workspace_roots[0] → --project-root)
// Output: stdout — {} or { "followup_message": "..." }

import { existsSync, readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';

const FOLLOWUP_PREFIX = '静态检查未通过，请修复以下错误后重新提交：\n\n';

export function resolveCliPath(pluginRoot = process.env.CLAUDE_PLUGIN_ROOT || '') {
  if (!pluginRoot) return path.join('', 'bin', 'dev-team-cli.cjs');
  return path.join(pluginRoot, 'bin', 'dev-team-cli.cjs');
}

export function mergeCliOutput(stdout, stderr) {
  const parts = [stdout, stderr].filter((s) => s != null && s !== '');
  return parts.join('\n');
}

export function buildFollowupMessage(cliOutput) {
  return FOLLOWUP_PREFIX + (cliOutput ?? '');
}

export function formatOutput({ status, stdout = '', stderr = '' }) {
  if (status === 0) return {};
  const combined = mergeCliOutput(stdout, stderr);
  return { followup_message: buildFollowupMessage(combined) };
}

export function handleMissingCli(cliPath) {
  return {
    followup_message: buildFollowupMessage(`dev-team CLI not found at ${cliPath ?? ''}`),
  };
}

export function parseWorkspaceRoot(stdinRaw) {
  try {
    const event = JSON.parse(stdinRaw);
    const roots = event?.workspace_roots;
    if (Array.isArray(roots) && roots.length > 0 && typeof roots[0] === 'string') {
      return path.resolve(roots[0]);
    }
  } catch {
    // ignore JSON parse errors
  }
  return null;
}

function main() {
  let stdinRaw = '';
  try {
    stdinRaw = readFileSync(0, 'utf-8');
  } catch {
    // stdin may not be available
  }

  const workspaceRoot = parseWorkspaceRoot(stdinRaw);
  const cliPath = resolveCliPath();

  if (!existsSync(cliPath)) {
    process.stdout.write(`${JSON.stringify(handleMissingCli(cliPath))}\n`);
    return;
  }

  const args = [cliPath, 'run_static_analysis'];
  if (workspaceRoot) {
    args.push('--project-root', workspaceRoot);
  }

  const result = spawnSync(process.execPath, args, { encoding: 'utf-8' });

  const stdout = (result.stdout || '').trim();
  const stderr = (result.stderr || '').trim();
  const output = formatOutput({
    status: result.status ?? 1,
    stdout,
    stderr,
  });

  process.stdout.write(`${JSON.stringify(output)}\n`);
}

if (process.argv[1] && fileURLToPath(import.meta.url) === path.resolve(process.argv[1])) {
  main();
}
