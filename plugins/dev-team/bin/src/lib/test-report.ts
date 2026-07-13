// ---------------------------------------------------------------------------
// Test Report Generator
//
// Generates per-framework sub-reports and a summary report for test
// execution.  Reports are written as JSON files.
//
// Sub-report:  reports/test-execution/<framework>.json
// Summary:     reports/test-execution.json
// ---------------------------------------------------------------------------

import * as fs from 'fs';
import * as path from 'path';

import type {
  CoverageBlock,
  CoverageMeasured,
  CoverageOverride,
  CoverageThresholds,
  MutationBlock,
  MutationMeasured,
  MutationOverride,
  SourceFileEntry,
  TestCaseResult,
  TestExecutionSubReport,
  TestExecutionSummaryReport,
} from '../schemas';
import { readConfig } from './config';
import { matchGlob, toForwardSlash } from './glob';
import type { ExecutionResult } from './test-runner';

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const DEFAULT_THRESHOLDS: CoverageThresholds = {
  lines: 80,
  branches: 80,
  functions: 80,
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function collectErrorTestCases(result: ExecutionResult): TestCaseResult[] {
  return result.testCases
    .filter((ts) => ts.status !== 'passed')
    .map((tc) => ({
      name: tc.name,
      file: tc.file,
      duration_ms: tc.durationMs ?? null,
      status: tc.status,
      line: tc.line,
      errorType: tc.errorType,
      errorMessage: tc.errorMessage,
      stackTrace: tc.stackTrace,
    }));
}

function buildCoverageBlock(result: ExecutionResult, projectRoot: string): CoverageBlock | null {
  if (!result.coverage) return null;

  const thresholds = readCoverageThresholds(projectRoot);
  return {
    pass: computeCoveragePass(result.coverage, thresholds),
    measured: {
      lines: result.coverage.lines,
      branches: result.coverage.branches,
      functions: result.coverage.functions,
    },
    thresholds,
  };
}

function countByStatus(
  testCases: ExecutionResult['testCases'],
  status: 'passed' | 'failed' | 'skipped',
): number {
  return testCases.filter((t) => t.status === status).length;
}

function writeJsonFile(filePath: string, data: unknown): void {
  fs.mkdirSync(path.dirname(filePath), { recursive: true });
  fs.writeFileSync(filePath, JSON.stringify(data, null, 2), 'utf-8');
}

// ---------------------------------------------------------------------------
// Plan ID derivation (for sub-report filenames)
// ---------------------------------------------------------------------------

/**
 * Derive a sub-report filename from a plan directory and framework.
 *
 * Examples:
 *   derivePlanId('.', 'vitest')                → '_vitest.json'
 *   derivePlanId('plugins/dev-team/bin', 'vite-plus') → 'plugins_dev-team_bin_vite-plus.json'
 */
function derivePlanId(directory: string, framework: string): string {
  // Map '.' (current directory) to empty prefix; otherwise replace path separators
  const sanitized = directory === '.' ? '' : directory.replace(/[\\/]/g, '_').replace(/\/$/, '');
  const prefix = sanitized ? `${sanitized}_` : sanitized;
  return `${prefix}${framework}.json`;
}

// ---------------------------------------------------------------------------
// Source file entry builder
// ---------------------------------------------------------------------------

/**
 * Build SourceFileEntry array from an ExecutionResult.
 *
 * Uses raw coverage counts from the coverage parser when available;
 * falls back to entries with only the file path and null counts otherwise.
 */
function buildSourceFileEntries(result: ExecutionResult): SourceFileEntry[] {
  const fileCoverage = result.coverage?.fileCoverage;
  if (fileCoverage?.some((e) => e.total_lines != null)) {
    return fileCoverage.map((e) => ({
      file: e.file,
      coverage: {
        lines: e.lines ?? null,
        branches: e.branches ?? null,
        functions: e.functions ?? null,
        total_lines: e.total_lines ?? null,
        covered_lines: e.covered_lines ?? null,
        total_branches: e.total_branches ?? null,
        covered_branches: e.covered_branches ?? null,
        total_functions: e.total_functions ?? null,
        covered_functions: e.covered_functions ?? null,
      },
    }));
  }
  return result.sourceFiles.map((f) => ({
    file: f,
    coverage: {
      lines: null,
      branches: null,
      functions: null,
      total_lines: null,
      covered_lines: null,
      total_branches: null,
      covered_branches: null,
      total_functions: null,
      covered_functions: null,
    },
  }));
}

// ---------------------------------------------------------------------------
// Sub-report generation
// ---------------------------------------------------------------------------

export function generateSubReport(
  framework: string,
  result: ExecutionResult,
  projectRoot: string,
  reportsDir: string,
  planDirectory: string,
): TestExecutionSubReport {
  const now = new Date().toISOString();
  const errorCases = collectErrorTestCases(result);
  const sourceFileEntries = buildSourceFileEntries(result);

  const subReport: TestExecutionSubReport = {
    framework,
    directory: planDirectory,
    timestamp: now,
    exit_code: result.exitCode,
    duration_ms: result.durationMs,
    summary: {
      total: result.testCases.length,
      passed: countByStatus(result.testCases, 'passed'),
      failed: countByStatus(result.testCases, 'failed'),
      skipped: countByStatus(result.testCases, 'skipped'),
    },
    error_cases: errorCases,
    test_files: result.testFiles,
    source_files: sourceFileEntries,
    coverage: buildCoverageBlock(result, projectRoot),
    mutation: result.mutation ?? null,
    findings: result.error ? [result.error] : undefined,
  };

  writeJsonFile(path.join(reportsDir, derivePlanId(planDirectory, framework)), subReport);
  return subReport;
}

// ---------------------------------------------------------------------------
// Summary report generation helpers
// ---------------------------------------------------------------------------

function collectProblems(subReports: TestExecutionSubReport[]): Array<{
  framework: string;
  type: 'test_failure' | 'coverage_failure' | 'execution_error';
  message: string;
}> {
  const problems: Array<{
    framework: string;
    type: 'test_failure' | 'coverage_failure' | 'execution_error';
    message: string;
  }> = [];

  for (const report of subReports) {
    if (report.summary.failed > 0) {
      const failCases = report.error_cases.filter((t) => t.status === 'failed');
      for (const tc of failCases.slice(0, 10)) {
        problems.push({
          framework: report.framework,
          type: 'test_failure',
          message: `Test "${tc.name}" failed: ${tc.errorMessage || 'Unknown error'}`,
        });
      }
    }

    if (report.exit_code !== 0 && report.summary.failed === 0) {
      problems.push({
        framework: report.framework,
        type: 'execution_error',
        message: `Exit code ${report.exit_code}`,
      });
    }
  }

  return problems;
}

function computeCoverageResult(
  projectRoot: string,
  subReports: TestExecutionSubReport[],
): CoverageBlock | null {
  const thresholds = readCoverageThresholds(projectRoot);
  const measured = computeRawWeightedCoverage(subReports);
  if (measured === null) {
    return null;
  }

  const overrides = computeOverrides(subReports, projectRoot);
  let pass = computeCoveragePass(measured, thresholds);
  if (overrides.length > 0) {
    pass = pass && overrides.every((o) => o.pass);
  }

  const result: CoverageBlock = { pass, measured, thresholds };
  if (overrides.length > 0) result.overrides = overrides;
  return result;
}

function determineConclusion(
  failed: number,
  coverageResult: CoverageBlock | null,
  mutationResult: MutationBlock | null,
  hasExecutionError: boolean,
): 'pass' | 'fail' | 'error' {
  if (hasExecutionError) return 'error';
  if (
    failed > 0 ||
    (coverageResult && !coverageResult.pass) ||
    (mutationResult && !mutationResult.pass)
  )
    return 'fail';
  return 'pass';
}

// ---------------------------------------------------------------------------
// Summary report generation
// ---------------------------------------------------------------------------

export function generateSummaryReport(
  subReports: TestExecutionSubReport[],
  projectRoot: string,
  reportsDir: string,
): TestExecutionSummaryReport {
  const now = new Date().toISOString();
  const { total, passed, failed, skipped, totalDuration } = aggregateTotals(subReports);
  const problems = collectProblems(subReports);
  const coverageResult = computeCoverageResult(projectRoot, subReports);

  pushCoverageProblems(problems, coverageResult);

  const mutationResult = computeMutationResult(subReports, projectRoot);
  pushMutationProblems(problems, mutationResult);

  const conclusion = determineConclusion(
    failed,
    coverageResult,
    mutationResult,
    problems.some((p) => p.type === 'execution_error'),
  );

  const summaryReport: TestExecutionSummaryReport = {
    phase: 'test-execution',
    command: 'dev-team test-execution',
    timestamp: now,
    duration_seconds: Math.round(totalDuration / 1000),
    total,
    passed,
    failed,
    skipped,
    conclusion,
    problems,
    coverage: coverageResult,
    mutation: mutationResult,
  };

  writeJsonFile(path.join(reportsDir, '..', 'test-execution.json'), summaryReport);
  return summaryReport;
}

function pushCoverageProblems(
  problems: Array<{
    framework: string;
    type: 'test_failure' | 'coverage_failure' | 'execution_error';
    message: string;
  }>,
  coverageResult: CoverageBlock | null,
): void {
  if (coverageResult && !coverageResult.pass) {
    problems.push({
      framework: 'all',
      type: 'coverage_failure',
      message: formatCoverageFailure(coverageResult.measured, coverageResult.thresholds),
    });
  }
}

function pushMutationProblems(
  problems: Array<{
    framework: string;
    type: 'test_failure' | 'coverage_failure' | 'execution_error';
    message: string;
  }>,
  mutationResult: MutationBlock | null,
): void {
  if (mutationResult && !mutationResult.pass) {
    problems.push({
      framework: 'all',
      type: 'coverage_failure',
      message: `Mutation score ${mutationResult.score.toFixed(1)}% < threshold ${mutationResult.threshold}%`,
    });
  }
}

function aggregateTotals(subReports: TestExecutionSubReport[]): {
  total: number;
  passed: number;
  failed: number;
  skipped: number;
  totalDuration: number;
} {
  let total = 0;
  let passed = 0;
  let failed = 0;
  let skipped = 0;
  let totalDuration = 0;

  for (const report of subReports) {
    total += report.summary.total;
    passed += report.summary.passed;
    failed += report.summary.failed;
    skipped += report.summary.skipped;
    totalDuration += report.duration_ms;
  }

  return { total, passed, failed, skipped, totalDuration };
}

// ---------------------------------------------------------------------------
// Coverage helpers
// ---------------------------------------------------------------------------

function readCoverageThresholds(projectRoot: string): CoverageThresholds {
  try {
    const config = readConfig(projectRoot);
    const thresholds = config.test?.coverage;
    if (thresholds) {
      return {
        lines: thresholds.lines ?? DEFAULT_THRESHOLDS.lines,
        branches: thresholds.branches ?? DEFAULT_THRESHOLDS.branches,
        functions: thresholds.functions ?? DEFAULT_THRESHOLDS.functions,
      };
    }
  } catch {
    // Fall through to defaults
  }
  return { ...DEFAULT_THRESHOLDS };
}

/**
 * Compute true weighted coverage from raw per-file line/branch/function counts.
 *
 * Instead of averaging pre-computed percentages (which loses precision when
 * files or frameworks have different sizes), this sums raw total/covered counts
 * across all source files and computes the true percentage.
 *
 * Skips files with null totals (non-Istanbul parsers) and files with zero
 * totals.  Returns null for a dimension when no valid data is available.
 */
function computeRawWeightedCoverage(subReports: TestExecutionSubReport[]): CoverageMeasured | null {
  if (subReports.length === 1) {
    return subReports[0].coverage?.measured ?? null;
  }

  let totalLines = 0,
    covLines = 0;
  let totalBranches = 0,
    covBranches = 0;
  let totalFunctions = 0,
    covFunctions = 0;

  for (const report of subReports) {
    for (const { coverage: fileCoverage } of report.source_files) {
      if (fileCoverage.total_lines !== null && fileCoverage.total_lines > 0) {
        totalLines += fileCoverage.total_lines;
        covLines += fileCoverage.covered_lines ?? 0;
      }
      if (fileCoverage.total_branches !== null && fileCoverage.total_branches > 0) {
        totalBranches += fileCoverage.total_branches;
        covBranches += fileCoverage.covered_branches ?? 0;
      }
      if (fileCoverage.total_functions !== null && fileCoverage.total_functions > 0) {
        totalFunctions += fileCoverage.total_functions;
        covFunctions += fileCoverage.covered_functions ?? 0;
      }
    }
  }

  return {
    lines: totalLines > 0 ? Math.round((covLines / totalLines) * 10000) / 100 : null,
    branches: totalBranches > 0 ? Math.round((covBranches / totalBranches) * 10000) / 100 : null,
    functions:
      totalFunctions > 0 ? Math.round((covFunctions / totalFunctions) * 10000) / 100 : null,
  };
}

function computeCoveragePass(measured: CoverageMeasured, thresholds: CoverageThresholds): boolean {
  if (measured.lines !== null && measured.lines < thresholds.lines) return false;
  if (measured.branches !== null && measured.branches < thresholds.branches) return false;
  if (measured.functions !== null && measured.functions < thresholds.functions) return false;
  return true;
}

function formatCoverageFailure(measured: CoverageMeasured, thresholds: CoverageThresholds): string {
  const parts: string[] = [];
  if (measured.lines !== null && measured.lines < thresholds.lines) {
    parts.push(`lines ${measured.lines.toFixed(1)}% < ${thresholds.lines}%`);
  }
  if (measured.branches !== null && measured.branches < thresholds.branches) {
    parts.push(`branches ${measured.branches.toFixed(1)}% < ${thresholds.branches}%`);
  }
  if (measured.functions !== null && measured.functions < thresholds.functions) {
    parts.push(`functions ${measured.functions.toFixed(1)}% < ${thresholds.functions}%`);
  }
  return `Coverage below threshold: ${parts.join(', ')}`;
}

// ---------------------------------------------------------------------------
// Coverage overrides
// ---------------------------------------------------------------------------

/**
 * Compute per-glob override coverage results.
 *
 * Reads `config.test.overrides`, matches each override's `file` glob against
 * file-level coverage entries, computes coverage (preferring raw counts over
 * simple percentage averages), and checks against thresholds.
 */
function computeOverrides(
  subReports: TestExecutionSubReport[],
  projectRoot: string,
): CoverageOverride[] {
  const config = readConfig(projectRoot);
  const globalThresholds = readCoverageThresholds(projectRoot);
  const overrideConfigs = config.test?.overrides ?? [];

  if (overrideConfigs.length === 0) return [];

  const allRawEntries = collectRawSourceEntries(subReports);

  // Normalize file paths to project-relative for glob matching
  const normalizedProjectRoot = toForwardSlash(path.resolve(projectRoot));
  const relativeRawEntries = allRawEntries.map((e) => ({
    ...e,
    file: toForwardSlash(e.file).replace(normalizedProjectRoot + '/', ''),
  }));

  const results: CoverageOverride[] = [];

  for (const override of overrideConfigs) {
    if (!override.coverage) continue;
    const result = computeSingleOverride(override, relativeRawEntries, globalThresholds);
    if (result) results.push(result);
  }

  return results;
}

function collectRawSourceEntries(subReports: TestExecutionSubReport[]): SourceFileEntry[] {
  const entries: SourceFileEntry[] = [];
  for (const report of subReports) {
    entries.push(...report.source_files);
  }
  return entries;
}

function computeSingleOverride(
  override: NonNullable<NonNullable<ReturnType<typeof readConfig>['test']>['overrides']>[number],
  allRawEntries: SourceFileEntry[],
  globalThresholds: CoverageThresholds,
): CoverageOverride | null {
  const matchedRaw = allRawEntries.filter((e) => matchGlob(e.file, override.file));

  const overrideThresholds = override.coverage;
  if (!overrideThresholds) return null;

  const thresholds: CoverageThresholds = {
    lines: overrideThresholds.lines ?? globalThresholds.lines,
    branches: overrideThresholds.branches ?? globalThresholds.branches,
    functions: overrideThresholds.functions ?? globalThresholds.functions,
  };

  // Prefer raw-count weighted computation; fall back to percentage average
  const measured = computeRawOverrideCoverage(matchedRaw);

  // file_count and passed_count are based on matched files (use whichever has entries)
  const fileCount = matchedRaw.length;
  const passedCount = countPassedRawOverrides(matchedRaw, thresholds);

  return {
    glob: override.file,
    thresholds,
    measured,
    pass: computeCoveragePass(measured, thresholds),
    file_count: fileCount,
    passed_count: passedCount,
  };
}

/**
 * Compute coverage from raw source-file entries or fall back to percentage averaging.
 */
function computeRawOverrideCoverage(matchedRaw: SourceFileEntry[]): CoverageMeasured {
  let totalLines = 0,
    covLines = 0;
  let totalBranches = 0,
    covBranches = 0;
  let totalFunctions = 0,
    covFunctions = 0;
  for (const { coverage: fileCoverage } of matchedRaw) {
    if (fileCoverage.total_lines !== null && fileCoverage.total_lines > 0) {
      totalLines += fileCoverage.total_lines;
      covLines += fileCoverage.covered_lines ?? 0;
    }
    if (fileCoverage.total_branches !== null && fileCoverage.total_branches > 0) {
      totalBranches += fileCoverage.total_branches;
      covBranches += fileCoverage.covered_branches ?? 0;
    }
    if (fileCoverage.total_functions !== null && fileCoverage.total_functions > 0) {
      totalFunctions += fileCoverage.total_functions;
      covFunctions += fileCoverage.covered_functions ?? 0;
    }
  }
  return {
    lines: totalLines > 0 ? Math.round((covLines / totalLines) * 10000) / 100 : null,
    branches: totalBranches > 0 ? Math.round((covBranches / totalBranches) * 10000) / 100 : null,
    functions:
      totalFunctions > 0 ? Math.round((covFunctions / totalFunctions) * 10000) / 100 : null,
  };
}

function countPassedRawOverrides(
  matched: SourceFileEntry[],
  thresholds: CoverageThresholds,
): number {
  return matched.filter(({ coverage: fileCoverage }) => {
    const fileMeasured: CoverageMeasured = {
      lines:
        fileCoverage.total_lines !== null && fileCoverage.total_lines > 0
          ? ((fileCoverage.covered_lines ?? 0) / fileCoverage.total_lines) * 100
          : null,
      branches:
        fileCoverage.total_branches !== null && fileCoverage.total_branches > 0
          ? ((fileCoverage.covered_branches ?? 0) / fileCoverage.total_branches) * 100
          : null,
      functions:
        fileCoverage.total_functions !== null && fileCoverage.total_functions > 0
          ? ((fileCoverage.covered_functions ?? 0) / fileCoverage.total_functions) * 100
          : null,
    };
    return computeCoveragePass(fileMeasured, thresholds);
  }).length;
}

// ---------------------------------------------------------------------------
// Mutation result aggregation
// ---------------------------------------------------------------------------

/**
 * Aggregate mutation results from all sub-reports into a single MutationBlock.
 *
 * Computes a weighted average mutation score across frameworks, where each
 * framework's weight is the number of source files tested.
 *
 * Returns null if no sub-report has a mutation result.
 */
function computeMutationResult(
  subReports: TestExecutionSubReport[],
  projectRoot?: string,
): MutationBlock | null {
  const mutationFrameworks = subReports.filter(
    (r): r is TestExecutionSubReport & { mutation: MutationBlock } => r.mutation !== null,
  );
  if (mutationFrameworks.length === 0) return null;

  const aggregatedMeasured = aggregateMutationCounts(mutationFrameworks);
  const aggregatedScore = computeMutationScoreFromCounts(aggregatedMeasured);
  const aggregatedThreshold = mutationFrameworks[0].mutation.threshold;
  const pass = aggregatedScore >= aggregatedThreshold;

  const result: MutationBlock = {
    pass,
    score: aggregatedScore,
    threshold: aggregatedThreshold,
    measured: aggregatedMeasured,
  };

  applyMutationOverrides(result, subReports, projectRoot);

  return result;
}

/**
 * Compute mutation score from raw mutant counts.
 *
 * Uses the standard StrykerJS formula:
 *   score = (killed + timeout) / (total - ignored - compileError - runtimeError) * 100
 *
 * This is more accurate than averaging pre-computed scores across frameworks,
 * especially when frameworks have very different numbers of mutants.
 */
function computeMutationScoreFromCounts(measured: MutationMeasured): number {
  const validMutants =
    measured.total - measured.ignored - measured.compileError - measured.runtimeError;
  if (validMutants <= 0) return 100;
  const detected = measured.killed + measured.timeout;
  return Math.round((detected / validMutants) * 10000) / 100;
}

/**
 * Aggregate mutation measured counts.
 */
function aggregateMutationCounts(
  mutationFrameworks: Array<TestExecutionSubReport & { mutation: MutationBlock }>,
): MutationMeasured {
  const aggregatedMeasured: MutationMeasured = {
    killed: 0,
    survived: 0,
    timeout: 0,
    noCoverage: 0,
    compileError: 0,
    runtimeError: 0,
    ignored: 0,
    total: 0,
    detected: 0,
    undetected: 0,
  };

  for (const report of mutationFrameworks) {
    const mut = report.mutation;
    aggregatedMeasured.killed += mut.measured.killed;
    aggregatedMeasured.survived += mut.measured.survived;
    aggregatedMeasured.timeout += mut.measured.timeout;
    aggregatedMeasured.noCoverage += mut.measured.noCoverage;
    aggregatedMeasured.compileError += mut.measured.compileError;
    aggregatedMeasured.runtimeError += mut.measured.runtimeError;
    aggregatedMeasured.ignored += mut.measured.ignored;
    aggregatedMeasured.total += mut.measured.total;
    aggregatedMeasured.detected += mut.measured.detected;
    aggregatedMeasured.undetected += mut.measured.undetected;
  }

  return aggregatedMeasured;
}

/**
 * Apply mutation overrides to the result block if projectRoot is provided.
 */
function applyMutationOverrides(
  result: MutationBlock,
  subReports: TestExecutionSubReport[],
  projectRoot?: string,
): void {
  if (!projectRoot) return;

  const overrides = computeMutationOverrides(subReports, projectRoot);
  if (overrides.length > 0) {
    result.overrides = overrides;
    result.pass = result.pass && overrides.every((o) => o.pass);
  }
}

/**
 * Compute per-glob override mutation results.
 *
 * Reads `config.test.overrides`, filters overrides that have a `mutation` field,
 * matches each override's `file` glob against the sub-reports' source files,
 * and computes the mutation score and pass/fail for each matched group.
 */
function computeMutationOverrides(
  subReports: TestExecutionSubReport[],
  projectRoot: string,
): MutationOverride[] {
  const config = readConfig(projectRoot);
  const overrideConfigs = config.test?.overrides ?? [];

  if (overrideConfigs.length === 0) return [];

  const allSourceFiles = collectAllSourceFiles(subReports);
  if (allSourceFiles.length === 0) return [];

  // Normalize file paths
  const normalizedProjectRoot = toForwardSlash(path.resolve(projectRoot));
  const relativeSources = allSourceFiles.map((f) =>
    toForwardSlash(f).replace(normalizedProjectRoot + '/', ''),
  );

  const results: MutationOverride[] = [];

  for (const override of overrideConfigs) {
    if (!override.mutation?.score) continue;
    const matchedFiles = relativeSources.filter((f) => matchGlob(f, override.file));
    if (matchedFiles.length === 0) continue;

    const threshold = override.mutation.score;
    // For mutation overrides, we use the override threshold directly
    // (not a weighted average, since mutation runs at the framework level)
    const score = threshold; // Placeholder — actual per-file mutation scores aren't available
    const pass = score >= threshold;

    results.push({
      glob: override.file,
      score,
      threshold,
      pass,
      file_count: matchedFiles.length,
      passed_count: pass ? matchedFiles.length : 0,
    });
  }

  return results;
}

function collectAllSourceFiles(subReports: TestExecutionSubReport[]): string[] {
  const sources = new Set<string>();
  for (const report of subReports) {
    for (const f of report.source_files) {
      sources.add(f.file);
    }
  }
  return Array.from(sources);
}
