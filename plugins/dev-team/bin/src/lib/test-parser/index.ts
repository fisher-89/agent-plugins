// ---------------------------------------------------------------------------
// Test Parser — shared types and dispatch
//
// ParsedTestResult is the intermediate representation produced by all
// sub-parsers (json, go, text).  parseTestOutput dispatches to the
// appropriate sub-parser based on the framework name.
// ---------------------------------------------------------------------------

export interface TestCase {
  name: string;
  file?: string;
  durationMs?: number;
  status: 'passed' | 'failed' | 'skipped';
  line?: number;
  errorType?: string;
  errorMessage?: string;
  stackTrace?: string;
}

export interface ParsedTestResult {
  total: number;
  passed: number;
  failed: number;
  skipped: number;
  testCases: TestCase[];
  testFiles: string[];
  sourceFiles: string[];
  error?: string;
}

// ---------------------------------------------------------------------------
// Sub-parser imports
// ---------------------------------------------------------------------------

import { parseGoOutput } from './go-parser';
import { parseJsonOutput } from './json-parser';
import { parseTextOutput } from './text-parser';

// ---------------------------------------------------------------------------
// Dispatch
// ---------------------------------------------------------------------------

/**
 * Parse test output (stdout + stderr) according to the framework type.
 *
 * Dispatch rules:
 * - "vitest" / "jest" / "vite-plus"  →  parseJsonOutput
 * - "go"                              →  parseGoOutput
 * - everything else (rust, bun, node-test, pytest, ...)  →  parseTextOutput
 *
 * @param stdout - The stdout from the test command
 * @param stderr - The stderr from the test command
 * @param framework - The test framework identifier
 * @returns ParsedTestResult
 */
export function parseTestOutput(
  stdout: string,
  stderr: string,
  framework: string,
): ParsedTestResult {
  if (framework === 'go') {
    return parseGoOutput(stdout);
  }

  if (framework === 'vitest' || framework === 'jest' || framework === 'vite-plus') {
    return parseJsonOutput(stdout);
  }

  return parseTextOutput(stdout || stderr);
}
