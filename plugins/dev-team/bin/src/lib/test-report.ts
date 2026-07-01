// ---------------------------------------------------------------------------
// Test Report Generator
//
// Generates per-framework sub-reports and a summary report for unit test
// execution.  Reports are written as JSON files.
//
// Sub-report:  reports/unit-test/<framework>.json
// Summary:     reports/unit-test-execution.json
// ---------------------------------------------------------------------------

import * as fs from 'fs';
import * as path from 'path';

import type {
  CoverageBlock,
  CoverageMeasured,
  CoverageOverride,
  CoverageThresholds,
  FileCoverageEntry,
  TestCaseResult,
  UnitTestSubReport,
  UnitTestSummaryReport,
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

function buildTestCases(result: ExecutionResult): TestCaseResult[] {
  return result.testCases.map((tc) => ({
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

function buildCoverageBlock(
  framework: string,
  result: ExecutionResult,
  projectRoot: string,
): CoverageBlock | null {
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
    by_framework: {
      [framework]: {
        measured: {
          lines: result.coverage.lines,
          branches: result.coverage.branches,
          functions: result.coverage.functions,
        },
        source_files: result.sourceFiles,
      },
    },
  };
}

function countByStatus(
  testCases: TestCaseResult[],
  status: 'passed' | 'failed' | 'skipped',
): number {
  return testCases.filter((t) => t.status === status).length;
}

function writeJsonFile(filePath: string, data: unknown): void {
  fs.mkdirSync(path.dirname(filePath), { recursive: true });
  fs.writeFileSync(filePath, JSON.stringify(data, null, 2), 'utf-8');
}

// ---------------------------------------------------------------------------
// Sub-report generation
// ---------------------------------------------------------------------------

export function generateSubReport(
  framework: string,
  result: ExecutionResult,
  projectRoot: string,
  reportsDir: string,
): UnitTestSubReport {
  const now = new Date().toISOString();
  const testCases = buildTestCases(result);

  const subReport: UnitTestSubReport = {
    framework,
    timestamp: now,
    exit_code: result.exitCode,
    duration_ms: result.durationMs,
    summary: {
      total: testCases.length,
      passed: countByStatus(testCases, 'passed'),
      failed: countByStatus(testCases, 'failed'),
      skipped: countByStatus(testCases, 'skipped'),
    },
    test_cases: testCases,
    test_files: result.testFiles,
    source_files: result.sourceFiles,
    file_coverage: result.coverage?.fileCoverage ?? null,
    coverage: buildCoverageBlock(framework, result, projectRoot),
    findings: result.error ? [result.error] : undefined,
  };

  writeJsonFile(path.join(reportsDir, `${framework}.json`), subReport);
  return subReport;
}

// ---------------------------------------------------------------------------
// Summary report generation helpers
// ---------------------------------------------------------------------------

function collectProblemsAndCoverage(
  subReports: UnitTestSubReport[],
  coverageFrameworks: Array<{
    measured: CoverageMeasured;
    sourceFiles: string[];
    framework: string;
  }>,
): Array<{
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
      const failCases = report.test_cases.filter((t) => t.status === 'failed');
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

    if (report.coverage) {
      coverageFrameworks.push({
        measured: report.coverage.measured,
        sourceFiles: report.source_files,
        framework: report.framework,
      });
    }
  }

  return problems;
}

function computeCoverageResult(
  coverageFrameworks: Array<{
    measured: CoverageMeasured;
    sourceFiles: string[];
    framework: string;
  }>,
  projectRoot: string,
  subReports: UnitTestSubReport[],
): CoverageBlock | null {
  if (coverageFrameworks.length === 0) return null;

  const thresholds = readCoverageThresholds(projectRoot);

  const measured: CoverageMeasured = {
    lines: weightedAverage(coverageFrameworks, 'lines'),
    branches: weightedAverage(coverageFrameworks, 'branches'),
    functions: weightedAverage(coverageFrameworks, 'functions'),
  };

  const byFramework: Record<string, { measured: CoverageMeasured; source_files: string[] }> = {};
  for (const fw of coverageFrameworks) {
    byFramework[fw.framework] = {
      measured: fw.measured,
      source_files: fw.sourceFiles,
    };
  }

  const overrides = computeOverrides(subReports, projectRoot);
  let pass = computeCoveragePass(measured, thresholds);
  if (overrides.length > 0) {
    const allOverridesPass = overrides.every((o) => o.pass);
    pass = pass && allOverridesPass;
  }

  const result: CoverageBlock = {
    pass,
    measured,
    thresholds,
    by_framework: byFramework,
  };
  if (overrides.length > 0) {
    result.overrides = overrides;
  }

  return result;
}

function determineConclusion(
  failed: number,
  coverageResult: CoverageBlock | null,
  hasExecutionError: boolean,
): 'pass' | 'fail' | 'error' {
  if (hasExecutionError) return 'error';
  if (failed > 0 || (coverageResult && !coverageResult.pass)) return 'fail';
  return 'pass';
}

// ---------------------------------------------------------------------------
// Summary report generation
// ---------------------------------------------------------------------------

export function generateSummaryReport(
  subReports: UnitTestSubReport[],
  projectRoot: string,
  reportsDir: string,
): UnitTestSummaryReport {
  const now = new Date().toISOString();
  const { total, passed, failed, skipped, totalDuration } = aggregateTotals(subReports);

  const coverageFrameworks: Array<{
    measured: CoverageMeasured;
    sourceFiles: string[];
    framework: string;
  }> = [];
  const problems = collectProblemsAndCoverage(subReports, coverageFrameworks);
  const coverageResult = computeCoverageResult(coverageFrameworks, projectRoot, subReports);

  if (coverageResult && !coverageResult.pass) {
    problems.push({
      framework: 'all',
      type: 'coverage_failure',
      message: formatCoverageFailure(coverageResult.measured, coverageResult.thresholds),
    });
  }

  const conclusion = determineConclusion(
    failed,
    coverageResult,
    problems.some((p) => p.type === 'execution_error'),
  );

  const summaryReport: UnitTestSummaryReport = {
    phase: '06-unit-test',
    command: 'dev-team unit-test',
    timestamp: now,
    duration_seconds: Math.round(totalDuration / 1000),
    total,
    passed,
    failed,
    skipped,
    conclusion,
    problems,
    coverage: coverageResult,
  };

  writeJsonFile(path.join(reportsDir, '..', 'unit-test-execution.json'), summaryReport);
  return summaryReport;
}

function aggregateTotals(subReports: UnitTestSubReport[]): {
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

function weightedAverage(
  frameworks: Array<{ measured: CoverageMeasured; sourceFiles: string[] }>,
  dimension: 'lines' | 'branches' | 'functions',
): number | null {
  let totalWeight = 0;
  let weightedSum = 0;
  let hasNonNull = false;

  for (const fw of frameworks) {
    const value = fw.measured[dimension];
    if (value === null) continue;

    const weight = fw.sourceFiles.length;
    if (weight <= 0) continue;

    hasNonNull = true;
    weightedSum += value * weight;
    totalWeight += weight;
  }

  if (!hasNonNull || totalWeight === 0) return null;
  return weightedSum / totalWeight;
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
 * file-level coverage entries, computes weighted averages, and checks against
 * thresholds.
 */
function computeOverrides(
  subReports: UnitTestSubReport[],
  projectRoot: string,
): CoverageOverride[] {
  const config = readConfig(projectRoot);
  const globalThresholds = readCoverageThresholds(projectRoot);
  const overrideConfigs = config.test?.overrides ?? [];

  if (overrideConfigs.length === 0) return [];

  const allFileCoverage = collectFileCoverage(subReports);
  if (allFileCoverage.length === 0) return [];

  // Normalize file paths to project-relative for glob matching
  const normalizedProjectRoot = toForwardSlash(path.resolve(projectRoot));
  const relativeFileCoverage = allFileCoverage.map((fc) => ({
    ...fc,
    file: toForwardSlash(fc.file).replace(normalizedProjectRoot + '/', ''),
  }));

  const results: CoverageOverride[] = [];

  for (const override of overrideConfigs) {
    if (!override.coverage) continue;
    const result = computeSingleOverride(override, relativeFileCoverage, globalThresholds);
    if (result) results.push(result);
  }

  return results;
}

function collectFileCoverage(subReports: UnitTestSubReport[]): FileCoverageEntry[] {
  const allFileCoverage: FileCoverageEntry[] = [];
  for (const report of subReports) {
    if (report.file_coverage) {
      allFileCoverage.push(...report.file_coverage);
    }
  }
  return allFileCoverage;
}

function computeSingleOverride(
  override: NonNullable<NonNullable<ReturnType<typeof readConfig>['test']>['overrides']>[number],
  allFileCoverage: FileCoverageEntry[],
  globalThresholds: CoverageThresholds,
): CoverageOverride | null {
  const matchedFiles = allFileCoverage.filter((fc) => matchGlob(fc.file, override.file));
  if (matchedFiles.length === 0) return null;

  const overrideThresholds = override.coverage;
  if (!overrideThresholds) return null;

  const measured: CoverageMeasured = {
    lines: avgFileCoverage(matchedFiles, 'lines'),
    branches: avgFileCoverage(matchedFiles, 'branches'),
    functions: avgFileCoverage(matchedFiles, 'functions'),
  };

  const thresholds: CoverageThresholds = {
    lines: overrideThresholds.lines ?? globalThresholds.lines,
    branches: overrideThresholds.branches ?? globalThresholds.branches,
    functions: overrideThresholds.functions ?? globalThresholds.functions,
  };

  return {
    glob: override.file,
    thresholds,
    measured,
    pass: computeCoveragePass(measured, thresholds),
    file_count: matchedFiles.length,
    passed_count: matchedFiles.filter((fc) => {
      const fileMeasured: CoverageMeasured = {
        lines: fc.lines,
        branches: fc.branches,
        functions: fc.functions,
      };
      return computeCoveragePass(fileMeasured, thresholds);
    }).length,
  };
}

/**
 * Compute the average of a coverage dimension across file coverage entries.
 * Null values are skipped. If all values are null, returns null.
 */
function avgFileCoverage(
  entries: FileCoverageEntry[],
  dimension: 'lines' | 'branches' | 'functions',
): number | null {
  let sum = 0;
  let count = 0;

  for (const entry of entries) {
    const value = entry[dimension];
    if (value === null) continue;
    sum += value;
    count++;
  }

  if (count === 0) return null;
  return sum / count;
}
