// ---------------------------------------------------------------------------
// Test Execution Command Handler
//
// CLI entry point for `dev-team test-execution`.  Orchestrates the full flow:
//   1. Detect frameworks via runTestDetectFrameworks
//   2. With --change: gate plan entries to the change file inventory
//   3. For each framework plan entry, execute via executePlanEntry
//   4. Generate per-plan atomic reports via generateSubReport
//   5. Generate summary report via generateSummaryReport
// ---------------------------------------------------------------------------

import { execFileSync } from 'node:child_process';
import * as fs from 'node:fs';
import * as path from 'path';

import { getProjectDir } from '../lib/project-root';
import { deriveSourcePathFromTestFile } from '../lib/test-path-naming';
import { isUnderPlanRoot, resolvePlanFiles } from '../lib/test-plan';
import { generateSubReport, generateSummaryReport } from '../lib/test-report';
import { executePlanEntry } from '../lib/test-runner';
import { getChangedFiles } from '../modules/workflow';
import type { TestExecutionSubReport, TestPlan } from '../schemas';
import { runTestDetectFrameworks } from './test-detect-frameworks';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface TestExecutionOptions {
  /**
   * Change name. Locates the reports directory AND narrows execution to the
   * change file inventory (`workflow.json` file_log net state):
   * - Plan gating: plan entries whose suite `root` contains no inventory file
   *   (written ∪ deleted) are skipped; inventory read failures fail open
   *   (all entries run).
   * - Mutation scope (when mutation is not skipped): derived from
   *   `written` with the net-zero denoise filter applied.
   * No dedicated scope option exists.
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
 * Resolve the mutation scope from the change file inventory (`written` bucket
 * of the net state already read by the caller).
 */
function resolveMutationDiffFiles(written: string[], projectRoot: string): string[] | undefined {
  const expanded = expandMutationDiffWithInferredSources(written);
  const scoped = filterNetZeroFiles(expanded, projectRoot);
  console.log(
    `mutation scope (change inventory): ${scoped.length} files (${scoped.slice(0, 5).join(', ')})`,
  );
  return scoped.map((file) => path.resolve(projectRoot, file).replace(/\\/g, '/'));
}

// ---------------------------------------------------------------------------
// Change-scope plan gating
// ---------------------------------------------------------------------------

/**
 * One --change-scoped execution context: the inventory read plus the gate
 * file set derived from it.
 */
interface ChangeScope {
  /** `written` bucket of the net state; undefined without --change or on fail-open. */
  inventoryWritten?: string[];
  /** written ∪ deleted (POSIX); undefined when gating is inactive. */
  gateFiles?: string[];
}

/**
 * Read the change file inventory once. Gate reads fail open — an unreadable
 * inventory never blocks execution (the `--skip-mutation` escape hatch);
 * the mutation scope keeps its hard-error contract when mutation is enabled.
 */
function resolveChangeScope(
  change: string | undefined,
  noMutation: boolean,
  projectRoot: string,
): ChangeScope {
  if (!change) return {};
  let net: ReturnType<typeof getChangedFiles>;
  try {
    net = getChangedFiles(change, projectRoot);
  } catch (e) {
    if (!noMutation) throw e;
    const message = e instanceof Error ? e.message : String(e);
    console.log(`change scope gate: inventory read failed (${message}) — running all plan entries`);
    return {};
  }
  const gateFiles = [...net.written, ...net.deleted].map((file) => file.replace(/\\/g, '/'));
  return {
    inventoryWritten: net.written,
    gateFiles: gateFiles.length > 0 ? gateFiles : undefined,
  };
}

/**
 * Filter plan entries to those whose suite `root` contains at least one
 * changed file (written ∪ deleted). Root-level matching only — colocated
 * test/source pairs share a directory so both resolve to the same root, and
 * includes/excludes are deliberately ignored (over-trigger runs an unrelated
 * suite; under-trigger would skip a suite whose tests should run).
 *
 * Safety net: a non-empty change that matches no suite root at all suggests a
 * layout the root rule cannot see — all entries run in that case.
 */
function gatePlanEntries(planEntries: TestPlan[], gateFiles: string[]): TestPlan[] {
  const inScope = planEntries.filter((entry) =>
    gateFiles.some((file) => isUnderPlanRoot(file, entry.root)),
  );

  if (inScope.length === 0) {
    console.log(
      'change scope gate: no changed files under any suite root — running all plan entries',
    );
    return planEntries;
  }

  for (const entry of planEntries) {
    if (!inScope.includes(entry)) {
      console.log(
        `Skipping ${entry.framework} in ${entry.root}: no changed files under suite root (change scope)`,
      );
    }
  }
  console.log(
    `change scope gate: ${inScope.length}/${planEntries.length} plan entries in change scope`,
  );
  return inScope;
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

  const { inventoryWritten, gateFiles } = resolveChangeScope(
    options.change,
    options.noMutation === true,
    projectRoot,
  );

  const mutationDiffFiles =
    inventoryWritten !== undefined && !options.noMutation
      ? resolveMutationDiffFiles(inventoryWritten, projectRoot)
      : undefined;
  const reportsDir = resolveReportsDir(projectRoot, options.change);
  const subReports = [];

  const effectivePlanEntries = gateFiles ? gatePlanEntries(planEntries, gateFiles) : planEntries;

  for (const entry of effectivePlanEntries) {
    const subReport = runPlanEntry(entry, projectRoot, options, mutationDiffFiles, reportsDir);
    if (subReport) {
      subReports.push(subReport);
    }
  }

  const summaryReport = generateSummaryReport(subReports, projectRoot, reportsDir);
  logSummary(summaryReport);

  return summaryReport.conclusion === 'pass' ? 0 : 1;
}
