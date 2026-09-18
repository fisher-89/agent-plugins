/**
 * gitignore.ts — hierarchical `.gitignore` filter for the PostToolUse
 * recording pipeline (internal consumer: `files/record.ts`; deliberately not
 * re-exported from the workflow barrel).
 *
 * The `ignore` package carries the single-file gitignore semantics (negative
 * patterns, directory recursion, last-match-wins, no re-include under an
 * excluded parent). This wrapper adds what the library does not do: the
 * hierarchy — the project root `.gitignore` plus the `.gitignore` of every
 * directory on the target path's ancestor chain, with deep file rules
 * overriding shallow ones and a short-circuit for excluded directories (git
 * does not descend into them, so a deeper `.gitignore` inside an excluded
 * directory never participates).
 *
 * Fail-open throughout: a missing layer means "no rules for this layer" (the
 * normal path, not a warning); any read/parse/evaluation failure is reported
 * via `onWarn` and treated as "not ignored" — this module never throws, in
 * line with the recorder's exit-0 swallow-everything policy and the fold
 * principle of over-recording rather than losing real change paths.
 */

import * as fs from 'node:fs';
import * as path from 'node:path';

import ignore, { type Ignore } from 'ignore';

const GITIGNORE_FILE = '.gitignore';

/**
 * Opaque hierarchical filter handle: the project root, the root layer parsed
 * eagerly at build time, a cache of ancestor-chain layers keyed by their
 * root-relative directory prefix, and the diagnostic sink. Consumers never
 * inspect the internals — the handle only flows back into `isGitIgnored`.
 */
export interface GitignoreFilter {
  projectRoot: string;
  rootLayer: Ignore | null;
  layers: Map<string, Ignore | null>;
  onWarn?: (message: string) => void;
}

/**
 * One activated layer of the hierarchy: its rule engine instance plus the
 * root-relative directory prefix the instance's patterns are relative to
 * (`''` for the root layer).
 */
interface ActivatedLayer {
  ignore: Ignore;
  prefix: string;
}

/**
 * Parse one layer's `.gitignore` into a single-file rule engine instance.
 * A missing file is a normal no-rules layer; any other read or parse failure
 * is reported via `onWarn` and treated as no rules (fail-open).
 */
function loadLayer(
  gitignorePath: string,
  onWarn: ((message: string) => void) | undefined,
): Ignore | null {
  if (!fs.existsSync(gitignorePath)) return null;
  try {
    const instance = ignore();
    instance.add(fs.readFileSync(gitignorePath, 'utf-8'));
    return instance;
  } catch (e: unknown) {
    const message = e instanceof Error ? e.message : String(e);
    onWarn?.(`读取 .gitignore 失败，该层按无规则处理: ${gitignorePath} (${message})`);
    return null;
  }
}

/**
 * Build a hierarchical filter: the root `.gitignore` is parsed immediately;
 * ancestor-chain layers resolve lazily on first `isGitIgnored` hit and are
 * cached on the handle (one filter per hook call).
 */
export function loadGitignoreFilter(
  projectRoot: string,
  onWarn?: (message: string) => void,
): GitignoreFilter {
  return {
    projectRoot,
    rootLayer: loadLayer(path.join(projectRoot, GITIGNORE_FILE), onWarn),
    layers: new Map(),
    onWarn,
  };
}

/** Load (and cache) the `.gitignore` layer at a root-relative directory prefix. */
function loadCachedLayer(filter: GitignoreFilter, prefix: string): Ignore | null {
  const cached = filter.layers.get(prefix);
  if (cached !== undefined) return cached;
  const layer = loadLayer(path.join(filter.projectRoot, prefix, GITIGNORE_FILE), filter.onWarn);
  filter.layers.set(prefix, layer);
  return layer;
}

/** The part of `relPath` relative to a layer's directory prefix. */
function relativeTo(prefix: string, relPath: string): string {
  return prefix ? relPath.slice(prefix.length + 1) : relPath;
}

/**
 * Walk the ancestor chain shallow → deep. At each directory prefix, test it
 * as a directory against the layers activated so far — an ignored directory
 * short-circuits the whole path to `true` (its own `.gitignore` is not
 * loaded, matching git's no-descent behavior); otherwise its layer joins the
 * activated set.
 */
function activateAncestorChain(filter: GitignoreFilter, relPath: string): ActivatedLayer[] {
  const activated: ActivatedLayer[] = filter.rootLayer
    ? [{ ignore: filter.rootLayer, prefix: '' }]
    : [];
  const segments = relPath.split('/');
  segments.pop(); // only directories on the chain carry layers

  let prefix = '';
  for (const segment of segments) {
    const dirPath = prefix ? `${prefix}/${segment}` : segment;
    const dirHit = activated.some(
      (layer) => layer.ignore.test(`${relativeTo(layer.prefix, dirPath)}/`).ignored,
    );
    if (dirHit) break;
    const layer = loadCachedLayer(filter, dirPath);
    if (layer) activated.push({ ignore: layer, prefix: dirPath });
    prefix = dirPath;
  }
  return activated;
}

/** Evaluate the path against every activated layer; the last match decides. */
function evaluatePath(filter: GitignoreFilter, relPath: string): boolean {
  const activated = activateAncestorChain(filter, relPath);
  let ignored = false;
  for (const layer of activated) {
    const result = layer.ignore.test(relativeTo(layer.prefix, relPath));
    if (result.ignored || result.unignored) {
      ignored = result.ignored;
    }
  }
  return ignored;
}

/**
 * Evaluate a project-root-relative POSIX path against the hierarchical
 * rules. An empty path is never ignored; any evaluation error is a fail-open
 * `false` with an `onWarn` diagnostic — never a thrown error.
 */
export function isGitIgnored(filter: GitignoreFilter, relPath: string): boolean {
  if (!relPath) return false;
  try {
    return evaluatePath(filter, relPath);
  } catch (e: unknown) {
    const message = e instanceof Error ? e.message : String(e);
    filter.onWarn?.(`gitignore 判定异常，按不忽略处理: ${relPath} (${message})`);
    return false;
  }
}
