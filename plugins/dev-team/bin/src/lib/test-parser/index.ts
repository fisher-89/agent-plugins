// ---------------------------------------------------------------------------
// Test Parser — dispatch barrel
//
// parsePlanArtifacts is the execute-path entry that reads vertical artifacts
// from a plan report directory. Shared types live in ./types so sub-parsers
// can import them without a cycle through this module.
// parseTestOutput remains for text-segment internal reuse.
// ---------------------------------------------------------------------------

import * as fs from 'fs';
import * as path from 'path';

import { parseCoverageFromFile, type ParsedCoverage } from './coverage-parser';
import { parseGoOutput } from './go-parser';
import { parseJsOutput } from './js-parser';
import { parseTextOutput } from './text-parser';
import type { ParsedPlanArtifacts, ParsedTestResult } from './types';

export type { ParsedPlanArtifacts, TestCase } from './types';

// ---------------------------------------------------------------------------
// Vertical artifact file names (relative to reportDir)
// ---------------------------------------------------------------------------

const RESULTS_FILE: Record<string, string> = {
  jest: 'results.json',
  vitest: 'results.json',
  'vite-plus': 'results.json',
  bun: 'results.txt',
  go: 'results.ndjson',
  rust: 'results.txt',
  pytest: 'results.txt',
  'node-test': 'results.txt',
};

const COVERAGE_FILE: Record<string, { file: string; format: string }> = {
  jest: { file: 'coverage-summary.json', format: 'istanbul' },
  vitest: { file: 'coverage-summary.json', format: 'istanbul' },
  'vite-plus': { file: 'coverage-summary.json', format: 'istanbul' },
  bun: { file: 'lcov.info', format: 'lcov' },
  go: { file: 'func-summary.txt', format: 'go-cover' },
  rust: { file: 'coverage-summary.json', format: 'llvm-cov' },
  pytest: { file: 'coverage.json', format: 'coverage-py' },
  'node-test': { file: 'results.txt', format: 'node-test' },
};

function emptyArtifacts(error: string): ParsedPlanArtifacts {
  return {
    total: 0,
    passed: 0,
    failed: 0,
    skipped: 0,
    testCases: [],
    testFiles: [],
    sourceFiles: [],
    coverage: null,
    error,
  };
}

function readFileIfPresent(filePath: string): string | null {
  try {
    if (!fs.existsSync(filePath)) return null;
    const content = fs.readFileSync(filePath, 'utf-8');
    return content;
  } catch {
    return null;
  }
}

// ---------------------------------------------------------------------------
// parsePlanArtifacts — execute main path
// ---------------------------------------------------------------------------

/**
 * Parse vertical test/coverage artifacts from a plan report directory.
 *
 * Reads framework-specific result and coverage files under `reportDir`.
 * Does NOT parse dirty process stdout as JSON.
 */
export function parsePlanArtifacts(framework: string, reportDir: string): ParsedPlanArtifacts {
  const resultsName = RESULTS_FILE[framework];
  if (!resultsName) {
    return emptyArtifacts(`Unknown framework "${framework}" for plan artifact parsing`);
  }

  const resultsPath = path.join(reportDir, resultsName);
  const content = readFileIfPresent(resultsPath);

  if (content === null || content.trim().length === 0) {
    return emptyArtifacts(`Missing or empty results file: ${resultsName}`);
  }

  const parsed = parseTestOutput(content, '', framework);

  const covSpec = COVERAGE_FILE[framework];
  let coverage: ParsedCoverage | null = null;
  if (covSpec) {
    const covPath = path.join(reportDir, covSpec.file);
    coverage = parseCoverageFromFile(covPath, covSpec.format);
  }

  return {
    ...parsed,
    coverage,
  };
}

// ---------------------------------------------------------------------------
// parseTestOutput — shared dispatch for plan artifacts / text segments
// ---------------------------------------------------------------------------

/**
 * Parse test output (stdout + stderr) according to the framework type.
 *
 * Dispatch rules:
 * - "vitest" / "jest" / "vite-plus"  →  parseJsOutput
 * - "go"                              →  parseGoOutput
 * - everything else (rust, bun, node-test, pytest, ...)  →  parseTextOutput
 */
function parseTestOutput(stdout: string, stderr: string, framework: string): ParsedTestResult {
  if (framework === 'go') {
    return parseGoOutput(stdout);
  }

  if (framework === 'vitest' || framework === 'jest' || framework === 'vite-plus') {
    return parseJsOutput(stdout);
  }

  return parseTextOutput(stdout || stderr);
}
