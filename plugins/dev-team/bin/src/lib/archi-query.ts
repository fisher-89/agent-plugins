/**
 * archi-query.ts — Query C4 model elements and relations.
 * Ported from Python archi-model.py --command query.
 */

import { readAllModels, parseC4Dsl } from './c4-parser';
import type { C4Element, C4Relation, ArchiQueryResult } from './c4-types';

/**
 * Query the model and return element structure.
 * If elementFqn is provided, filter to that specific element.
 */
export async function queryModel(
  projectRoot: string,
  elementFqn?: string,
): Promise<ArchiQueryResult> {
  const dsl = readAllModels(projectRoot);
  if (dsl === null) {
    return { error: 'No model files found' };
  }

  const parsed = await parseC4Dsl(dsl);

  if (elementFqn) {
    return filterElement(parsed.elements, parsed.relationships, elementFqn);
  }

  return {
    elements: parsed.elements,
    relationships: parsed.relationships,
  };
}

/**
 * Filter model result for a specific element FQN.
 */
function filterElement(
  elements: C4Element[],
  relationships: C4Relation[],
  fqn: string,
): ArchiQueryResult {
  const element = elements.find((e) => e.name === fqn);
  if (!element) {
    return { error: `Element '${fqn}' not found` };
  }

  const related = relationships.filter((r) => r.source === fqn || r.target === fqn);

  return { element, relationships: related };
}
