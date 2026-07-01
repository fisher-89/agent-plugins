/**
 * Tests for lib/test-parser/coverage-parser -- reads and parses coverage files
 * in 5 formats: istanbul, llvm-cov, node-test, go-cover, coverage-py.
 *
 * Covers:
 * - istanbul: lines/branches/functions from JSON summary
 * - llvm-cov: lines/branches/functions from data[0].totals
 * - go-cover: lines only from "total:" line, branches/functions=null
 * - coverage-py: lines/branches from JSON, functions=null
 * - node-test: text table parsing
 * - Edge: file not found, empty file, all 100%, all 0%, decimal precision
 *
 * @see openspec/changes/cli-unit-test-execute/test-design.md
 */

import * as fs from 'fs';
import * as os from 'os';
import * as path from 'path';

import { describe, it, expect } from 'vite-plus/test';

import { parseCoverageFromFile } from './coverage-parser';

// ===========================================================================
// Helpers
// ===========================================================================

function writeTempFile(prefix: string, content: string): { filePath: string; cleanup: () => void } {
  const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), `${prefix}-`));
  const filePath = path.join(tmpDir, 'coverage-output');
  fs.writeFileSync(filePath, content, 'utf-8');
  return {
    filePath,
    cleanup: () => fs.rmSync(tmpDir, { recursive: true, force: true }),
  };
}

// ===========================================================================
// Istanbul format
// ===========================================================================

describe('parseCoverageFromFile -- istanbul format', () => {
  it('should parse istanbul JSON and extract lines/branches/functions', () => {
    const content = JSON.stringify({
      total: {
        lines: { pct: 85.5 },
        branches: { pct: 72.3 },
        functions: { pct: 90.1 },
      },
      'src/foo.ts': {
        lines: { pct: 90 },
        branches: { pct: 80 },
        functions: { pct: 95 },
      },
    });

    const { filePath, cleanup } = writeTempFile('istanbul', content);
    try {
      const result = parseCoverageFromFile(filePath, 'istanbul');
      expect(result).not.toBeNull();
      expect(result!.lines).toBe(85.5);
      expect(result!.branches).toBe(72.3);
      expect(result!.functions).toBe(90.1);
      expect(result!.fileCoverage).not.toBeNull();
      expect(result!.fileCoverage).toHaveLength(1);
      expect(result!.fileCoverage![0].file).toBe('src/foo.ts');
      expect(result!.fileCoverage![0].lines).toBe(90);
    } finally {
      cleanup();
    }
  });

  it('should return null for non-existent file', () => {
    const result = parseCoverageFromFile('/nonexistent/file.json', 'istanbul');
    expect(result).toBeNull();
  });

  it('should return null for empty file', () => {
    const { filePath, cleanup } = writeTempFile('istanbul-empty', '');
    try {
      const result = parseCoverageFromFile(filePath, 'istanbul');
      expect(result).toBeNull();
    } finally {
      cleanup();
    }
  });

  it('should handle all 100% coverage', () => {
    const content = JSON.stringify({
      total: {
        lines: { pct: 100 },
        branches: { pct: 100 },
        functions: { pct: 100 },
      },
    });

    const { filePath, cleanup } = writeTempFile('istanbul-100', content);
    try {
      const result = parseCoverageFromFile(filePath, 'istanbul');
      expect(result!.lines).toBe(100);
      expect(result!.branches).toBe(100);
      expect(result!.functions).toBe(100);
    } finally {
      cleanup();
    }
  });

  it('should handle all 0% coverage', () => {
    const content = JSON.stringify({
      total: {
        lines: { pct: 0 },
        branches: { pct: 0 },
        functions: { pct: 0 },
      },
    });

    const { filePath, cleanup } = writeTempFile('istanbul-0', content);
    try {
      const result = parseCoverageFromFile(filePath, 'istanbul');
      expect(result!.lines).toBe(0);
      expect(result!.branches).toBe(0);
      expect(result!.functions).toBe(0);
    } finally {
      cleanup();
    }
  });

  it('should preserve decimal precision', () => {
    const content = JSON.stringify({
      total: {
        lines: { pct: 85.57 },
        branches: { pct: 72.34 },
        functions: { pct: 91.01 },
      },
    });

    const { filePath, cleanup } = writeTempFile('istanbul-decimal', content);
    try {
      const result = parseCoverageFromFile(filePath, 'istanbul');
      expect(result!.lines).toBe(85.57);
      expect(result!.branches).toBe(72.34);
      expect(result!.functions).toBe(91.01);
    } finally {
      cleanup();
    }
  });
});

// ===========================================================================
// LLVM-cov format
// ===========================================================================

describe('parseCoverageFromFile -- llvm-cov format', () => {
  it('should parse llvm-cov JSON and extract lines/branches/functions', () => {
    const content = JSON.stringify({
      data: [
        {
          totals: {
            lines: { percent: 80.0 },
            branches: { percent: 75.5 },
            functions: { percent: 88.3 },
          },
        },
      ],
    });

    const { filePath, cleanup } = writeTempFile('llvm', content);
    try {
      const result = parseCoverageFromFile(filePath, 'llvm-cov');
      expect(result).not.toBeNull();
      expect(result!.lines).toBe(80.0);
      expect(result!.branches).toBe(75.5);
      expect(result!.functions).toBe(88.3);
    } finally {
      cleanup();
    }
  });

  it('should handle missing data array', () => {
    const { filePath, cleanup } = writeTempFile('llvm-empty', '{}');
    try {
      const result = parseCoverageFromFile(filePath, 'llvm-cov');
      expect(result).toBeNull();
    } finally {
      cleanup();
    }
  });
});

// ===========================================================================
// Go-cover format
// ===========================================================================

describe('parseCoverageFromFile -- go-cover format', () => {
  it('should parse go-cover text and extract lines, branches/functions=null', () => {
    const content = [
      'github.com/foo/bar/baz.go:10: FooBar 85.5%',
      'total:\t(statements)\t85.5%',
    ].join('\n');

    const { filePath, cleanup } = writeTempFile('go-cover', content);
    try {
      const result = parseCoverageFromFile(filePath, 'go-cover');
      expect(result).not.toBeNull();
      expect(result!.lines).toBe(85.5);
      expect(result!.branches).toBeNull();
      expect(result!.functions).toBeNull();
    } finally {
      cleanup();
    }
  });

  it('should return null when no total line found', () => {
    const { filePath, cleanup } = writeTempFile('go-cover-no-total', 'nothing here');
    try {
      const result = parseCoverageFromFile(filePath, 'go-cover');
      expect(result).toBeNull();
    } finally {
      cleanup();
    }
  });
});

// ===========================================================================
// Coverage-py format
// ===========================================================================

describe('parseCoverageFromFile -- coverage-py format', () => {
  it('should parse coverage.py JSON and extract lines/branches, functions=null', () => {
    const content = JSON.stringify({
      totals: {
        percent_covered: 90.5,
        percent_covered_branches: 85.0,
      },
    });

    const { filePath, cleanup } = writeTempFile('coverage-py', content);
    try {
      const result = parseCoverageFromFile(filePath, 'coverage-py');
      expect(result).not.toBeNull();
      expect(result!.lines).toBe(90.5);
      expect(result!.branches).toBe(85.0);
      expect(result!.functions).toBeNull();
    } finally {
      cleanup();
    }
  });

  it('should handle missing percent_covered_branches', () => {
    const content = JSON.stringify({
      totals: {
        percent_covered: 85.0,
      },
    });

    const { filePath, cleanup } = writeTempFile('coverage-py-no-branch', content);
    try {
      const result = parseCoverageFromFile(filePath, 'coverage-py');
      expect(result!.lines).toBe(85.0);
      expect(result!.branches).toBeNull();
    } finally {
      cleanup();
    }
  });
});

// ===========================================================================
// Unknown format
// ===========================================================================

describe('parseCoverageFromFile -- unknown format', () => {
  it('should return null for unsupported format', () => {
    const { filePath, cleanup } = writeTempFile('unknown', '{}');
    try {
      const result = parseCoverageFromFile(filePath, 'unknown-format');
      expect(result).toBeNull();
    } finally {
      cleanup();
    }
  });
});

// ===========================================================================
// Node-test format (text table)
// ===========================================================================

describe('parseCoverageFromFile -- node-test format', () => {
  it('should parse node-test text table and extract lines/branches/functions', () => {
    const content = [
      '| file       | line coverage | branch coverage | function coverage |',
      '|------------|---------------|-----------------|-------------------|',
      '| All files  | 85.50         | 72.30           | 90.10             |',
      '| src/foo.ts | 90.00         | 80.00           | 95.00             |',
    ].join('\n');

    const { filePath, cleanup } = writeTempFile('node-test', content);
    try {
      const result = parseCoverageFromFile(filePath, 'node-test');
      expect(result).not.toBeNull();
      expect(result!.lines).toBe(85.5);
      expect(result!.branches).toBe(72.3);
      expect(result!.functions).toBe(90.1);
    } finally {
      cleanup();
    }
  });

  it('should handle node-test with only lines column (branches/functions null)', () => {
    const content = [
      '| file       | line coverage |',
      '|------------|---------------|',
      '| All files  | 75.00         |',
    ].join('\n');

    const { filePath, cleanup } = writeTempFile('node-test-lines-only', content);
    try {
      const result = parseCoverageFromFile(filePath, 'node-test');
      expect(result).not.toBeNull();
      expect(result!.lines).toBe(75);
      expect(result!.branches).toBeNull();
      expect(result!.functions).toBeNull();
    } finally {
      cleanup();
    }
  });

  it('should handle node-test with dash placeholder for missing dimensions', () => {
    const content = [
      '| file       | line coverage | branch coverage | function coverage |',
      '|------------|---------------|-----------------|-------------------|',
      '| All files  | 80.00         | -               | 90.00             |',
    ].join('\n');

    const { filePath, cleanup } = writeTempFile('node-test-dash', content);
    try {
      const result = parseCoverageFromFile(filePath, 'node-test');
      expect(result).not.toBeNull();
      expect(result!.lines).toBe(80);
      expect(result!.branches).toBeNull();
      expect(result!.functions).toBe(90);
    } finally {
      cleanup();
    }
  });

  it('should return null when no "All files" line found', () => {
    const content = [
      '| file       | line coverage |',
      '|------------|---------------|',
      '| src/foo.ts | 75.00         |',
    ].join('\n');

    const { filePath, cleanup } = writeTempFile('node-test-no-all', content);
    try {
      const result = parseCoverageFromFile(filePath, 'node-test');
      expect(result).toBeNull();
    } finally {
      cleanup();
    }
  });

  it('should handle all 100% in node-test format', () => {
    const content = [
      '| file       | line coverage | branch coverage | function coverage |',
      '|------------|---------------|-----------------|-------------------|',
      '| All files  | 100.00        | 100.00          | 100.00            |',
    ].join('\n');

    const { filePath, cleanup } = writeTempFile('node-test-100', content);
    try {
      const result = parseCoverageFromFile(filePath, 'node-test');
      expect(result!.lines).toBe(100);
      expect(result!.branches).toBe(100);
      expect(result!.functions).toBe(100);
    } finally {
      cleanup();
    }
  });

  it('should handle all 0% in node-test format', () => {
    const content = [
      '| file       | line coverage | branch coverage | function coverage |',
      '|------------|---------------|-----------------|-------------------|',
      '| All files  | 0.00          | 0.00            | 0.00              |',
    ].join('\n');

    const { filePath, cleanup } = writeTempFile('node-test-0', content);
    try {
      const result = parseCoverageFromFile(filePath, 'node-test');
      expect(result!.lines).toBe(0);
      expect(result!.branches).toBe(0);
      expect(result!.functions).toBe(0);
    } finally {
      cleanup();
    }
  });
});

// ===========================================================================
// Null dimension support
// ===========================================================================

describe('parseCoverageFromFile -- null dimension support', () => {
  it('should return branches=null for go-cover', () => {
    const content = 'total:\t(statements)\t85.5%';
    const { filePath, cleanup } = writeTempFile('go-null', content);
    try {
      const result = parseCoverageFromFile(filePath, 'go-cover');
      expect(result!.branches).toBeNull();
      expect(result!.functions).toBeNull();
    } finally {
      cleanup();
    }
  });

  it('should return functions=null for coverage-py', () => {
    const content = JSON.stringify({ totals: { percent_covered: 90 } });
    const { filePath, cleanup } = writeTempFile('py-null', content);
    try {
      const result = parseCoverageFromFile(filePath, 'coverage-py');
      expect(result!.functions).toBeNull();
    } finally {
      cleanup();
    }
  });
});
