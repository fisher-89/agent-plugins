/**
 * archi-write.ts — Validate and write C4 DSL to a model file.
 * Ported from Python archi-model.py --command write.
 */

import * as fs from 'fs';
import * as path from 'path';

import { validateDsl } from './archi-validate';
import type { ArchiWriteResult } from './c4-types';

const ARCHITECTURE_DIR = 'openspec/specs/architecture';
const MODELS_DIR = path.join(ARCHITECTURE_DIR, 'models');

/**
 * Write DSL text to a specific file in models/ after validation.
 * Returns { success: true, path } on success, or { success: false, error } on failure.
 */
export async function writeDsl(
  projectRoot: string,
  dslText: string,
  targetPath: string,
): Promise<ArchiWriteResult> {
  const modelsDir = path.resolve(projectRoot, MODELS_DIR);
  const fullPath = path.resolve(modelsDir, targetPath);

  // Security check: path must be inside models/ directory
  const normalizedFull = path.normalize(fullPath);
  const normalizedModels = path.normalize(modelsDir);
  if (!normalizedFull.startsWith(normalizedModels)) {
    return {
      success: false,
      error: `Path '${targetPath}' is outside models/ directory`,
    };
  }

  // Validate before writing
  const validation = await validateDsl(projectRoot, dslText);
  if (!validation.valid) {
    return {
      success: false,
      error: validation.error || validation.errors?.join('; ') || 'Validation failed',
      validation,
    };
  }

  // Ensure models directory exists
  fs.mkdirSync(modelsDir, { recursive: true });

  // Write file
  fs.writeFileSync(fullPath, dslText, 'utf-8');

  return { success: true, path: fullPath };
}
