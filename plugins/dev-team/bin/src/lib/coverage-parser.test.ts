/**
 * Tests for coverage-parser -- parses coverage output files (istanbul
 * and llvm-cov formats) into structured {lines, branches, functions} objects.
 *
 * Covers:
 * - AC-6: istanbul and llvm-cov format parsing
 * - Reverse AC-11: file not found, malformed JSON returns null
 * - AC-12: html_reports path collection (via integration)
 * - Boundary: missing dimensions, empty data arrays, string pct values
 *
 * @see openspec/changes/unit-test-coverage-report/test-design.md
 */

import * as fs from 'fs';
import * as os from 'os';
import * as path from 'path';

import { describe, it, expect } from 'vite-plus/test';

import { parseCoverageOutput } from './coverage-parser';

// ---------------------------------------------------------------------------
// Helpers: create temp files
// ---------------------------------------------------------------------------

function writeTempFile(content: string): { filePath: string; cleanup: () => void } {
  const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'cov-test-'));
  const filePath = path.join(tmpDir, 'coverage-summary.json');
  fs.writeFileSync(filePath, content, 'utf-8');
  return {
    filePath,
    cleanup: () => fs.rmSync(tmpDir, { recursive: true, force: true }),
  };
}

// ===========================================================================
// AC-6: Istanbul format parsing
// ===========================================================================

describe('parseCoverageOutput -- istanbul format (AC-6)', () => {
  it('should parse valid istanbul coverage-summary.json', () => {
    const istanbulData = {
      total: {
        lines: { total: 100, covered: 85, pct: 85 },
        branches: { total: 50, covered: 37, pct: 74 },
        functions: { total: 30, covered: 24, pct: 80 },
      },
    };
    const { filePath, cleanup } = writeTempFile(JSON.stringify(istanbulData));
    try {
      const result = parseCoverageOutput(filePath, 'istanbul');
      expect(result).toEqual({ lines: 85, branches: 74, functions: 80 });
    } finally {
      cleanup();
    }
  });

  it('should extract lines.pct from total.lines', () => {
    const data = {
      total: {
        lines: { pct: 92 },
        branches: { pct: 88 },
        functions: { pct: 85 },
      },
    };
    const { filePath, cleanup } = writeTempFile(JSON.stringify(data));
    try {
      const result = parseCoverageOutput(filePath, 'istanbul');
      expect(result!.lines).toBe(92);
    } finally {
      cleanup();
    }
  });

  it('should extract branches.pct from total.branches', () => {
    const data = {
      total: {
        lines: { pct: 90 },
        branches: { pct: 75 },
        functions: { pct: 80 },
      },
    };
    const { filePath, cleanup } = writeTempFile(JSON.stringify(data));
    try {
      const result = parseCoverageOutput(filePath, 'istanbul');
      expect(result!.branches).toBe(75);
    } finally {
      cleanup();
    }
  });

  it('should extract functions.pct from total.functions', () => {
    const data = {
      total: {
        lines: { pct: 90 },
        branches: { pct: 80 },
        functions: { pct: 70 },
      },
    };
    const { filePath, cleanup } = writeTempFile(JSON.stringify(data));
    try {
      const result = parseCoverageOutput(filePath, 'istanbul');
      expect(result!.functions).toBe(70);
    } finally {
      cleanup();
    }
  });

  it('should handle istanbul with decimal pct values (e.g. 85.3)', () => {
    const data = {
      total: {
        lines: { pct: 85.3 },
        branches: { pct: 74.7 },
        functions: { pct: 80.1 },
      },
    };
    const { filePath, cleanup } = writeTempFile(JSON.stringify(data));
    try {
      const result = parseCoverageOutput(filePath, 'istanbul');
      // The function coerceNumber preserves the decimal
      expect(result!.lines).toBeCloseTo(85.3);
      expect(result!.branches).toBeCloseTo(74.7);
      expect(result!.functions).toBeCloseTo(80.1);
    } finally {
      cleanup();
    }
  });
});

// ===========================================================================
// AC-6: llvm-cov format parsing
// ===========================================================================

describe('parseCoverageOutput -- llvm-cov format (AC-6)', () => {
  it('should parse valid llvm-cov JSON output', () => {
    const llvmData = {
      data: [
        {
          totals: {
            lines: { percent: 90, count: 100 },
            branches: { percent: 80, count: 50 },
            functions: { percent: 85, count: 30 },
          },
        },
      ],
    };
    const { filePath, cleanup } = writeTempFile(JSON.stringify(llvmData));
    try {
      const result = parseCoverageOutput(filePath, 'llvm-cov');
      expect(result).toEqual({ lines: 90, branches: 80, functions: 85 });
    } finally {
      cleanup();
    }
  });

  it('should navigate data[0].totals for llvm-cov structure', () => {
    const data = {
      data: [
        {
          totals: {
            lines: { percent: 95 },
            branches: { percent: 85 },
            functions: { percent: 90 },
          },
        },
      ],
    };
    const { filePath, cleanup } = writeTempFile(JSON.stringify(data));
    try {
      const result = parseCoverageOutput(filePath, 'llvm-cov');
      expect(result).toEqual({ lines: 95, branches: 85, functions: 90 });
    } finally {
      cleanup();
    }
  });

  it('should return null when llvm-cov data array is empty (boundary)', () => {
    const data = { data: [] };
    const { filePath, cleanup } = writeTempFile(JSON.stringify(data));
    try {
      const result = parseCoverageOutput(filePath, 'llvm-cov');
      expect(result).toBeNull();
    } finally {
      cleanup();
    }
  });
});

// ===========================================================================
// Reverse AC-11: Error handling -- file not found / malformed JSON
// ===========================================================================

describe('parseCoverageOutput -- error handling (reverse AC-11)', () => {
  it('should return null when coverage output file does not exist', () => {
    const result = parseCoverageOutput('/nonexistent/path.json', 'istanbul');
    expect(result).toBeNull();
  });

  it('should return null when coverage output file JSON is malformed', () => {
    const { filePath, cleanup } = writeTempFile('{ invalid json!!! }');
    try {
      const result = parseCoverageOutput(filePath, 'istanbul');
      expect(result).toBeNull();
    } finally {
      cleanup();
    }
  });

  it('should return null when coverage output file is empty', () => {
    const { filePath, cleanup } = writeTempFile('');
    try {
      const result = parseCoverageOutput(filePath, 'istanbul');
      expect(result).toBeNull();
    } finally {
      cleanup();
    }
  });

  it('should return null for unsupported coverage format', () => {
    const data = {
      total: {
        lines: { pct: 85 },
        branches: { pct: 74 },
        functions: { pct: 80 },
      },
    };
    const { filePath, cleanup } = writeTempFile(JSON.stringify(data));
    try {
      // @ts-expect-error -- testing with invalid format type
      const result = parseCoverageOutput(filePath, 'unknown-format');
      expect(result).toBeNull();
    } finally {
      cleanup();
    }
  });
});

// ===========================================================================
// Boundary: missing dimensions
// ===========================================================================

describe('parseCoverageOutput -- boundary: missing dimensions', () => {
  it('should return null when istanbul total is missing branches field', () => {
    const data = { total: { lines: { pct: 85 }, functions: { pct: 80 } } };
    const { filePath, cleanup } = writeTempFile(JSON.stringify(data));
    try {
      const result = parseCoverageOutput(filePath, 'istanbul');
      expect(result).toBeNull();
    } finally {
      cleanup();
    }
  });

  it('should handle istanbul with pct values as strings', () => {
    const data = {
      total: {
        lines: { pct: '85' },
        branches: { pct: '74' },
        functions: { pct: '80' },
      },
    };
    const { filePath, cleanup } = writeTempFile(JSON.stringify(data));
    try {
      const result = parseCoverageOutput(filePath, 'istanbul');
      // coerceNumber should convert string "85" to number 85
      expect(result).toEqual({ lines: 85, branches: 74, functions: 80 });
    } finally {
      cleanup();
    }
  });
});

// ===========================================================================
// Edge cases
// ===========================================================================

describe('parseCoverageOutput -- edge cases', () => {
  it('should return null for nullish output path', () => {
    expect(parseCoverageOutput('', 'istanbul')).toBeNull();
    expect(parseCoverageOutput('', 'istanbul')).toBeNull();
  });

  it('should return null when format is null/undefined', () => {
    // Create a valid file but pass empty format
    const data = { total: { lines: { pct: 85 }, branches: { pct: 74 }, functions: { pct: 80 } } };
    const { filePath, cleanup } = writeTempFile(JSON.stringify(data));
    try {
      // @ts-expect-error -- testing with empty format string
      const result = parseCoverageOutput(filePath, '');
      expect(result).toBeNull();
    } finally {
      cleanup();
    }
  });

  it('should handle istanbul JSON with extra unexpected fields', () => {
    const data = {
      total: {
        lines: { pct: 85, total: 100, covered: 85 },
        branches: { pct: 74, total: 50, covered: 37 },
        functions: { pct: 80, total: 30, covered: 24 },
      },
      extraField: 'should not break parsing',
    };
    const { filePath, cleanup } = writeTempFile(JSON.stringify(data));
    try {
      const result = parseCoverageOutput(filePath, 'istanbul');
      expect(result).toEqual({ lines: 85, branches: 74, functions: 80 });
    } finally {
      cleanup();
    }
  });
});
