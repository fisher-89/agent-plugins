import * as fs from 'fs';
import * as path from 'path';

import { readConfig } from '../lib/config';
import { toForwardSlash } from '../lib/glob';
import { getProjectDir } from '../lib/project-root';
import { isFileExcluded } from '../lib/test-exclude';
import { type FrameworkConfig, detectFrameworkVersion } from '../lib/test-framework';
import { type ResolvedSuite, isInSuiteScope, resolveAllSuites } from '../lib/test-plan';
import {
  type TestDetectFrameworksResult,
  type TestPlan,
  type OpenSpecConfig,
  type TestFramework,
  type TestSuite,
} from '../schemas';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

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
// Suite config validation
// ---------------------------------------------------------------------------

/**
 * Validate that a suite declaring `config` uses a framework that supports injection.
 * Actual `{config_args}` expansion is deferred to execute (preparePlanArtifacts).
 */
function validateSuiteConfig(
  suite: TestSuite,
  frameworkConfig: FrameworkConfig,
  absConfig: string | null,
): void {
  if (!suite.config) {
    return;
  }
  if (!frameworkConfig.config_flag) {
    throw new Error(
      `Suite root "${suite.root}" declares config "${suite.config}" but framework ` +
        `"${suite.framework}" does not support config injection (config_flag is null)`,
    );
  }
  if (!absConfig) {
    throw new Error(
      `Suite root "${suite.root}" declares config "${suite.config}" but absConfig could not be resolved`,
    );
  }
}

// ---------------------------------------------------------------------------
// generateShellScript / generateCmdScript
// ---------------------------------------------------------------------------

/**
 * Generate a POSIX shell (bash) execution script from the framework template.
 *
 * Report-related placeholders ({results_file}, {coverage_file}, {report_dir},
 * {config_args}, …) are intentionally left unexpanded — execute resolves them
 * against the plan report directory. Suite cwd cleanup is no longer injected;
 * execute clears the plan directory instead. The script is run with cwd=absCwd,
 * so no `cd` prefix is added.
 */
function generateShellScript(frameworkConfig: FrameworkConfig, version: string): string {
  if (frameworkConfig === null || frameworkConfig === undefined) {
    throw new TypeError('generateShellScript input must not be null or undefined');
  }

  const test_execution = frameworkConfig.shell.test_execution(version);
  if (typeof test_execution !== 'string') {
    throw new TypeError('generateShellScript: test_execution must return a string');
  }

  return test_execution + '\n';
}

/**
 * Generate a Windows cmd.exe execution script from the framework registry.
 *
 * Same placeholder-deferral rules as {@link generateShellScript}. Execute runs
 * with cwd=absCwd, so no `cd /d` prefix is added.
 */
function generateCmdScript(frameworkConfig: FrameworkConfig, version: string): string {
  if (frameworkConfig === null || frameworkConfig === undefined) {
    throw new TypeError('generateCmdScript input must not be null or undefined');
  }

  const test_execution = frameworkConfig.cmd.test_execution(version);
  if (typeof test_execution !== 'string') {
    throw new TypeError('generateCmdScript: test_execution must return a string');
  }

  return test_execution;
}

/** Copy mutation_execution templates onto the plan; null when unsupported. */
function buildMutationScript(
  frameworkConfig: FrameworkConfig,
  version: string,
): NonNullable<TestPlan['mutation_script']> | null {
  const shell = frameworkConfig.shell.mutation_execution?.(version);
  if (!shell) return null;
  return {
    shell,
    cmd: frameworkConfig.cmd.mutation_execution
      ? frameworkConfig.cmd.mutation_execution(version)
      : shell,
  };
}

// ---------------------------------------------------------------------------
// Plan and detection helpers
// ---------------------------------------------------------------------------

/** Build plan from already-resolved suites (dedupe by cwd + framework). */
function buildPlanFromSuites(resolvedSuites: ResolvedSuite[]): TestPlan[] {
  const plan: TestPlan[] = [];
  const seen = new Set<string>();

  for (const resolved of resolvedSuites) {
    const { suite, frameworkConfig } = resolved;
    const dedupeKey = `${resolved.cwd}::${suite.framework}`;
    if (seen.has(dedupeKey)) {
      continue;
    }
    seen.add(dedupeKey);

    validateSuiteConfig(suite, frameworkConfig, resolved.absConfig);
    const version = detectFrameworkVersion(suite.framework, resolved.absCwd);

    plan.push({
      cwd: resolved.cwd,
      root: resolved.root,
      framework: frameworkConfig.framework,
      coverage_format: frameworkConfig.coverage_format,
      coverage_output: frameworkConfig.coverage_output,
      mutation_config: null,
      mutation_score: suite.mutation?.score ?? null,
      mutation_script: buildMutationScript(frameworkConfig, version),
      script: {
        shell: generateShellScript(frameworkConfig, version),
        cmd: generateCmdScript(frameworkConfig, version),
      },
    });
  }

  return plan;
}

function detectFrameworksForFiles(
  filesToCheck: string[],
  projectRoot: string,
  resolvedSuites: ResolvedSuite[],
  isAutoScan: boolean,
  config: OpenSpecConfig,
): { detected: DetectedFile[] } {
  const detected: DetectedFile[] = [];

  for (const file of filesToCheck) {
    const relativePath = toForwardSlash(
      path.isAbsolute(file) ? path.relative(projectRoot, file) : file,
    );

    // Skip excluded files — they don't participate in framework detection
    if (isFileExcluded(relativePath, config) || isFileExcluded(file, config)) {
      continue;
    }

    let matched = false;

    // Array order priority: first matching suite wins
    for (const resolved of resolvedSuites) {
      if (isInSuiteScope(relativePath, resolved.suite)) {
        detected.push({ file, framework: resolved.suite.framework });
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
    return options.files.map((f) => (path.isAbsolute(f) ? f : path.resolve(projectRoot, f)));
  }
  if (options.files !== undefined && options.files.length === 0) {
    return 'empty';
  }
  return collectFiles(projectRoot);
}

function buildNoSuitesResult(
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

// ---------------------------------------------------------------------------
// Main handler
// ---------------------------------------------------------------------------

/**
 * Run `test_detect_frameworks`: detect which test framework(s) each file
 * belongs to, based on the `tests[]` suite mappings in config.json.
 *
 * When `files` is provided, match only those files.  When omitted,
 * auto-scan the project for files matching any of the configured suite scopes.
 */
export function runTestDetectFrameworks(
  options: TestDetectFrameworksOptions,
): TestDetectFrameworksResult {
  const projectRoot = options.projectRoot || getProjectDir();

  const config = readConfig(projectRoot);
  const suites = config.tests ?? [];
  const resolvedSuites = resolveAllSuites(suites, projectRoot);
  const plan = buildPlanFromSuites(resolvedSuites);

  const filesResult = resolveFilesToCheck(options, projectRoot);
  if (filesResult === 'empty') {
    return { detected: [], plan: [] };
  }
  const filesToCheck = filesResult;
  const isAutoScan = options.files === undefined;

  if (suites.length === 0) {
    return buildNoSuitesResult(filesToCheck, isAutoScan);
  }

  const { detected } = detectFrameworksForFiles(
    filesToCheck,
    projectRoot,
    resolvedSuites,
    isAutoScan,
    config,
  );

  return { detected, plan };
}
