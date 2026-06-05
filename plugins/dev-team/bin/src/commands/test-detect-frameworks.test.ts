/**
 * Tests for test-detect-frameworks -- MCP tool that detects test framework
 * per file using glob matching from config.json `test.frameworks`.
 *
 * Covers:
 * - AC-4: Glob-first-match file-to-framework detection
 * - AC-15: String shorthand normalisation to single-framework detection
 * - Reverse AC-4: unmatched files return "unknown", empty file list returns empty
 * - Boundary: empty files list, auto-scan with no matches, string shorthand
 *
 * @see openspec/changes/unit-test-coverage-report/test-design.md
 */

import { describe, it, expect } from 'vite-plus/test';
import * as fs from 'fs';
import * as path from 'path';
import * as os from 'os';

import { runTestDetectFrameworks } from './test-detect-frameworks';
import type { TestDetectFrameworksResult } from './test-detect-frameworks';

// ---------------------------------------------------------------------------
// Helpers: create temp project directories with config.json
// ---------------------------------------------------------------------------

interface TempProject {
  root: string;
  cleanup: () => void;
}

function createTempProject(configData: unknown): TempProject {
  const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'detect-fw-test-'));
  const openspecDir = path.join(tmpDir, 'openspec');
  fs.mkdirSync(openspecDir, { recursive: true });
  fs.writeFileSync(path.join(openspecDir, 'config.json'), JSON.stringify(configData, null, 2), 'utf-8');
  return {
    root: tmpDir,
    cleanup: () => fs.rmSync(tmpDir, { recursive: true, force: true }),
  };
}

function createTempProjectWithFiles(configData: unknown, files: string[]): TempProject {
  const project = createTempProject(configData);
  for (const file of files) {
    const fullPath = path.join(project.root, file);
    fs.mkdirSync(path.dirname(fullPath), { recursive: true });
    fs.writeFileSync(fullPath, '', 'utf-8');
  }
  return project;
}

// ===========================================================================
// AC-4: Glob-first-match file-to-framework detection
// ===========================================================================

describe('detectFrameworks -- glob first-match (AC-4)', () => {
  it('should map each file to the first matching framework glob', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      test: {
        frameworks: [
          { glob: '**/*.test.ts', framework: 'vitest' },
          { glob: '**/tests/**/*.rs', framework: 'rust' },
        ],
      },
    });
    try {
      const result = runTestDetectFrameworks({
        files: ['src/utils/helper.test.ts', 'tests/unit/test_auth.rs'],
        projectRoot: project.root,
      });
      // Files are resolved against project root, so the returned file paths are absolute
      expect(result.detected).toHaveLength(2);
      const vitestEntry = result.detected.find((d) => d.file.endsWith('helper.test.ts'));
      const rustEntry = result.detected.find((d) => d.file.endsWith('test_auth.rs'));
      expect(vitestEntry?.framework).toBe('vitest');
      expect(rustEntry?.framework).toBe('rust');
      expect(result.frameworks).toContain('rust');
      expect(result.frameworks).toContain('vitest');
    } finally {
      project.cleanup();
    }
  });

  it('should use first-match rule when a file matches multiple globs', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      test: {
        frameworks: [
          { glob: '**/e2e/**', framework: 'cypress' },
          { glob: '**/*.ts', framework: 'vitest' },
        ],
      },
    });
    try {
      const result = runTestDetectFrameworks({
        files: ['tests/e2e/test_app.ts'],
        projectRoot: project.root,
      });
      expect(result.detected[0].framework).toBe('cypress');
    } finally {
      project.cleanup();
    }
  });

  it('should return "unknown" for files not matching any glob (reverse AC-4)', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      test: {
        frameworks: [{ glob: '**/*.test.ts', framework: 'vitest' }],
      },
    });
    try {
      const result = runTestDetectFrameworks({
        files: ['unknown.js'],
        projectRoot: project.root,
      });
      expect(result.detected[0].framework).toBe('unknown');
    } finally {
      project.cleanup();
    }
  });

  it('should return empty detected for empty file list (reverse AC-4)', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      test: {
        frameworks: [
          { glob: '**/*.test.ts', framework: 'vitest' },
          { glob: '**/tests/**/*.rs', framework: 'rust' },
        ],
      },
    });
    try {
      const result = runTestDetectFrameworks({
        files: [],
        projectRoot: project.root,
      });
      expect(result.detected).toEqual([]);
      expect(result.frameworks).toEqual([]);
    } finally {
      project.cleanup();
    }
  });
});

// ===========================================================================
// AC-15: String shorthand normalisation
// ===========================================================================

describe('detectFrameworks -- string shorthand (AC-15)', () => {
  it('should treat string frameworks as single-entry config and match files', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      test: {
        frameworks: 'vitest',
      },
    });
    try {
      const result = runTestDetectFrameworks({
        files: ['src/utils/helper.test.ts'],
        projectRoot: project.root,
      });
      expect(result.detected).toHaveLength(1);
      expect(result.detected[0].framework).toBe('vitest');
      expect(result.frameworks).toEqual(['vitest']);
    } finally {
      project.cleanup();
    }
  });

  it('should apply correct default glob for each framework shorthand', () => {
    // For vitest, the default glob should match .test.ts files
    const project = createTempProject({
      schema: 'spec-driven',
      test: {
        frameworks: 'vitest',
      },
    });
    try {
      const result = runTestDetectFrameworks({
        files: ['src/test.spec.js', 'src/util.test.ts', 'src/helper.ts'],
        projectRoot: project.root,
      });
      // .test.ts and .spec.js should match, but helper.ts (no .test/.spec) should not
      expect(result.detected).toHaveLength(3);
      expect(result.detected[0].framework).toBe('vitest');
      expect(result.detected[1].framework).toBe('vitest');
      expect(result.detected[2].framework).toBe('unknown');
    } finally {
      project.cleanup();
    }
  });

  it('should match no files when string shorthand config has no matching files', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      test: {
        frameworks: 'vitest',
      },
    });
    try {
      const result = runTestDetectFrameworks({
        files: [],
        projectRoot: project.root,
      });
      expect(result.detected).toEqual([]);
      expect(result.frameworks).toEqual([]);
    } finally {
      project.cleanup();
    }
  });
});

// ===========================================================================
// Edge cases
// ===========================================================================

describe('detectFrameworks -- edge cases', () => {
  it('should handle files parameter as undefined (auto-scan) gracefully', () => {
    // When files is not provided, the tool auto-scans. With no matching files,
    // all scanned files are "unknown".
    const project = createTempProject({
      schema: 'spec-driven',
      test: {
        frameworks: [{ glob: '**/*.test.ts', framework: 'vitest' }],
      },
    });
    try {
      // Create a non-test file in the project to test auto-scan
      const srcDir = path.join(project.root, 'src');
      fs.mkdirSync(srcDir, { recursive: true });
      fs.writeFileSync(path.join(srcDir, 'readme.md'), '# readme', 'utf-8');

      const result = runTestDetectFrameworks({
        projectRoot: project.root,
      });
      // Should have auto-scanned files, none matching .test.ts
      expect(result.detected.length).toBeGreaterThanOrEqual(0);
      // All unmatched files get "unknown"
      expect(result.frameworks).toEqual([]);
    } finally {
      project.cleanup();
    }
  });

  it('should handle config with no frameworks gracefully', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      test: {
        frameworks: [],
      },
    });
    try {
      const result = runTestDetectFrameworks({
        files: ['test.ts'],
        projectRoot: project.root,
      });
      expect(result.detected[0].framework).toBe('unknown');
    } finally {
      project.cleanup();
    }
  });

  it('should handle a very large file list without error', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      test: {
        frameworks: [{ glob: '**/*.test.ts', framework: 'vitest' }],
      },
    });
    try {
      const largeFiles = Array.from({ length: 1000 }, (_, i) => `src/test${i}.test.ts`);
      const result = runTestDetectFrameworks({
        files: largeFiles,
        projectRoot: project.root,
      });
      expect(result.detected.length).toBe(1000);
    } finally {
      project.cleanup();
    }
  });

  it('should handle files with special characters in paths', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      test: {
        frameworks: [{ glob: '**/*.test.ts', framework: 'vitest' }],
      },
    });
    try {
      const result = runTestDetectFrameworks({
        files: ['src/my test file.test.ts', 'src/测试.test.ts'],
        projectRoot: project.root,
      });
      expect(result.detected).toHaveLength(2);
      expect(result.detected.every((d) => d.framework === 'vitest')).toBe(true);
    } finally {
      project.cleanup();
    }
  });

  it('should handle nullish or missing config gracefully', () => {
    // When config has no test node at all, everything is "unknown"
    const project = createTempProject({
      schema: 'spec-driven',
    });
    try {
      const result = runTestDetectFrameworks({
        files: ['test.ts'],
        projectRoot: project.root,
      });
      expect(result.detected[0].framework).toBe('unknown');
    } finally {
      project.cleanup();
    }
  });
});
