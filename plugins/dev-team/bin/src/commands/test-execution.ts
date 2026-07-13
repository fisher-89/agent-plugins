// ---------------------------------------------------------------------------
// Test Execution Command Handler
//
// CLI entry point for `dev-team test-execution`.  Orchestrates the full flow:
//   1. Detect frameworks via runTestDetectFrameworks
//   2. For each framework plan entry, execute via executePlanEntry
//   3. Generate per-framework sub-reports via generateSubReport
//   4. Generate summary report via generateSummaryReport
// ---------------------------------------------------------------------------

import * as path from 'path';

import { getGitDiffFiles } from '../lib/git';
import { generateSubReport, generateSummaryReport } from '../lib/test-report';
import { executePlanEntry } from '../lib/test-runner';
import { getProjectDir } from '../utils';
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
    return path.resolve(projectRoot, 'openspec', 'changes', change, 'reports', 'test-execution');
  }
  return path.resolve(projectRoot, 'reports', 'test-execution');
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
  console.log(`--mutation-diff-only: ${files.length} files in working tree diff`);
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
// Main handler
// ---------------------------------------------------------------------------

export async function runTestExecution(options: TestExecutionOptions): Promise<number> {
  const projectRoot = options.projectRoot || getProjectDir();
  const detectResult = runTestDetectFrameworks({ files: options.files, projectRoot });

  if (detectResult.plan.length === 0) {
    console.log('No test configuration found. Configure test.framework in openspec/config.json');
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
    console.log(`Running ${entry.framework} tests in ${entry.directory}...`);
    const planFiles = options.files
      ?.map((filePath) => path.posix.relative(entry.directory, filePath))
      .filter((filePath) => !filePath.startsWith('..'));
    const result = executePlanEntry(entry, projectRoot, {
      files: planFiles,
      noMutation: options.noMutation,
      mutationDiffFiles,
    });
    const subReport = generateSubReport(
      entry.framework,
      result,
      projectRoot,
      reportsDir,
      entry.directory,
    );
    logResult(entry.framework, result);
    subReports.push(subReport);
  }

  const summaryReport = generateSummaryReport(subReports, projectRoot, reportsDir);
  logSummary(summaryReport);

  return summaryReport.conclusion === 'pass' ? 0 : 1;
}
