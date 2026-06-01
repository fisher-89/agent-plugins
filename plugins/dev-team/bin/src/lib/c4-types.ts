/**
 * C4 type definitions for archi CLI commands.
 */

/**
 * Represents a parsed C4 element from DSL.
 */
export interface C4Element {
  kind: string;
  name: string;
  paths: string[];
  metadata: Record<string, string[]>;
}

/**
 * Represents a relationship between two C4 elements.
 */
export interface C4Relation {
  source: string;
  target: string;
  description: string | null;
}

/**
 * Result from parsing C4 DSL.
 */
export interface C4ParseResult {
  elements: C4Element[];
  relationships: C4Relation[];
  path_to_element: Record<string, string>;
  errors: string[];
}

/**
 * Result of dev-team archi query command.
 */
export interface ArchiQueryResult {
  element?: C4Element;
  relationships?: C4Relation[];
  elements?: C4Element[];
  error?: string;
}

/**
 * Result of dev-team archi validate command.
 */
export interface ArchiValidateResult {
  valid: boolean;
  error?: string;
  errors?: string[];
  warnings?: string[];
}

/**
 * Result of dev-team archi write command.
 */
export interface ArchiWriteResult {
  success: boolean;
  path?: string;
  error?: string;
  validation?: ArchiValidateResult;
}

/**
 * A cross-reference violation from archi check.
 */
export interface CrossRefViolation {
  type:
    | 'unmodeled_dependency'
    | 'unmapped_import_target'
    | 'unused_relationship'
    | 'path_not_found';
  source: string;
  target: string;
  file: string;
  description: string;
}

/**
 * Result of dev-team archi check command.
 */
export interface ArchiCheckResult {
  violations: CrossRefViolation[];
  warnings: string[];
  matched: { element_id: string; files: string[] }[];
  unmatched_files: string[];
  status: 'clean' | 'violations_found' | 'no_changes' | 'skipped';
}

/**
 * Element kind constants supported by the model.
 */
export const ELEMENT_KINDS = [
  'package',
  'domain',
  'module',
  'component',
  'softwareSystem',
  'container',
  'system',
  'person',
] as const;

/**
 * C4 element definition from a parsed DSL element line.
 */
export interface ParsedElementDef {
  kind: string;
  name: string;
  paths: string[];
  metadata: Record<string, string[]>;
}
