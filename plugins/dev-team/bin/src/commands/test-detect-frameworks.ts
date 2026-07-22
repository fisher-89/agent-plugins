import * as fs from 'fs';
import * as path from 'path';

import { readConfig } from '../lib/config';
import { matchGlob } from '../lib/glob';
import { isFileExcluded } from '../lib/test-exclude';
import { type FrameworkConfig, getFrameworkConfig } from '../lib/test-framework';
import {
  type TestDetectFrameworksResult,
  type TestPlan,
  type OpenSpecConfig,
  type TestFramework,
} from '../schemas';
import { getProjectDir } from '../utils';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

interface FrameworkMapping {
  glob: string;
  framework: TestFramework;
}

interface DetectedFile {
  file: string;
  framework: TestFramework | 'unknown';
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
    const glob = getFrameworkConfig(framework).default_glob;
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
function deriveWorkingDirectory(glob: string): string {
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
// generateShellScript
// ---------------------------------------------------------------------------

/**
 * Generate a POSIX shell (bash) execution script from plan entry fields.
 * Uses Unix shell syntax: `cd`, `rm -rf`, `\n` line separation, `;` chaining
 * and `_X=$?` exit code capture (embedded in the test_execution template).
 */
function generateShellScript(directory: string, frameworkConfig: FrameworkConfig): string {
  if (frameworkConfig === null || frameworkConfig === undefined) {
    throw new TypeError('generateShellScript input must not be null or undefined');
  }

  const { test_execution, coverage_cleanup } = frameworkConfig.shell;

  if (typeof directory !== 'string') {
    throw new TypeError('generateShellScript: directory must be a string');
  }
  if (typeof test_execution !== 'string') {
    throw new TypeError('generateShellScript: test_execution must be a string');
  }
  if (!Array.isArray(coverage_cleanup)) {
    throw new TypeError('generateShellScript: coverage_cleanup must be an array');
  }

  const lines: string[] = [];

  if (directory !== '.') {
    lines.push(`cd ${directory}`);
  }

  for (const item of coverage_cleanup) {
    lines.push(`rm -rf ${item}`);
  }

  lines.push(test_execution);

  return lines.join('\n') + '\n';
}

// ---------------------------------------------------------------------------
// generateCmdScript
// ---------------------------------------------------------------------------

/**
 * Generate a Windows cmd.exe execution script from plan entry fields.
 * Uses cmd.exe compatible syntax:
 *   - `cd /d <dir>` instead of `cd <dir>` (cross-drive safe)
 *   - `if exist <item> (rmdir /s /q <item> 2>nul & del /f /q <item> 2>nul)` instead of `rm -rf`
 *   - `\r\n` line separators for cmd.exe compatibility
 *
 * Exit code capture relies on multi-line parsing where cmd.exe's `%errorlevel%`
 * correctly reflects the previous line's exit code (see design decision D1).
 */
function generateCmdScript(directory: string, frameworkConfig: FrameworkConfig): string {
  if (frameworkConfig === null || frameworkConfig === undefined) {
    throw new TypeError('generateCmdScript input must not be null or undefined');
  }

  const { test_execution, coverage_cleanup } = frameworkConfig.cmd;

  if (typeof directory !== 'string') {
    throw new TypeError('generateCmdScript: directory must be a string');
  }
  if (typeof test_execution !== 'string') {
    throw new TypeError('generateCmdScript: test_execution must be a string');
  }
  if (!Array.isArray(coverage_cleanup)) {
    throw new TypeError('generateCmdScript: coverage_cleanup must be an array');
  }

  const lines: string[] = [];

  if (directory !== '.') {
    // Quote paths with spaces for cmd.exe safety
    const quotedDir =
      directory.includes(' ') || directory.includes('\t')
        ? `"${directory.replace(/"/g, '\\"')}"`
        : directory;
    lines.push(`cd /d ${quotedDir}`);
  }

  for (const item of coverage_cleanup) {
    // rmdir/s/q works for directories, del/f/q works for files — combining
    // both ensures the target is removed regardless of its type.
    lines.push(`if exist "${item}" (rmdir /s /q "${item}" 2>nul & del /f /q "${item}" 2>nul)`);
  }

  lines.push(test_execution);

  return lines.join('\r\n') + '\r\n';
}

// ---------------------------------------------------------------------------
// Plan and detection helpers
// ---------------------------------------------------------------------------

function buildPlanFromMappings(mappings: FrameworkMapping[], projectRoot?: string): TestPlan[] {
  const plan: TestPlan[] = [];
  for (const mapping of mappings) {
    try {
      const directory = deriveWorkingDirectory(mapping.glob);
      const config = getFrameworkConfig(mapping.framework);
      plan.push({
        directory,
        framework: config.framework,
        coverage_format: config.coverage_format,
        coverage_output: config.coverage_output,
        coverage_artifacts: config.coverage_artifacts,
        mutation_framework: config.mutation_framework,
        mutation_config: null,
        mutation_score: null,
        script: {
          shell: generateShellScript(directory, config),
          cmd: generateCmdScript(directory, config),
        },
      });
    } catch {
      // Skip entries for frameworks not in the registry
    }
  }

  populateMutationConfig(plan, projectRoot);
  return plan;
}

/**
 * Populate mutation_config and mutation_score for each plan entry
 * from the project's config.json.
 */
function populateMutationConfig(plan: TestPlan[], projectRoot?: string): void {
  if (plan.length === 0) return;

  try {
    const root = projectRoot || getProjectDir();
    const config = readConfig(root);
    const mutationScore = config.test?.mutation?.score ?? null;
    const overrideConfigs = config.test?.overrides ?? [];

    for (const entry of plan) {
      if (entry.mutation_framework) {
        let matchedScore: number | null = null;
        for (const override of overrideConfigs) {
          if (override.mutation?.score !== undefined) {
            matchedScore = override.mutation.score;
          }
        }
        entry.mutation_score = mutationScore;
        entry.mutation_config = matchedScore !== null ? { score: matchedScore } : null;
      }
    }
  } catch {
    // Config read failure — mutation fields remain null
  }
}

function detectFrameworksForFiles(
  filesToCheck: string[],
  projectRoot: string,
  mappings: FrameworkMapping[],
  isAutoScan: boolean,
  config: OpenSpecConfig,
): { detected: DetectedFile[] } {
  const detected: DetectedFile[] = [];

  for (const file of filesToCheck) {
    // Skip excluded files — they don't participate in framework detection
    if (isFileExcluded(file, config)) {
      continue;
    }

    const relativePath = path.isAbsolute(file) ? path.relative(projectRoot, file) : file;
    let matched = false;

    for (const mapping of mappings) {
      if (matchGlob(relativePath, mapping.glob)) {
        detected.push({ file, framework: mapping.framework });
        matched = true;
        break;
      }
    }

    if (!matched && !isAutoScan) {
      detected.push({ file, framework: 'unknown' });
    }
  }

  return { detected };
}

function resolveFilesToCheck(
  options: TestDetectFrameworksOptions,
  projectRoot: string,
): string[] | 'empty' {
  if (options.files && options.files.length > 0) {
    // Provided file list — resolve relative paths against project root
    return options.files.map((f) => (path.isAbsolute(f) ? f : path.resolve(projectRoot, f)));
  }
  if (options.files !== undefined && options.files.length === 0) {
    // Empty file list — signal early return
    return 'empty';
  }
  // Auto-scan
  return collectFiles(projectRoot);
}

// ---------------------------------------------------------------------------
// Main handler
// ---------------------------------------------------------------------------

function buildNoMappingsResult(
  filesToCheck: string[],
  isAutoScan: boolean,
): TestDetectFrameworksResult {
  const detected: DetectedFile[] = isAutoScan
    ? []
    : filesToCheck.map((file) => ({
        file,
        framework: 'unknown',
      }));
  return { detected, plan: [] };
}

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
  const plan = buildPlanFromMappings(mappings, projectRoot);

  const filesResult = resolveFilesToCheck(options, projectRoot);
  if (filesResult === 'empty') {
    return { detected: [], plan: [] };
  }
  const filesToCheck = filesResult;
  const isAutoScan = options.files === undefined;

  if (mappings.length === 0) {
    return buildNoMappingsResult(filesToCheck, isAutoScan);
  }

  const { detected } = detectFrameworksForFiles(
    filesToCheck,
    projectRoot,
    mappings,
    isAutoScan,
    config,
  );

  return { detected, plan };
}
