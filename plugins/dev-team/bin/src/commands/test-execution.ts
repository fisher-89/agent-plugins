// ---------------------------------------------------------------------------
// Test Execution Command Handler
//
// CLI entry point for `dev-team test-execution`.  Orchestrates the full flow:
//   1. Detect frameworks via runTestDetectFrameworks
//   2. For each framework plan entry, execute via executePlanEntry
//   3. Generate per-plan atomic reports via generateSubReport
//   4. Generate summary report via generateSummaryReport
// ---------------------------------------------------------------------------

import * as path from 'path';

import { getGitDiffFiles } from '../lib/git';
import { getProjectDir } from '../lib/project-root';
import { resolvePlanFiles } from '../lib/test-plan';
import { generateSubReport, generateSummaryReport } from '../lib/test-report';
import { executePlanEntry } from '../lib/test-runner';
import type { TestExecutionSubReport, TestPlan } from '../schemas';
import { runTestDetectFrameworks } from './test-detect-frameworks';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface TestExecutionOptions {
  change?: string;
  projectRoot?: string;
  /** test files to run */
  files?: string[];
  framework?: string;
  noMutation?: boolean;
  /** Only mutate files changed in git diff (mutation scope restricted to working tree changes) */
  mutationDiffOnly?: boolean;
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
// Mutation diff file resolution
// ---------------------------------------------------------------------------

/**
 * Resolve git diff file list when --mutation-diff-only is enabled.
 * Returns undefined when the option is not active.
 */
async function resolveMutationDiffFiles(
  mutationDiffOnly: boolean | undefined,
  projectRoot: string,
): Promise<string[] | undefined> {
  if (!mutationDiffOnly) return undefined;
  const files = await getGitDiffFiles(projectRoot);
  console.log(
    `--mutation-diff-only: ${files.length} files in working tree diff (${files.slice(0, 5).join(', ')})`,
  );
  return files.map((file) => path.resolve(projectRoot, file).replace(/\\/g, '/'));
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

  const mutationDiffFiles = await resolveMutationDiffFiles(options.mutationDiffOnly, projectRoot);
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
