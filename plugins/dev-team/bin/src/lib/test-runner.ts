// ---------------------------------------------------------------------------
// Test Runner
//
// Executes a single plan entry's test command.  Handles template placeholder
// substitution ({files}, {directory}, {project_root}), executes the command
// via child_process, parses output through the test parser dispatch, and
// reads coverage from the coverage file if available.
//
// When mutation testing is configured and not disabled, runs StrykerJS after
// coverage collection and includes the mutation results in the execution
// result.
// ---------------------------------------------------------------------------

import { execSync } from 'child_process';
import * as fs from 'fs';
import * as path from 'path';

import type { MutationBlock, MutationMeasured, TestPlan } from '../schemas';
import { parseCoverageFromFile, type ParsedCoverage } from './test-parser/coverage-parser';
import { parseTestOutput, type TestCase } from './test-parser/index';
import { type MutationReport, parseMutationReport } from './test-parser/mutation-parser';
import { resolveStrykerConfig } from './test-parser/stryker-config';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface ExecutionResult {
  framework: string;
  exitCode: number;
  stdout: string;
  stderr: string;
  testCases: TestCase[];
  coverage: ParsedCoverage | null;
  mutation?: MutationBlock | null;
  durationMs: number;
  testFiles: string[];
  sourceFiles: string[];
  error?: string;
}

// ---------------------------------------------------------------------------
// Template substitution
// ---------------------------------------------------------------------------

/**
 * Replace template placeholders in a test command string.
 *
 * Supported placeholders:
 *   {files}         — space-separated list of file paths (relative to project root)
 *   {directory}     — the plan entry's working directory
 *   {project_root}  — absolute path to the project root
 *
 * @param cmd       - Command template containing placeholders
 * @param files     - Array of file paths to substitute for {files}
 * @param directory - Working directory value for {directory}
 * @param projectRoot - Absolute project root path for {project_root}
 * @returns The command string with placeholders replaced
 */
function substitutePlaceholders(
  cmd: string,
  files: string[],
  directory: string,
  projectRoot: string,
): string {
  let result = cmd;

  // {files} — space-separated file paths
  if (files && files.length > 0) {
    result = result.replace(/\{files\}/g, files.join(' '));
  } else {
    result = result.replace(/\{files\}/g, '');
  }

  // {directory}
  result = result.replace(/\{directory\}/g, directory);

  // {project_root}
  result = result.replace(/\{project_root\}/g, projectRoot);

  return result;
}

// ---------------------------------------------------------------------------
// Coverage file path resolution
// ---------------------------------------------------------------------------

/**
 * Resolve the coverage output file path for a given plan entry.
 *
 * The file is expected to be relative to the working directory (entry.directory).
 */
function resolveCoveragePath(entry: TestPlan, projectRoot: string): string | null {
  const coverageOutput = entry.coverage_output;
  if (!coverageOutput) return null;

  if (entry.directory && entry.directory !== '.') {
    return path.resolve(projectRoot, entry.directory, coverageOutput);
  }
  return path.resolve(projectRoot, coverageOutput);
}

// ---------------------------------------------------------------------------
// Source file derivation
// ---------------------------------------------------------------------------

/**
 * Derive source file paths from test file paths.
 *
 * Conventions:
 *   - .test.ts → .ts
 *   - .spec.ts → .ts
 *   - _test.go → .go
 *   - test_*.py → *.py
 */
function deriveSourceFiles(testFiles: string[]): string[] {
  const sourceSet = new Set<string>();

  for (const tf of testFiles) {
    const posix = tf.replace(/\\/g, '/');
    const src = posix
      .replace(/\.test\./g, '.')
      .replace(/\.spec\./g, '.')
      .replace(/_test\.go$/, '.go')
      .replace(/^test_(.+)\.py$/, '$1.py');

    if (src !== posix) {
      sourceSet.add(src);
    }
  }

  return Array.from(sourceSet).sort();
}

// ---------------------------------------------------------------------------
// Main execution function
// ---------------------------------------------------------------------------

/**
 * Extract stdout, stderr, exitCode, and error message from a caught execSync error.
 */
interface ExecError {
  stdout?: string | Buffer;
  stderr?: string | Buffer;
  status?: number;
  message?: string;
}

function isExecError(e: unknown): e is ExecError {
  return e !== null && typeof e === 'object';
}

function extractExecError(e: unknown): {
  stdout: string;
  stderr: string;
  exitCode: number;
  execError: string;
} {
  const err = isExecError(e) ? e : ({} as ExecError);
  const stdout = extractBuffer(err.stdout);
  const stderr = extractBuffer(err.stderr);
  const exitCode = err.status ?? -1;
  const execError = err.message ?? 'Command execution failed';
  return { stdout, stderr, exitCode, execError };
}

function extractBuffer(buf: string | Buffer | undefined): string {
  if (!buf) return '';
  return typeof buf === 'string' ? buf : buf.toString('utf-8');
}

// ---------------------------------------------------------------------------
// Main execution function
// ---------------------------------------------------------------------------

/**
 * Execute a single plan entry's test command.
 *
 * Steps:
 * 1. Substitute placeholders in the test command
 * 2. cd to the plan entry's directory
 * 3. Execute the shell command
 * 4. Parse stdout/stderr through the test parser dispatch
 * 5. Read coverage from the coverage output file
 * 6. Return ExecutionResult
 *
 * @param entry       - The plan entry to execute
 * @param projectRoot - Absolute project root path
 * @param options     - Optional overrides (files, timeout)
 * @returns ExecutionResult
 */
export function executePlanEntry(
  entry: TestPlan,
  projectRoot: string,
  options: { files?: string[]; timeout?: number; noMutation?: boolean } = {},
): ExecutionResult {
  const startTime = Date.now();
  const testCmd = buildTestCommand(entry, projectRoot, options.files);
  if (!testCmd || testCmd.trim().length === 0) {
    return emptyResult(entry.framework, startTime, 'Empty test command');
  }

  console.log(`Executing test cmd: "${testCmd}"`);
  const { stdout, stderr, exitCode, execError } = runCommand(testCmd, projectRoot, options.timeout);
  const durationMs = Date.now() - startTime;

  const parsed = parseTestOutput(stdout, stderr, entry.framework);
  const coveragePath = resolveCoveragePath(entry, projectRoot);
  const coverage = coveragePath ? parseCoverageFromFile(coveragePath, entry.coverage_format) : null;

  // Derive source files from test files
  const sourceFiles =
    parsed.sourceFiles.length > 0 ? parsed.sourceFiles : deriveSourceFiles(parsed.testFiles);

  // Mutation testing phase
  const mutation =
    parsed.failed === 0 ? runMutationPhase(entry, projectRoot, options, sourceFiles) : null;

  return {
    framework: entry.framework,
    exitCode,
    stdout,
    stderr,
    testCases: parsed.testCases,
    coverage,
    mutation,
    durationMs,
    testFiles: parsed.testFiles,
    sourceFiles,
    error: execError || parsed.error,
  };
}

/**
 * Run the mutation testing phase using StrykerJS.
 *
 * Checks whether mutation testing is applicable (mutation_framework is set
 * and noMutation is not true), resolves the StrykerJS configuration, executes
 * StrykerJS, parses the report, and cleans up temporary files.
 *
 * Returns a MutationBlock on success, or null if mutation testing is skipped
 * or fails (errors are silently caught to avoid breaking the test flow).
 */
function runMutationPhase(
  entry: TestPlan,
  projectRoot: string,
  options: { noMutation?: boolean },
  sourceFiles: string[],
): MutationBlock | null {
  // Skip mutation if not supported or explicitly disabled
  if (!entry.mutation_framework || options.noMutation) {
    return null;
  }

  const absoluteDirectory = path.resolve(projectRoot, entry.directory);
  try {
    const { configPath, tempDirPath } = resolveStrykerConfig(
      absoluteDirectory,
      sourceFiles,
      entry.framework,
    );

    // Normalize configPath to forward slashes to avoid backslash escape issues in shell
    const normalizedConfigPath = configPath.replace(/\\/g, '/');
    const strykerCmd = `npx stryker run "${normalizedConfigPath}"`;
    console.log(
      `Running StrykerJS mutation testing (cmd: ${strykerCmd}, cwd: ${absoluteDirectory})...`,
    );
    runCommand(strykerCmd, absoluteDirectory, 600000);

    const mutationBlock = buildMutationBlockFromReport(entry, absoluteDirectory, sourceFiles);

    cleanupMutationArtifacts(absoluteDirectory, configPath, tempDirPath);

    if (!mutationBlock) {
      console.log('  Mutation report not found or invalid — skipping mutation result');
      return null;
    }

    console.log(
      `  Mutation score: ${mutationBlock.score.toFixed(1)}% (threshold: ${mutationBlock.threshold}%)`,
    );

    return mutationBlock;
  } catch (e) {
    console.log(`  Mutation testing skipped: ${e instanceof Error ? e.message : 'Unknown error'}`);
    return null;
  }
}

/**
 * Parse the StrykerJS mutation report and build a MutationBlock.
 * Returns null if the report cannot be parsed.
 */
function buildMutationBlockFromReport(
  entry: TestPlan,
  rootPath: string,
  sourceFiles: string[],
): MutationBlock | null {
  const reportPath = path.resolve(rootPath, 'reports', 'mutation', 'mutation.json');
  const mutationReport = parseMutationReport(reportPath);
  if (!mutationReport) return null;

  const threshold = entry.mutation_score ?? 80;
  const pass = mutationReport.score >= threshold;

  return {
    pass,
    score: mutationReport.score,
    threshold,
    measured: extractMutationMeasured(mutationReport),
    by_framework: {
      [entry.framework]: {
        score: mutationReport.score,
        measured: extractMutationMeasured(mutationReport),
        source_files: sourceFiles,
      },
    },
  };
}

/**
 * Clean up temporary StrykerJS artifacts.
 *
 * Removes the reports/mutation/ directory and the temporary config file
 */
function cleanupMutationArtifacts(rootPath: string, configPath: string, tempDirPath: string): void {
  // Remove reports/mutation/ directory
  const mutationReportDir = path.resolve(rootPath, 'reports', 'mutation');
  try {
    if (fs.existsSync(mutationReportDir)) {
      fs.rmSync(mutationReportDir, { recursive: true, force: true });
    }
  } catch {
    // Best-effort cleanup
  }

  // Remove temporary config file
  try {
    if (fs.existsSync(configPath)) {
      fs.rmSync(configPath);
    }
  } catch {
    // Best-effort cleanup
  }

  try {
    if (fs.existsSync(tempDirPath)) {
      fs.rmSync(tempDirPath);
    }
  } catch {
    // Best-effort cleanup
  }
}

/**
 * Extract MutationMeasured fields from a MutationReport.
 * Shared helper to avoid duplicating the field mapping.
 */
function extractMutationMeasured(report: MutationReport): MutationMeasured {
  return {
    killed: report.killed,
    survived: report.survived,
    timeout: report.timeout,
    noCoverage: report.noCoverage,
    compileError: report.compileError,
    runtimeError: report.runtimeError,
    ignored: report.ignored,
    total: report.total,
    detected: report.detected,
    undetected: report.undetected,
  };
}

function buildTestCommand(entry: TestPlan, projectRoot: string, files?: string[]): string {
  return substitutePlaceholders(entry.script, files ?? [], entry.directory, projectRoot);
}

function resolveShell(): string | undefined {
  // On Windows, test scripts use Unix shell syntax (rm -rf, \n separation, etc.)
  // which requires a POSIX shell (e.g. Git Bash) rather than cmd.exe.
  // process.env.SHELL is set by Git Bash for Windows.
  if (process.platform === 'win32') {
    return process.env.SHELL || 'bash';
  }
  return undefined; // Use default shell on Unix
}

function runCommand(
  cmd: string,
  cwd: string,
  timeout?: number,
): { stdout: string; stderr: string; exitCode: number; execError?: string } {
  try {
    const stdout = execSync(cmd, {
      cwd,
      encoding: 'utf-8',
      timeout: timeout ?? 60000,
      maxBuffer: 10 * 1024 * 1024,
      shell: resolveShell(),
    });
    return { stdout, stderr: '', exitCode: 0 };
  } catch (e: unknown) {
    const result = extractExecError(e);
    return {
      stdout: result.stdout,
      stderr: result.stderr,
      exitCode: result.exitCode,
      execError: result.execError,
    };
  }
}

function emptyResult(framework: string, startTime: number, error: string): ExecutionResult {
  return {
    framework,
    exitCode: -1,
    stdout: '',
    stderr: '',
    testCases: [],
    coverage: null,
    mutation: null,
    durationMs: Date.now() - startTime,
    testFiles: [],
    sourceFiles: [],
    error,
  };
}
