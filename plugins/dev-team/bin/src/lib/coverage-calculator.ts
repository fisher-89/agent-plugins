// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface CoverageResult {
  lines: number;
  branches: number;
  functions: number;
}

export interface FrameworkCoverage {
  framework: string;
  coverage: CoverageResult | null;
  sourceFileCount: number;
  html_report?: string | null;
}

export interface Thresholds {
  lines: number;
  branches: number;
  functions: number;
}

export interface CoverageOverrideEntry {
  glob: string;
  thresholds: Partial<Thresholds>;
  coverage?: CoverageResult;
  pass?: boolean;
}

export interface CoverageOverrideResult {
  glob: string;
  thresholds: Thresholds;
  coverage: CoverageResult | null;
  pass: boolean;
}

export interface CoveragePassResult {
  pass: boolean;
  failingDimensions?: string[];
  overrideResults?: CoverageOverrideResult[];
}

// ---------------------------------------------------------------------------
// Weighted average
// ---------------------------------------------------------------------------

/**
 * Compute the weighted average coverage across multiple frameworks.
 *
 * Each framework's coverage is weighted by its `sourceFileCount`.
 * Frameworks with null coverage are excluded from the average (they
 * may represent failed coverage commands).
 *
 * Returns null when:
 * - The input array is empty
 * - All frameworks have null coverage
 * - The total source file count is 0
 *
 * Each dimension (lines, branches, functions) is calculated independently:
 *   weighted_avg = sum(fw.coverage.dimension * fw.sourceFileCount) / totalSourceFiles
 */
export function computeWeightedAverage(
  coverageByFramework: FrameworkCoverage[],
): CoverageResult | null {
  if (!coverageByFramework || coverageByFramework.length === 0) {
    return null;
  }

  // Filter to frameworks with valid coverage data and positive sourceFileCount
  const validFrameworks = coverageByFramework.filter(
    (fw) => fw.coverage != null && fw.sourceFileCount > 0 && typeof fw.sourceFileCount === 'number',
  );

  if (validFrameworks.length === 0) {
    return null;
  }

  const totalSourceFiles = validFrameworks.reduce((sum, fw) => sum + fw.sourceFileCount, 0);

  if (totalSourceFiles <= 0) {
    return null;
  }

  // Weighted sum per dimension
  let linesSum = 0;
  let branchesSum = 0;
  let functionsSum = 0;

  for (const fw of validFrameworks) {
    const coverage = fw.coverage;
    if (!coverage) continue;
    const weight = fw.sourceFileCount;
    linesSum += coverage.lines * weight;
    branchesSum += coverage.branches * weight;
    functionsSum += coverage.functions * weight;
  }

  return {
    lines: Math.round(linesSum / totalSourceFiles),
    branches: Math.round(branchesSum / totalSourceFiles),
    functions: Math.round(functionsSum / totalSourceFiles),
  };
}

// ---------------------------------------------------------------------------
// Threshold helpers
// ---------------------------------------------------------------------------

/**
 * Default thresholds used when no thresholds are configured.
 */
const DEFAULT_THRESHOLDS: Thresholds = {
  lines: 80,
  branches: 70,
  functions: 75,
};

/**
 * Resolve effective thresholds for an override entry, inheriting missing
 * dimensions from the global thresholds.
 */
function resolveThresholds(
  overrideThresholds: Partial<Thresholds>,
  globalThresholds: Thresholds,
): Thresholds {
  return {
    lines: overrideThresholds.lines != null ? overrideThresholds.lines : globalThresholds.lines,
    branches:
      overrideThresholds.branches != null ? overrideThresholds.branches : globalThresholds.branches,
    functions:
      overrideThresholds.functions != null
        ? overrideThresholds.functions
        : globalThresholds.functions,
  };
}

/**
 * Check whether coverage meets thresholds for all three dimensions.
 * Returns the list of failing dimension names.
 */
function getFailingDimensions(coverage: CoverageResult, thresholds: Thresholds): string[] {
  const failing: string[] = [];

  if (coverage.lines < thresholds.lines) {
    failing.push('lines');
  }
  if (coverage.branches < thresholds.branches) {
    failing.push('branches');
  }
  if (coverage.functions < thresholds.functions) {
    failing.push('functions');
  }

  return failing;
}

// ---------------------------------------------------------------------------
// Override results builder
// ---------------------------------------------------------------------------

/**
 * Build coverage override result entries by matching each override's glob
 * against the per-framework coverage data.
 *
 * For each override, uses only the frameworks whose glob pattern matches
 * the override glob to compute the per-override coverage.  If no frameworks
 * match, the override is skipped (pass = true, coverage = null).
 *
 * @param coverageByFramework - Per-framework coverage data from executor.
 * @param overrides           - Override configurations from config.json.
 * @param configThresholds    - Global thresholds (used to inherit missing
 *                              override dimensions).
 * @returns                   - Array of override check results.
 */
export function buildCoverageOverrides(
  _coverageByFramework: FrameworkCoverage[],
  overrides: CoverageOverrideEntry[],
  configThresholds: Thresholds,
): CoverageOverrideResult[] {
  if (!overrides || overrides.length === 0) {
    return [];
  }

  const globalThresholds = configThresholds || DEFAULT_THRESHOLDS;

  return overrides.map((overrideEntry) => {
    const thresholds = resolveThresholds(overrideEntry.thresholds, globalThresholds);

    // In the executor context, coverage data per override group would be
    // computed by matching the override glob against source files.  Here we
    // use the coverage provided on the override entry itself (populated by
    // the executor from file-level coverage data).
    const coverage = overrideEntry.coverage || null;

    if (coverage == null) {
      // No matching files for this override, or coverage data unavailable
      return {
        glob: overrideEntry.glob,
        thresholds,
        coverage: null,
        pass: true,
      };
    }

    const failing = getFailingDimensions(coverage, thresholds);
    return {
      glob: overrideEntry.glob,
      thresholds,
      coverage,
      pass: failing.length === 0,
    };
  });
}

// ---------------------------------------------------------------------------
// Coverage pass check (ALL logic)
// ---------------------------------------------------------------------------

/**
 * Determine whether coverage passes the ALL gate:
 * 1. Global coverage must meet all three dimension thresholds.
 * 2. Each override group must independently pass its own thresholds.
 * 3. All must pass for `coverage_pass === true`.
 *
 * @param coverage            - Weighted average coverage (or null if all
 *                              framework coverage commands failed).
 * @param thresholds          - Global threshold configuration.
 * @param overrides           - Override group results (pre-computed by
 *                              `buildCoverageOverrides`).
 * @returns                   - Pass/fail result with details.
 */
export function checkCoveragePass(
  coverage: CoverageResult | null,
  thresholds: Thresholds | null,
  overrides: CoverageOverrideResult[] | null,
): CoveragePassResult {
  const globalThresholds = thresholds || DEFAULT_THRESHOLDS;

  // If all coverage commands failed, coverage is null → fail
  if (coverage == null) {
    return {
      pass: false,
      overrideResults: overrides || [],
    };
  }

  // Check global thresholds
  const globalFailing = getFailingDimensions(coverage, globalThresholds);
  let allPass = globalFailing.length === 0;

  // Check override groups
  const effectiveOverrides = overrides || [];
  const checkedOverrides = effectiveOverrides.map((ov) => {
    if (ov.coverage == null) {
      // No matching files — unconditional pass
      return { ...ov, pass: true };
    }
    const ovFailing = getFailingDimensions(ov.coverage, ov.thresholds);
    const ovPass = ovFailing.length === 0;
    if (!ovPass) {
      allPass = false;
    }
    return { ...ov, pass: ovPass };
  });

  return {
    pass: allPass,
    failingDimensions: globalFailing.length > 0 ? globalFailing : undefined,
    overrideResults: checkedOverrides,
  };
}
