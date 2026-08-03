// ---------------------------------------------------------------------------
// Test Runner
//
// Executes a single plan entry's test command.  Resolves plan reportDir,
// prepares placeholders / temp configs, runs the command with cwd=absCwd,
// parses vertical artifacts from the plan directory, and optionally runs
// mutation testing (Stryker → reportDir/mutation.json).
// ---------------------------------------------------------------------------

import { execSync } from 'child_process';
import * as crypto from 'crypto';
import * as fs from 'fs';
import * as path from 'path';

import {
  TEST_MUTATION_SCORE_DEFAULT,
  type MutationBlock,
  type MutationMeasured,
  type OpenSpecConfig,
  type TestPlan,
} from '../schemas';
import { readConfig } from './config';
import { toForwardSlash } from './glob';
import { isFileExcluded } from './test-exclude';
import { getFrameworkConfig } from './test-framework';
import { type ParsedCoverage } from './test-parser/coverage-parser';
import { parsePlanArtifacts, type TestCase } from './test-parser/index';
import { type MutationReport, parseMutationReport } from './test-parser/mutation-parser';
import { resolveStrykerConfig } from './test-parser/stryker-config';
import { derivePlanId, findSuite, pathFilterFromPlan } from './test-plan';

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
  planId: string;
  reportDir: string;
  resultsFile?: string;
}

interface PlanPlaceholders {
  report_dir: string;
  results_file: string;
  coverage_file: string;
  coverprofile_file?: string;
  mutation_file?: string;
}

interface PreparePlanArtifactsInput {
  framework: string;
  absCwd: string;
  reportDir: string;
  userConfigPath: string | null;
  projectRoot: string;
}

interface PreparePlanArtifactsResult {
  configArgs: string;
  redirectStdoutToResults: boolean;
  placeholders: PlanPlaceholders;
  tempPaths: string[];
  env?: Record<string, string>;
}

/** Frameworks that write test results via native CLI outputFile (no shell `>`). */
const NATIVE_OUTPUT_FILE_FRAMEWORKS = new Set(['jest', 'vitest', 'vite-plus']);

/** Vertical results file names under reportDir. */
const RESULTS_FILE_BY_FRAMEWORK: Record<string, string> = {
  jest: 'results.json',
  vitest: 'results.json',
  'vite-plus': 'results.json',
  bun: 'results.txt',
  go: 'results.ndjson',
  rust: 'results.txt',
  pytest: 'results.txt',
  'node-test': 'results.txt',
};

// ---------------------------------------------------------------------------
// Path helpers
// ---------------------------------------------------------------------------

/** Absolute path with POSIX separators (avoids rootDir/cwd ambiguity in CLIs). */
function toPosixAbsolute(p: string): string {
  return toForwardSlash(path.resolve(p));
}

/**
 * Resolve the suite's framework config path (absolute) for this plan entry.
 */
function resolveUserConfigPath(entry: TestPlan, projectRoot: string): string | null {
  try {
    const config = readConfig(projectRoot);
    const match = findSuite(config, entry.framework, entry.root);
    if (!match?.config) return null;
    return path.resolve(projectRoot, match.root, match.config);
  } catch {
    return null;
  }
}

/**
 * Clear all contents of a directory (recreate empty). Does not touch siblings.
 */
function clearDirectory(dir: string): void {
  if (fs.existsSync(dir)) {
    fs.rmSync(dir, { recursive: true, force: true });
  }
  fs.mkdirSync(dir, { recursive: true });
}

function cleanupTempPaths(tempPaths: string[]): void {
  for (const p of tempPaths) {
    try {
      if (fs.existsSync(p)) {
        fs.rmSync(p, { recursive: true, force: true });
      }
    } catch {
      // Best-effort cleanup
    }
  }
}

// ---------------------------------------------------------------------------
// preparePlanArtifacts
// ---------------------------------------------------------------------------

/**
 * Resolve placeholders, config args, redirect policy, and temp files for a plan.
 *
 * Path placeholders and `--config` args are absolute (POSIX) so CLIs that
 * resolve relative paths against config root (jest/vitest) — or against cwd —
 * land artifacts in reportDir without ambiguity. Templates quote paths for spaces.
 */
function preparePlanArtifacts(input: PreparePlanArtifactsInput): PreparePlanArtifactsResult {
  const { framework, absCwd, reportDir, userConfigPath } = input;
  const frameworkConfig = getFrameworkConfig(framework);
  const tempPaths: string[] = [];

  const resultsName = RESULTS_FILE_BY_FRAMEWORK[framework] ?? 'results.txt';
  const coverageName = frameworkConfig.coverage_output;

  const absResults = path.join(reportDir, resultsName);
  const absCoverage = path.join(reportDir, coverageName);
  const absCoverprofile = path.join(reportDir, 'coverage.out');
  const absMutation = path.join(reportDir, 'mutation.json');

  const placeholders: PlanPlaceholders = {
    report_dir: toPosixAbsolute(reportDir),
    results_file: toPosixAbsolute(absResults),
    coverage_file: toPosixAbsolute(absCoverage),
    coverprofile_file: toPosixAbsolute(absCoverprofile),
    mutation_file: toPosixAbsolute(absMutation),
  };

  const redirectStdoutToResults = !NATIVE_OUTPUT_FILE_FRAMEWORKS.has(framework);

  let configArgs = '';
  if (framework === 'bun') {
    const bunfigPath = writeTempBunfig(absCwd, placeholders.report_dir, userConfigPath);
    tempPaths.push(bunfigPath);
    configArgs = `${frameworkConfig.config_flag} "${toPosixAbsolute(bunfigPath)}"`;
  } else if (userConfigPath && frameworkConfig.config_flag) {
    configArgs = `${frameworkConfig.config_flag} "${toPosixAbsolute(userConfigPath)}"`;
  }

  return {
    configArgs,
    redirectStdoutToResults,
    placeholders,
    tempPaths,
  };
}

/**
 * Write a temporary bunfig that overlays coverageDir / coverageReporter=lcov.
 * Reads the user bunfig (if any) and appends an overlay — never modifies it.
 */
function writeTempBunfig(
  absCwd: string,
  coverageDir: string,
  _userConfigPath: string | null,
): string {
  const randomSuffix = crypto.randomBytes(4).toString('hex');
  const tempPath = path.join(absCwd, `bunfig.dev-team-${randomSuffix}.toml`);

  let base = '';
  const defaultBunfig = path.join(absCwd, 'bunfig.toml');
  if (fs.existsSync(defaultBunfig)) {
    try {
      base = fs.readFileSync(defaultBunfig, 'utf-8');
    } catch {
      base = '';
    }
  }

  const overlay = [
    '',
    '# Generated by dev-team test-execution — temporary overlay; do not commit',
    '[test]',
    `coverageDir = ${JSON.stringify(coverageDir)}`,
    'coverageReporter = ["lcov"]',
    '',
  ].join('\n');

  fs.writeFileSync(tempPath, base + overlay, 'utf-8');
  return tempPath;
}

// ---------------------------------------------------------------------------
// Template substitution
// ---------------------------------------------------------------------------

/**
 * Expand {config_args} and path placeholders, then {files}/{directory}/{project_root}.
 *
 * `pathFilter` is suite root relative to cwd — used when `{files}` is empty and for go `{directory}`.
 */
function expandCommandTemplate(
  cmd: string,
  prepared: PreparePlanArtifactsResult,
  files: string[],
  projectRoot: string,
  pathFilter: string,
): string {
  let result = cmd;

  result = result.replace(/\{project_root\}/g, projectRoot);

  if (prepared.configArgs) {
    result = result.replace(/\{config_args\}/g, prepared.configArgs);
  } else {
    result = result.replace(/\s*\{config_args\}/g, '').replace(/\{config_args\}/g, '');
  }

  const ph = prepared.placeholders;
  result = result.replace(/\{report_dir\}/g, ph.report_dir);
  result = result.replace(/\{results_file\}/g, ph.results_file);
  result = result.replace(/\{coverage_file\}/g, ph.coverage_file);
  if (ph.coverprofile_file) {
    result = result.replace(/\{coverprofile_file\}/g, ph.coverprofile_file);
  }
  if (ph.mutation_file) {
    result = result.replace(/\{mutation_file\}/g, ph.mutation_file);
  }

  if (files && files.length > 0) {
    result = result.replace(/\{files\}/g, files.join(' '));
  } else {
    const filesFallback = pathFilter && pathFilter !== '.' ? pathFilter : '';
    result = result.replace(/\{files\}/g, filesFallback);
  }

  const directoryArg = pathFilter && pathFilter !== '.' ? `./${pathFilter}/...` : './...';
  result = result.replace(/\{directory\}/g, directoryArg);

  return result;
}

/**
 * Append shell/cmd redirect to the test-results segment when needed.
 */
function applyResultsRedirect(
  cmd: string,
  resultsFile: string,
  framework: string,
  isWinCmd: boolean,
): string {
  const redirect = `> "${resultsFile}"`;

  switch (framework) {
    case 'go': {
      // Redirect only the go test segment (before first chain separator)
      if (isWinCmd) {
        return cmd.replace(/^(go test\b.*?)(\s*&)/, `$1 ${redirect}$2`);
      }
      return cmd.replace(/^(go test\b.*?)(\s*;)/, `$1 ${redirect}$2`);
    }
    case 'rust': {
      // Insert after `cargo test` before the chain separator
      return cmd.replace(/^(cargo test\b)/, `$1 ${redirect}`);
    }
    case 'pytest': {
      if (isWinCmd) {
        // first pytest segment ends at ` && `
        return cmd.replace(/^(pytest\b.*?)(\s+&&\s+)/, `$1 ${redirect}$2`);
      }
      return cmd.replace(/^(pytest\b.*?)(\s*;\s*)/, `$1 ${redirect}$2`);
    }
    default:
      // bun, node-test, and any other single-segment redirect framework
      return `${cmd} ${redirect}`;
  }
}

// ---------------------------------------------------------------------------
// Exec helpers
// ---------------------------------------------------------------------------

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

function resolveShell(): string | undefined {
  if (process.platform === 'win32') {
    return process.env.SHELL || process.env.COMSPEC || 'cmd.exe';
  }
  return undefined;
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

// ---------------------------------------------------------------------------
// Mutation scope restriction
// ---------------------------------------------------------------------------

function restrictMutationScope(sourceFiles: string[], mutationDiffFiles?: string[]): string[] {
  if (mutationDiffFiles === undefined) return sourceFiles;
  const diffSet = new Set(mutationDiffFiles);
  return sourceFiles.filter((f) => diffSet.has(f));
}

function resolveTestCommand(
  entry: TestPlan,
  prepared: PreparePlanArtifactsResult,
  options: { files?: string[] },
  projectRoot: string,
  isWinCmd: boolean,
): string | null {
  const script = isWinCmd ? entry.script.cmd : entry.script.shell;
  if (!script || script.trim().length === 0) {
    return null;
  }

  let testCmd = expandCommandTemplate(
    script.trim(),
    prepared,
    options.files ?? [],
    projectRoot,
    pathFilterFromPlan(entry),
  );

  if (prepared.redirectStdoutToResults) {
    testCmd = applyResultsRedirect(
      testCmd,
      prepared.placeholders.results_file,
      entry.framework,
      isWinCmd,
    );
  }
  return testCmd;
}

function buildPlanExecutionResult(
  entry: TestPlan,
  projectRoot: string,
  absCwd: string,
  reportDir: string,
  planId: string,
  startTime: number,
  exitCode: number,
  execError: string | undefined,
  options: { noMutation?: boolean; mutationDiffFiles?: string[] },
): ExecutionResult {
  const parsed = parsePlanArtifacts(entry.framework, reportDir);
  const resultsFile = path.join(
    reportDir,
    RESULTS_FILE_BY_FRAMEWORK[entry.framework] ?? 'results.txt',
  );

  // Missing/unparseable results after a non-zero exit → execution_error
  const parseError =
    parsed.error ||
    (exitCode !== 0 && parsed.testCases.length === 0
      ? 'Missing or unparseable results file in plan directory'
      : undefined);

  const mutationFiles = restrictMutationScope(parsed.sourceFiles, options.mutationDiffFiles);
  const mutation =
    parsed.failed === 0
      ? runMutationPhase(entry, projectRoot, absCwd, reportDir, options, mutationFiles)
      : null;

  return {
    framework: entry.framework,
    exitCode,
    testCases: parsed.testCases,
    coverage: parsed.coverage,
    mutation,
    durationMs: Date.now() - startTime,
    testFiles: parsed.testFiles,
    sourceFiles: parsed.sourceFiles,
    error: execError || parseError,
    planId,
    reportDir,
    resultsFile,
  };
}

type ExecutePlanOptions = {
  files?: string[];
  timeout?: number;
  noMutation?: boolean;
  mutationDiffFiles?: string[];
  reportsDir: string;
};

function runPreparedPlanEntry(
  entry: TestPlan,
  projectRoot: string,
  absCwd: string,
  reportDir: string,
  planId: string,
  startTime: number,
  prepared: PreparePlanArtifactsResult,
  options: ExecutePlanOptions,
  isWinCmd: boolean,
): ExecutionResult {
  const testCmd = resolveTestCommand(entry, prepared, options, projectRoot, isWinCmd);
  if (testCmd === null) {
    return emptyResult(entry.framework, startTime, 'Empty test command', planId, reportDir);
  }

  console.log(`Executing test cmd: "${testCmd}" in "${absCwd}"`);
  const { exitCode, execError } = runCommand(testCmd, absCwd, options.timeout);

  return buildPlanExecutionResult(
    entry,
    projectRoot,
    absCwd,
    reportDir,
    planId,
    startTime,
    exitCode,
    execError,
    options,
  );
}

// ---------------------------------------------------------------------------
// Main execution function
// ---------------------------------------------------------------------------

/**
 * Execute a single plan entry's test command.
 *
 * @param entry       - The plan entry to execute
 * @param projectRoot - Absolute project root path
 * @param options     - reportsDir (required) plus optional files/timeout/mutation flags
 */
export function executePlanEntry(
  entry: TestPlan,
  projectRoot: string,
  options: ExecutePlanOptions,
): ExecutionResult {
  const startTime = Date.now();
  const planId = derivePlanId(entry.root, entry.framework);
  const reportDir = path.join(options.reportsDir, planId);
  const absCwd = path.resolve(projectRoot, entry.cwd);
  const isWinCmd = process.platform === 'win32' && !process.env.SHELL;

  let prepared: PreparePlanArtifactsResult | null = null;

  try {
    clearDirectory(reportDir);
    prepared = preparePlanArtifacts({
      framework: entry.framework,
      absCwd,
      reportDir,
      userConfigPath: resolveUserConfigPath(entry, projectRoot),
      projectRoot,
    });
    return runPreparedPlanEntry(
      entry,
      projectRoot,
      absCwd,
      reportDir,
      planId,
      startTime,
      prepared,
      options,
      isWinCmd,
    );
  } catch (e) {
    const message = e instanceof Error ? e.message : 'preparePlanArtifacts failed';
    return emptyResult(entry.framework, startTime, message, planId, reportDir);
  } finally {
    if (prepared) {
      cleanupTempPaths(prepared.tempPaths);
    }
  }
}

// ---------------------------------------------------------------------------
// Mutation testing
// ---------------------------------------------------------------------------

function runMutationPhase(
  entry: TestPlan,
  projectRoot: string,
  absCwd: string,
  reportDir: string,
  options: { noMutation?: boolean },
  sourceFiles: string[],
): MutationBlock | null {
  if (!entry.mutation_script || options.noMutation) {
    return null;
  }

  let projectConfig: OpenSpecConfig;
  try {
    projectConfig = readConfig(projectRoot);
  } catch {
    return null;
  }

  const filteredSources = sourceFiles.filter((f) => !isFileExcluded(f, projectConfig));
  if (filteredSources.length === 0) {
    return null;
  }

  const sourcesRelativeToCwd = filteredSources.map((f) => {
    const abs = path.isAbsolute(f) ? path.resolve(f) : path.resolve(projectRoot, f);
    return path.relative(absCwd, abs).replace(/\\/g, '/');
  });

  try {
    return executeStrykerMutation(entry, absCwd, reportDir, sourcesRelativeToCwd);
  } catch (e) {
    console.log(`  Mutation testing skipped: ${e instanceof Error ? e.message : 'Unknown error'}`);
    return null;
  }
}

function executeStrykerMutation(
  entry: TestPlan,
  absCwd: string,
  reportDir: string,
  filteredSources: string[],
): MutationBlock | null {
  const { configPath, tempDirPath } = resolveStrykerConfig(
    absCwd,
    filteredSources,
    entry.framework,
    reportDir,
  );

  const strykerCmd = genStrykerCommand(entry, configPath.replace(/\\/g, '/'));
  console.log(`Running StrykerJS mutation testing (cmd: ${strykerCmd}, cwd: ${absCwd})...`);
  const strykerStart = Date.now();
  const cmdResult = runCommand(strykerCmd, absCwd, 1200000);
  const strykerDuration = (Date.now() - strykerStart) / 1000;

  if (cmdResult.exitCode !== 0) {
    logCommandFailure(cmdResult, strykerDuration);
    cleanupMutationArtifacts(configPath, tempDirPath);
    return null;
  }

  const mutationBlock = buildMutationBlockFromReport(entry, reportDir);
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
  const script = entry.mutation_script;
  if (!script) {
    throw new Error(`Framework "${entry.framework}" does not support mutation testing`);
  }
  const isWinCmd = process.platform === 'win32' && !process.env.SHELL;
  const mutationTemplate = isWinCmd ? script.cmd : script.shell;
  return mutationTemplate.replace(/\{config\}/g, configPath);
}

/**
 * Parse the StrykerJS mutation report from planDir/mutation.json.
 */
function buildMutationBlockFromReport(entry: TestPlan, reportDir: string): MutationBlock | null {
  const reportPath = path.join(reportDir, 'mutation.json');
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

function cleanupMutationArtifacts(configPath: string, tempDirPath: string): void {
  try {
    if (fs.existsSync(configPath)) {
      fs.rmSync(configPath);
    }
  } catch {
    // Best-effort cleanup
  }

  try {
    if (fs.existsSync(tempDirPath)) {
      fs.rmSync(tempDirPath, { recursive: true });
    }
  } catch {
    // Best-effort cleanup
  }
}

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

function logMissingReport(result: { exitCode: number }): void {
  if (result.exitCode === 0) {
    console.log('  Mutation report not found or invalid — skipping mutation result');
  }
}

function emptyResult(
  framework: string,
  startTime: number,
  error: string,
  planId: string,
  reportDir: string,
): ExecutionResult {
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
    planId,
    reportDir,
  };
}
