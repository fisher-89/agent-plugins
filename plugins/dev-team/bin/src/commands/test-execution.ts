// ---------------------------------------------------------------------------
// Test Execution Command Handler
//
// CLI entry point for `dev-team test-execution`.  Orchestrates the full flow:
//   1. Detect frameworks via runTestDetectFrameworks
//   2. With --change: gate plan entries to the change file inventory
//   3. For each framework plan entry, execute via executePlanEntry
//   4. Generate per-plan atomic reports via generateSubReport
//   5. Generate summary report via generateSummaryReport
// ---------------------------------------------------------------------------

import { execFileSync } from 'node:child_process';
import * as fs from 'node:fs';
import * as path from 'path';

import { getProjectDir } from '../lib/project-root';
import { deriveSourcePathFromTestFile } from '../lib/test-path-naming';
import { derivePlanId, isUnderPlanRoot, resolvePlanFiles } from '../lib/test-plan';
import { generateSubReport, generateSummaryReport } from '../lib/test-report';
import { executePlanEntry } from '../lib/test-runner';
import { getChangedFiles } from '../modules/workflow';
import {
  testExecutionSummaryReportSchema,
  type TestExecutionSubReport,
  type TestExecutionSummaryReport,
  type TestPlan,
} from '../schemas';
import { runTestDetectFrameworks } from './test-detect-frameworks';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface TestExecutionOptions {
  /**
   * Change name. Locates the reports directory AND narrows execution to the
   * change file inventory (`workflow.json` file_log net state):
   * - Plan gating: plan entries whose suite `root` contains no inventory file
   *   (written ∪ deleted) are skipped; inventory read failures fail open
   *   (all entries run).
   * - Mutation scope (when mutation is not skipped): derived from
   *   `written` with the net-zero denoise filter applied.
   * No dedicated scope option exists.
   */
  change?: string;
  projectRoot?: string;
  /** test files to run */
  files?: string[];
  framework?: string;
  noMutation?: boolean;
  /** Skip the fresh-summary reuse gate and re-execute unconditionally. */
  forceRerun?: boolean;
}

// ---------------------------------------------------------------------------
// Reports directory resolution
// ---------------------------------------------------------------------------

function resolveReportsDir(projectRoot: string, change?: string): string {
  if (change) {
    return path.resolve(projectRoot, 'openspec', 'changes', change, 'reports', 'test');
  }
  return path.resolve(projectRoot, 'reports', 'test');
}

// ---------------------------------------------------------------------------
// Mutation scope resolution (change file inventory)
// ---------------------------------------------------------------------------

/**
 * Expand inventory paths so test-file entries also contribute their
 * colocated sources (via test-path-naming reverse mapping).
 */
function expandMutationDiffWithInferredSources(files: string[]): string[] {
  const result = new Set<string>();
  for (const file of files) {
    const posix = file.replace(/\\/g, '/');
    result.add(posix);
    const source = deriveSourcePathFromTestFile(posix);
    if (source) {
      result.add(source);
    }
  }
  return Array.from(result);
}

/**
 * Return the `HEAD` version of a file, or null when HEAD has no such file or
 * the read fails — the caller then keeps the file (overstate direction only).
 */
function headFileContent(projectRoot: string, file: string): string | null {
  try {
    return execFileSync('git', ['show', `HEAD:${file}`], {
      cwd: projectRoot,
      encoding: 'utf-8',
      timeout: 10000,
      stdio: ['ignore', 'pipe', 'ignore'],
    });
  } catch {
    return null;
  }
}

/**
 * Net-zero denoise: drop files whose working-tree content is identical to
 * their HEAD version (written then reverted in place) — mutating them is pure
 * noise. The filter is an optimization only, never the scope authority;
 * correctness of filtered files stays with the evaluator content check.
 * Unreadable working-tree files and HEAD-less (new) files are kept.
 */
function filterNetZeroFiles(files: string[], projectRoot: string): string[] {
  const kept: string[] = [];
  let excluded = 0;
  for (const file of files) {
    let working: string | null = null;
    try {
      working = fs.readFileSync(path.resolve(projectRoot, file), 'utf-8');
    } catch {
      kept.push(file);
      continue;
    }
    const head = headFileContent(projectRoot, file);
    if (head !== null && head === working) {
      excluded += 1;
      continue;
    }
    kept.push(file);
  }
  if (excluded > 0) {
    console.log(
      `mutation denoise: ${excluded} net-zero files excluded (content identical to HEAD)`,
    );
  }
  return kept;
}

/**
 * Resolve the mutation scope from the change file inventory (`written` bucket
 * of the net state already read by the caller).
 */
function resolveMutationDiffFiles(written: string[], projectRoot: string): string[] | undefined {
  const expanded = expandMutationDiffWithInferredSources(written);
  const scoped = filterNetZeroFiles(expanded, projectRoot);
  console.log(
    `mutation scope (change inventory): ${scoped.length} files (${scoped.slice(0, 5).join(', ')})`,
  );
  return scoped.map((file) => path.resolve(projectRoot, file).replace(/\\/g, '/'));
}

// ---------------------------------------------------------------------------
// Change-scope plan gating
// ---------------------------------------------------------------------------

/**
 * One --change-scoped execution context: the inventory read plus the gate
 * file set derived from it.
 */
interface ChangeScope {
  /** `written` bucket of the net state; undefined without --change or on fail-open. */
  inventoryWritten?: string[];
  /** written ∪ deleted (POSIX); undefined when gating is inactive. */
  gateFiles?: string[];
}

/**
 * Read the change file inventory once. Gate reads fail open — an unreadable
 * inventory never blocks execution (the `--skip-mutation` escape hatch);
 * the mutation scope keeps its hard-error contract when mutation is enabled.
 */
function resolveChangeScope(
  change: string | undefined,
  noMutation: boolean,
  projectRoot: string,
): ChangeScope {
  if (!change) return {};
  let net: ReturnType<typeof getChangedFiles>;
  try {
    net = getChangedFiles(change, projectRoot);
  } catch (e) {
    if (!noMutation) throw e;
    const message = e instanceof Error ? e.message : String(e);
    console.log(`change scope gate: inventory read failed (${message}) — running all plan entries`);
    return {};
  }
  const gateFiles = [...net.written, ...net.deleted].map((file) => file.replace(/\\/g, '/'));
  return {
    inventoryWritten: net.written,
    gateFiles: gateFiles.length > 0 ? gateFiles : undefined,
  };
}

/**
 * Filter plan entries to those whose suite `root` contains at least one
 * changed file (written ∪ deleted). Root-level matching only — colocated
 * test/source pairs share a directory so both resolve to the same root, and
 * includes/excludes are deliberately ignored (over-trigger runs an unrelated
 * suite; under-trigger would skip a suite whose tests should run).
 *
 * Safety net: a non-empty change that matches no suite root at all suggests a
 * layout the root rule cannot see — all entries run in that case.
 */
function gatePlanEntries(planEntries: TestPlan[], gateFiles: string[]): TestPlan[] {
  const inScope = planEntries.filter((entry) =>
    gateFiles.some((file) => isUnderPlanRoot(file, entry.root)),
  );

  if (inScope.length === 0) {
    console.log(
      'change scope gate: no changed files under any suite root — running all plan entries',
    );
    return planEntries;
  }

  for (const entry of planEntries) {
    if (!inScope.includes(entry)) {
      console.log(
        `Skipping ${entry.framework} in ${entry.root}: no changed files under suite root (change scope)`,
      );
    }
  }
  console.log(
    `change scope gate: ${inScope.length}/${planEntries.length} plan entries in change scope`,
  );
  return inScope;
}

// ---------------------------------------------------------------------------
// Logging helpers
// ---------------------------------------------------------------------------

function logResult(entryFramework: string, result: { testCases: Array<{ status: string }> }): void {
  const testCases = result.testCases;
  console.log(
    `  ${entryFramework}: ${testCases.length} tests, ` +
      `${testCases.filter((t) => t.status === 'passed').length} passed, ` +
      `${testCases.filter((t) => t.status === 'failed').length} failed, ` +
      `${testCases.filter((t) => t.status === 'skipped').length} skipped`,
  );
}

function logSummary(report: {
  conclusion: string;
  total: number;
  passed: number;
  failed: number;
  skipped: number;
  duration_seconds: number;
  problems: Array<{ type: string; framework: string; message: string }>;
}): void {
  console.log(`\nSummary: ${report.conclusion.toUpperCase()}`);
  console.log(`  Total: ${report.total}`);
  console.log(`  Passed: ${report.passed}`);
  console.log(`  Failed: ${report.failed}`);
  console.log(`  Skipped: ${report.skipped}`);
  console.log(`  Duration: ${report.duration_seconds}s`);

  if (report.problems.length > 0) {
    console.log(`\nProblems (${report.problems.length}):`);
    for (const problem of report.problems) {
      console.log(`  [${problem.type}] ${problem.framework}: ${problem.message}`);
    }
  }
}

// ---------------------------------------------------------------------------
// Fresh-summary reuse
// ---------------------------------------------------------------------------

/**
 * Directory names pruned from the plan-root freshness scan — dependencies and
 * tool output, never execution inputs.
 */
const REUSE_SCAN_EXCLUDED_DIRS = new Set([
  'node_modules',
  'dist',
  'build',
  'target',
  'coverage',
  '_stryker-tmp',
]);

/** Newest input file found by the reuse scan (absolute path + mtime). */
interface NewestInput {
  file: string;
  mtimeMs: number;
}

/** Whether `candidate` is inside `dir` (inclusive), platform-native paths. */
function isInsideDir(dir: string, candidate: string): boolean {
  const rel = path.relative(dir, candidate);
  return rel === '' || (!rel.startsWith('..') && !path.isAbsolute(rel));
}

/** Fold one file into the newest-input tracker; unreadable files are ignored. */
function considerInput(abs: string, newest: NewestInput | null): NewestInput | null {
  let mtimeMs: number;
  try {
    mtimeMs = fs.statSync(abs).mtimeMs;
  } catch {
    return newest;
  }
  return !newest || mtimeMs > newest.mtimeMs ? { file: abs, mtimeMs } : newest;
}

/**
 * Track the newest mtime among all files under `dir`, recursively. Dot
 * directories (`.git`, tool caches) and {@link REUSE_SCAN_EXCLUDED_DIRS} names
 * are pruned, plus the absolute `pruneDirs` (workflow state under `openspec/`
 * and the CLI's own report output — neither is an execution input).
 */
function scanNewestMtime(
  dir: string,
  pruneDirs: string[],
  newest: NewestInput | null,
): NewestInput | null {
  if (!fs.existsSync(dir)) {
    return newest;
  }
  const stack: string[] = [dir];
  for (let current = stack.pop(); current !== undefined; current = stack.pop()) {
    let entries: fs.Dirent[];
    try {
      entries = fs.readdirSync(current, { withFileTypes: true });
    } catch {
      continue;
    }
    for (const dirent of entries) {
      const abs = path.join(current, dirent.name);
      if (dirent.isDirectory()) {
        if (dirent.name.startsWith('.') || REUSE_SCAN_EXCLUDED_DIRS.has(dirent.name)) {
          continue;
        }
        if (pruneDirs.some((prune) => isInsideDir(prune, abs))) {
          continue;
        }
        stack.push(abs);
      } else if (dirent.isFile()) {
        newest = considerInput(abs, newest);
      }
    }
  }
  return newest;
}

/** Read and shape-check the existing summary; null when absent or malformed. */
function readExistingSummary(reportsDir: string): TestExecutionSummaryReport | null {
  try {
    const raw: unknown = JSON.parse(
      fs.readFileSync(path.join(reportsDir, 'summary.json'), 'utf-8'),
    );
    const parsed = testExecutionSummaryReportSchema.safeParse(raw);
    return parsed.success ? parsed.data : null;
  } catch {
    return null;
  }
}

/** Inputs to the fresh-summary reuse gate. */
interface ReuseCheckArgs {
  projectRoot: string;
  reportsDir: string;
  effectivePlanEntries: TestPlan[];
  inventoryWritten?: string[];
  mutationRequired: boolean;
}

/**
 * Reuse gate — guarantees mutation (and the whole execution) runs at most once
 * per code state, whoever ran the previous attempt (a backtrack-loop ad-hoc
 * verification run, the previous test-execution phase, a manual CLI call).
 *
 * A summary is reusable when ALL hold:
 * 1. It exists and is complete (`conclusion !== "error"` — error reports are
 *    partial executions and would also poison the executor's Step 1b loop).
 * 2. Mutation results are present whenever this run would measure mutation
 *    (a `--skip-mutation` report must never satisfy the mutation gate).
 * 3. Its plan set is exactly what this run would execute (scope changed
 *    ⇒ stale).
 * 4. No input file is newer than the summary timestamp (see findNewestInput).
 *
 * mtime-based by design; content hashing is out of scope. Fail direction is
 * always "re-run" (git checkout / clock skew can only over-invalidate).
 */
function tryReuseFreshSummary(
  args: ReuseCheckArgs,
): { summary: TestExecutionSummaryReport; newestInput: NewestInput | null } | null {
  const summary = readExistingSummary(args.reportsDir);
  if (!summary || !summaryMatchesScope(summary, args)) {
    return null;
  }
  const summaryTime = Date.parse(summary.timestamp);
  const newest = findNewestInput(args);
  if (Number.isNaN(summaryTime) || (newest !== null && newest.mtimeMs > summaryTime)) {
    return null;
  }
  return { summary, newestInput: newest };
}

/** Scope-level checks: complete report, mutation coverage, identical plan set. */
function summaryMatchesScope(summary: TestExecutionSummaryReport, args: ReuseCheckArgs): boolean {
  if (summary.conclusion === 'error') {
    return false;
  }
  if (args.mutationRequired && summary.mutation === null) {
    return false;
  }
  const expectedIds = new Set(
    args.effectivePlanEntries.map((entry) => derivePlanId(entry.root, entry.framework)),
  );
  const recordedIds = new Set(summary.plans.map((plan) => plan.id));
  return (
    expectedIds.size === recordedIds.size && [...expectedIds].every((id) => recordedIds.has(id))
  );
}

/**
 * Newest mtime among all execution inputs: every file under each plan root
 * (not just the change inventory — out-of-inventory test files count too),
 * plus `openspec/config.json` (thresholds / suite mapping) and inventory
 * files outside all plan roots.
 */
function findNewestInput(args: ReuseCheckArgs): NewestInput | null {
  const pruneDirs = [path.resolve(args.projectRoot, 'openspec'), path.resolve(args.reportsDir)];
  const planRoots = [
    ...new Set(
      args.effectivePlanEntries.map((entry) => path.resolve(args.projectRoot, entry.root)),
    ),
  ];
  let newest: NewestInput | null = null;
  for (const root of planRoots) {
    newest = scanNewestMtime(root, pruneDirs, newest);
  }
  newest = considerInput(path.resolve(args.projectRoot, 'openspec', 'config.json'), newest);
  for (const file of args.inventoryWritten ?? []) {
    const abs = path.resolve(args.projectRoot, file);
    if (!planRoots.some((root) => isInsideDir(root, abs))) {
      newest = considerInput(abs, newest);
    }
  }
  return newest;
}

/**
 * Reuse-gate wrapper: returns the exit code when the existing summary is
 * fresh enough to reuse (after logging the reuse decision), or null when
 * execution should proceed.
 */
function reuseFreshSummaryExit(args: {
  projectRoot: string;
  reportsDir: string;
  effectivePlanEntries: TestPlan[];
  inventoryWritten?: string[];
  noMutation: boolean;
}): number | null {
  const reuse = tryReuseFreshSummary({
    ...args,
    mutationRequired:
      !args.noMutation && args.effectivePlanEntries.some((entry) => entry.mutation_script),
  });
  if (!reuse) {
    return null;
  }
  const newest = reuse.newestInput
    ? `${reuse.newestInput.file} @ ${new Date(reuse.newestInput.mtimeMs).toISOString()}`
    : 'none';
  console.log(
    `Reusing fresh summary (timestamp ${reuse.summary.timestamp} >= newest input ${newest}, ` +
      `conclusion ${reuse.summary.conclusion}); pass --force to re-run`,
  );
  logSummary(reuse.summary);
  return reuse.summary.conclusion === 'pass' ? 0 : 1;
}

// ---------------------------------------------------------------------------
// Per-plan execution
// ---------------------------------------------------------------------------

/**
 * Run one plan entry. Returns null when explicit `--files` has no overlap with
 * the suite root (skip — do not fall back to full-scope discovery).
 */
function runPlanEntry(
  entry: TestPlan,
  projectRoot: string,
  options: TestExecutionOptions,
  mutationDiffFiles: string[] | undefined,
  reportsDir: string,
): TestExecutionSubReport | null {
  const planFiles = resolvePlanFiles(options.files, entry);
  if (options.files !== undefined && (planFiles?.length ?? 0) === 0) {
    console.log(`Skipping ${entry.framework} in ${entry.root}: no files under suite root`);
    return null;
  }

  console.log(`Running ${entry.framework} tests in ${entry.cwd} (root: ${entry.root})...`);
  const result = executePlanEntry(entry, projectRoot, {
    files: planFiles,
    noMutation: options.noMutation,
    mutationDiffFiles,
    reportsDir,
  });
  const subReport = generateSubReport(entry.framework, result, projectRoot, reportsDir, entry.root);
  logResult(entry.framework, result);
  return subReport;
}

// ---------------------------------------------------------------------------
// Main handler
// ---------------------------------------------------------------------------

/**
 * Resolve the plan entries for this invocation (framework filter applied).
 * Null when there is nothing to execute — the caller exits 0 after logging.
 */
function resolveExecutionPlans(
  options: TestExecutionOptions,
  projectRoot: string,
): TestPlan[] | null {
  const detectResult = runTestDetectFrameworks({ files: options.files, projectRoot });

  if (detectResult.plan.length === 0) {
    console.log('No test configuration found. Configure tests in openspec/config.json');
    return null;
  }

  const planEntries = options.framework
    ? detectResult.plan.filter((entry) => entry.framework === options.framework)
    : detectResult.plan;

  if (planEntries.length === 0) {
    console.log(`No plan entries found for framework "${options.framework}"`);
    return null;
  }
  return planEntries;
}

export async function runTestExecution(options: TestExecutionOptions): Promise<number> {
  const projectRoot = options.projectRoot || getProjectDir();
  const planEntries = resolveExecutionPlans(options, projectRoot);
  if (planEntries === null) {
    return 0;
  }

  const { inventoryWritten, gateFiles } = resolveChangeScope(
    options.change,
    options.noMutation === true,
    projectRoot,
  );

  const reportsDir = resolveReportsDir(projectRoot, options.change);
  const effectivePlanEntries = gateFiles ? gatePlanEntries(planEntries, gateFiles) : planEntries;

  if (options.forceRerun !== true) {
    const reused = reuseFreshSummaryExit({
      projectRoot,
      reportsDir,
      effectivePlanEntries,
      inventoryWritten,
      noMutation: options.noMutation === true,
    });
    if (reused !== null) {
      return reused;
    }
  }

  const mutationDiffFiles =
    inventoryWritten !== undefined && !options.noMutation
      ? resolveMutationDiffFiles(inventoryWritten, projectRoot)
      : undefined;
  const subReports = [];

  for (const entry of effectivePlanEntries) {
    const subReport = runPlanEntry(entry, projectRoot, options, mutationDiffFiles, reportsDir);
    if (subReport) {
      subReports.push(subReport);
    }
  }

  const summaryReport = generateSummaryReport(subReports, projectRoot, reportsDir);
  logSummary(summaryReport);

  return summaryReport.conclusion === 'pass' ? 0 : 1;
}
