// ---------------------------------------------------------------------------
// StrykerJS Mutation Report Parser
//
// Reads a StrykerJS JSON report file and extracts mutation metrics.
// The report is expected at the path configured in the StrykerJS config
// (default: reports/mutation/mutation.json).
// ---------------------------------------------------------------------------

import * as fs from 'fs';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/**
 * Mutation report metrics extracted from a StrykerJS JSON report.
 */
export interface MutationReport {
  /** Overall mutation score (percentage 0-100) */
  score: number;
  /** Number of killed mutants */
  killed: number;
  /** Number of survived mutants */
  survived: number;
  /** Number of timed out mutants */
  timeout: number;
  /** Number of mutants not covered by tests */
  noCoverage: number;
  /** Number of mutants that caused compile errors */
  compileError: number;
  /** Number of mutants that caused runtime errors */
  runtimeError: number;
  /** Number of ignored mutants */
  ignored: number;
  /** Total number of mutants */
  total: number;
  /** Number of detected mutants (killed + timeout) */
  detected: number;
  /** Number of undetected mutants (survived + noCoverage) */
  undetected: number;
}

// ---------------------------------------------------------------------------
// StrykerJS JSON report shape (the subset we care about)
// ---------------------------------------------------------------------------

interface StrykerJsonReport {
  /** The 'final' schema version */
  schemaVersion?: string;
  /** Test framework configuration name (e.g. "vitest-runner") */
  testRunner?: string;
  thresholds?: {
    high?: number;
    low?: number;
    break?: number | null;
  };
  /** Array of file results — each has its own metrics */
  files?: Record<string, StrykerFileResult>;
  /** Aggregate metrics across all files */
  metrics?: StrykerMetrics;
}

interface StrykerFileResult {
  mutants: StrykerMutant[];
  language?: string;
  source?: string;
}

interface StrykerMutant {
  id: string;
  mutatorName: string;
  replacement: string;
  location?: { start: { line: number; column: number }; end: { line: number; column: number } };
  status: string;
  statusReason?: string;
  testsRan: string[];
  duration?: number;
}

interface StrykerMetrics {
  mutationScore: number;
  mutationScoreBasedOnCoveredCode: number;
  killed: number;
  survived: number;
  timeout: number;
  noCoverage: number;
  compileErrors: number;
  runtimeErrors: number;
  ignored: number;
  totalDetected: number;
  totalUndetected: number;
  totalMutants: number;
  totalCoveredMutants?: number;
  totalValidMutants?: number;
}

// ---------------------------------------------------------------------------
// Parser
// ---------------------------------------------------------------------------

/**
 * Parse a StrykerJS JSON mutation report file.
 *
 * Reads the report file, extracts the metrics section, and returns a
 * MutationReport object.  Returns null if the file does not exist or
 * cannot be parsed.
 *
 * @param reportPath - Absolute path to the StrykerJS JSON report file
 * @returns MutationReport or null if parsing fails
 */
export function parseMutationReport(reportPath: string): MutationReport | null {
  try {
    if (!fs.existsSync(reportPath)) {
      return null;
    }

    const raw = fs.readFileSync(reportPath, 'utf-8');
    const report: StrykerJsonReport = JSON.parse(raw);

    // StrykerJS can output metrics at the top level or nested
    const metrics = report.metrics ?? extractMetricsFromFiles(report);
    if (!metrics) {
      return null;
    }

    return {
      score: metrics.mutationScore ?? 0,
      killed: metrics.killed ?? 0,
      survived: metrics.survived ?? 0,
      timeout: metrics.timeout ?? 0,
      noCoverage: metrics.noCoverage ?? 0,
      compileError: metrics.compileErrors ?? 0,
      runtimeError: metrics.runtimeErrors ?? 0,
      ignored: metrics.ignored ?? 0,
      total: metrics.totalMutants ?? 0,
      detected: metrics.totalDetected ?? 0,
      undetected: metrics.totalUndetected ?? 0,
    };
  } catch {
    // File doesn't exist, isn't valid JSON, or doesn't have the expected shape
    return null;
  }
}

/**
 * Fallback: compute aggregate metrics from the files map when top-level
 * metrics are not available.
 */
function extractMetricsFromFiles(report: StrykerJsonReport): StrykerMetrics | null {
  if (!report.files || Object.keys(report.files).length === 0) {
    return null;
  }

  const c = {
    killed: 0,
    survived: 0,
    timeout: 0,
    noCoverage: 0,
    compileErrors: 0,
    runtimeErrors: 0,
    ignored: 0,
    totalMutants: 0,
    totalDetected: 0,
    totalUndetected: 0,
  };

  for (const fileResult of Object.values(report.files)) {
    for (const mutant of fileResult.mutants) {
      c.totalMutants++;
      tallyMutant(mutant.status, c);
    }
  }

  const validMutants = c.totalMutants - c.ignored - c.compileErrors - c.runtimeErrors;
  const mutationScore = validMutants > 0 ? (c.totalDetected / validMutants) * 100 : 100;

  return {
    mutationScore,
    mutationScoreBasedOnCoveredCode: 0,
    killed: c.killed,
    survived: c.survived,
    timeout: c.timeout,
    noCoverage: c.noCoverage,
    compileErrors: c.compileErrors,
    runtimeErrors: c.runtimeErrors,
    ignored: c.ignored,
    totalDetected: c.totalDetected,
    totalUndetected: c.totalUndetected,
    totalMutants: c.totalMutants,
  };
}

interface MutantCounters {
  killed: number;
  survived: number;
  timeout: number;
  noCoverage: number;
  compileErrors: number;
  runtimeErrors: number;
  ignored: number;
  totalMutants: number;
  totalDetected: number;
  totalUndetected: number;
}

function tallyMutant(status: string, c: MutantCounters): void {
  switch (status) {
    case 'Killed':
      c.killed++;
      c.totalDetected++;
      break;
    case 'Survived':
      c.survived++;
      c.totalUndetected++;
      break;
    case 'TimedOut':
      c.timeout++;
      c.totalDetected++;
      break;
    case 'NoCoverage':
      c.noCoverage++;
      c.totalUndetected++;
      break;
    case 'CompileError':
      c.compileErrors++;
      break;
    case 'RuntimeError':
      c.runtimeErrors++;
      break;
    case 'Ignored':
      c.ignored++;
      break;
  }
}
