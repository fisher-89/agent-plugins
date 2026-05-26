/**
 * archi-validate.ts — Validate C4 DSL syntax.
 * Ported from Python archi-model.py --command validate.
 */

import { readAllModels, validateC4Dsl, findSpecificationBlock } from "./c4-parser";
import type { ArchiValidateResult } from "./c4-types";

/**
 * Validate DSL syntax.
 * If dslText is provided, validate that. Otherwise validate current model.
 * When validating a single file that lacks a specification block,
 * the spec is automatically prepended from the existing model files.
 */
export async function validateDsl(
  projectRoot: string,
  dslText?: string,
): Promise<ArchiValidateResult> {
  let sourceText: string | null;

  if (dslText !== undefined) {
    sourceText = dslText;
    // If source lacks a specification block, prepend it from existing model files
    if (!sourceText.includes("specification")) {
      const specBlock = findSpecificationBlock(projectRoot);
      if (specBlock) {
        sourceText = specBlock + "\n" + sourceText;
      }
    }
  } else {
    sourceText = readAllModels(projectRoot);
    if (sourceText === null) {
      return { valid: false, error: "No model files found" };
    }
  }

  const result = await validateC4Dsl(sourceText, dslText !== undefined ? undefined : projectRoot);
  return {
    valid: result.valid,
    errors: result.errors,
    warnings: [],
  };
}
