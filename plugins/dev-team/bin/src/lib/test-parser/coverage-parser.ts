// ---------------------------------------------------------------------------
// Coverage Parser
//
// Reads coverage output files in various formats and extracts lines, branches,
// and functions coverage percentages.
//
// Supported formats:
//   - istanbul   -- JSON summary (coverage-summary.json)
//   - llvm-cov   -- JSON output (data[0].totals.*.percent)
//   - node-test  -- Text table
//   - go-cover   -- func-summary.txt (total: line)
//   - coverage-py -- JSON coverage.json
//   - lcov       -- lcov.info (LF:/LH: records; bun)
// ---------------------------------------------------------------------------

import * as fs from 'fs';

import { z } from 'zod';

import { type FileCoverageEntry } from '../../schemas';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

type FileCoverage = { file: string } & FileCoverageEntry;

export interface ParsedCoverage {
  lines: number;
  branches: number | null;
  functions: number | null;
  fileCoverage: FileCoverage[] | null;
}

// ---------------------------------------------------------------------------
// Safe JSON parse helper (zod-based)
// ---------------------------------------------------------------------------

function safeParseJson<T>(json: string, schema: z.ZodType<T>): T | null {
  let parsed: unknown;
  try {
    parsed = JSON.parse(json);
  } catch {
    return null;
  }
  const result = schema.safeParse(parsed);
  return result.success ? result.data : null;
}

// ---------------------------------------------------------------------------
// Zod schemas
// ---------------------------------------------------------------------------

const pctMetricSchema = z.object({
  pct: z.number(),
});

const istanbulSummarySchema = z.object({
  total: z.object({
    lines: pctMetricSchema,
    branches: pctMetricSchema,
    functions: pctMetricSchema,
  }),
});

const llvmCovDataSchema = z.object({
  data: z
    .array(
      z.object({
        totals: z
          .object({
            lines: z.object({ percent: z.number() }).optional(),
            branches: z.object({ percent: z.number() }).optional(),
            functions: z.object({ percent: z.number() }).optional(),
          })
          .optional(),
      }),
    )
    .nonempty(),
});

const coveragePyDataSchema = z.object({
  totals: z
    .object({
      percent_covered: z.number(),
      percent_branches_covered: z.number().optional(),
    })
    .optional(),
});

/** Per-file metric entry in istanbul summary (pct is required; total/covered optional). */
const fileMetricSchema = z.object({
  total: z.number().optional(),
  covered: z.number().optional(),
  pct: z.number(),
});

const fileEntrySchema = z.object({
  lines: fileMetricSchema,
  branches: fileMetricSchema.optional(),
  functions: fileMetricSchema.optional(),
});

// ---------------------------------------------------------------------------
// Istanbul parser
// ---------------------------------------------------------------------------

function parseIstanbul(content: string): ParsedCoverage | null {
  // Parse full JSON with permissive record — we validate total and per-file entries separately
  let raw: unknown;
  try {
    raw = JSON.parse(content);
  } catch {
    return null;
  }

  const totalResult = istanbulSummarySchema.safeParse(raw);
  if (!totalResult.success) return null;

  const data = totalResult.data;
  const fileCoverage: FileCoverage[] = [];

  // raw is already parsed — iterate entries with zod validation
  const rawRecord = z.record(z.string(), z.unknown()).safeParse(raw);
  if (rawRecord.success) {
    for (const [file, metrics] of Object.entries(rawRecord.data)) {
      if (file === 'total') continue;

      const parsed = fileEntrySchema.safeParse(metrics);
      if (!parsed.success) continue;

      const m = parsed.data;
      fileCoverage.push({
        file,
        lines: m.lines.pct,
        branches: m.branches?.pct ?? null,
        functions: m.functions?.pct ?? null,
        total_lines: m.lines.total ?? 0,
        covered_lines: m.lines.covered ?? 0,
        total_branches: m.branches?.total ?? null,
        covered_branches: m.branches?.covered ?? null,
        total_functions: m.functions?.total ?? null,
        covered_functions: m.functions?.covered ?? null,
      });
    }
  }

  return {
    lines: data.total.lines.pct,
    branches: data.total.branches.pct ?? null,
    functions: data.total.functions.pct ?? null,
    fileCoverage: fileCoverage.length > 0 ? fileCoverage : null,
  };
}

// ---------------------------------------------------------------------------
// LLVM-cov parser
// ---------------------------------------------------------------------------

function parseLlvmCov(content: string): ParsedCoverage | null {
  const data = safeParseJson(content, llvmCovDataSchema);
  if (!data) return null;

  const totals = data.data[0].totals;
  if (!totals) return null;

  return {
    lines: totals.lines?.percent ?? 0,
    branches: totals.branches?.percent ?? null,
    functions: totals.functions?.percent ?? null,
    fileCoverage: null,
  };
}

// ---------------------------------------------------------------------------
// Node-test parser (text table)
// ---------------------------------------------------------------------------

function parseNodeTest(content: string): ParsedCoverage | null {
  const lines = content.split('\n');

  for (const line of lines) {
    const trimmed = line.trim();
    // Node-test table format: | All files | lines_pct | branches_pct | functions_pct |
    // The line starts with | so we use includes() instead of startsWith()
    if (!trimmed.includes('All files')) continue;

    const columns = trimmed.split('|').map((c) => c.trim());
    if (columns.length < 4) continue;

    // columns[0] is empty (from leading |), columns[1] is "All files" (file name)
    // columns[2] is lines %, columns[3] (if present) is branches %, columns[4] (if present) is functions %
    const pctLines = parsePct(columns[2]);
    const pctBranches =
      columns.length > 3 && columns[3] && columns[3] !== '-' ? parsePct(columns[3]) : null;
    const pctFunctions =
      columns.length > 4 && columns[4] && columns[4] !== '-' ? parsePct(columns[4]) : null;

    if (pctLines === null) continue;

    return {
      lines: pctLines,
      branches: pctBranches,
      functions: pctFunctions,
      fileCoverage: null,
    };
  }

  return null;
}

// ---------------------------------------------------------------------------
// Go-cover parser (func-summary.txt)
// ---------------------------------------------------------------------------

function parseGoCover(content: string): ParsedCoverage | null {
  const lines = content.split('\n');

  for (const line of lines) {
    const trimmed = line.trim();
    if (!trimmed.startsWith('total')) continue;

    const match = trimmed.match(/(\d+\.?\d*)%\s*$/);
    if (match) {
      const pct = parseFloat(match[1]);
      return {
        lines: pct,
        branches: null,
        functions: null,
        fileCoverage: null,
      };
    }
  }

  return null;
}

// ---------------------------------------------------------------------------
// Coverage-py parser (coverage.json)
// ---------------------------------------------------------------------------

function parseCoveragePy(content: string): ParsedCoverage | null {
  const data = safeParseJson(content, coveragePyDataSchema);
  if (!data?.totals) return null;

  return {
    lines: data.totals.percent_covered ?? 0,
    branches: data.totals.percent_branches_covered ?? null,
    functions: null,
    fileCoverage: null,
  };
}

// ---------------------------------------------------------------------------
// LCOV parser (lcov.info)
// ---------------------------------------------------------------------------

interface LcovTotals {
  totalLines: number;
  coveredLines: number;
  totalBranches: number;
  coveredBranches: number;
  totalFunctions: number;
  coveredFunctions: number;
  sawRecord: boolean;
}

function accumulateLcovTotals(content: string): LcovTotals {
  const totals: LcovTotals = {
    totalLines: 0,
    coveredLines: 0,
    totalBranches: 0,
    coveredBranches: 0,
    totalFunctions: 0,
    coveredFunctions: 0,
    sawRecord: false,
  };

  for (const rawLine of content.split('\n')) {
    const line = rawLine.trim();
    if (line.startsWith('LF:')) {
      totals.totalLines += parseInt(line.slice(3), 10) || 0;
      totals.sawRecord = true;
    } else if (line.startsWith('LH:')) {
      totals.coveredLines += parseInt(line.slice(3), 10) || 0;
      totals.sawRecord = true;
    } else if (line.startsWith('BRF:')) {
      totals.totalBranches += parseInt(line.slice(4), 10) || 0;
    } else if (line.startsWith('BRH:')) {
      totals.coveredBranches += parseInt(line.slice(4), 10) || 0;
    } else if (line.startsWith('FNF:')) {
      totals.totalFunctions += parseInt(line.slice(4), 10) || 0;
    } else if (line.startsWith('FNH:')) {
      totals.coveredFunctions += parseInt(line.slice(4), 10) || 0;
    }
  }
  return totals;
}

/** Some lcov emitters only list DA: hits without LF/LH — count DA lines. */
function fallbackLcovFromDa(content: string): { totalLines: number; coveredLines: number } | null {
  const daLines = content.split('\n').filter((l) => l.trim().startsWith('DA:'));
  if (daLines.length === 0) return null;
  return {
    totalLines: daLines.length,
    coveredLines: daLines.filter((l) => {
      const parts = l.trim().slice(3).split(',');
      return (parseInt(parts[1] ?? '0', 10) || 0) > 0;
    }).length,
  };
}

/**
 * Parse lcov.info into ParsedCoverage.
 *
 * Aggregates LF (lines found) / LH (lines hit) across all SF records.
 * Branch and function percentages are derived from BRF/BRH and FNF/FNH when present.
 */
function parseLcov(content: string): ParsedCoverage | null {
  if (!content || content.trim().length === 0) return null;

  const totals = accumulateLcovTotals(content);
  let { totalLines, coveredLines } = totals;

  if (!totals.sawRecord || totalLines <= 0) {
    if (!totals.sawRecord) {
      const da = fallbackLcovFromDa(content);
      if (!da) return null;
      totalLines = da.totalLines;
      coveredLines = da.coveredLines;
    } else {
      return null;
    }
  }

  const pct = (covered: number, total: number): number | null =>
    total > 0 ? Math.round((covered / total) * 10000) / 100 : null;

  return {
    lines: pct(coveredLines, totalLines) ?? 0,
    branches: pct(totals.coveredBranches, totals.totalBranches),
    functions: pct(totals.coveredFunctions, totals.totalFunctions),
    fileCoverage: null,
  };
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function parsePct(value: string): number | null {
  const cleaned = value.replace(/%/g, '').trim();
  const num = parseFloat(cleaned);
  return isNaN(num) ? null : num;
}

// ---------------------------------------------------------------------------
// Main entry
// ---------------------------------------------------------------------------

export function parseCoverageFromFile(filePath: string, format: string): ParsedCoverage | null {
  let content: string;
  try {
    content = fs.readFileSync(filePath, 'utf-8');
  } catch {
    return null;
  }

  switch (format) {
    case 'istanbul':
      return parseIstanbul(content);
    case 'llvm-cov':
      return parseLlvmCov(content);
    case 'node-test':
      return parseNodeTest(content);
    case 'go-cover':
      return parseGoCover(content);
    case 'coverage-py':
      return parseCoveragePy(content);
    case 'lcov':
      return parseLcov(content);
    default:
      return null;
  }
}
