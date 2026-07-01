// ---------------------------------------------------------------------------
// Test Runner
//
// Executes a single plan entry's test command.  Handles template placeholder
// substitution ({files}, {directory}, {project_root}), executes the command
// via child_process, parses output through the test parser dispatch, and
// reads coverage from the coverage file if available.
// ---------------------------------------------------------------------------

import { execSync } from 'child_process';
import * as path from 'path';

import type { PlanEntry } from '../commands/test-detect-frameworks';
import { parseCoverageFromFile, type ParsedCoverage } from './test-parser/coverage-parser';
import { parseTestOutput, type TestCase } from './test-parser/index';

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
function resolveCoveragePath(entry: PlanEntry, projectRoot: string): string | null {
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
  entry: PlanEntry,
  projectRoot: string,
  options: { files?: string[]; timeout?: number } = {},
): ExecutionResult {
  const startTime = Date.now();
  const testCmd = buildTestCommand(entry, projectRoot, options.files);
  if (!testCmd || testCmd.trim().length === 0) {
    return emptyResult(entry.framework, startTime, 'Empty test command');
  }

  const execCwd = resolveExecCwd(entry, projectRoot);
  console.log(`Executing test cmd: "${testCmd}"`);
  const { stdout, stderr, exitCode, execError } = runCommand(testCmd, execCwd, options.timeout);
  const durationMs = Date.now() - startTime;

  const parsed = parseTestOutput(stdout, stderr, entry.framework);
  const coveragePath = resolveCoveragePath(entry, projectRoot);
  const coverage = coveragePath ? parseCoverageFromFile(coveragePath, entry.coverage_format) : null;

  return {
    framework: entry.framework,
    exitCode,
    stdout,
    stderr,
    testCases: parsed.testCases,
    coverage,
    durationMs,
    testFiles: parsed.testFiles,
    sourceFiles:
      parsed.sourceFiles.length > 0 ? parsed.sourceFiles : deriveSourceFiles(parsed.testFiles),
    error: execError || parsed.error,
  };
}

function buildTestCommand(entry: PlanEntry, projectRoot: string, files?: string[]): string {
  return substitutePlaceholders(
    entry.framework === 'go' ? getGoTestCmd(entry, projectRoot) : (entry.test_cmd ?? ''),
    files ?? [],
    entry.directory,
    projectRoot,
  );
}

function resolveExecCwd(entry: PlanEntry, projectRoot: string): string {
  return entry.directory && entry.directory !== '.'
    ? path.resolve(projectRoot, entry.directory)
    : projectRoot;
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
    durationMs: Date.now() - startTime,
    testFiles: [],
    sourceFiles: [],
    error,
  };
}

/**
 * Build the go test command with the correct directory parameter.
 * For go, {directory} should be the relative path from the working directory
 * to the package under test, or "." if testing the current directory.
 */
function getGoTestCmd(entry: PlanEntry, _projectRoot: string): string {
  const baseCmd =
    entry.test_cmd ?? 'go test -json -coverprofile=coverage.out -covermode=atomic {directory}';

  // For go, if files are specified, derive directory from the first file
  // Otherwise use "." for the current directory
  const dir = entry.directory && entry.directory !== '.' ? entry.directory : '.';
  return baseCmd.replace(/\{directory\}/g, dir);
}
