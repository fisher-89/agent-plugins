import * as fs from 'fs';
import * as path from 'path';

import { readConfig } from '../lib/config';
import { matchGlob } from '../lib/glob';
import { type OpenSpecConfig, type TestFrameworks } from '../schemas';
import { getProjectDir } from '../utils';
import { getDefaultGlobForFramework, runTestGetFrameworkConfig } from './test-get-framework-config';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

interface FrameworkMapping {
  glob: string;
  framework: TestFrameworks;
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

interface PlanEntry {
  directory: string;
  framework: string;
  coverage_cmd: string;
  coverage_format: 'istanbul' | 'llvm-cov';
  coverage_output: string;
  coverage_artifacts: string[];
  coverage_cleanup: string[];
  script: string;
}

export interface TestDetectFrameworksOptions {
  files?: string[];
  projectRoot?: string;
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
 * Normalise the `test.framework` + `test.overrides` config into an array of
 * `{glob, framework}` mappings.
 *
 * - If `framework` is set, it becomes the first mapping using the framework's default glob.
 * - For each override with a `framework` field, a mapping is added using the override's `file` glob.
 * - If both are empty/undefined, return an empty array.
 */
function normalizeFrameworks(
  framework: OpenSpecConfig['test']['framework'],
  overrides: OpenSpecConfig['test']['overrides'],
): FrameworkMapping[] {
  const frameworkMapping: FrameworkMapping[] = [];
  if (framework) {
    const glob = getDefaultGlobForFramework(framework);
    frameworkMapping.push({ glob, framework });
  }

  if (Array.isArray(overrides)) {
    for (const override of overrides) {
      if (override.framework) {
        frameworkMapping.push({ glob: override.file, framework: override.framework });
      }
    }
  }

  return frameworkMapping;
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
// generateScript
// ---------------------------------------------------------------------------

export interface GenerateScriptInput {
  directory: string;
  coverage_cmd: string;
  coverage_cleanup: string[];
}

/**
 * Generate a bash execution script from plan entry fields.
 *
 * Rules:
 * 1. First line is always `#!/bin/bash`
 * 2. Second line is always `set -e`
 * 3. When `directory !== "."`, insert `cd <directory>` line
 * 4. For each entry in `coverage_cleanup`, insert `rm -rf <item>` line
 * 5. When `coverage_cleanup` is empty, skip all `rm -rf` lines
 * 6. Last line is `<coverage_cmd>`
 * 7. Lines are separated by `\n`, trailing newline included
 */
export function generateScript(input: GenerateScriptInput): string {
  if (input === null || input === undefined) {
    throw new TypeError('generateScript input must not be null or undefined');
  }

  const { directory, coverage_cmd, coverage_cleanup } = input;

  if (typeof directory !== 'string') {
    throw new TypeError('generateScript: directory must be a string');
  }
  if (typeof coverage_cmd !== 'string') {
    throw new TypeError('generateScript: coverage_cmd must be a string');
  }
  if (!Array.isArray(coverage_cleanup)) {
    throw new TypeError('generateScript: coverage_cleanup must be an array');
  }

  const lines: string[] = [];

  lines.push('#!/bin/bash');
  lines.push('set -e');

  if (directory !== '.') {
    lines.push(`cd ${directory}`);
  }

  for (const item of coverage_cleanup) {
    lines.push(`rm -rf ${item}`);
  }

  lines.push(coverage_cmd);

  return lines.join('\n') + '\n';
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
  const projectRoot = options.projectRoot || getProjectDir();

  const config = readConfig(projectRoot);
  const { framework, overrides } = config.test;
  const mappings = normalizeFrameworks(framework, overrides);

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
        script: generateScript({
          directory,
          coverage_cleanup: config.coverage_cleanup,
          coverage_cmd: config.coverage_cmd,
        }),
      });
    } catch {
      // Skip entries for frameworks not in the registry
    }
  }

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

  const isAutoScan = options.files === undefined;

  // First-match per file (use relative path for glob matching)
  const detected: DetectedFile[] = [];
  const frameworkSet = new Set<string>();

  for (const file of filesToCheck) {
    const relativePath = path.isAbsolute(file) ? path.relative(projectRoot, file) : file;
    let matched = false;

    for (const mapping of mappings) {
      if (matchGlob(relativePath, mapping.glob)) {
        detected.push({ file, framework: mapping.framework });
        frameworkSet.add(mapping.framework);
        matched = true;
        break;
      }
    }

    if (!matched && !isAutoScan) {
      detected.push({ file, framework: 'unknown' });
    }
  }

  return {
    detected,
    frameworks: Array.from(frameworkSet).sort(),
    plan,
  };
}
