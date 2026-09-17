// ---------------------------------------------------------------------------
// Test Execution Command Handler
//
// CLI entry point for `dev-team test-execution`.  Orchestrates the full flow:
//   1. Detect frameworks via runTestDetectFrameworks
//   2. For each framework plan entry, execute via executePlanEntry
//   3. Generate per-plan atomic reports via generateSubReport
//   4. Generate summary report via generateSummaryReport
// ---------------------------------------------------------------------------

import { execFileSync } from 'node:child_process';
import * as fs from 'node:fs';
import * as path from 'path';

import { getChangeDir } from '../lib/change';
import { readFileInventory } from '../lib/file-inventory';
import { getProjectDir } from '../lib/project-root';
import { deriveSourcePathFromTestFile } from '../lib/test-path-naming';
import { resolvePlanFiles } from '../lib/test-plan';
import { generateSubReport, generateSummaryReport } from '../lib/test-report';
import { executePlanEntry } from '../lib/test-runner';
import type { TestExecutionSubReport, TestPlan } from '../schemas';
import { runTestDetectFrameworks } from './test-detect-frameworks';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface TestExecutionOptions {
  /**
   * Change name. Locates the reports directory AND selects the mutation
   * scope: when set (and mutation is not skipped), the scope comes from the
   * change file inventory (`workflow.json.files.written`) with the net-zero
   * denoise filter applied. No dedicated scope option exists.
   */
  change?: string;
  projectRoot?: string;
  /** test files to run */
  files?: string[];
  framework?: string;
  noMutation?: boolean;
}

// ---------------------------------------------------------------------------
// Reports directory resolution
// ---------------------------------------------------------------------------

function resolveReportsDir(projectRoot: string, change?: string): string {
  if (change) {
    return path.resolve(projectRoot, 'openspec', 'changes', change, 'reports', 'test');
  }
  return path.resolve(projectRoot, 'reports', 'test');
}

// ---------------------------------------------------------------------------
// Mutation scope resolution (change file inventory)
// ---------------------------------------------------------------------------

/**
 * Expand inventory paths so test-file entries also contribute their
 * colocated sources (via test-path-naming reverse mapping).
 */
function expandMutationDiffWithInferredSources(files: string[]): string[] {
  const result = new Set<string>();
  for (const file of files) {
    const posix = file.replace(/\\/g, '/');
    result.add(posix);
    const source = deriveSourcePathFromTestFile(posix);
    if (source) {
      result.add(source);
    }
  }
  return Array.from(result);
}

/**
 * Return the `HEAD` version of a file, or null when HEAD has no such file or
 * the read fails — the caller then keeps the file (overstate direction only).
 */
function headFileContent(projectRoot: string, file: string): string | null {
  try {
    return execFileSync('git', ['show', `HEAD:${file}`], {
      cwd: projectRoot,
      encoding: 'utf-8',
      timeout: 10000,
      stdio: ['ignore', 'pipe', 'ignore'],
    });
  } catch {
    return null;
  }
}

/**
 * Net-zero denoise: drop files whose working-tree content is identical to
 * their HEAD version (written then reverted in place) — mutating them is pure
 * noise. The filter is an optimization only, never the scope authority;
 * correctness of filtered files stays with the evaluator content check.
 * Unreadable working-tree files and HEAD-less (new) files are kept.
 */
function filterNetZeroFiles(files: string[], projectRoot: string): string[] {
  const kept: string[] = [];
  let excluded = 0;
  for (const file of files) {
    let working: string | null = null;
    try {
      working = fs.readFileSync(path.resolve(projectRoot, file), 'utf-8');
    } catch {
      kept.push(file);
      continue;
    }
    const head = headFileContent(projectRoot, file);
    if (head !== null && head === working) {
      excluded += 1;
      continue;
    }
    kept.push(file);
  }
  if (excluded > 0) {
    console.log(
      `mutation denoise: ${excluded} net-zero files excluded (content identical to HEAD)`,
    );
  }
  return kept;
}

/**
 * Resolve the mutation scope from the change file inventory.
 */
function resolveMutationDiffFiles(change: string, projectRoot: string): string[] | undefined {
  const changeDir = getChangeDir(change, projectRoot);
  const written = readFileInventory(changeDir).written;
  const expanded = expandMutationDiffWithInferredSources(written);
  const scoped = filterNetZeroFiles(expanded, projectRoot);
  console.log(
    `mutation scope (change inventory): ${scoped.length} files (${scoped.slice(0, 5).join(', ')})`,
  );
  return scoped.map((file) => path.resolve(projectRoot, file).replace(/\\/g, '/'));
}

// ---------------------------------------------------------------------------
// Logging helpers
// ---------------------------------------------------------------------------

function logResult(entryFramework: string, result: { testCases: Array<{ status: string }> }): void {
  const testCases = result.testCases;
  console.log(
    `  ${entryFramework}: ${testCases.length} tests, ` +
      `${testCases.filter((t) => t.status === 'passed').length} passed, ` +
      `${testCases.filter((t) => t.status === 'failed').length} failed, ` +
      `${testCases.filter((t) => t.status === 'skipped').length} skipped`,
  );
}

function logSummary(report: {
  conclusion: string;
  total: number;
  passed: number;
  failed: number;
  skipped: number;
  duration_seconds: number;
  problems: Array<{ type: string; framework: string; message: string }>;
}): void {
  console.log(`\nSummary: ${report.conclusion.toUpperCase()}`);
  console.log(`  Total: ${report.total}`);
  console.log(`  Passed: ${report.passed}`);
  console.log(`  Failed: ${report.failed}`);
  console.log(`  Skipped: ${report.skipped}`);
  console.log(`  Duration: ${report.duration_seconds}s`);

  if (report.problems.length > 0) {
    console.log(`\nProblems (${report.problems.length}):`);
    for (const problem of report.problems) {
      console.log(`  [${problem.type}] ${problem.framework}: ${problem.message}`);
    }
  }
}

// ---------------------------------------------------------------------------
// Per-plan execution
// ---------------------------------------------------------------------------

/**
 * Run one plan entry. Returns null when explicit `--files` has no overlap with
 * the suite root (skip — do not fall back to full-scope discovery).
 */
function runPlanEntry(
  entry: TestPlan,
  projectRoot: string,
  options: TestExecutionOptions,
  mutationDiffFiles: string[] | undefined,
  reportsDir: string,
): TestExecutionSubReport | null {
  const planFiles = resolvePlanFiles(options.files, entry);
  if (options.files !== undefined && (planFiles?.length ?? 0) === 0) {
    console.log(`Skipping ${entry.framework} in ${entry.root}: no files under suite root`);
    return null;
  }

  console.log(`Running ${entry.framework} tests in ${entry.cwd} (root: ${entry.root})...`);
  const result = executePlanEntry(entry, projectRoot, {
    files: planFiles,
    noMutation: options.noMutation,
    mutationDiffFiles,
    reportsDir,
  });
  const subReport = generateSubReport(entry.framework, result, projectRoot, reportsDir, entry.root);
  logResult(entry.framework, result);
  return subReport;
}

// ---------------------------------------------------------------------------
// Main handler
// ---------------------------------------------------------------------------

export async function runTestExecution(options: TestExecutionOptions): Promise<number> {
  const projectRoot = options.projectRoot || getProjectDir();
  const detectResult = runTestDetectFrameworks({ files: options.files, projectRoot });

  if (detectResult.plan.length === 0) {
    console.log('No test configuration found. Configure tests in openspec/config.json');
    return 0;
  }

  const planEntries = options.framework
    ? detectResult.plan.filter((entry) => entry.framework === options.framework)
    : detectResult.plan;

  if (planEntries.length === 0) {
    console.log(`No plan entries found for framework "${options.framework}"`);
    return 0;
  }

  const mutationDiffFiles =
    options.change && !options.noMutation
      ? resolveMutationDiffFiles(options.change, projectRoot)
      : undefined;
  const reportsDir = resolveReportsDir(projectRoot, options.change);
  const subReports = [];

  for (const entry of planEntries) {
    const subReport = runPlanEntry(entry, projectRoot, options, mutationDiffFiles, reportsDir);
    if (subReport) {
      subReports.push(subReport);
    }
  }

  const summaryReport = generateSummaryReport(subReports, projectRoot, reportsDir);
  logSummary(summaryReport);

  return summaryReport.conclusion === 'pass' ? 0 : 1;
}
