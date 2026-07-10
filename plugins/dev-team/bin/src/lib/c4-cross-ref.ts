/**
 * c4-cross-ref.ts — Import cross-reference validation for C4 architecture model.
 *
 * Validates code changes against the C4 architecture model by:
 * 1. Parsing the model for metadata.path -> element mappings
 * 2. Extracting import statements from changed files (TS/JS/Python)
 * 3. Cross-referencing imports against declared model relationships
 * 4. Generating violations/warnings
 *
 * Ported from Python archi-validate.py.
 */

import { execSync } from 'child_process';
import * as fs from 'fs';
import * as path from 'path';

import { readAllModels, parseC4Dsl } from './c4-parser';
import type {
  C4Element,
  C4Relation,
  C4ParseResult,
  CrossRefViolation,
  ArchiCheckResult,
} from './c4-types';

// Python stdlib modules for filtering
const PYTHON_STDLIB = new Set([
  'os',
  'sys',
  're',
  'json',
  'math',
  'time',
  'datetime',
  'collections',
  'itertools',
  'functools',
  'typing',
  'io',
  'pathlib',
  'shutil',
  'subprocess',
  'argparse',
  'logging',
  'unittest',
  'abc',
  'base64',
  'hashlib',
  'random',
  'threading',
  'multiprocessing',
  'asyncio',
  'socket',
  'http',
  'urllib',
  'xml',
  'html',
  'csv',
  'configparser',
  'copy',
  'enum',
  'gc',
  'inspect',
  'struct',
  'tempfile',
  'textwrap',
  'traceback',
  'uuid',
  'warnings',
  'zipfile',
]);

/**
 * Load and parse the architecture model from models/*.c4 directory.
 */
async function loadModel(projectRoot: string): Promise<C4ParseResult> {
  const dsl = readAllModels(projectRoot);
  if (dsl === null) {
    return {
      elements: [],
      relationships: [],
      path_to_element: {},
      errors: ['Model not found: models/ directory does not exist or contains no .c4 files'],
    };
  }
  return parseC4Dsl(dsl);
}

/**
 * Normalize a path for comparison. Strips ./ prefix and trailing slash.
 */
function normalizePath(p: string): string {
  let s = p.trim();
  if (s.startsWith('./')) s = s.slice(2);
  return s.replace(/[/\\]$/, '');
}

/**
 * Get changed files from git diff --cached (staged) or a provided file list.
 */
function getChangedFiles(
  projectRoot: string,
  options: { staged?: boolean; files?: string[] } = {},
): string[] {
  if (options.files && options.files.length > 0) {
    return options.files;
  }

  if (options.staged) {
    try {
      const result = execSync('git diff --cached --name-only', {
        cwd: projectRoot,
        encoding: 'utf-8',
        timeout: 10000,
        stdio: 'pipe',
      });
      return result
        .split('\n')
        .map((s) => s.trim())
        .filter(Boolean);
    } catch {
      return [];
    }
  }

  return [];
}

/**
 * Parse import statements from a source file.
 */
function parseImports(filepath: string, projectRoot: string): string[] {
  const fullPath = path.resolve(projectRoot, filepath);
  if (!fs.existsSync(fullPath)) return [];

  let content: string;
  try {
    content = fs.readFileSync(fullPath, 'utf-8');
  } catch {
    return [];
  }

  const ext = path.extname(filepath).toLowerCase();
  const imports: string[] = [];

  if (['.ts', '.tsx', '.js', '.jsx', '.mjs', '.cjs'].includes(ext)) {
    imports.push(...parseJsImports(content));
  } else if (['.py', '.pyi'].includes(ext)) {
    imports.push(...parsePythonImports(content));
  }

  return imports;
}

/**
 * Parse JavaScript/TypeScript import statements.
 */
function parseJsImports(content: string): string[] {
  const imports: string[] = [];

  // import { x } from './path', import x from './path', import './path', import * as x from './path'
  const pattern = /import\s+(?:type\s+)?(?:(?:\{[^}]*\}|[\w*\s,]+)\s+from\s+)?['"]([^'"]+)['"]/g;
  let match: RegExpExecArray | null;
  while ((match = pattern.exec(content)) !== null) {
    const target = match[1];
    if (target && (target.startsWith('./') || target.startsWith('..') || target.startsWith('@/'))) {
      imports.push(target);
    }
  }

  // require('./path')
  const requirePattern = /require\s*\(\s*['"]([^'"]+)['"]\s*\)/g;
  while ((match = requirePattern.exec(content)) !== null) {
    const target = match[1];
    if (target && (target.startsWith('./') || target.startsWith('..') || target.startsWith('@/'))) {
      imports.push(target);
    }
  }

  return imports;
}

/**
 * Parse Python import statements.
 */
function parsePythonImports(content: string): string[] {
  const imports: string[] = [];

  // from .module import X, from ..module import X, from package.module import X
  const pattern = /^from\s+(\S+)\s+import/gm;
  let match: RegExpExecArray | null;
  while ((match = pattern.exec(content)) !== null) {
    const target = match[1];
    if (target.startsWith('.')) {
      imports.push(target);
    } else {
      const top = target.split('.')[0];
      if (!PYTHON_STDLIB.has(top)) {
        imports.push(target);
      }
    }
  }

  return imports;
}

/**
 * Resolve an import target to a relative file path.
 */
function resolveImportToPath(sourceFile: string, importTarget: string): string | null {
  const sourceDir = path.dirname(sourceFile);

  if (importTarget.startsWith('./') || importTarget.startsWith('..')) {
    const resolved = path.normalize(path.join(sourceDir, importTarget));
    return resolved.replace(/\\/g, '/');
  } else if (importTarget.startsWith('@/')) {
    return importTarget.slice(2);
  }

  return null;
}

/**
 * Match a resolved import path to an element name via path_to_element lookup.
 */
function matchPathToElement(
  resolvedPath: string,
  pathToElement: Record<string, string>,
): string | null {
  const norm = normalizePath(resolvedPath);

  // Direct match
  if (norm in pathToElement) return pathToElement[norm];

  // Try as directory (append /)
  if (norm + '/' in pathToElement) return pathToElement[norm + '/'];

  // Check if norm is under any registered path
  for (const [elemPath, elemName] of Object.entries(pathToElement)) {
    if (norm.startsWith(elemPath + '/') || norm === elemPath) {
      return elemName;
    }
  }

  // Try with common extensions
  for (const ext of ['.ts', '.tsx', '.js', '.jsx', '.py']) {
    const candidate = norm + ext;
    if (candidate in pathToElement) return pathToElement[candidate];
  }

  return null;
}

/**
 * Map files to model elements via metadata.path matching.
 */
function mapFilesToElements(
  files: string[],
  pathToElement: Record<string, string>,
): Record<string, string | null> {
  const mapping: Record<string, string | null> = {};

  for (const filepath of files) {
    const normFile = normalizePath(filepath);
    let matched: string | null = null;

    for (const [elemPath, elemName] of Object.entries(pathToElement)) {
      const normElem = normalizePath(elemPath);

      // Check if file is under a directory path
      if (normFile.startsWith(normElem + '/') || normFile.startsWith(normElem + '\\')) {
        matched = elemName;
        break;
      }

      // Exact file match
      if (normFile === normElem) {
        matched = elemName;
        break;
      }
    }

    mapping[filepath] = matched;
  }

  return mapping;
}

function buildRelationshipLookup(relationships: C4Relation[]): Set<string> {
  const relLookup = new Set<string>();
  for (const rel of relationships) {
    if (rel.source && rel.target) {
      relLookup.add(`${rel.source}::${rel.target}`);
    }
  }
  return relLookup;
}

function checkImportsAgainstRelationships(
  importsByFile: Record<string, string[]>,
  fileElementMap: Record<string, string | null>,
  pathToElement: Record<string, string>,
  relLookup: Set<string>,
): { violations: CrossRefViolation[]; warnings: CrossRefViolation[] } {
  const violations: CrossRefViolation[] = [];
  const warnings: CrossRefViolation[] = [];

  for (const [filepath, importTargets] of Object.entries(importsByFile)) {
    const sourceElem = fileElementMap[filepath];
    if (!sourceElem) continue;

    for (const imp of importTargets) {
      const resolved = resolveImportToPath(filepath, imp);
      if (resolved === null) continue;

      const targetElem = matchPathToElement(resolved, pathToElement);

      // Self-import within same element — ignore
      if (targetElem === sourceElem) continue;

      if (targetElem === null) {
        warnings.push({
          type: 'unmapped_import_target',
          source: sourceElem,
          target: imp,
          file: filepath,
          description: `Import '${imp}' resolves to '${resolved}' which is not mapped to any model element`,
        });
        continue;
      }

      // Check if relationship exists
      if (!relLookup.has(`${sourceElem}::${targetElem}`)) {
        violations.push({
          type: 'unmodeled_dependency',
          source: sourceElem,
          target: targetElem,
          file: filepath,
          description: `Element '${sourceElem}' imports '${targetElem}' (via '${imp}') but model has no '${sourceElem} -> ${targetElem}' relationship`,
        });
      }
    }
  }

  return { violations, warnings };
}

function detectUnusedRelationships(
  importsByFile: Record<string, string[]>,
  fileElementMap: Record<string, string | null>,
  pathToElement: Record<string, string>,
  relationships: C4Relation[],
): CrossRefViolation[] {
  const importedPairs = new Set<string>();
  for (const [filepath, importTargets] of Object.entries(importsByFile)) {
    const sourceElem = fileElementMap[filepath];
    if (!sourceElem) continue;
    for (const imp of importTargets) {
      const resolved = resolveImportToPath(filepath, imp);
      if (resolved === null) continue;
      const targetElem = matchPathToElement(resolved, pathToElement);
      if (targetElem && targetElem !== sourceElem) {
        importedPairs.add(`${sourceElem}::${targetElem}`);
      }
    }
  }

  const warnings: CrossRefViolation[] = [];
  for (const rel of relationships) {
    if (rel.source && rel.target) {
      if (!importedPairs.has(`${rel.source}::${rel.target}`)) {
        warnings.push({
          type: 'unused_relationship',
          source: rel.source,
          target: rel.target,
          file: '',
          description: `Model declares '${rel.source} -> ${rel.target}' but no import evidence found in changed files`,
        });
      }
    }
  }

  return warnings;
}

function checkPathNotFoundWarnings(
  elements: C4Element[],
  projectRoot: string,
): CrossRefViolation[] {
  const warnings: CrossRefViolation[] = [];

  for (const elem of elements) {
    for (const p of elem.paths) {
      const absPath = path.resolve(projectRoot, p.replace(/^\.\//, ''));
      if (!fs.existsSync(absPath)) {
        warnings.push({
          type: 'path_not_found',
          source: elem.name,
          target: p,
          file: '',
          description: `metadata.path '${p}' for element '${elem.name}' does not exist`,
        });
      }
    }
  }

  return warnings;
}

/**
 * Cross-reference imports against model relationships.
 */
function crossReference(
  importsByFile: Record<string, string[]>,
  fileElementMap: Record<string, string | null>,
  elements: C4Element[],
  relationships: C4Relation[],
  pathToElement: Record<string, string>,
  projectRoot: string,
): { violations: CrossRefViolation[]; warnings: CrossRefViolation[] } {
  const relLookup = buildRelationshipLookup(relationships);

  const { violations, warnings: importWarnings } = checkImportsAgainstRelationships(
    importsByFile,
    fileElementMap,
    pathToElement,
    relLookup,
  );

  const warnings: CrossRefViolation[] = [...importWarnings];
  warnings.push(
    ...detectUnusedRelationships(importsByFile, fileElementMap, pathToElement, relationships),
  );
  warnings.push(...checkPathNotFoundWarnings(elements, projectRoot));

  return { violations, warnings };
}

function emptyCrossRefResult(status: 'skipped' | 'no_changes'): ArchiCheckResult {
  return {
    violations: [],
    warnings: [],
    matched: [],
    unmatched_files: [],
    status,
  };
}

function getEarlyCrossRefResult(
  model: C4ParseResult,
  changedFiles?: string[],
): ArchiCheckResult | null {
  const hasError = model.errors.some(
    (e) => e.toLowerCase().includes('not found') || e.toLowerCase().includes('not exist'),
  );
  if (hasError) {
    return emptyCrossRefResult('skipped');
  }

  if (changedFiles !== undefined && changedFiles.length === 0) {
    return emptyCrossRefResult('no_changes');
  }

  return null;
}

function buildMatchedList(
  fileElementMap: Record<string, string | null>,
): Array<{ element_id: string; files: string[] }> {
  const seenElements = new Map<string, string[]>();
  for (const [filepath, elemName] of Object.entries(fileElementMap)) {
    if (elemName) {
      let files = seenElements.get(elemName);
      if (!files) {
        files = [];
        seenElements.set(elemName, files);
      }
      files.push(filepath);
    }
  }

  return Array.from(seenElements.entries()).map(([elementId, files]) => ({
    element_id: elementId,
    files,
  }));
}

/**
 * Run the full cross-reference check.
 * Returns an ArchiCheckResult with violations, warnings, and matched files.
 */
export async function runCrossRefCheck(
  projectRoot: string,
  options: { staged?: boolean; files?: string[] } = {},
): Promise<ArchiCheckResult> {
  const model = await loadModel(projectRoot);

  const skippedResult = getEarlyCrossRefResult(model);
  if (skippedResult) {
    return skippedResult;
  }

  const changedFiles = getChangedFiles(projectRoot, options);

  const noChangesResult = getEarlyCrossRefResult(model, changedFiles);
  if (noChangesResult) {
    return noChangesResult;
  }

  const fileElementMap = mapFilesToElements(changedFiles, model.path_to_element);

  const importsByFile: Record<string, string[]> = {};
  for (const filepath of changedFiles) {
    importsByFile[filepath] = parseImports(filepath, projectRoot);
  }

  const { violations, warnings } = crossReference(
    importsByFile,
    fileElementMap,
    model.elements,
    model.relationships,
    model.path_to_element,
    projectRoot,
  );

  const matched = buildMatchedList(fileElementMap);
  const unmatchedFiles = changedFiles.filter((f) => fileElementMap[f] === null);

  return {
    violations,
    warnings: warnings.map((w) => w.description),
    matched,
    unmatched_files: unmatchedFiles,
    status: violations.length > 0 ? 'violations_found' : 'clean',
  };
}
