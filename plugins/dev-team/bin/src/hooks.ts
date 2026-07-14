// hooks.ts — Entry point for dev-team hooks (protect-files, static-check)
//
// Bundled as CJS via vite-plus (dev-team-hooks.cjs), invoked as:
//   node dev-team-hooks.cjs protect-files   (PreToolUse hook — stdin: PreToolUse event JSON)
//   node dev-team-hooks.cjs static-check    (SubagentStop hook — stdin: SubagentStop event JSON)
//
// Architecture notes:
//   - protect-files uses matchGlob (picomatch) instead of hand-written globToRegex
//   - static-check calls runStaticAnalysis in-process instead of spawnSync

import { readFileSync } from 'node:fs';
import * as path from 'node:path';

import { runStaticAnalysis } from './commands/run-static-analysis';
import { readConfig } from './lib/config';
import { matchGlob } from './lib/glob';
import { getProjectDir } from './utils';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

interface ProtectedPattern {
  glob: string;
  reason?: string;
}

interface ToolDecision {
  decision: 'allow' | 'deny';
  reason?: string;
}

interface ProtectionResult {
  matched: boolean;
  matchedGlob?: string;
  reason?: string;
}

// ---------------------------------------------------------------------------
// Type Guards
// ---------------------------------------------------------------------------

/**
 * Type guard: value is a non-null, non-array object.
 * Allows safe narrowing from `unknown` to `Record<string, unknown>`.
 */
function isRecord(value: unknown): value is Record<string, unknown> {
  return value != null && typeof value === 'object' && !Array.isArray(value);
}

/** Sentinel return value: allow decision. */
const ALLOW: ToolDecision = Object.freeze({ decision: 'allow' });

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
    glob: 'openspec/changes/**/eval.json',
    reason: '该文件受写入保护：%s。detected via %t。请使用 phase_log MCP 工具替代。',
  });
  patterns.push({
    glob: 'openspec/config.json',
    reason: '该文件受写入保护：%s。detected via %t。请使用 config_get/config_set MCP 工具替代。',
  });

  const config = readConfig(projectRoot);
  const files = config.write_protection?.files ?? [];
  for (const file of files) {
    patterns.push({
      glob: file.glob,
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
export function isProtected(filePath: string, patterns: ProtectedPattern[]): ProtectionResult {
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
export function buildDenyReason(
  pattern: ProtectionResult | null,
  filePath: string,
  toolName: string,
): string {
  if (pattern && pattern.reason) {
    return pattern.reason.replace(/%s/g, filePath).replace(/%t/g, toolName);
  }
  return `该文件受写入保护：${filePath}。detected via ${toolName}。`;
}

// ---------------------------------------------------------------------------
// Bash Write Detection
// ---------------------------------------------------------------------------

/**
 * Single-pass: extract file paths after bash write operators.
 * If no targets are extracted, the command contains no file-write operations.
 *
 * Supported operators:
 *   - > / >> redirect (not preceded by - or part of ->)
 *   - >| redirect (noclobber override)
 *   - tee (possibly with -a or other flags)
 *   - >& redirect
 *   - heredoc via cat > / tee (captured by the > / tee patterns above)
 */
const BASH_WRITE_REGEXES: readonly RegExp[] = [
  // > and >> redirect (not preceded by - or part of ->)
  /(?:^|[^-])>{1,2}\s+['"]?([^\s;|`$&()'"]+)['"]?/g,
  // >| redirect (noclobber override)
  />\|\s+['"]?([^\s;|`$&()'"]+)['"]?/g,
  // tee (possibly with -a or other flags)
  /(?:^|[\s;|&(])\s*tee\s+(?:-[a-zA-Z]+\s+)?['"]?([^\s;|`$&()'"]+)['"]?/g,
  // >& redirect
  />&\s*['"]?([^\s;|`$&()'"]+)['"]?/g,
];

function extractBashWriteTargets(cmd: string): string[] {
  const targets: string[] = [];

  for (const re of BASH_WRITE_REGEXES) {
    // Reset lastIndex for reused regex literals in global mode
    re.lastIndex = 0;
    let m: RegExpExecArray | null;
    while ((m = re.exec(cmd)) !== null) {
      targets.push(m[1]);
    }
  }

  return targets;
}

/**
 * Detect whether a bash command tries to write to a protected file.
 * Returns { decision: 'allow' } or { decision: 'deny', reason }.
 *
 * Uses a single regex extraction pass — if no write targets are found,
 * the command is safe.
 */
function detectBashWrite(cmd: string, patterns: ProtectedPattern[]): ToolDecision {
  const normalized = cmd.replace(/\\/g, '/');

  // Exempt python/node script runners
  if (/^(python|python3|node)\s/.test(normalized)) return ALLOW;

  // Single-pass: extract + check in one flow (no separate hasBashWriteOperator guard)
  const targets = extractBashWriteTargets(normalized);

  // Check extracted targets against patterns
  for (const target of targets) {
    const result = isProtected(target, patterns);
    if (result.matched) {
      return {
        decision: 'deny',
        reason: buildDenyReason(result, target, 'Bash'),
      };
    }
  }

  return ALLOW;
}

// ---------------------------------------------------------------------------
// PowerShell Write Detection
// ---------------------------------------------------------------------------

/**
 * Extract file paths that appear after PowerShell write operators and cmdlets.
 */
function extractPowerShellWriteTargets(cmd: string): string[] {
  const targets: string[] = [];

  // Redirect operators: >, >>, *>, 1>, 2>, etc.
  let re = /(?:^|[\s;|&(])(?:\d*|\*)?>{1,2}\s+['"]?([^\s;|`$&()'",=]+)['"]?/g;
  let m: RegExpExecArray | null;
  while ((m = re.exec(cmd)) !== null) {
    targets.push(m[1]);
  }

  // PowerShell file-writing cmdlets
  const cmdlets = [
    'Set-Content',
    'Out-File',
    'Add-Content',
    'Export-Csv',
    'Export-CliXml',
    'Tee-Object',
  ];
  for (const cmdlet of cmdlets) {
    re = new RegExp(
      `\\b${cmdlet}\\s+(?:-(?:Path|FilePath)\\s+)?['"]?([^\\s;\`$&()'",=]+)['"]?`,
      'gi',
    );
    while ((m = re.exec(cmd)) !== null) {
      targets.push(m[1]);
    }
  }

  // .NET file writing methods
  const dotnetMethods = ['WriteAllText', 'WriteAllLines', 'WriteAllBytes', 'AppendAllText'];
  for (const method of dotnetMethods) {
    re = new RegExp(`\\[System\\.IO\\.File\\]::${method}\\s*\\(\\s*['"]?([^,;\\)'"]+)['"]?`, 'gi');
    while ((m = re.exec(cmd)) !== null) {
      targets.push(m[1]);
    }
  }

  return targets;
}

/**
 * Detect whether a PowerShell command tries to write to a protected file.
 * Returns { decision: 'allow' } or { decision: 'deny', reason }.
 */
function detectPowerShellWrite(cmd: string, patterns: ProtectedPattern[]): ToolDecision {
  const normalized = cmd.replace(/\\/g, '/');

  // Exempt python/node script runners
  if (/^(python|python3|node)\s/.test(normalized)) return ALLOW;

  // Extract target file paths
  const targets = extractPowerShellWriteTargets(normalized);

  // Check targets against patterns
  for (const target of targets) {
    const result = isProtected(target, patterns);
    if (result.matched) {
      return {
        decision: 'deny',
        reason: buildDenyReason(result, target, 'PowerShell'),
      };
    }
  }

  return ALLOW;
}

// ---------------------------------------------------------------------------
// Change Name Extraction
// ---------------------------------------------------------------------------

/**
 * Extract the change name from a file path under openspec/changes/.
 */
export function extractChangeName(filePath: string): string {
  const normalized = filePath.replace(/\\/g, '/');
  const match = normalized.match(/openspec\/changes\/([^/]+)/);
  return match ? match[1] : '';
}

// ---------------------------------------------------------------------------
// Output Helpers
// ---------------------------------------------------------------------------

function outputAllow(): string {
  return JSON.stringify({
    hookSpecificOutput: {
      hookEventName: 'PreToolUse',
      permissionDecision: 'allow',
    },
  });
}

function outputDeny(reason: string): string {
  return JSON.stringify({
    hookSpecificOutput: {
      hookEventName: 'PreToolUse',
      permissionDecision: 'deny',
      permissionDecisionReason: reason,
    },
  });
}

// ---------------------------------------------------------------------------
// Input Parser
// ---------------------------------------------------------------------------

/**
 * Parse the stdin JSON received from the hook runtime, check the tool
 * invocation against the protected patterns, and return a decision.
 *
 * Fail-open: returns { decision: 'allow' } when input is missing, invalid,
 * or the tool is not a write-capable tool.
 */
function parseInput(raw: string, patterns: ProtectedPattern[]): ToolDecision {
  if (!raw || !raw.trim()) return ALLOW;

  let parsed: Record<string, unknown>;
  try {
    parsed = JSON.parse(raw);
  } catch {
    return ALLOW;
  }

  const toolName = parsed.tool_name;
  if (!toolName || typeof toolName !== 'string') return ALLOW;

  const toolInput = isRecord(parsed.tool_input) ? parsed.tool_input : undefined;

  if (toolName === 'Write' || toolName === 'Edit') {
    const filePath = toolInput?.file_path;
    if (!filePath || typeof filePath !== 'string') return ALLOW;
    const result = isProtected(filePath, patterns);
    if (result.matched) {
      return {
        decision: 'deny',
        reason: buildDenyReason(result, filePath, toolName),
      };
    }
    return ALLOW;
  }

  if (toolName === 'Bash') {
    const command = toolInput?.command;
    if (!command || typeof command !== 'string') return ALLOW;
    return detectBashWrite(command, patterns);
  }

  if (toolName === 'PowerShell') {
    const command = toolInput?.command;
    if (!command || typeof command !== 'string') return ALLOW;
    return detectPowerShellWrite(command, patterns);
  }

  return ALLOW;
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
  const projectRoot = process.env.CLAUDE_PROJECT_ROOT || getProjectDir();
  const patterns = loadPatterns(projectRoot);

  const input = readFileSync(0, 'utf-8');
  const result = parseInput(input, patterns);

  if (result.decision === 'deny') {
    process.stdout.write(`${outputDeny(result.reason ?? '')}\n`);
    return;
  }

  process.stdout.write(`${outputAllow()}\n`);
}

// ---------------------------------------------------------------------------
// Stderr Capture
// ---------------------------------------------------------------------------

/**
 * Temporarily replace process.stderr.write to collect output.
 *
 * Returns a tuple [getCaptured, restore]:
 *   - getCaptured(): returns all stderr text collected so far
 *   - restore(): restores the original process.stderr.write
 *
 * Used by runStaticCheck to capture runStaticAnalysis output without
 * letting it leak to the real stderr.
 */
export function captureStderr(): [getCaptured: () => string, restore: () => void] {
  const chunks: string[] = [];
  const origWrite = process.stderr.write.bind(process.stderr);

  // The type assertion is necessary because the replacement function's
  // simplified signature doesn't match the full overloaded Writable type.
  process.stderr.write = ((chunk: string | Uint8Array): boolean => {
    chunks.push(typeof chunk === 'string' ? chunk : Buffer.from(chunk).toString());
    return true;
  }) as typeof process.stderr.write;

  function restore(): void {
    process.stderr.write = origWrite;
  }

  return [() => chunks.join(''), restore];
}

// ---------------------------------------------------------------------------
// static-check subcommand
// ---------------------------------------------------------------------------

const FOLLOWUP_PREFIX = '静态检查未通过，请修复以下错误后重新提交：\n\n';

/**
 * Parse workspace root from SubagentStop event JSON.
 *
 * Internal logic is written as a two-step check (extract first element,
 * then type-check) rather than a single compound condition, which makes
 * Stryker boundary/logical mutations individually killable.
 */
function parseWorkspaceRoot(stdinRaw: string): string | null {
  try {
    const parsed: unknown = JSON.parse(stdinRaw);
    if (!isRecord(parsed)) return null;
    const roots = parsed.workspace_roots;
    const firstRoot: unknown = Array.isArray(roots) && roots.length > 0 ? roots[0] : undefined;
    if (typeof firstRoot === 'string') {
      return path.posix.join(...firstRoot.split(path.sep));
    }
  } catch {
    // ignore JSON parse errors
  }
  return null;
}

/**
 * static-check subcommand entry point.
 *
 * Reads SubagentStop event JSON from stdin, extracts workspace root,
 * calls runStaticAnalysis in-process (no spawnSync), captures stderr
 * output, and formats the result.
 */
export function runStaticCheck(): void {
  const stdinRaw = readFileSync(0, 'utf-8');
  const workspaceRoot = parseWorkspaceRoot(stdinRaw);
  const projectRoot = workspaceRoot || process.env.CLAUDE_PROJECT_ROOT || getProjectDir();

  const [getCaptured, restore] = captureStderr();
  let exitCode: number;
  try {
    exitCode = runStaticAnalysis({ projectRoot });
  } finally {
    restore();
  }

  if (exitCode === 0) {
    process.stdout.write('{}\n');
    return;
  }

  const captured = getCaptured();
  const reason = FOLLOWUP_PREFIX + captured;
  process.stdout.write(`${JSON.stringify({ decision: 'block', reason })}\n`);
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

/**
 * Top-level entry function.
 * Parses process.argv[2] as the subcommand name and dispatches.
 */
export function main(): void {
  const subcommand = process.argv[2];

  switch (subcommand) {
    case 'protect-files':
      runProtectFiles();
      break;
    case 'static-check':
      runStaticCheck();
      break;
    default: {
      process.stderr.write(`Unknown subcommand: ${subcommand}\n`);
      process.exit(1);
    }
  }
}

// Execute main with a catch-all for unexpected errors.
// Outputs { decision: "block", reason: <error message> } on crash per design risk mitigation.
try {
  main();
} catch (error: unknown) {
  const message = error instanceof Error ? error.message : String(error);
  process.stdout.write(`${JSON.stringify({ decision: 'block', reason: message })}\n`);
  process.exit(1);
}
