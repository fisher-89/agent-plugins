// ---------------------------------------------------------------------------
// Zod v4 schemas for test-execution sub-report and summary-report output
//
// These schemas define the JSON structure written by test-report.ts and read
// by the test-execution-executor agent.
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
  overrides: z
    .array(coverageOverrideSchema)
    .optional()
    .describe('Per-glob override coverage results'),
});

// ---------------------------------------------------------------------------
// Mutation schemas
// ---------------------------------------------------------------------------

const mutationMeasuredSchema = z.object({
  killed: z.number().int().min(0).describe('Number of killed mutants'),
  survived: z.number().int().min(0).describe('Number of survived mutants'),
  timeout: z.number().int().min(0).describe('Number of timed out mutants'),
  noCoverage: z.number().int().min(0).describe('Number of mutants not covered by tests'),
  compileError: z.number().int().min(0).describe('Number of mutants that caused compile errors'),
  runtimeError: z.number().int().min(0).describe('Number of mutants that caused runtime errors'),
  ignored: z.number().int().min(0).describe('Number of ignored mutants'),
  total: z.number().int().min(0).describe('Total number of mutants'),
  detected: z.number().int().min(0).describe('Number of detected mutants (killed + timeout)'),
  undetected: z
    .number()
    .int()
    .min(0)
    .describe('Number of undetected mutants (survived + noCoverage)'),
});

const mutationOverrideSchema = z.object({
  glob: z.string().describe('Glob pattern for files this override applies to'),
  score: z.number().min(0).max(100).describe('Mutation score for this override group'),
  threshold: z
    .number()
    .min(0)
    .max(100)
    .describe('Mutation score threshold for this override group'),
  pass: z.boolean().describe('Whether this override group meets its mutation threshold'),
  file_count: z.number().int().min(0).describe('Number of files matching this override glob'),
  passed_count: z
    .number()
    .int()
    .min(0)
    .describe('Number of matching files that meet the mutation threshold'),
});

const mutationBlockSchema = z.object({
  pass: z.boolean().describe('Whether mutation score meets threshold'),
  score: z.number().min(0).max(100).describe('Aggregated mutation score'),
  threshold: z.number().min(0).max(100).describe('Mutation score threshold'),
  measured: mutationMeasuredSchema.describe('Aggregated mutation measurements'),
  overrides: z
    .array(mutationOverrideSchema)
    .optional()
    .describe('Per-glob override mutation results'),
});

// ---------------------------------------------------------------------------
// FileCoverageEntrySchema
// ---------------------------------------------------------------------------

const fileCoverageEntrySchema = z.object({
  lines: z.number().min(0).max(100).describe('Line coverage percentage'),
  branches: z.number().min(0).max(100).nullable().describe('Branch coverage percentage'),
  functions: z.number().min(0).max(100).nullable().describe('Function coverage percentage'),
  total_lines: z.number().int().min(0).nullable().describe('Total executable lines'),
  covered_lines: z.number().int().min(0).nullable().describe('Covered lines'),
  total_branches: z.number().int().min(0).nullable().describe('Total branches'),
  covered_branches: z.number().int().min(0).nullable().describe('Covered branches'),
  total_functions: z.number().int().min(0).nullable().describe('Total functions'),
  covered_functions: z.number().int().min(0).nullable().describe('Covered functions'),
});

// ---------------------------------------------------------------------------
// SourceFileEntrySchema — per-file raw coverage counts
// ---------------------------------------------------------------------------

const sourceFileEntrySchema = z.object({
  file: z.string().describe('Source file path (relative to project root)'),
  coverage: fileCoverageEntrySchema.optional().describe('源文件覆盖率数据'),
});

// ---------------------------------------------------------------------------
// TestExecutionSubReportSchema
// ---------------------------------------------------------------------------

const testExecutionSubReportSchema = z.object({
  framework: z.string().describe('Test framework identifier'),
  directory: z.string().describe('Plan working directory (relative to project root)'),
  timestamp: z.string().describe('ISO 8601 timestamp of report generation'),
  exit_code: z.number().int().describe('Command exit code'),
  duration_ms: z.number().min(0).describe('Execution duration in milliseconds'),
  summary: summarySchema.describe('Test case summary counts'),
  error_cases: z
    .array(testCaseResultSchema)
    .describe('List of test case results which is failed or timeout'),
  test_files: z.array(z.string()).describe('Test file paths involved'),
  source_files: z
    .array(sourceFileEntrySchema)
    .describe('Source files with per-file raw coverage counts'),
  coverage: coverageBlockSchema
    .nullable()
    .describe('Coverage conclusion (null when coverage collection failed)'),
  mutation: mutationBlockSchema
    .nullable()
    .describe('Mutation test conclusion (null when mutation testing is skipped or not supported)'),
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
// PlanIndexEntrySchema — path index in summary.plans[]
// ---------------------------------------------------------------------------

const planIndexEntrySchema = z.object({
  id: z.string().describe('Plan directory id (same as planId / directory name under reports/test)'),
  framework: z.string().describe('Test framework identifier'),
  directory: z.string().describe('Plan working directory (relative to project root)'),
  path: z
    .string()
    .describe(
      'Plan artifact directory relative to project root (POSIX, e.g. reports/test/<planId>)',
    ),
});

// ---------------------------------------------------------------------------
// TestExecutionSummaryReportSchema
// ---------------------------------------------------------------------------

const testExecutionSummaryReportSchema = z.object({
  phase: z.string().describe('Workflow phase, fixed to "test-execution"'),
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
  mutation: mutationBlockSchema
    .nullable()
    .describe(
      'Overall mutation test conclusion (null when mutation testing is skipped or not supported)',
    ),
  plans: z
    .array(planIndexEntrySchema)
    .describe(
      'Path index of attempted plans (no status fields; success/failure live in report.json)',
    ),
  findings: z.array(z.string()).optional().describe('Diagnostic findings from agent analysis'),
});

// ---------------------------------------------------------------------------
// Inferred types
// ---------------------------------------------------------------------------

export type TestCaseResult = z.infer<typeof testCaseResultSchema>;
export type TestExecutionSubReport = z.infer<typeof testExecutionSubReportSchema>;
export type TestExecutionSummaryReport = z.infer<typeof testExecutionSummaryReportSchema>;
export type PlanIndexEntry = z.infer<typeof planIndexEntrySchema>;
export type CoverageBlock = z.infer<typeof coverageBlockSchema>;
export type CoverageMeasured = z.infer<typeof coverageMeasuredSchema>;
export type CoverageThresholds = z.infer<typeof coverageThresholdsSchema>;
export type CoverageOverride = z.infer<typeof coverageOverrideSchema>;
export type FileCoverageEntry = z.infer<typeof fileCoverageEntrySchema>;
export type SourceFileEntry = z.infer<typeof sourceFileEntrySchema>;
export type MutationMeasured = z.infer<typeof mutationMeasuredSchema>;
export type MutationBlock = z.infer<typeof mutationBlockSchema>;
export type MutationOverride = z.infer<typeof mutationOverrideSchema>;
