import * as fs from 'fs';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface CoverageResult {
  lines: number;
  branches: number;
  functions: number;
}

type CoverageFormat = 'istanbul' | 'llvm-cov';

// ---------------------------------------------------------------------------
// Istanbul format parser (coverage-summary.json)
// ---------------------------------------------------------------------------

interface IstanbulSummary {
  total: {
    lines?: { pct: number };
    branches?: { pct: number };
    functions?: { pct: number };
  };
}

/**
 * Parse an Istanbul-format coverage summary JSON file.
 *
 * Expected structure:
 * ```json
 * {
 *   "total": {
 *     "lines": { "total": 100, "covered": 85, "pct": 85 },
 *     "branches": { "total": 50, "covered": 37, "pct": 74 },
 *     "functions": { "total": 30, "covered": 24, "pct": 80 }
 *   }
 * }
 * ```
 *
 * Returns null if any required dimension is missing or invalid.
 */
function parseIstanbul(data: unknown): CoverageResult | null {
  if (!data || typeof data !== 'object') {
    return null;
  }

  // eslint-disable-next-line typescript/no-unsafe-type-assertion -- narrowing from unknown
  const obj = data as Record<string, unknown>;
  // eslint-disable-next-line typescript/no-unsafe-type-assertion -- narrowing from unknown
  const total = obj.total as IstanbulSummary['total'] | undefined;

  if (!total || typeof total !== 'object') {
    return null;
  }

  const lines = total.lines?.pct;
  const branches = total.branches?.pct;
  const functions = total.functions?.pct;

  if (lines == null || branches == null || functions == null) {
    return null;
  }

  return {
    lines: coerceNumber(lines),
    branches: coerceNumber(branches),
    functions: coerceNumber(functions),
  };
}

// ---------------------------------------------------------------------------
// llvm-cov format parser
// ---------------------------------------------------------------------------

interface LlvmCovOutput {
  data?: Array<{
    totals?: {
      lines?: { percent?: number };
      branches?: { percent?: number };
      functions?: { percent?: number };
    };
  }>;
}

/**
 * Parse a llvm-cov JSON output file.
 *
 * Expected structure:
 * ```json
 * {
 *   "data": [
 *     {
 *       "totals": {
 *         "lines": { "percent": 90, "count": 100 },
 *         "branches": { "percent": 80, "count": 50 },
 *         "functions": { "percent": 85, "count": 30 }
 *       }
 *     }
 *   ]
 * }
 * ```
 *
 * Returns null if the data array is empty or any required dimension is
 * missing.
 */
function parseLlvmCov(data: unknown): CoverageResult | null {
  if (!data || typeof data !== 'object') {
    return null;
  }

  const obj = data as LlvmCovOutput;
  if (!obj.data || obj.data.length === 0 || !obj.data[0]) {
    return null;
  }

  const totals = obj.data[0].totals;
  if (!totals) {
    return null;
  }

  const lines = totals.lines?.percent;
  const branches = totals.branches?.percent;
  const functions = totals.functions?.percent;

  if (lines == null || branches == null || functions == null) {
    return null;
  }

  return {
    lines: coerceNumber(lines),
    branches: coerceNumber(branches),
    functions: coerceNumber(functions),
  };
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/**
 * Coerce a value to a number.  If the value is a numeric string (e.g. "85"),
 * parse it.  If it is already a number, return it as-is.
 * Returns NaN for non-numeric values.
 */
function coerceNumber(value: unknown): number {
  if (typeof value === 'number') {
    return value;
  }
  if (typeof value === 'string') {
    const parsed = Number(value);
    return isNaN(parsed) ? NaN : parsed;
  }
  return NaN;
}

// ---------------------------------------------------------------------------
// Main entry point
// ---------------------------------------------------------------------------

/**
 * Parse a coverage output file and return the three-dimensional coverage
 * percentages (lines, branches, functions).
 *
 * @param filePath - Absolute path to the coverage output file.
 * @param format   - Coverage output format identifier.
 * @returns        - Coverage result, or null if the file cannot be read,
 *                   the JSON is malformed, or the expected structure is
 *                   missing.
 */
export function parseCoverageOutput(
  filePath: string,
  format: CoverageFormat,
): CoverageResult | null {
  if (!filePath || !format) {
    return null;
  }

  // Read the file
  let raw: string;
  try {
    raw = fs.readFileSync(filePath, 'utf-8');
  } catch {
    return null;
  }

  if (!raw || raw.trim().length === 0) {
    return null;
  }

  // Parse JSON
  let parsed: unknown;
  try {
    parsed = JSON.parse(raw);
  } catch {
    return null;
  }

  // Delegate to format-specific parser
  switch (format) {
    case 'istanbul':
      return parseIstanbul(parsed);
    case 'llvm-cov':
      return parseLlvmCov(parsed);
    default:
      return null;
  }
}
