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
// ---------------------------------------------------------------------------

/* eslint-disable @typescript-eslint/no-unsafe-type-assertion, @typescript-eslint/no-non-null-assertion */

import * as fs from 'fs';

import { type FileCoverageEntry } from '../../schemas';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface ParsedCoverage {
  lines: number;
  branches: number | null;
  functions: number | null;
  fileCoverage: FileCoverageEntry[] | null;
}

// ---------------------------------------------------------------------------
// Safe JSON parse helper
// ---------------------------------------------------------------------------

function safeParse<T>(json: string, guard: (data: unknown) => data is T): T | null {
  let parsed: unknown;
  try {
    parsed = JSON.parse(json);
  } catch {
    return null;
  }
  return guard(parsed) ? parsed : null;
}

// ---------------------------------------------------------------------------
// Type guards
// ---------------------------------------------------------------------------

interface IstanbulSummary {
  total: {
    lines: { pct: number };
    branches: { pct: number };
    functions: { pct: number };
  };
  [key: string]: unknown;
}

function isIstanbulSummary(data: unknown): data is IstanbulSummary {
  if (!data || typeof data !== 'object') return false;
  const d = data as Record<string, unknown>;
  const total = d.total;
  if (!total || typeof total !== 'object') return false;
  const t = total as Record<string, unknown>;
  return (
    typeof t.lines === 'object' &&
    t.lines !== null &&
    typeof (t.lines as Record<string, unknown>).pct === 'number' &&
    typeof t.branches === 'object' &&
    t.branches !== null &&
    typeof (t.branches as Record<string, unknown>).pct === 'number' &&
    typeof t.functions === 'object' &&
    t.functions !== null &&
    typeof (t.functions as Record<string, unknown>).pct === 'number'
  );
}

interface LlvmCovData {
  data?: Array<{
    totals?: {
      lines?: { percent: number };
      branches?: { percent: number };
      functions?: { percent: number };
    };
  }>;
}

function isLlvmCovData(data: unknown): data is LlvmCovData {
  if (!data || typeof data !== 'object') return false;
  const d = data as Record<string, unknown>;
  const dataArr = d.data;
  if (!Array.isArray(dataArr) || dataArr.length === 0) return false;
  const first = dataArr[0];
  if (!first || typeof first !== 'object') return false;
  const totals = (first as Record<string, unknown>).totals;
  if (!totals || typeof totals !== 'object') return false;
  const lines = (totals as Record<string, unknown>).lines;
  return typeof lines === 'object' && lines !== null;
}

interface CoveragePyData {
  totals?: {
    percent_covered?: number;
    percent_covered_branches?: number;
  };
}

function isCoveragePyData(data: unknown): data is CoveragePyData {
  if (!data || typeof data !== 'object') return false;
  const d = data as Record<string, unknown>;
  const totals = d.totals;
  if (!totals || typeof totals !== 'object') return false;
  return typeof (totals as Record<string, unknown>).percent_covered === 'number';
}

// ---------------------------------------------------------------------------
// Istanbul parser
// ---------------------------------------------------------------------------

function parseIstanbul(content: string): ParsedCoverage | null {
  const data = safeParse(content, isIstanbulSummary);
  if (!data) return null;

  const fileCoverage: FileCoverageEntry[] = [];
  for (const [file, metrics] of Object.entries(data)) {
    if (file === 'total') continue;
    const m = metrics as Record<string, unknown>;
    const lines = m.lines as { pct: number } | undefined;
    if (!lines || typeof lines.pct !== 'number') continue;
    fileCoverage.push({
      file,
      lines: lines.pct,
      branches: (m.branches as { pct: number } | undefined)?.pct ?? null,
      functions: (m.functions as { pct: number } | undefined)?.pct ?? null,
    });
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
  const data = safeParse(content, isLlvmCovData);
  if (!data) return null;

  const totals = data.data![0].totals;
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
  const data = safeParse(content, isCoveragePyData);
  if (!data?.totals) return null;

  return {
    lines: data.totals.percent_covered ?? 0,
    branches: data.totals.percent_covered_branches ?? null,
    functions: null,
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
    default:
      return null;
  }
}
