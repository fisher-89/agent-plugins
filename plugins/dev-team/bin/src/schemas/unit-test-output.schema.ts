// ---------------------------------------------------------------------------
// Zod v4 schemas for unit-test sub-report and summary-report output
//
// These schemas define the JSON structure written by test-report.ts and read
// by the unit-test-executor agent.
// ---------------------------------------------------------------------------

import { z } from 'zod/v4';

// ---------------------------------------------------------------------------
// TestCaseResultSchema
// ---------------------------------------------------------------------------

const testCaseResultSchema = z.object({
  name: z.string().describe('Test case full name (hierarchical, e.g. "describe > it title")'),
  file: z.string().optional().describe('Test file path (relative to project root)'),
  duration_ms: z.number().optional().nullable().describe('Execution time in milliseconds'),
  status: z
    .enum(['passed', 'failed', 'skipped'])
    .describe('Test case status: passed, failed, or skipped'),
  line: z.number().optional().describe('Line number of the test (primarily for failed tests)'),
  errorType: z.string().optional().describe('Error type (only for failed tests)'),
  errorMessage: z.string().optional().describe('Error message (only for failed tests)'),
  stackTrace: z.string().optional().describe('Stack trace (only for failed tests)'),
});

// ---------------------------------------------------------------------------
// SummarySchema — nested inside sub-report
// ---------------------------------------------------------------------------

const summarySchema = z.object({
  total: z.number().int().min(0).describe('Total number of test cases'),
  passed: z.number().int().min(0).describe('Number of passed test cases'),
  failed: z.number().int().min(0).describe('Number of failed test cases'),
  skipped: z.number().int().min(0).describe('Number of skipped test cases'),
});

// ---------------------------------------------------------------------------
// CoverageBlockSchema — coverage conclusion inside sub-report and summary
// ---------------------------------------------------------------------------

const coverageMeasuredSchema = z.object({
  lines: z.number().min(0).max(100).nullable().describe('Line coverage percentage'),
  branches: z.number().min(0).max(100).nullable().describe('Branch coverage percentage'),
  functions: z.number().min(0).max(100).nullable().describe('Function coverage percentage'),
});

const coverageThresholdsSchema = z.object({
  lines: z.number().min(0).max(100).describe('Line coverage threshold'),
  branches: z.number().min(0).max(100).describe('Branch coverage threshold'),
  functions: z.number().min(0).max(100).describe('Function coverage threshold'),
});

const frameworkCoverageSchema = z.object({
  measured: coverageMeasuredSchema.describe('Measured coverage per dimension'),
  source_files: z
    .array(z.string())
    .describe("Source files contributing to this framework's weight"),
});

const coverageOverrideSchema = z.object({
  glob: z.string().describe('Glob pattern for files this override applies to'),
  thresholds: coverageThresholdsSchema.describe('Coverage thresholds for this override group'),
  measured: coverageMeasuredSchema.describe('Measured coverage for files matching this glob'),
  pass: z.boolean().describe('Whether this override group meets its thresholds'),
  file_count: z.number().int().min(0).describe('Number of files matching this override glob'),
  passed_count: z
    .number()
    .int()
    .min(0)
    .describe('Number of matching files that individually meet the override thresholds'),
});

const coverageBlockSchema = z.object({
  pass: z.boolean().describe('Whether coverage meets thresholds'),
  measured: coverageMeasuredSchema.describe('Aggregated measured coverage'),
  thresholds: coverageThresholdsSchema.describe('Coverage thresholds used for pass/fail'),
  by_framework: z
    .record(z.string(), frameworkCoverageSchema)
    .describe('Per-framework coverage breakdown'),
  overrides: z
    .array(coverageOverrideSchema)
    .optional()
    .describe('Per-glob override coverage results'),
});

// ---------------------------------------------------------------------------
// FileCoverageEntrySchema
// ---------------------------------------------------------------------------

const fileCoverageEntrySchema = z.object({
  file: z.string().describe('File path (relative to project root)'),
  lines: z.number().min(0).max(100).describe('Line coverage percentage'),
  branches: z.number().min(0).max(100).nullable().describe('Branch coverage percentage'),
  functions: z.number().min(0).max(100).nullable().describe('Function coverage percentage'),
});

// ---------------------------------------------------------------------------
// UnitTestSubReportSchema
// ---------------------------------------------------------------------------

const unitTestSubReportSchema = z.object({
  framework: z.string().describe('Test framework identifier'),
  timestamp: z.string().describe('ISO 8601 timestamp of report generation'),
  exit_code: z.number().int().describe('Command exit code'),
  duration_ms: z.number().min(0).describe('Execution duration in milliseconds'),
  summary: summarySchema.describe('Test case summary counts'),
  test_cases: z.array(testCaseResultSchema).describe('List of individual test case results'),
  test_files: z.array(z.string()).describe('Test file paths involved'),
  source_files: z.array(z.string()).describe('Corresponding source file paths'),
  file_coverage: z
    .array(fileCoverageEntrySchema)
    .nullable()
    .describe('Per-file coverage data (null when framework does not support file-level coverage)'),
  coverage: coverageBlockSchema
    .nullable()
    .describe('Coverage conclusion (null when coverage collection failed)'),
  findings: z.array(z.string()).optional().describe('Diagnostic findings from agent analysis'),
});

// ---------------------------------------------------------------------------
// ProblemSchema
// ---------------------------------------------------------------------------

const problemSchema = z.object({
  framework: z.string().describe('Framework where the problem occurred'),
  type: z
    .enum(['test_failure', 'coverage_failure', 'execution_error'])
    .describe('Problem category'),
  message: z.string().describe('Human-readable problem description'),
});

// ---------------------------------------------------------------------------
// UnitTestSummaryReportSchema
// ---------------------------------------------------------------------------

const unitTestSummaryReportSchema = z.object({
  phase: z.string().describe('Workflow phase, fixed to "06-unit-test"'),
  command: z.string().describe('CLI command description that generated the report'),
  timestamp: z.string().describe('ISO 8601 timestamp of report generation'),
  duration_seconds: z.number().min(0).describe('Total duration in seconds'),
  total: z.number().int().min(0).describe('Aggregated total test cases'),
  passed: z.number().int().min(0).describe('Aggregated passed test cases'),
  failed: z.number().int().min(0).describe('Aggregated failed test cases'),
  skipped: z.number().int().min(0).describe('Aggregated skipped test cases'),
  conclusion: z
    .enum(['pass', 'fail', 'error'])
    .describe('Overall conclusion: pass, fail, or error'),
  problems: z.array(problemSchema).describe('List of problems found'),
  coverage: coverageBlockSchema
    .nullable()
    .describe('Overall coverage (null when all frameworks failed to collect coverage)'),
  findings: z.array(z.string()).optional().describe('Diagnostic findings from agent analysis'),
});

// ---------------------------------------------------------------------------
// Inferred types
// ---------------------------------------------------------------------------

export type TestCaseResult = z.infer<typeof testCaseResultSchema>;
export type UnitTestSubReport = z.infer<typeof unitTestSubReportSchema>;
export type UnitTestSummaryReport = z.infer<typeof unitTestSummaryReportSchema>;
export type CoverageBlock = z.infer<typeof coverageBlockSchema>;
export type CoverageMeasured = z.infer<typeof coverageMeasuredSchema>;
export type CoverageThresholds = z.infer<typeof coverageThresholdsSchema>;
export type CoverageOverride = z.infer<typeof coverageOverrideSchema>;
export type FileCoverageEntry = z.infer<typeof fileCoverageEntrySchema>;
