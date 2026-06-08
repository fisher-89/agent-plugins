import * as fs from 'fs';
import * as path from 'path';

import { readConfig } from '../lib/config';
import { getDefaultGlobForFramework, runTestGetFrameworkConfig } from './test-get-framework-config';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

interface FrameworkMapping {
  glob: string;
  framework: string;
}

interface DetectedFile {
  file: string;
  framework: string;
}

export interface TestDetectFrameworksResult {
  detected: DetectedFile[];
  frameworks: string[];
  plan: PlanEntry[];
}

export interface PlanEntry {
  directory: string;
  framework: string;
  coverage_cmd: string;
  coverage_format: 'istanbul' | 'llvm-cov';
  coverage_output: string;
  coverage_artifacts: string[];
  coverage_cleanup: string[];
}

export interface TestDetectFrameworksOptions {
  files?: string[];
  projectRoot?: string;
}

// ---------------------------------------------------------------------------
// Glob matching (simple implementation for common patterns)
// ---------------------------------------------------------------------------

/**
 * Convert a glob pattern to a RegExp for matching file paths.
 *
 * Supports:
 * - `**` — matches any number of path segments (including zero)
 * - `*` — matches any characters within a single path segment
 * - `{a,b,c}` — matches any of the comma-separated alternatives
 *
 * Normalises path separators to forward slashes for cross-platform matching.
 */
function globToRegex(pattern: string): RegExp {
  const normalised = pattern.replace(/\\/g, '/');

  // Escape regex special characters except glob tokens
  let regexStr = '';
  let i = 0;

  while (i < normalised.length) {
    const ch = normalised[i];

    if (ch === '*' && normalised[i + 1] === '*') {
      // ** matches any number of path segments
      // When followed by /, handle zero-or-more directory levels properly
      if (normalised[i + 2] === '/') {
        // **/ -- match zero or more complete path segments
        regexStr += '(?:.+/)?';
        i += 3;
      } else {
        regexStr += '.*';
        i += 2;
      }
    } else if (ch === '*') {
      // * matches any characters within a single segment (non-greedy)
      regexStr += '[^/]*';
      i += 1;
    } else if (ch === '?') {
      regexStr += '[^/]';
      i += 1;
    } else if (ch === '{') {
      // Find the matching closing brace
      const closing = normalised.indexOf('}', i);
      if (closing !== -1) {
        const content = normalised.slice(i + 1, closing);
        const alternatives = content
          .split(',')
          .map((alt) => alt.trim())
          .map((alt) => alt.replace(/[.+^${}()|[\]\\]/g, '\\$&'));
        regexStr += `(?:${alternatives.join('|')})`;
        i = closing + 1;
      } else {
        // No closing brace, treat as literal
        regexStr += '\\{';
        i += 1;
      }
    } else if (ch === ',') {
      // Inside { } groups, commas are handled above.
      // Outside groups, comma is a literal.
      regexStr += ',';
      i += 1;
    } else if (ch === '}') {
      regexStr += '\\}';
      i += 1;
    } else if ('.+^${}()|[\\]'.includes(ch)) {
      regexStr += '\\' + ch;
      i += 1;
    } else {
      regexStr += ch;
      i += 1;
    }
  }

  return new RegExp(`^${regexStr}$`);
}

/**
 * Check whether a file path matches a glob pattern.
 * Cross-platform: normalises backslashes to forward slashes.
 */
function matchGlob(filePath: string, pattern: string): boolean {
  const normalisedPath = filePath.replace(/\\/g, '/');
  const regex = globToRegex(pattern);
  return regex.test(normalisedPath);
}

// ---------------------------------------------------------------------------
// Auto-scan: recursively find files matching any configured glob pattern
// ---------------------------------------------------------------------------

/**
 * Recursively collect all file paths under a root directory, excluding
 * node_modules, .git, and other common non-source directories.
 */
function collectFiles(rootDir: string): string[] {
  const results: string[] = [];
  const excludedDirs = new Set([
    'node_modules',
    '.git',
    '.claude',
    'dist',
    'build',
    'target',
    '.vp',
    'coverage',
    '.nyc_output',
  ]);

  function walk(dir: string): void {
    let entries: fs.Dirent[];
    try {
      entries = fs.readdirSync(dir, { withFileTypes: true });
    } catch {
      return;
    }

    for (const entry of entries) {
      const fullPath = path.join(dir, entry.name);

      if (entry.isDirectory()) {
        if (!excludedDirs.has(entry.name)) {
          walk(fullPath);
        }
      } else if (entry.isFile()) {
        results.push(fullPath);
      }
    }
  }

  walk(rootDir);
  return results;
}

// ---------------------------------------------------------------------------
// Config normalisation
// ---------------------------------------------------------------------------

/**
 * Normalise the `test.frameworks` config value into an array of
 * `{glob, framework}` mappings.
 *
 * - If it is a string (shorthand for known frameworks), expand to a single
 *   entry using the framework's default glob pattern.
 * - If it is already an array, return as-is.
 * - If it is undefined/null/empty, return an empty array.
 */
function normaliseFrameworks(frameworks: unknown): FrameworkMapping[] {
  if (!frameworks) {
    return [];
  }

  if (typeof frameworks === 'string') {
    const fw = frameworks.trim();
    // The shorthand is assumed to be valid since the Zod schema enforces the enum.
    // Defensively return empty for unknown framework names.
    try {
      const glob = getDefaultGlobForFramework(fw);
      return [{ glob, framework: fw }];
    } catch {
      return [];
    }
  }

  if (Array.isArray(frameworks)) {
    return frameworks.filter(
      (entry): entry is FrameworkMapping =>
        entry != null &&
        typeof entry === 'object' &&
        typeof entry.glob === 'string' &&
        typeof entry.framework === 'string',
    );
  }

  return [];
}

// ---------------------------------------------------------------------------
// deriveWorkingDirectory
// ---------------------------------------------------------------------------

/**
 * Derive the working directory from a glob pattern.
 *
 * Rules:
 * 1. Normalise backslashes to forward slashes (cross-platform).
 * 2. Merge consecutive forward slashes into one.
 * 3. Find the first wildcard character (`*`, `?`, `{`).
 * 4. If no wildcard is found, return the entire (normalised) string.
 * 5. Take the substring before the first wildcard, then strip any trailing
 *    path separator (`/`).  If the result is empty, return `"."`.
 *
 * Examples:
 *   - `"plugins/dev-team/bin"`      -> `"plugins/dev-team/bin"`
 *   - `"tests/?nit/*.test.ts"`      -> `"tests"`
 *   - `"{src,lib}/*.test.ts"`       -> `"."`
 */
export function deriveWorkingDirectory(glob: string): string {
  // 1. Normalise backslashes to forward slashes
  let normalised = glob.replace(/\\/g, '/');

  // 2. Merge consecutive forward slashes
  normalised = normalised.replace(/\/+/g, '/');

  // 3. Find first wildcard character
  const WILDCARD_PATTERN = /[*?{]/;
  const match = WILDCARD_PATTERN.exec(normalised);

  if (!match) {
    // 4. No wildcard — return the entire normalised string
    return normalised;
  }

  // 5. Take the substring before the first wildcard
  const prefix = normalised.slice(0, match.index);
  const trimmed = prefix.replace(/\/+$/, '');

  return trimmed || '.';
}

// ---------------------------------------------------------------------------
// Main handler
// ---------------------------------------------------------------------------

/**
 * Run `test_detect_frameworks`: detect which test framework(s) each file
 * belongs to, based on the glob-to-framework mappings in config.json.
 *
 * When `files` is provided, match only those files.  When omitted,
 * auto-scan the project for files matching any of the configured globs.
 */
export function runTestDetectFrameworks(
  options: TestDetectFrameworksOptions,
): TestDetectFrameworksResult {
  const projectRoot = options.projectRoot || process.env.CLAUDE_PROJECT_DIR || process.cwd();

  const config = readConfig(projectRoot);
  const configFrameworks = config.test?.frameworks;
  const mappings = normaliseFrameworks(configFrameworks);

  // Determine the list of files to process
  let filesToCheck: string[];

  if (options.files && options.files.length > 0) {
    // Provided file list
    // Resolve relative paths against project root
    filesToCheck = options.files.map((f) =>
      path.isAbsolute(f) ? f : path.resolve(projectRoot, f),
    );
  } else if (options.files !== undefined && options.files.length === 0) {
    // Empty file list — return empty result
    return { detected: [], frameworks: [], plan: [] };
  } else {
    // Auto-scan
    filesToCheck = collectFiles(projectRoot);
  }

  // If no mappings exist, everything is "unknown"
  if (mappings.length === 0) {
    const detected: DetectedFile[] = filesToCheck.map((file) => ({
      file,
      framework: 'unknown',
    }));
    return { detected, frameworks: [], plan: [] };
  }

  // Generate execution plan from the configured framework mappings
  const plan: PlanEntry[] = [];
  for (const mapping of mappings) {
    try {
      const directory = deriveWorkingDirectory(mapping.glob);
      const config = runTestGetFrameworkConfig({ framework: mapping.framework });
      plan.push({
        directory,
        framework: config.framework,
        coverage_cmd: config.coverage_cmd,
        coverage_format: config.coverage_format,
        coverage_output: config.coverage_output,
        coverage_artifacts: config.coverage_artifacts,
        coverage_cleanup: config.coverage_cleanup,
      });
    } catch {
      // Skip entries for frameworks not in the registry
    }
  }

  // First-match per file
  const detected: DetectedFile[] = [];
  const frameworkSet = new Set<string>();

  for (const file of filesToCheck) {
    let matched = false;

    for (const mapping of mappings) {
      if (matchGlob(file, mapping.glob)) {
        detected.push({ file, framework: mapping.framework });
        frameworkSet.add(mapping.framework);
        matched = true;
        break;
      }
    }

    if (!matched) {
      detected.push({ file, framework: 'unknown' });
    }
  }

  return {
    detected,
    frameworks: Array.from(frameworkSet).sort(),
    plan,
  };
}
