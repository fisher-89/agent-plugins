/**
 * C4 DSL Parser — Parses .c4 files via LikeC4.fromSources().
 *
 * The preprocessor converts double-quoted relationship descriptions to single-quoted
 * (LikeC4 requires single quotes). Everything else is already LikeC4-compatible.
 */

import * as fs from 'node:fs';
import * as path from 'node:path';

import { fromSource } from '@likec4/language-services';

import { type C4ParseResult } from './c4-types';

// ---------------------------------------------------------------------------
// Preprocessor — minimal: convert "desc" to 'desc' in relationships
// ---------------------------------------------------------------------------

function preprocessForLikeC4(dsl: string): string {
  // Convert relationship descriptions: A -> B "desc" → A -> B 'desc'
  return dsl.replace(/^(\s*\S[\S\s]*?\s*->\s*\S+)\s+"([^"]+)"\s*$/gm, "$1 '$2'");
}

// ---------------------------------------------------------------------------
// Public API — file management
// ---------------------------------------------------------------------------

const ARCHITECTURE_DIR = 'openspec/specs/architecture';
const MODELS_DIR = path.join(ARCHITECTURE_DIR, 'models');

export function getModelFiles(projectRoot: string): { filename: string; filepath: string }[] {
  const modelsDir = path.resolve(projectRoot, MODELS_DIR);
  if (!fs.existsSync(modelsDir)) return [];
  return fs
    .readdirSync(modelsDir)
    .filter((f) => f.endsWith('.c4'))
    .sort()
    .map((f) => ({ filename: f, filepath: path.join(modelsDir, f) }));
}

export function readAllModels(projectRoot: string): string | null {
  const files = getModelFiles(projectRoot);
  if (files.length === 0) return null;

  const contents = files.map((f) => fs.readFileSync(f.filepath, 'utf-8'));

  // If only one file, return as-is
  if (contents.length === 1) return contents[0];

  // Merge multiple files: extract specification from first file,
  // and merge model blocks from all files into a single model { } block
  let spec = '';
  const modelContents: string[] = [];

  for (const content of contents) {
    // Extract specification block (only from first file that has one)
    if (!spec) {
      const specMatch = content.match(/(specification\s*\{[\s\S]*?\n\})/);
      if (specMatch) spec = specMatch[1];
    }
    // Extract inner content of model { } blocks
    const modelMatch = content.match(/\bmodel\s*\{/);
    if (modelMatch && modelMatch.index !== undefined) {
      const inner = content.slice(modelMatch.index + modelMatch[0].length);
      let depth = 1;
      let i = 0;
      for (; i < inner.length && depth > 0; i++) {
        if (inner[i] === '{') depth++;
        if (inner[i] === '}') depth--;
      }
      modelContents.push(inner.slice(0, i - 1).trim());
    } else if (content.trim()) {
      // Content without model block — treat as model body (for single-file cases)
      modelContents.push(content.trim());
    }
  }

  return (spec ? spec + '\n' : '') + 'model {\n' + modelContents.join('\n\n') + '\n}';
}

export function findSpecificationBlock(projectRoot: string): string | null {
  const files = getModelFiles(projectRoot);
  for (const f of files) {
    const content = fs.readFileSync(f.filepath, 'utf-8');
    const specMatch = content.match(/specification\s*\{[\s\S]*?\n\}/);
    if (specMatch) return specMatch[0];
  }
  return null;
}

// ---------------------------------------------------------------------------
// Public API — parsing (delegates to likec4)
// ---------------------------------------------------------------------------

export async function parseC4Dsl(dslText: string): Promise<C4ParseResult> {
  const result: C4ParseResult = {
    elements: [],
    relationships: [],
    path_to_element: {},
    errors: [],
  };

  if (!dslText.trim()) return result;

  const preprocessed = preprocessForLikeC4(dslText);

  try {
    const likec4 = await fromSource(preprocessed);
    // Only keep parse-level errors (not model validation warnings)
    const allErrors = likec4.getErrors().map((e: { message: string }) => e.message);
    result.errors = allErrors.filter(
      (m: string) => m.startsWith('Expecting') || m.includes('Unexpected') || m.includes('token'),
    );

    const model = likec4.syncComputedModel();

    for (const el of model.elements()) {
      const paths: string[] = [];
      const metadata: Record<string, string[]> = {};
      if (el.metadata) {
        for (const [key, val] of Object.entries(el.metadata)) {
          const arr = Array.isArray(val) ? val : [String(val)];
          metadata[key] = arr;
          if (key === 'path') paths.push(...arr);
        }
      }

      result.elements.push({
        kind: String(el.kind),
        name: String(el.id),
        paths,
        metadata,
      });
    }

    for (const rel of model.relationships()) {
      result.relationships.push({
        source: String(rel.source.id),
        target: String(rel.target.id),
        description: rel.title ?? null,
      });
    }

    for (const el of result.elements) {
      for (const p of el.paths) {
        result.path_to_element[normalizePath(p)] = el.name;
      }
    }

    await likec4.dispose();
  } catch (err) {
    result.errors.push(`LikeC4 parse error: ${err instanceof Error ? err.message : String(err)}`);
  }

  return result;
}

export async function validateC4Dsl(
  dslText: string,
  projectRoot?: string,
): Promise<C4ParseResult & { valid: boolean }> {
  const errors: string[] = [];

  if (!dslText.includes('specification')) {
    errors.push("Missing 'specification' block");
  }

  const preprocessed = preprocessForLikeC4(dslText);

  try {
    const likec4 = await fromSource(preprocessed);
    // Only keep parse-level errors (not model validation warnings)
    errors.push(
      ...likec4
        .getErrors()
        .map((e: { message: string }) => e.message)
        .filter(
          (m: string) =>
            m.startsWith('Expecting') || m.includes('Unexpected') || m.includes('token'),
        ),
    );

    if (projectRoot) {
      const files = getModelFiles(projectRoot);
      if (files.length > 1) {
        const specFiles = files.filter((f) =>
          fs.readFileSync(f.filepath, 'utf-8').includes('specification'),
        );
        if (specFiles.length > 1) {
          errors.push(
            `Duplicate 'specification' blocks found in files: ${specFiles.map((f) => f.filename).join(', ')}. ` +
              'Only one file may contain a specification block.',
          );
        }
      }
    }

    await likec4.dispose();
  } catch (err: unknown) {
    errors.push(`LikeC4 parse error: ${err instanceof Error ? err.message : String(err)}`);
  }

  const parse = await parseC4Dsl(dslText);

  return {
    ...parse,
    valid: errors.length === 0,
    errors: [...errors, ...parse.errors],
  };
}

function normalizePath(p: string): string {
  let s = p.trim();
  if (s.startsWith('./')) s = s.slice(2);
  return s.replace(/[/\\]$/, '');
}
