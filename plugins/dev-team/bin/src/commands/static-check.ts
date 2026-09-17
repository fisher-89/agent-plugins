// static-check.ts — SubagentStop hook: run static analysis in-process and
// emit a block decision with the captured diagnostics on failure.

import { readFileSync } from 'node:fs';
import * as path from 'node:path';

import { getProjectDir } from '../lib/project-root';
import { runStaticAnalysis } from './run-static-analysis';

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
    const parsed: Record<string, unknown> = JSON.parse(stdinRaw);
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
  const projectRoot = workspaceRoot || getProjectDir();

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
  process.stdout.write(
    `${JSON.stringify({ decision: 'block', reason, followup_message: reason })}\n`,
  );
}
