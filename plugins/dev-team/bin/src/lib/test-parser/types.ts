// ---------------------------------------------------------------------------
// Test Parser — shared types
//
// Kept separate from index.ts so sub-parsers can import types without creating
// a circular dependency through the dispatch barrel.
// ---------------------------------------------------------------------------

import type { ParsedCoverage } from './coverage-parser';

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

export interface ParsedPlanArtifacts extends ParsedTestResult {
  coverage: ParsedCoverage | null;
}
