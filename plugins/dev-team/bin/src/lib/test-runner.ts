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

import {
  TEST_MUTATION_SCORE_DEFAULT,
  type MutationBlock,
  type MutationMeasured,
  type TestPlan,
} from '../schemas';
import { readConfig } from './config';
import { isFileExcluded } from './test-exclude';
import { getFrameworkConfig } from './test-framework';
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
// Mutation scope restriction
// ---------------------------------------------------------------------------

/**
 * When --mutation-diff-only is active, restrict the mutation file set to
 * only those files appearing in the git diff.
 */
function restrictMutationScope(sourceFiles: string[], mutationDiffFiles?: string[]): string[] {
  if (mutationDiffFiles === undefined) return sourceFiles;
  const diffSet = new Set(mutationDiffFiles);
  return sourceFiles.filter((f) => diffSet.has(f));
}

// ---------------------------------------------------------------------------
// Main execution function
// ---------------------------------------------------------------------------

/**
 * Execute a single plan entry's test command.
 *
 * @param entry       - The plan entry to execute
 * @param projectRoot - Absolute project root path
 * @param options     - Optional overrides (files, timeout, mutationDiffFiles)
 * @returns ExecutionResult
 */
export function executePlanEntry(
  entry: TestPlan,
  projectRoot: string,
  options: {
    files?: string[];
    timeout?: number;
    noMutation?: boolean;
    /** Git diff file list used to restrict mutation scope (--mutation-diff-only) */
    mutationDiffFiles?: string[];
  } = {},
): ExecutionResult {
  const startTime = Date.now();
  const testCmd = buildTestCommand(entry, projectRoot, options.files);
  if (!testCmd || testCmd.trim().length === 0) {
    return emptyResult(entry.framework, startTime, 'Empty test command');
  }

  console.log(`Executing test cmd: "${testCmd}"`);
  const { stdout, stderr, exitCode, execError } = runCommand(testCmd, projectRoot, options.timeout);

  const { failed, error, sourceFiles, testFiles, testCases } = parseTestOutput(
    stdout,
    stderr,
    entry.framework,
  );
  const coveragePath = resolveCoveragePath(entry, projectRoot);
  const coverage = coveragePath ? parseCoverageFromFile(coveragePath, entry.coverage_format) : null;

  // Mutation testing phase
  const mutationFiles = restrictMutationScope(sourceFiles, options.mutationDiffFiles);
  const mutation =
    failed === 0 ? runMutationPhase(entry, projectRoot, options, mutationFiles) : null;

  const durationMs = Date.now() - startTime;

  return {
    framework: entry.framework,
    exitCode,
    testCases,
    coverage,
    mutation,
    durationMs,
    testFiles,
    sourceFiles,
    error: execError || error,
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

  // Filter out source files that match suite-scoped tests[].excludes
  const config = readConfig(projectRoot);
  const filteredSources = sourceFiles.filter((f) => !isFileExcluded(f, config));

  // If all source files are excluded, skip mutation testing entirely
  if (filteredSources.length === 0) {
    return null;
  }

  // Mutation cwd is absCwd (plan.directory); rewrite mutate paths relative to it
  const absoluteDirectory = path.resolve(projectRoot, entry.directory);
  const sourcesRelativeToCwd = filteredSources.map((f) => {
    const abs = path.isAbsolute(f) ? path.resolve(f) : path.resolve(projectRoot, f);
    return path.relative(absoluteDirectory, abs).replace(/\\/g, '/');
  });

  try {
    return executeStrykerMutation(entry, absoluteDirectory, sourcesRelativeToCwd);
  } catch (e) {
    console.log(`  Mutation testing skipped: ${e instanceof Error ? e.message : 'Unknown error'}`);
    return null;
  }
}

/**
 * Execute the StrykerJS mutation test run, parse the report, and clean up.
 * Returns a MutationBlock on success, or null if the report is missing.
 */
function executeStrykerMutation(
  entry: TestPlan,
  absoluteDirectory: string,
  filteredSources: string[],
): MutationBlock | null {
  const { configPath, tempDirPath } = resolveStrykerConfig(
    absoluteDirectory,
    filteredSources,
    entry.framework,
  );

  const strykerCmd = genStrykerCommand(entry, configPath.replace(/\\/g, '/'));
  console.log(
    `Running StrykerJS mutation testing (cmd: ${strykerCmd}, cwd: ${absoluteDirectory})...`,
  );
  const strykerStart = Date.now();
  const cmdResult = runCommand(strykerCmd, absoluteDirectory, 1200000);
  const strykerDuration = (Date.now() - strykerStart) / 1000;

  if (cmdResult.exitCode !== 0) {
    logCommandFailure(cmdResult, strykerDuration);
    return null;
  }

  const mutationBlock = buildMutationBlockFromReport(entry, absoluteDirectory);
  cleanupMutationArtifacts(configPath, tempDirPath);

  if (!mutationBlock) {
    logMissingReport(cmdResult);
    return null;
  }

  console.log(
    `  Mutation score: ${mutationBlock.score.toFixed(1)}% (threshold: ${mutationBlock.threshold}%, took ${strykerDuration.toFixed(1)}s)`,
  );

  return mutationBlock;
}

function genStrykerCommand(entry: TestPlan, configPath: string): string {
  const frameworkConfig = getFrameworkConfig(entry.framework);
  const isWinCmd = process.platform === 'win32' && !process.env.SHELL;
  const mutationTemplate = isWinCmd
    ? (frameworkConfig.cmd.mutation_execution ?? frameworkConfig.shell.mutation_execution)
    : frameworkConfig.shell.mutation_execution;
  return mutationTemplate
    ? mutationTemplate.replace(/\{config\}/g, configPath)
    : `npx stryker run "${configPath}"`;
}

/**
 * Parse the StrykerJS mutation report and build a MutationBlock.
 * Returns null if the report cannot be parsed.
 */
function buildMutationBlockFromReport(entry: TestPlan, rootPath: string): MutationBlock | null {
  const reportPath = path.resolve(rootPath, 'reports', 'mutation', 'mutation.json');
  const mutationReport = parseMutationReport(reportPath);
  if (!mutationReport) return null;

  const threshold = entry.mutation_score ?? TEST_MUTATION_SCORE_DEFAULT;
  const pass = mutationReport.score >= threshold;

  return {
    pass,
    score: mutationReport.score,
    threshold,
    measured: extractMutationMeasured(mutationReport),
  };
}

/**
 * Clean up temporary StrykerJS artifacts.
 *
 * Removes the reports/mutation/ directory and the temporary config file
 */
function cleanupMutationArtifacts(configPath: string, tempDirPath: string): void {
  // Remove temporary config file
  try {
    if (fs.existsSync(configPath)) {
      fs.rmSync(configPath);
    }
  } catch {
    // Best-effort cleanup
  }

  // Remove temporary snapshot files
  try {
    if (fs.existsSync(tempDirPath)) {
      fs.rmSync(tempDirPath, { recursive: true });
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

/**
 * Log diagnostic info when a mutation command exits with non-zero code.
 */
function logCommandFailure(
  result: { exitCode: number; stderr: string; execError?: string },
  durationS: number,
): void {
  console.log(`StrykerJS exited with code ${result.exitCode} (took ${durationS.toFixed(1)}s)`);
  if (result.stderr) {
    console.log(`StrykerJS stderr: ${result.stderr.slice(0, 500)}`);
  }
  if (result.execError) {
    console.log(`StrykerJS error: ${result.execError}`);
  }
}

/**
 * Log a message when the mutation report is missing.
 * Only emits when the command appeared to succeed (exitCode 0),
 * otherwise the failure was already logged by logCommandFailure.
 */
function logMissingReport(result: { exitCode: number }): void {
  if (result.exitCode === 0) {
    console.log('  Mutation report not found or invalid — skipping mutation result');
  }
}

function buildTestCommand(entry: TestPlan, projectRoot: string, files?: string[]): string {
  // Select platform-appropriate script:
  //   - Windows without SHELL env (no Git Bash) → entry.script.cmd (cmd.exe)
  //   - Otherwise → entry.script.shell (POSIX shell / bash)
  const isWinCmd = process.platform === 'win32' && !process.env.SHELL;
  const script = isWinCmd ? entry.script.cmd : entry.script.shell;
  return substitutePlaceholders(script, files ?? [], entry.directory, projectRoot);
}

function resolveShell(): string | undefined {
  if (process.platform === 'win32') {
    // Prefer SHELL (Git Bash) if available; fall back to COMSPEC or cmd.exe
    return process.env.SHELL || process.env.COMSPEC || 'cmd.exe';
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
      stdio: 'pipe',
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
    testCases: [],
    coverage: null,
    mutation: null,
    durationMs: Date.now() - startTime,
    testFiles: [],
    sourceFiles: [],
    error,
  };
}
