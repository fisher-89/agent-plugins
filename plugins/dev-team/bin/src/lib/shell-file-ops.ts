// shell-file-ops.ts — Shell command → FileOp extraction (write / delete / revert),
// shared by the protect-files (PreToolUse) and record-files (PostToolUse) hooks.

import { type FileOp } from './file-inventory';

// ---------------------------------------------------------------------------
// Argument Tokenizing
// ---------------------------------------------------------------------------

/** Split a command-argument string into whitespace tokens with quotes stripped. */
export function tokenize(args: string): string[] {
  return args
    .split(/\s+/)
    .filter(Boolean)
    .map((t) => t.replace(/^['"]|['"]$/g, ''))
    .filter(Boolean);
}

/** True for option tokens ( `-f`, `-rf`, `--cached` ). */
export function isFlag(token: string): boolean {
  return token.startsWith('-');
}

/**
 * Scan `command <args>` occurrences for the given command names.
 * The argument string stops at a command separator (`;`, `|`, `&`) or newline.
 */
function scanCommand(cmd: string, names: string): Array<{ args: string }> {
  const results: Array<{ args: string }> = [];
  const re = new RegExp(`(?:^|[\\s;&|(])(?:${names})\\s+([^;&|\\n]*)`, 'gi');
  re.lastIndex = 0;
  let m: RegExpExecArray | null;
  while ((m = re.exec(cmd)) !== null) {
    results.push({ args: m[1] });
  }
  return results;
}

// ---------------------------------------------------------------------------
// Git Working-Tree Op Extraction
// ---------------------------------------------------------------------------

/**
 * Extract file ops from git subcommands that touch working-tree paths:
 * - `git restore <paths>` (no source) → revert; with `--source=<commit>`
 *   (or `-s <commit>`) → write;
 * - `git checkout -- <paths>` → revert; `git checkout <commit> -- <paths>` → write;
 * - `git rm <paths>` → delete (`git rm` also hits the generic delete commands below;
 *   folding deduplicates).
 *
 * `git stash` / `git clean` are PreToolUse bulk-revert concerns only — the
 * recorder never sees them (they are denied).
 */
export function extractGitOps(cmd: string): FileOp[] {
  const ops: FileOp[] = [];
  const re = /\bgit\s+(restore|checkout|rm)\b([^\n;&|]*)/gi;
  re.lastIndex = 0;
  let m: RegExpExecArray | null;
  while ((m = re.exec(cmd)) !== null) {
    const sub = m[1].toLowerCase();
    const tokens = tokenize(m[2]);

    if (sub === 'rm') {
      for (const t of tokens) {
        if (!isFlag(t)) ops.push({ op: 'delete', path: t });
      }
      continue;
    }

    if (sub === 'restore') {
      let fromSource = false;
      const paths: string[] = [];
      for (let i = 0; i < tokens.length; i++) {
        const t = tokens[i];
        if (t.startsWith('--source=') || t === '-s') {
          fromSource = true;
          if (t === '-s') i++; // separate value form: skip the commit-ish
          continue;
        }
        if (isFlag(t)) continue;
        paths.push(t);
      }
      for (const p of paths) {
        ops.push({ op: fromSource ? 'write' : 'revert', path: p });
      }
      continue;
    }

    // checkout: only the path-restoring forms (with `--`) are recognized
    const sep = tokens.indexOf('--');
    if (sep === -1) continue;
    const commitish = tokens.slice(0, sep).filter((t) => !isFlag(t));
    const fromSource = commitish.length > 0;
    for (const p of tokens.slice(sep + 1).filter((t) => !isFlag(t))) {
      ops.push({ op: fromSource ? 'write' : 'revert', path: p });
    }
  }
  return ops;
}

// ---------------------------------------------------------------------------
// extractFileOps — three-state extractor shared by PreToolUse and PostToolUse
// ---------------------------------------------------------------------------

/** Write ops from bash redirections (existing regex set preserved). */
function extractBashRedirectWriteOps(cmd: string): FileOp[] {
  const ops: FileOp[] = [];
  const writeRegexes: readonly RegExp[] = [
    // > and >> redirect (not preceded by - or part of ->)
    /(?:^|[^-])>{1,2}\s+['"]?([^\s;|`$&()'"]+)['"]?/g,
    // >| redirect (noclobber override)
    />\|\s+['"]?([^\s;|`$&()'"]+)['"]?/g,
    // tee (possibly with -a or other flags)
    /(?:^|[\s;|&(])\s*tee\s+(?:-[a-zA-Z]+\s+)?['"]?([^\s;|`$&()'"]+)['"]?/g,
    // >& redirect
    />&\s*['"]?([^\s;|`$&()'"]+)['"]?/g,
  ];
  for (const re of writeRegexes) {
    re.lastIndex = 0;
    let m: RegExpExecArray | null;
    while ((m = re.exec(cmd)) !== null) {
      ops.push({ op: 'write', path: m[1] });
    }
  }
  return ops;
}

/** Write ops from PowerShell redirects, file-writing cmdlets and .NET methods. */
function extractPowerShellWriteOps(cmd: string): FileOp[] {
  const ops: FileOp[] = [];
  let m: RegExpExecArray | null;

  // Redirect operators: >, >>, *>, 1>, 2>, etc.
  const psRedirect = /(?:^|[\s;|&(])(?:\d*|\*)?>{1,2}\s+['"]?([^\s;|`$&()'",=]+)['"]?/g;
  while ((m = psRedirect.exec(cmd)) !== null) {
    ops.push({ op: 'write', path: m[1] });
  }

  const cmdlets = [
    'Set-Content',
    'Out-File',
    'Add-Content',
    'Export-Csv',
    'Export-CliXml',
    'Tee-Object',
  ];
  for (const cmdlet of cmdlets) {
    const re = new RegExp(
      `\\b${cmdlet}\\s+(?:-(?:Path|FilePath)\\s+)?['"]?([^\\s;\`$&()'",=]+)['"]?`,
      'gi',
    );
    while ((m = re.exec(cmd)) !== null) {
      ops.push({ op: 'write', path: m[1] });
    }
  }

  const dotnetMethods = ['WriteAllText', 'WriteAllLines', 'WriteAllBytes', 'AppendAllText'];
  for (const method of dotnetMethods) {
    const re = new RegExp(
      `\\[System\\.IO\\.File\\]::${method}\\s*\\(\\s*['"]?([^,;\\)'"]+)['"]?`,
      'gi',
    );
    while ((m = re.exec(cmd)) !== null) {
      ops.push({ op: 'write', path: m[1] });
    }
  }
  return ops;
}

/**
 * Delete ops from per-path delete commands (flags skipped). `git rm` is
 * covered by the `rm` match; folding deduplicates.
 */
function extractDeleteOps(cmd: string): FileOp[] {
  const ops: FileOp[] = [];
  for (const { args } of scanCommand(cmd, 'Remove-Item|rmdir|unlink|rm|del|rd')) {
    for (const t of tokenize(args)) {
      if (!isFlag(t)) ops.push({ op: 'delete', path: t });
    }
  }
  return ops;
}

/** Move ops: two-argument form → delete(old) + write(new) double entry. */
function extractMoveOps(cmd: string): FileOp[] {
  const ops: FileOp[] = [];
  for (const { args } of scanCommand(cmd, 'Move-Item|Rename-Item|ren|mv')) {
    const paths = tokenize(args).filter((t) => !isFlag(t));
    if (paths.length === 2) {
      ops.push({ op: 'delete', path: paths[0] });
      ops.push({ op: 'write', path: paths[1] });
    }
  }
  return ops;
}

/**
 * Extract file operations from a shell command (bash and PowerShell dialects
 * in one pass) as write / delete / revert ops:
 *
 * - write: `>` / `>>` / `>|` / `tee` / `>&`, PowerShell redirects and file-writing
 *   cmdlets (Set-Content / Out-File / Add-Content / Export-Csv / Export-CliXml /
 *   Tee-Object), `[System.IO.File]::WriteAll*`, `git restore --source=<commit>`,
 *   `git checkout <commit> -- <paths>`;
 * - delete: `rm` / `git rm` / `rmdir` / `unlink`, PowerShell `Remove-Item` / `del` / `rd`
 *   (per-path arguments, `-` flags skipped);
 * - delete(old) + write(new) double entry: two-argument `mv` / `Move-Item` /
 *   `ren` / `Rename-Item`;
 * - revert: `git restore <paths>` (no source), `git checkout -- <paths>`.
 *
 * Over-recording direction is safe: unknown long tails are reconciled by the
 * evaluator and the `change_files` tool. No python/node exemption here — the
 * exemption is a PreToolUse protection concern applied by the check functions.
 */
export function extractFileOps(cmd: string): FileOp[] {
  return [
    ...extractBashRedirectWriteOps(cmd),
    ...extractPowerShellWriteOps(cmd),
    ...extractDeleteOps(cmd),
    ...extractMoveOps(cmd),
    ...extractGitOps(cmd),
  ];
}
