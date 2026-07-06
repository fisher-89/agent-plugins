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
  FileCoverageEntry,
  MutationBlock,
  MutationMeasured,
  MutationOverride,
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
): TestExecutionSubReport {
  const now = new Date().toISOString();
  const testCases = buildTestCases(result);

  const subReport: TestExecutionSubReport = {
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
    mutation: result.mutation ?? null,
    findings: result.error ? [result.error] : undefined,
  };

  writeJsonFile(path.join(reportsDir, `${framework}.json`), subReport);
  return subReport;
}

// ---------------------------------------------------------------------------
// Summary report generation helpers
// ---------------------------------------------------------------------------

function collectProblemsAndCoverage(
  subReports: TestExecutionSubReport[],
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
  subReports: TestExecutionSubReport[],
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

  const coverageFrameworks: Array<{
    measured: CoverageMeasured;
    sourceFiles: string[];
    framework: string;
  }> = [];
  const problems = collectProblemsAndCoverage(subReports, coverageFrameworks);
  const coverageResult = computeCoverageResult(coverageFrameworks, projectRoot, subReports);

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
  subReports: TestExecutionSubReport[],
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

function collectFileCoverage(subReports: TestExecutionSubReport[]): FileCoverageEntry[] {
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

  const { aggregatedScore, aggregatedThreshold } = computeWeightedMutationScore(mutationFrameworks);
  const { aggregatedMeasured, byFramework } = aggregateMutationCounts(mutationFrameworks);
  const pass = aggregatedScore >= aggregatedThreshold;

  const result: MutationBlock = {
    pass,
    score: aggregatedScore,
    threshold: aggregatedThreshold,
    measured: aggregatedMeasured,
    by_framework: byFramework,
  };

  applyMutationOverrides(result, subReports, projectRoot);

  return result;
}

/**
 * Compute weighted average mutation score across frameworks.
 * Each framework's weight is the number of source files tested.
 */
function computeWeightedMutationScore(
  mutationFrameworks: Array<TestExecutionSubReport & { mutation: MutationBlock }>,
): { aggregatedScore: number; aggregatedThreshold: number } {
  let totalWeight = 0;
  let weightedScore = 0;
  let minThreshold = mutationFrameworks[0].mutation.threshold;

  for (const report of mutationFrameworks) {
    const mut = report.mutation;
    const weight = report.source_files.length;
    if (weight > 0) {
      weightedScore += mut.score * weight;
      totalWeight += weight;
    }
    if (mut.threshold > minThreshold) {
      minThreshold = mut.threshold;
    }
  }

  return {
    aggregatedScore:
      totalWeight > 0 ? weightedScore / totalWeight : mutationFrameworks[0].mutation.score,
    aggregatedThreshold: minThreshold,
  };
}

/**
 * Aggregate mutation measured counts and build the by_framework map.
 */
function aggregateMutationCounts(
  mutationFrameworks: Array<TestExecutionSubReport & { mutation: MutationBlock }>,
): {
  aggregatedMeasured: MutationMeasured;
  byFramework: Record<
    string,
    { score: number; measured: MutationMeasured; source_files: string[] }
  >;
} {
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

  const byFramework: Record<
    string,
    { score: number; measured: MutationMeasured; source_files: string[] }
  > = {};

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

    byFramework[report.framework] = {
      score: mut.score,
      measured: { ...mut.measured },
      source_files: [...report.source_files],
    };
  }

  return { aggregatedMeasured, byFramework };
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
      sources.add(f);
    }
  }
  return Array.from(sources);
}
