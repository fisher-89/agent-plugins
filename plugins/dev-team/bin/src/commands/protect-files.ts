// protect-files.ts — PreToolUse hook: deny writes / deletes / bulk-reverts on
// protected files (openspec workflow artifacts + user-configured globs).

import { readFileSync } from 'node:fs';
import * as path from 'node:path';

import { readConfig } from '../lib/config';
import { matchGlob } from '../lib/glob';
import { getProjectDir } from '../lib/project-root';
import {
  extractFileOps,
  extractGitOps,
  isFlag,
  maskHeredocs,
  maskQuotedAndHeredoc,
  tokenize,
} from '../lib/shell-file-ops';
import { isPlainObject } from '../utils';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

interface ProtectedPattern {
  glob: string;
  reason?: string;
}

interface ProtectionResult {
  matched: boolean;
  matchedGlob?: string;
  reason?: string;
}

// ---------------------------------------------------------------------------
// Pattern Loading
// ---------------------------------------------------------------------------

/**
 * Load and merge patterns from built-in defaults and user configuration.
 * User patterns come from openspec/config.json `write_protection.files`,
 * validated through the Zod schema via readConfig().
 */
function loadPatterns(projectRoot: string): ProtectedPattern[] {
  const patterns: ProtectedPattern[] = [];

  // Add built-in patterns inline to ensure perTest coverage analysis
  // attributes mutations on these strings to each calling test
  patterns.push({
    glob: '**/openspec/changes/**/workflow.json',
    reason:
      '该文件受写入保护：%s。detected via %t。请使用 phase_log / backtrack / change_create MCP 工具替代。',
  });
  patterns.push({
    glob: '**/openspec/config.json',
    reason: '该文件受写入保护：%s。detected via %t。请提供修改方案，通知用户自行操作。',
  });

  const config = readConfig(projectRoot);
  const files = config.write_protection?.files ?? [];
  for (const file of files) {
    if (!file.glob) {
      continue;
    }
    patterns.push({
      glob: path.isAbsolute(file.glob) ? file.glob : `**/${file.glob}`,
      reason: file.reason,
    });
  }

  return patterns;
}

// ---------------------------------------------------------------------------
// Path Protection Check
// ---------------------------------------------------------------------------

/**
 * Check a file path against an array of ProtectedPattern.
 * Returns the matched result or { matched: false }.
 * Uses matchGlob (picomatch) for glob matching — no hand-written globToRegex.
 *
 * @internal Exported for testing only.
 */
function isProtected(filePath: string, patterns: ProtectedPattern[]): ProtectionResult {
  if (!filePath) return { matched: false };
  const normalized = filePath.replace(/\\/g, '/');

  for (const p of patterns) {
    if (matchGlob(normalized, p.glob)) {
      return {
        matched: true,
        matchedGlob: p.glob,
        reason: p.reason,
      };
    }
  }

  return { matched: false };
}

// ---------------------------------------------------------------------------
// Deny Reason Builder
// ---------------------------------------------------------------------------

/**
 * Build a human-readable deny reason.
 * If the pattern has a custom reason, use it with %s (file path) and %t (tool name) substitution.
 * Otherwise use a generic fallback message.
 *
 * @internal Exported for testing only.
 */
function buildDenyReason(
  pattern: ProtectionResult | null,
  filePath: string,
  toolName: string,
): string {
  if (pattern && pattern.reason) {
    return pattern.reason.replace(/%s/g, filePath).replace(/%t/g, toolName);
  }
  return `该文件受写入保护：${filePath}。detected via ${toolName}。`;
}

/**
 * Deny reason for bulk-restore / bulk-clean commands: they rewrite the
 * workspace wholesale and are invisible to the file recorder, so they are
 * denied instead of recorded.
 */
function buildBulkRevertDenyReason(toolName: string, detail: string): string {
  return (
    `该命令被拦截（detected via ${toolName}）：${detail}。` +
    '批量还原/清理对文件清单记录器不可见；请改用逐路径 git restore <path>（可被记录并折叠为净 untouched）。'
  );
}

// ---------------------------------------------------------------------------
// Shell Command Checks
// ---------------------------------------------------------------------------

/** Directory globs for the bulk-revert interception rules. */
const OPENSPEC_GLOB = '**/openspec/**';
const OPENSPEC_CHANGES_GLOB = '**/openspec/changes/**';

/**
 * Deny when an extracted write/delete path matches a protected glob.
 * Revert ops are not denied here — they are the sanctioned undo action,
 * recorded by the PostToolUse recorder (openspec workflow artifacts are
 * handled by checkBulkRevert).
 */
function checkExtractedOps(
  cmd: string,
  patterns: ProtectedPattern[],
  toolName: string,
): string | null {
  for (const op of extractFileOps(cmd)) {
    if (op.op === 'revert') continue;
    const result = isProtected(op.path, patterns);
    if (result.matched) {
      return buildDenyReason(result, op.path, toolName);
    }
  }
  return null;
}

/**
 * Intercept bulk restore/clean commands that rewrite the workspace wholesale
 * and are invisible to the file recorder:
 * - `git stash` (bare / push / pop / apply / branch) → deny; `list` / `show` → allow;
 * - `git clean` without path arguments, or with any path hitting `openspec/**`
 *   → deny; `-n` / `--dry-run` → allow;
 * - `git restore` / `git checkout --` targeting `openspec/changes/**` workflow
 *   artifacts → deny; source-code targets → allow (recorded as revert).
 */
function checkBulkRevert(cmd: string, toolName: string): string | null {
  // `git stash` / `git clean` mentions inside quoted strings or heredoc bodies
  // are data, not commands: names are located on variant B (masked) and args
  // sliced from variant A (quotes preserved) — see shell-file-ops masking.
  const nameText = maskQuotedAndHeredoc(cmd);
  const argText = maskHeredocs(cmd);
  const stashRe = /\bgit\s+stash\b([^\n;&|]*)/gi;
  stashRe.lastIndex = 0;
  let m: RegExpExecArray | null;
  while ((m = stashRe.exec(nameText)) !== null) {
    const argsStart = m.index + m[0].length - m[1].length;
    const args = argText.slice(argsStart, m.index + m[0].length).trim();
    if (!/^(list|show)\b/.test(args)) {
      return buildBulkRevertDenyReason(toolName, 'git stash 会批量改写工作区');
    }
  }

  const cleanRe = /\bgit\s+clean\b([^\n;&|]*)/gi;
  cleanRe.lastIndex = 0;
  while ((m = cleanRe.exec(nameText)) !== null) {
    const argsStart = m.index + m[0].length - m[1].length;
    const tokens = tokenize(argText.slice(argsStart, m.index + m[0].length));
    const isDryRun = tokens.some((t) => t === '--dry-run' || /^-[^-]*n/.test(t));
    if (isDryRun) continue;
    const paths = tokens.filter((t) => !isFlag(t));
    const hitsOpenspec =
      paths.length === 0 || paths.some((p) => matchGlob(p.replace(/\\/g, '/'), OPENSPEC_GLOB));
    if (hitsOpenspec) {
      return buildBulkRevertDenyReason(toolName, 'git clean 会批量删除文件');
    }
  }

  for (const op of extractGitOps(cmd)) {
    if (op.op === 'revert' && matchGlob(op.path.replace(/\\/g, '/'), OPENSPEC_CHANGES_GLOB)) {
      return buildBulkRevertDenyReason(
        toolName,
        `git restore 不允许作用于 openspec/changes/** 工作流产物: ${op.path}`,
      );
    }
  }

  return null;
}

/**
 * Shared shell-command check: python/node exemption → extracted-path
 * protection (write + delete) → bulk-revert interception.
 */
function checkShellCommand(
  cmd: string,
  patterns: ProtectedPattern[],
  toolName: string,
): string | null {
  const normalized = cmd.replace(/\\/g, '/');

  // Exempt python/node script runners
  if (/^(python|python3|node)\s/.test(normalized)) return null;

  return checkExtractedOps(normalized, patterns, toolName) ?? checkBulkRevert(normalized, toolName);
}

/**
 * Check whether a bash command attempts to write to or delete a protected file.
 * Returns a deny reason string if protected, or null if the command is allowed.
 */
function checkBashCommand(cmd: string, patterns: ProtectedPattern[]): string | null {
  return checkShellCommand(cmd, patterns, 'Bash');
}

/**
 * Check whether a PowerShell command attempts to write to or delete a
 * protected file. Returns a deny reason string if protected, or null if the
 * command is allowed.
 */
function checkPowerShellCommand(cmd: string, patterns: ProtectedPattern[]): string | null {
  return checkShellCommand(cmd, patterns, 'PowerShell');
}

// ---------------------------------------------------------------------------
// Protected Path Matching
// ---------------------------------------------------------------------------

/**
 * Check a single file path against protected patterns.
 * Returns the deny reason if the path matches a protected pattern, or null.
 */
function matchProtectedPath(
  filePath: string,
  patterns: ProtectedPattern[],
  toolName: string,
): string | null {
  const result = isProtected(filePath, patterns);
  return result.matched ? buildDenyReason(result, filePath, toolName) : null;
}

// ---------------------------------------------------------------------------
// Tool Access Evaluation
// ---------------------------------------------------------------------------

/**
 * Evaluate whether a hook event's tool invocation is allowed to access
 * protected files.
 *
 * Parses the stdin JSON, routes to the appropriate checker based on tool_name,
 * and returns a deny reason string or null (fail-open).
 */
function evaluateToolAccess(raw: string, patterns: ProtectedPattern[]): string | null {
  if (!raw || !raw.trim()) return null;

  let parsed: Record<string, unknown>;
  try {
    parsed = JSON.parse(raw);
  } catch {
    return null;
  }

  const toolName = parsed.tool_name;
  if (!toolName || typeof toolName !== 'string') return null;

  const toolInput = isPlainObject(parsed.tool_input) ? parsed.tool_input : undefined;

  if (toolName === 'Write' || toolName === 'Edit' || toolName === 'StrReplace') {
    const filePath = toolInput?.file_path;
    if (!filePath || typeof filePath !== 'string') return null;
    return matchProtectedPath(filePath, patterns, toolName);
  }

  if (toolName === 'Shell') {
    const command = toolInput?.command;
    if (!command || typeof command !== 'string') return null;
    return checkBashCommand(command, patterns) ?? checkPowerShellCommand(command, patterns);
  }

  if (toolName === 'Bash') {
    const command = toolInput?.command;
    if (!command || typeof command !== 'string') return null;
    return checkBashCommand(command, patterns);
  }

  if (toolName === 'PowerShell') {
    const command = toolInput?.command;
    if (!command || typeof command !== 'string') return null;
    return checkPowerShellCommand(command, patterns);
  }

  return null;
}

// ---------------------------------------------------------------------------
// Hook Response Output
// ---------------------------------------------------------------------------

/** Write allow/deny hook response JSON to stdout. */
function writeHookResponse(denyReason: string | null): void {
  if (denyReason !== null) {
    process.stdout.write(
      `${JSON.stringify({
        hookSpecificOutput: {
          hookEventName: 'PreToolUse',
          permissionDecision: 'deny',
          permissionDecisionReason: denyReason,
        },
      })}\n`,
    );
  } else {
    process.stdout.write(
      `${JSON.stringify({
        hookSpecificOutput: {
          hookEventName: 'PreToolUse',
          permissionDecision: 'allow',
        },
      })}\n`,
    );
  }
}

// ---------------------------------------------------------------------------
// protect-files subcommand
// ---------------------------------------------------------------------------

/**
 * protect-files subcommand entry point.
 *
 * Reads PreToolUse event JSON from stdin, loads protection patterns,
 * checks the tool invocation, and outputs allow/deny JSON to stdout.
 */
export function runProtectFiles(): void {
  const patterns = loadPatterns(getProjectDir());
  const denyReason = evaluateToolAccess(readFileSync(0, 'utf-8'), patterns);
  writeHookResponse(denyReason);
}
