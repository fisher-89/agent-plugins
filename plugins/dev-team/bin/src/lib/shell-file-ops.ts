// shell-file-ops.ts — Shell command → FileOp extraction (write / delete / revert),
// shared by the protect-files (PreToolUse) and record-files (PostToolUse) hooks.

import { type FileOp } from '../modules/workflow';

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
 * False for unexpanded `$var` tokens. The hooks see the command text before
 * shell expansion, so the value behind `$var` is unknowable and the literal
 * token can never match a protection glob or a real file path. Applied at
 * op-push sites only — never inside `tokenize()` (the `git clean` zero-path
 * bulk check in protect-files depends on raw token counts).
 */
function isLiteralPath(token: string): boolean {
  return !token.includes('$');
}

// ---------------------------------------------------------------------------
// Quote / Heredoc Masking
// ---------------------------------------------------------------------------

/**
 * Filler character for masked regions: non-whitespace and not in the
 * exclusion set of any extraction character class, so masked text can never
 * form a matchable command name, operator, or captured path.
 */
const MASK_CHAR = String.fromCharCode(0);

/** Half-open [start, end) range within the command text. */
interface Span {
  start: number;
  end: number;
}

/**
 * Ranges of quoted strings: `'…'` (no escapes in bash single quotes) and
 * `"…"` (backslash escapes honored). Single/double do not nest. An
 * unterminated quote discards that candidate and stops masking altogether
 * (fail-open: everything from there on keeps today's unmasked behavior).
 */
function quoteSpans(cmd: string): Span[] {
  const spans: Span[] = [];
  const n = cmd.length;
  let i = 0;
  while (i < n) {
    const ch = cmd[i];
    if (ch === "'") {
      const close = cmd.indexOf("'", i + 1);
      if (close === -1) return spans;
      spans.push({ start: i, end: close + 1 });
      i = close + 1;
    } else if (ch === '"') {
      let j = i + 1;
      while (j < n && cmd[j] !== '"') {
        if (cmd[j] === '\\') j += 2;
        else j++;
      }
      if (j >= n) return spans;
      spans.push({ start: i, end: j + 1 });
      i = j + 1;
    } else {
      i++;
    }
  }
  return spans;
}

/**
 * Ranges of heredoc bodies (`<<MARKER` … a later line equal to `MARKER`).
 * The marker must be glued to `<<` (so JS bitshifts like `1 << 2` or `x <<2`
 * never form an opener), `<<` itself must sit outside quotes, and the body
 * only exists when both a newline and a terminating marker line are found —
 * otherwise the candidate is dropped (fail-open, no masking).
 */
function heredocSpans(cmd: string): Span[] {
  const spans: Span[] = [];
  const quotes = quoteSpans(cmd);
  const inQuotes = (idx: number): boolean => quotes.some((s) => idx >= s.start && idx < s.end);
  const openerRe = /(?:^|[\s;&|(])<<-?(['"]?)([A-Za-z0-9_]+)\1/g;
  openerRe.lastIndex = 0;
  let m: RegExpExecArray | null;
  while ((m = openerRe.exec(cmd)) !== null) {
    const lt = m.index + m[0].indexOf('<<');
    if (inQuotes(lt)) continue;
    const marker = m[2];
    const headerNl = cmd.indexOf('\n', m.index + m[0].length);
    if (headerNl === -1) continue;
    let pos = headerNl + 1;
    let end = -1;
    while (pos <= cmd.length) {
      const nl = cmd.indexOf('\n', pos);
      const line = cmd.slice(pos, nl === -1 ? cmd.length : nl);
      if (line.trim() === marker) {
        end = nl === -1 ? cmd.length : nl;
        break;
      }
      if (nl === -1) break;
      pos = nl + 1;
    }
    if (end === -1) continue;
    spans.push({ start: headerNl + 1, end });
  }
  return spans;
}

/** Replace the given ranges with MASK_CHAR runs; output length === input length. */
function maskSpans(cmd: string, spans: Span[]): string {
  if (spans.length === 0) return cmd;
  const sorted = [...spans].sort((a, b) => a.start - b.start);
  const merged: Span[] = [];
  for (const s of sorted) {
    const last = merged[merged.length - 1];
    if (last && s.start <= last.end) last.end = Math.max(last.end, s.end);
    else merged.push({ ...s });
  }
  const chars = cmd.split('');
  for (const s of merged) {
    for (let i = Math.max(0, s.start); i < s.end && i < chars.length; i++) {
      chars[i] = MASK_CHAR;
    }
  }
  return chars.join('');
}

/**
 * Variant A — heredoc bodies only, quotes preserved. The write-regex family
 * and op-argument slicing run on this text so that quoted real targets
 * (`echo x > 'a b.txt'`, `WriteAllText("a b.json", …)`) stay matchable.
 */
export function maskHeredocs(cmd: string): string {
  return maskSpans(cmd, heredocSpans(cmd));
}

/**
 * Variant B — heredoc bodies plus quoted strings. Command-NAME location runs
 * on this text so `rm` / `git stash` / `git restore` appearing inside a
 * string literal or heredoc body can never be taken for a real command.
 * Both variants preserve length, so match indices translate 1:1 to variant A.
 */
export function maskQuotedAndHeredoc(cmd: string): string {
  return maskSpans(cmd, [...heredocSpans(cmd), ...quoteSpans(cmd)]);
}

/**
 * Scan `command <args>` occurrences for the given command names.
 * The argument string stops at a command separator (`;`, `|`, `&`) or newline.
 *
 * Command names are located on variant B (quotes + heredocs masked) so names
 * appearing inside string literals or heredoc bodies never match; the args
 * are sliced from variant A (quotes preserved) at the same indices so quoted
 * real arguments (`rm 'a b.txt'`) keep today's tokenization. Invariant: the
 * args span ends at a newline and heredoc bodies start after one, so the
 * slice can never pick up MASK_CHAR.
 */
function scanCommand(cmd: string, names: string): Array<{ args: string }> {
  const results: Array<{ args: string }> = [];
  const nameText = maskQuotedAndHeredoc(cmd);
  const argText = maskHeredocs(cmd);
  const re = new RegExp(`(?:^|[\\s;&|(])(?:${names})\\s+([^;&|\\n]*)`, 'gi');
  re.lastIndex = 0;
  let m: RegExpExecArray | null;
  while ((m = re.exec(nameText)) !== null) {
    const argsStart = m.index + m[0].length - m[1].length;
    results.push({ args: argText.slice(argsStart, m.index + m[0].length) });
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
/** `git restore` paths → ops: with `--source=` (or `-s`) → write, bare → revert. */
function restoreOps(tokens: string[]): FileOp[] {
  let fromSource = false;
  const paths: string[] = [];
  for (let i = 0; i < tokens.length; i++) {
    const t = tokens[i];
    if (t.startsWith('--source=') || t === '-s') {
      fromSource = true;
      if (t === '-s') i++; // separate value form: skip the commit-ish
      continue;
    }
    if (isFlag(t) || !isLiteralPath(t)) continue;
    paths.push(t);
  }
  return paths.map((p) => ({ op: fromSource ? ('write' as const) : ('revert' as const), path: p }));
}

/**
 * `git checkout -- <paths>` → ops (path-restoring forms only). The commit-ish
 * side (write/revert discriminator) is NOT $-filtered so
 * `git checkout $REF -- src/a.ts` still classifies as write.
 */
function checkoutOps(tokens: string[]): FileOp[] {
  const sep = tokens.indexOf('--');
  if (sep === -1) return [];
  const fromSource = tokens.slice(0, sep).some((t) => !isFlag(t));
  return tokens
    .slice(sep + 1)
    .filter((t) => !isFlag(t) && isLiteralPath(t))
    .map((p) => ({ op: fromSource ? ('write' as const) : ('revert' as const), path: p }));
}

export function extractGitOps(cmd: string): FileOp[] {
  const ops: FileOp[] = [];
  // Names located on variant B (quoted/heredoc `git …` text never matches);
  // args sliced from variant A so quoted real paths keep tokenizing as today.
  const nameText = maskQuotedAndHeredoc(cmd);
  const argText = maskHeredocs(cmd);
  const re = /\bgit\s+(restore|checkout|rm)\b([^\n;&|]*)/gi;
  re.lastIndex = 0;
  let m: RegExpExecArray | null;
  while ((m = re.exec(nameText)) !== null) {
    const sub = m[1].toLowerCase();
    const argsStart = m.index + m[0].length - m[2].length;
    const tokens = tokenize(argText.slice(argsStart, m.index + m[0].length));

    if (sub === 'rm') {
      for (const t of tokens) {
        if (!isFlag(t) && isLiteralPath(t)) ops.push({ op: 'delete', path: t });
      }
    } else if (sub === 'restore') {
      ops.push(...restoreOps(tokens));
    } else if (sub === 'checkout') {
      ops.push(...checkoutOps(tokens));
    }
  }
  return ops;
}

// ---------------------------------------------------------------------------
// extractFileOps — three-state extractor shared by PreToolUse and PostToolUse
// ---------------------------------------------------------------------------

/**
 * Write ops from bash redirections. Runs on variant A (heredoc bodies
 * masked, quotes preserved) so redirects inside heredoc data never match
 * while quoted real targets stay matchable. The `[^-=]` prefix guard skips
 * `->` (pipe-to) and `=>` (JS arrow functions) — the latter previously
 * recorded arrow-body identifiers (`=> path.join`) as writes.
 */
function extractBashRedirectWriteOps(cmd: string): FileOp[] {
  const ops: FileOp[] = [];
  const masked = maskHeredocs(cmd);
  const writeRegexes: readonly RegExp[] = [
    // > and >> redirect (not preceded by -, = or part of ->)
    /(?:^|[^-=])>{1,2}\s+['"]?([^\s;|`$&()'"]+)['"]?/g,
    // >| redirect (noclobber override)
    />\|\s+['"]?([^\s;|`$&()'"]+)['"]?/g,
    // tee (possibly with -a or other flags)
    /(?:^|[\s;|&(])\s*tee\s+(?:-[a-zA-Z]+\s+)?['"]?([^\s;|`$&()'"]+)['"]?/g,
    // >& redirect (legacy `>&file`); the lookahead skips bare-integer targets —
    // fd duplication (`2>&1`, `1>&2`, `>&2`) names a descriptor, not a file
    />&\s*['"]?(?!\d+(?:$|[\s;|`$&()'"]))([^\s;|`$&()'"]+)['"]?/g,
  ];
  for (const re of writeRegexes) {
    re.lastIndex = 0;
    let m: RegExpExecArray | null;
    while ((m = re.exec(masked)) !== null) {
      ops.push({ op: 'write', path: m[1] });
    }
  }
  return ops;
}

/** Write ops from PowerShell redirects, file-writing cmdlets and .NET methods. */
function extractPowerShellWriteOps(cmd: string): FileOp[] {
  const ops: FileOp[] = [];
  const masked = maskHeredocs(cmd);
  let m: RegExpExecArray | null;

  // Redirect operators: >, >>, *>, 1>, 2>, etc.
  const psRedirect = /(?:^|[\s;|&(])(?:\d*|\*)?>{1,2}\s+['"]?([^\s;|`$&()'",=]+)['"]?/g;
  while ((m = psRedirect.exec(masked)) !== null) {
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
    while ((m = re.exec(masked)) !== null) {
      ops.push({ op: 'write', path: m[1] });
    }
  }

  const dotnetMethods = ['WriteAllText', 'WriteAllLines', 'WriteAllBytes', 'AppendAllText'];
  for (const method of dotnetMethods) {
    const re = new RegExp(
      `\\[System\\.IO\\.File\\]::${method}\\s*\\(\\s*['"]?([^,;\\)'"]+)['"]?`,
      'gi',
    );
    while ((m = re.exec(masked)) !== null) {
      // Unexpanded `$var` first arguments can never name a real file.
      if (isLiteralPath(m[1])) ops.push({ op: 'write', path: m[1] });
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
      if (!isFlag(t) && isLiteralPath(t)) ops.push({ op: 'delete', path: t });
    }
  }
  return ops;
}

/** Move ops: two-argument form → delete(old) + write(new) double entry. */
function extractMoveOps(cmd: string): FileOp[] {
  const ops: FileOp[] = [];
  for (const { args } of scanCommand(cmd, 'Move-Item|Rename-Item|ren|mv')) {
    const paths = tokenize(args).filter((t) => !isFlag(t) && isLiteralPath(t));
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
 * - write: `>` / `>>` / `>|` / `tee` / `>&` (bare-integer targets — fd
 *   duplication like `2>&1` — are skipped), PowerShell redirects and file-writing
 *   cmdlets (Set-Content / Out-File / Add-Content / Export-Csv / Export-CliXml /
 *   Tee-Object), `[System.IO.File]::WriteAll*`, `git restore --source=<commit>`,
 *   `git checkout <commit> -- <paths>`;
 * - delete: `rm` / `git rm` / `rmdir` / `unlink`, PowerShell `Remove-Item` / `del` / `rd`
 *   (per-path arguments, `-` flags skipped);
 * - delete(old) + write(new) double entry: two-argument `mv` / `Move-Item` /
 *   `ren` / `Rename-Item`;
 * - revert: `git restore <paths>` (no source), `git checkout -- <paths>`.
 *
 * False-positive guards layered on top of the raw patterns:
 * - command names (`rm` / `mv` / `git …`) are located with quoted strings and
 *   heredoc bodies masked, so names mentioned inside string data never match;
 *   their arguments are sliced from quote-preserving text, so quoted real
 *   paths (`rm 'a b.txt'`, `git restore 'p'`) keep tokenizing as before;
 * - write regexes run with heredoc bodies masked (redirects inside heredoc
 *   data are content, not commands) and skip `=>` (JS arrow functions);
 * - tokens containing `$` are dropped at op-push sites — unexpanded variable
 *   values are unknowable pre-expansion and never name a real file.
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
