#!/usr/bin/env node
// protect-files.mjs — PreToolUse hook: protect files from direct writes via config-driven glob patterns
//
// Reads openspec/config.json's write_protection configuration, merges with built-in defaults
// (openspec/changes/**/eval.json, openspec/config.json), and intercepts Write, Edit, Bash,
// and PowerShell tool calls that try to modify protected files.
//
// Input:  stdin  — JSON with { tool_name, tool_input: { file_path?, command? } }
// Output: stdout — JSON with { hookSpecificOutput: { permissionDecision, ... } }

import { readFileSync, existsSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

// ─── Built-in default protected patterns ───

const BUILT_IN_PATTERNS = [
  {
    glob: 'openspec/changes/**/eval.json',
    reason:
      '该文件受写入保护：%s。detected via %t。请使用 phase_log MCP 工具替代。',
  },
  {
    glob: 'openspec/config.json',
    reason:
      '该文件受写入保护：%s。detected via %t。请使用 config_get/config_set MCP 工具替代。',
  },
];

// ─── Glob to Regex ───

/**
 * Convert a simple glob pattern to a RegExp for path matching.
 * Supports ** (cross-path wildcard), * (segment-internal wildcard),
 * and ? (single character).  The returned RegExp matches anywhere
 * in the input string (not anchored).
 */
export function globToRegex(glob) {
  let result = '';
  let i = 0;

  while (i < glob.length) {
    const ch = glob[i];

    if (ch === '*' && glob[i + 1] === '*') {
      // ** matches any number of path segments including none
      i += 2;
      if (glob[i] === '/') {
        // **/ — make the trailing slash optional
        result += '(.+/)?';
        i++;
      } else {
        // ** at end or followed by non-/ — match everything
        result += '.*';
      }
    } else if (ch === '*') {
      // * matches within a single path segment (no /)
      result += '[^/]*';
      i++;
    } else if (ch === '?') {
      // ? matches a single non-/ character
      result += '[^/]';
      i++;
    } else {
      // Escape for regex
      result += ch.replace(/[.+^${}()|[\]\\]/g, '\\$&');
      i++;
    }
  }

  return new RegExp('^' + result + '$', 'i');
}

// ─── Config Loading ───

/**
 * Read and parse the `write_protection` field from openspec/config.json.
 * Returns an empty object when the config file does not exist or cannot be parsed.
 */
export function loadConfig(projectRoot) {
  try {
    const configPath = path.join(projectRoot, 'openspec', 'config.json');
    if (!existsSync(configPath)) return {};
    const raw = readFileSync(configPath, 'utf-8');
    const config = JSON.parse(raw);
    return config.write_protection || {};
  } catch {
    return {};
  }
}

// ─── Pattern Loading ───

/**
 * Load and merge patterns from built-in defaults and user configuration.
 * Returns an array of ProtectedPattern objects, each with:
 *   - match:  function(string): boolean  — pre-compiled regex test
 *   - glob:   string                      — original glob pattern
 *   - reason: string|undefined            — optional custom deny reason
 */
export function loadPatterns(projectRoot) {
  const patterns = [];

  // Built-in defaults
  for (const bp of BUILT_IN_PATTERNS) {
    patterns.push({
      match: globToRegex(bp.glob),
      glob: bp.glob,
      reason: bp.reason,
    });
  }

  // User-configured patterns from write_protection.files
  const config = loadConfig(projectRoot);
  if (config && Array.isArray(config.files)) {
    for (const file of config.files) {
      if (file && file.glob) {
        patterns.push({
          match: globToRegex(file.glob),
          glob: file.glob,
          reason: file.reason,
        });
      }
    }
  }

  return patterns;
}

// ─── Path Protection Check ───

/**
 * Check a file path against an array of ProtectedPattern.
 * Returns { matched: true, matchedGlob, reason } on first match,
 * or { matched: false } if no pattern matches.
 */
export function isProtected(filePath, patterns) {
  if (!filePath) return { matched: false };
  const normalized = filePath.replace(/\\/g, '/');

  for (const p of patterns) {
    if (p.match.test(normalized)) {
      return {
        matched: true,
        matchedGlob: p.glob,
        reason: p.reason,
      };
    }
  }

  return { matched: false };
}

// ─── Deny Reason Builder ───

/**
 * Build a human-readable deny reason.
 * - If the pattern has a custom reason, use it with %s (file path) and %t (tool name) substitution.
 * - Otherwise use a generic fallback message.
 */
export function buildDenyReason(pattern, filePath, toolName) {
  if (pattern && pattern.reason) {
    return pattern.reason.replace(/%s/g, filePath).replace(/%t/g, toolName);
  }
  return `该文件受写入保护：${filePath}。detected via ${toolName}。`;
}

// ─── Bash Write Detection ───

/**
 * Check if a bash command contains write operators.
 */
function hasBashWriteOperator(cmd) {
  // > or >> redirect (not preceded by -)
  if (/(?:^|[^-])>{1,2}\s/.test(cmd)) return true;
  // >| redirect (noclobber override)
  if (/>(?:\|)/.test(cmd)) return true;
  // tee
  if (/[|&;]\s*tee\s/.test(cmd)) return true;
  // heredoc
  if (/<<[^<]/.test(cmd)) return true;
  // >& redirect
  if (/>&\s/.test(cmd)) return true;
  return false;
}

/**
 * Extract file paths that appear after bash write operators.
 */
function extractBashWriteTargets(cmd) {
  const targets = [];

  // > and >> redirect (not preceded by - or part of ->)
  let re = /(?:^|[^-])>{1,2}\s+['"]?([^\s;|`$&()'"]+)['"]?/g;
  let m;
  while ((m = re.exec(cmd)) !== null) {
    if (m[1]) targets.push(m[1]);
  }

  // >| redirect (noclobber override)
  re = />\|\s+['"]?([^\s;|`$&()'"]+)['"]?/g;
  while ((m = re.exec(cmd)) !== null) {
    if (m[1]) targets.push(m[1]);
  }

  // tee (possibly with -a or other flags)
  re = /(?:^|[\s;|&(])\s*tee\s+(?:-[a-zA-Z]+\s+)?['"]?([^\s;|`$&()'"]+)['"]?/g;
  while ((m = re.exec(cmd)) !== null) {
    if (m[1]) targets.push(m[1]);
  }

  // >& redirect
  re = />&\s*['"]?([^\s;|`$&()'"]+)['"]?/g;
  while ((m = re.exec(cmd)) !== null) {
    if (m[1]) targets.push(m[1]);
  }

  return targets;
}

/**
 * Detect whether a bash command tries to write to a protected file.
 * Returns { decision: 'allow' } or { decision: 'deny', reason }.
 */
export function detectBashWrite(cmd, patterns) {
  if (!cmd) return { decision: 'allow' };
  const normalized = cmd.replace(/\\/g, '/');

  // Exempt python/node script runners
  if (/^(python|python3|node)\s/.test(normalized)) return { decision: 'allow' };

  // Check for write operators
  if (!hasBashWriteOperator(normalized)) return { decision: 'allow' };

  // Extract target file paths after write operators
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

  return { decision: 'allow' };
}

// ─── PowerShell Write Detection ───

/**
 * Extract file paths that appear after PowerShell write operators and cmdlets.
 */
function extractPowerShellWriteTargets(cmd) {
  const targets = [];

  // Redirect operators: >, >>, *>, 1>, 2>, etc.
  let re = /(?:^|[\s;|&(])(?:\d*|\*)?>{1,2}\s+['"]?([^\s;|`$&()'",=]+)['"]?/g;
  let m;
  while ((m = re.exec(cmd)) !== null) {
    if (m[1]) targets.push(m[1]);
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
      `\\b${cmdlet}\\s+(?:-(?:Path|FilePath)\\s+)?['"]?([^\\s;|\`$&()'",=]+)['"]?`,
      'gi',
    );
    while ((m = re.exec(cmd)) !== null) {
      if (m[1]) targets.push(m[1]);
    }
  }

  // .NET file writing methods
  const dotnetMethods = [
    'WriteAllText',
    'WriteAllLines',
    'WriteAllBytes',
    'AppendAllText',
  ];
  for (const method of dotnetMethods) {
    re = new RegExp(
      `\\[System\\.IO\\.File\\]::${method}\\s*\\(\\s*['"]?([^,;\\)'"]+)['"]?`,
      'gi',
    );
    while ((m = re.exec(cmd)) !== null) {
      if (m[1]) targets.push(m[1]);
    }
  }

  return targets;
}

/**
 * Detect whether a PowerShell command tries to write to a protected file.
 * Returns { decision: 'allow' } or { decision: 'deny', reason }.
 */
export function detectPowerShellWrite(cmd, patterns) {
  if (!cmd) return { decision: 'allow' };
  const normalized = cmd.replace(/\\/g, '/');

  // Exempt python/node script runners
  if (/^(python|python3|node)\s/.test(normalized)) return { decision: 'allow' };

  // Extract target file paths
  const targets = extractPowerShellWriteTargets(normalized);
  if (targets.length === 0) return { decision: 'allow' };

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

  return { decision: 'allow' };
}

// ─── Change Name Extraction ───

/**
 * Extract the change name from a file path under openspec/changes/.
 */
export function extractChangeName(filePath) {
  if (!filePath) return '';
  const normalized = filePath.replace(/\\/g, '/');
  const match = normalized.match(/openspec\/changes\/([^/]+)/);
  return match ? match[1] : '';
}

// ─── Output Helpers ───

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

// ─── Input Parser ───

/**
 * Parse the stdin JSON received from the hook runtime, check the tool
 * invocation against the protected patterns, and return a decision.
 *
 * Fail-open: returns { decision: 'allow' } when input is missing, invalid,
 * or the tool is not a write-capable tool.
 */
export function parseInput(raw, patterns) {
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
    const result = isProtected(filePath, patterns);
    if (result.matched) {
      return {
        decision: 'deny',
        reason: buildDenyReason(result, filePath, toolName),
      };
    }
    return { decision: 'allow' };
  }

  if (toolName === 'Bash') {
    const command = parsed.tool_input?.command;
    if (!command) return { decision: 'allow' };
    return detectBashWrite(command, patterns);
  }

  if (toolName === 'PowerShell') {
    const command = parsed.tool_input?.command;
    if (!command) return { decision: 'allow' };
    return detectPowerShellWrite(command, patterns);
  }

  return { decision: 'allow' };
}

// ─── Main ───

function main() {
  const projectRoot = process.env.CLAUDE_PROJECT_ROOT || process.cwd();
  const patterns = loadPatterns(projectRoot);

  const input = readFileSync(0, 'utf-8');
  const result = parseInput(input, patterns);

  if (result.decision === 'deny') {
    process.stdout.write(`${outputDeny(result.reason)}\n`);
    return;
  }

  process.stdout.write(`${outputAllow()}\n`);
}

if (process.argv[1] && fileURLToPath(import.meta.url) === path.resolve(process.argv[1])) {
  main();
}
