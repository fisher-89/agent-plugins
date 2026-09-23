// ---------------------------------------------------------------------------
// Test plan helpers — suite resolve, scope matching, planId, file filters
// ---------------------------------------------------------------------------

import * as path from 'path';

import type { OpenSpecConfig, TestPlan, TestSuite } from '../schemas';
import { matchGlob, toForwardSlash } from './glob';
import { isExcludedBySuite, isFileExcluded } from './test-exclude';
import { type FrameworkConfig, getFrameworkConfig } from './test-framework';

/** Suite paths and framework config resolved against projectRoot. */
export interface ResolvedSuite {
  suite: TestSuite;
  absCwd: string;
  absConfig: string | null;

  /** absCwd relative to projectRoot (POSIX) */
  cwd: string;
  /** absRoot relative to projectRoot (POSIX) */
  root: string;
  /** absMutationCwd relative to projectRoot (POSIX); default is LCA(absRoot, absCwd, dirname(absConfig)?) */
  mutationCwd: string;
  frameworkConfig: FrameworkConfig;
}

function toPosixRelative(from: string, to: string): string {
  const rel = path.relative(from, to);
  const posix = toForwardSlash(rel);
  return posix === '' ? '.' : posix;
}

/** Whether `absPath` is inside or equal to `projectRoot` (platform path). */
function isInsideProjectRoot(absPath: string, projectRoot: string): boolean {
  const rel = path.relative(path.resolve(projectRoot), path.resolve(absPath));
  return rel === '' || (!rel.startsWith('..') && !path.isAbsolute(rel));
}

/** Directory LCA of two absolute paths; throws when no common ancestor exists. */
function directoryLcaPair(a: string, b: string): string {
  let current = path.resolve(a);
  const target = path.resolve(b);
  const fsRoot = path.parse(current).root;

  while (true) {
    const rel = path.relative(current, target);
    if (rel === '' || (!rel.startsWith('..') && !path.isAbsolute(rel))) {
      return current;
    }
    if (current === fsRoot) {
      throw new Error(
        'Cannot compute mutation_cwd: paths have no common ancestor (cross-drive or disjoint)',
      );
    }
    current = path.dirname(current);
  }
}

/** Directory LCA of multiple paths; throws on cross-drive or no common ancestor. */
function directoryLca(dirs: string[]): string {
  const resolved = dirs.map((d) => path.resolve(d));
  const roots = resolved.map((p) => path.parse(p).root);
  if (roots.some((r, i) => i > 0 && r !== roots[0])) {
    throw new Error(
      'Cannot compute mutation_cwd: paths span multiple drive roots or have no common ancestor',
    );
  }

  let lca = resolved[0];
  for (let i = 1; i < resolved.length; i++) {
    lca = directoryLcaPair(lca, resolved[i]);
  }
  return lca;
}

function resolveAbsMutationCwd(
  suite: TestSuite,
  absRoot: string,
  absCwd: string,
  absConfig: string | null,
  projectRoot: string,
): string {
  if (suite.mutation.cwd !== undefined) {
    return path.resolve(absRoot, suite.mutation.cwd);
  }

  const dirs = [absRoot, absCwd];
  if (absConfig !== null) {
    dirs.push(path.dirname(absConfig));
  }

  let absMutationCwd = directoryLca(dirs);
  if (!isInsideProjectRoot(absMutationCwd, projectRoot)) {
    absMutationCwd = path.resolve(projectRoot);
  }
  return absMutationCwd;
}

/**
 * Resolve suite.root / suite.cwd against projectRoot and load framework registry entry.
 */
function resolveSuite(suite: TestSuite, projectRoot: string): ResolvedSuite {
  const frameworkConfig = getFrameworkConfig(suite.framework);
  const absRoot = path.resolve(projectRoot, suite.root);
  const absCwd = path.resolve(absRoot, suite.cwd);
  const absConfig = suite.config ? path.resolve(absRoot, suite.config) : null;
  const absMutationCwd = resolveAbsMutationCwd(suite, absRoot, absCwd, absConfig, projectRoot);
  const cwd = toPosixRelative(projectRoot, absCwd);
  const root = toPosixRelative(projectRoot, absRoot);
  const mutationCwd = toPosixRelative(projectRoot, absMutationCwd);

  return {
    suite,
    absCwd,
    absConfig,
    cwd,
    root,
    mutationCwd,
    frameworkConfig,
  };
}

/** Resolve every suite in config order. */
export function resolveAllSuites(suites: TestSuite[], projectRoot: string): ResolvedSuite[] {
  return suites.map((suite) => resolveSuite(suite, projectRoot));
}

/**
 * Find a suite by framework + plan.root, then framework-only, then first suite.
 */
export function findSuite(
  config: OpenSpecConfig,
  framework?: string,
  planRoot?: string,
): TestSuite | undefined {
  const suites = config.tests ?? [];
  if (framework && planRoot !== undefined) {
    const match = suites.find(
      (s) => s.framework === framework && toForwardSlash(s.root) === toForwardSlash(planRoot),
    );
    if (match) return match;
  }
  if (framework) {
    const byFw = suites.find((s) => s.framework === framework);
    if (byFw) return byFw;
  }
  return suites[0];
}

/**
 * Derive a plan directory id from plan.root and framework.
 *
 * Examples:
 *   derivePlanId('.', 'vitest')                           → 'vitest'
 *   derivePlanId('plugins/dev-team/bin', 'vite-plus')     → 'plugins_dev-team_bin_vite-plus'
 *   derivePlanId('plugins/dev-team/bin/src', 'vite-plus') → 'plugins_dev-team_bin_src_vite-plus'
 */
export function derivePlanId(root: string, framework: string): string {
  const sanitized = root === '.' ? '' : root.replace(/[\\/]/g, '_').replace(/\/$/, '');
  const prefix = sanitized ? `${sanitized}_` : sanitized;
  return `${prefix}${framework}`;
}

/** CLI path filter: suite root relative to cwd (`"."` when equal). */
export function pathFilterFromPlan(entry: Pick<TestPlan, 'cwd' | 'root'>): string {
  const rel = path.posix.relative(toForwardSlash(entry.cwd), toForwardSlash(entry.root));
  return rel === '' ? '.' : rel;
}

/** Whether a project-relative path is under plan.root (inclusive). */
export function isUnderPlanRoot(filePath: string, root: string): boolean {
  const relToRoot = path.posix.relative(toForwardSlash(root), toForwardSlash(filePath));
  return relToRoot === '' || !relToRoot.startsWith('..');
}

/** Keep files under plan.root, then rewrite paths relative to plan.cwd. */
export function resolvePlanFiles(
  files: string[] | undefined,
  entry: Pick<TestPlan, 'cwd' | 'root'>,
): string[] | undefined {
  return files
    ?.filter((filePath) => isUnderPlanRoot(filePath, entry.root))
    .map((filePath) => path.posix.relative(toForwardSlash(entry.cwd), toForwardSlash(filePath)));
}

/**
 * Whether a project-relative file is in a suite's scope:
 * under(root) ∧ match(includesEffective) ∧ ¬excludes.
 *
 * Exclude mode:
 * - `config` omitted → this suite's `excludes` only (`isExcludedBySuite`)
 * - `config` provided → any suite's excludes (`isFileExcluded`)
 *
 * includesEffective = suite.includes ?? framework.default_glob (relative to suite.root).
 */
export function isInSuiteScope(
  relativePath: string,
  suite: TestSuite,
  config?: OpenSpecConfig,
): boolean {
  const posix = toForwardSlash(relativePath);
  const root = path.posix.normalize(toForwardSlash(suite.root)).replace(/\/$/, '');

  if (posix !== root && !posix.startsWith(`${root}/`)) {
    return false;
  }

  if (config !== undefined) {
    if (isFileExcluded(posix, config)) {
      return false;
    }
  } else if (isExcludedBySuite(posix, suite)) {
    return false;
  }

  const includePatterns = suite.includes?.length
    ? suite.includes
    : [getFrameworkConfig(suite.framework).default_glob];

  return includePatterns.some((pattern) => {
    const scoped = path.posix.normalize(
      path.posix.join(toForwardSlash(suite.root), toForwardSlash(pattern)),
    );
    return matchGlob(posix, scoped);
  });
}
