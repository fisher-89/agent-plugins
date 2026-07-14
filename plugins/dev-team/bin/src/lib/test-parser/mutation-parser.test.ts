/**
 * 单元测试: lib/test-parser/mutation-parser -- StrykerJS JSON 报告解析
 */

import * as fs from 'fs';
import * as os from 'os';
import * as path from 'path';

import { afterEach, beforeEach, describe, expect, it } from 'vite-plus/test';

import { parseMutationReport } from './mutation-parser';

// ===========================================================================
// Helpers
// ===========================================================================

interface TempDir {
  root: string;
  cleanup: () => void;
}

function createTempDir(): TempDir {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'mutation-parser-'));
  return {
    root,
    cleanup: () => fs.rmSync(root, { recursive: true, force: true }),
  };
}

function writeReport(dir: string, data: unknown): string {
  const filePath = path.join(dir, 'mutation.json');
  fs.writeFileSync(filePath, JSON.stringify(data, null, 2), 'utf-8');
  return filePath;
}

function makeFullReport(overrides: Record<string, unknown> = {}): Record<string, unknown> {
  return {
    schemaVersion: '1.0',
    testRunner: 'vitest-runner',
    thresholds: { high: 80, low: 60, break: null },
    metrics: {
      mutationScore: 85.5,
      mutationScoreBasedOnCoveredCode: 90.0,
      killed: 10,
      survived: 1,
      timeout: 1,
      noCoverage: 0,
      compileErrors: 0,
      runtimeErrors: 0,
      ignored: 0,
      totalDetected: 11,
      totalUndetected: 1,
      totalMutants: 12,
      totalCoveredMutants: 11,
      totalValidMutants: 12,
    },
    ...overrides,
  };
}

// ===========================================================================
// 正向测试
// ===========================================================================

describe('parseMutationReport -- 正向', () => {
  let tmp: TempDir;

  beforeEach(() => {
    tmp = createTempDir();
  });

  afterEach(() => {
    tmp.cleanup();
  });

  it('解析完整 StrykerJS JSON 报告，正确提取 score 和 killed/survived/timeout 等计数', () => {
    const reportPath = writeReport(tmp.root, makeFullReport());
    const result = parseMutationReport(reportPath);

    expect(result).not.toBeNull();
    expect(result!.score).toBe(85.5);
    expect(result!.killed).toBe(10);
    expect(result!.survived).toBe(1);
    expect(result!.timeout).toBe(1);
    expect(result!.noCoverage).toBe(0);
    expect(result!.compileError).toBe(0);
    expect(result!.runtimeError).toBe(0);
    expect(result!.ignored).toBe(0);
    expect(result!.total).toBe(12);
    expect(result!.detected).toBe(11);
    expect(result!.undetected).toBe(1);
  });

  it('解析的 score 为浮点数时保留精度', () => {
    const reportPath = writeReport(
      tmp.root,
      makeFullReport({
        metrics: {
          mutationScore: 92.345,
          mutationScoreBasedOnCoveredCode: 95.0,
          killed: 20,
          survived: 1,
          timeout: 1,
          noCoverage: 0,
          compileErrors: 0,
          runtimeErrors: 0,
          ignored: 0,
          totalDetected: 21,
          totalUndetected: 1,
          totalMutants: 22,
        },
      }),
    );
    const result = parseMutationReport(reportPath);
    expect(result).not.toBeNull();
    expect(result!.score).toBe(92.345);
  });
});

// ===========================================================================
// 异常测试
// ===========================================================================

describe('parseMutationReport -- 异常', () => {
  it('报告文件不存在时返回 null', () => {
    const result = parseMutationReport('/nonexistent/path/mutation.json');
    expect(result).toBeNull();
  });

  it('报告文件为空时返回 null', () => {
    const tmp = createTempDir();
    try {
      const filePath = path.join(tmp.root, 'mutation.json');
      fs.writeFileSync(filePath, '', 'utf-8');
      const result = parseMutationReport(filePath);
      expect(result).toBeNull();
    } finally {
      tmp.cleanup();
    }
  });

  it('报告 JSON 格式异常时返回 null', () => {
    const tmp = createTempDir();
    try {
      const filePath = path.join(tmp.root, 'mutation.json');
      fs.writeFileSync(filePath, 'not valid json', 'utf-8');
      const result = parseMutationReport(filePath);
      expect(result).toBeNull();
    } finally {
      tmp.cleanup();
    }
  });
});

// ===========================================================================
// 边界测试
// ===========================================================================

describe('parseMutationReport -- 边界', () => {
  it('所有变异体计数均为 0（无变异体）时返回 score=100', () => {
    const tmp = createTempDir();
    try {
      const reportPath = writeReport(tmp.root, {
        schemaVersion: '1.0',
        testRunner: 'vitest-runner',
        metrics: {
          mutationScore: 100,
          mutationScoreBasedOnCoveredCode: 100,
          killed: 0,
          survived: 0,
          timeout: 0,
          noCoverage: 0,
          compileErrors: 0,
          runtimeErrors: 0,
          ignored: 0,
          totalDetected: 0,
          totalUndetected: 0,
          totalMutants: 0,
        },
      });
      const result = parseMutationReport(reportPath);
      expect(result).not.toBeNull();
      expect(result!.score).toBe(100);
      expect(result!.total).toBe(0);
    } finally {
      tmp.cleanup();
    }
  });

  it('score 为 0（所有变异体存活）时正确返回 0', () => {
    const tmp = createTempDir();
    try {
      const reportPath = writeReport(tmp.root, {
        schemaVersion: '1.0',
        testRunner: 'vitest-runner',
        metrics: {
          mutationScore: 0,
          mutationScoreBasedOnCoveredCode: 0,
          killed: 0,
          survived: 10,
          timeout: 0,
          noCoverage: 0,
          compileErrors: 0,
          runtimeErrors: 0,
          ignored: 0,
          totalDetected: 0,
          totalUndetected: 10,
          totalMutants: 10,
        },
      });
      const result = parseMutationReport(reportPath);
      expect(result).not.toBeNull();
      expect(result!.score).toBe(0);
      expect(result!.survived).toBe(10);
    } finally {
      tmp.cleanup();
    }
  });

  it('metrics 缺失但 files 存在时从 files 中统计', () => {
    const tmp = createTempDir();
    try {
      const reportPath = writeReport(tmp.root, {
        schemaVersion: '1.0',
        testRunner: 'vitest-runner',
        files: {
          'src/foo.ts': {
            mutants: [
              {
                id: '1',
                mutatorName: 'StringLiteral',
                replacement: '""',
                status: 'Killed',
                testsRan: ['t1'],
              },
              {
                id: '2',
                mutatorName: 'BinaryOperator',
                replacement: '+',
                status: 'Survived',
                testsRan: ['t1'],
              },
              {
                id: '3',
                mutatorName: 'StringLiteral',
                replacement: '""',
                status: 'TimedOut',
                testsRan: ['t1'],
              },
            ],
          },
          'src/bar.ts': {
            mutants: [
              {
                id: '4',
                mutatorName: 'BooleanLiteral',
                replacement: 'false',
                status: 'NoCoverage',
                testsRan: [],
              },
              {
                id: '5',
                mutatorName: 'StringLiteral',
                replacement: '""',
                status: 'CompileError',
                testsRan: [],
              },
              {
                id: '6',
                mutatorName: 'BinaryOperator',
                replacement: '-',
                status: 'RuntimeError',
                testsRan: [],
              },
              {
                id: '7',
                mutatorName: 'StringLiteral',
                replacement: '""',
                status: 'Ignored',
                testsRan: [],
              },
            ],
          },
        },
      });
      const result = parseMutationReport(reportPath);
      expect(result).not.toBeNull();
      // killed(1) + timeout(1) = 2 detected out of 7 total
      // ignored(1) + compileErrors(1) + runtimeErrors(1) = 3 invalid
      // validMutants = 7 - 1 - 1 - 1 = 4
      // score = 2/4 * 100 = 50
      expect(result!.score).toBeCloseTo(50, 1);
      expect(result!.killed).toBe(1);
      expect(result!.survived).toBe(1);
      expect(result!.timeout).toBe(1);
      expect(result!.noCoverage).toBe(1);
      expect(result!.compileError).toBe(1);
      expect(result!.runtimeError).toBe(1);
      expect(result!.ignored).toBe(1);
      expect(result!.total).toBe(7);
      expect(result!.detected).toBe(2);
      expect(result!.undetected).toBe(2);
    } finally {
      tmp.cleanup();
    }
  });

  it('metrics 缺失且 files 为空对象时返回 null', () => {
    const tmp = createTempDir();
    try {
      const reportPath = writeReport(tmp.root, {
        schemaVersion: '1.0',
        testRunner: 'vitest-runner',
        files: {},
      });
      const result = parseMutationReport(reportPath);
      expect(result).toBeNull();
    } finally {
      tmp.cleanup();
    }
  });

  it('metrics 缺失且 files 为 undefined 时返回 null', () => {
    const tmp = createTempDir();
    try {
      const reportPath = writeReport(tmp.root, {
        schemaVersion: '1.0',
        testRunner: 'vitest-runner',
      });
      const result = parseMutationReport(reportPath);
      expect(result).toBeNull();
    } finally {
      tmp.cleanup();
    }
  });

  it('metrics 中所有计数为 undefined 时默认为 0', () => {
    const tmp = createTempDir();
    try {
      const reportPath = writeReport(tmp.root, {
        schemaVersion: '1.0',
        testRunner: 'vitest-runner',
        metrics: {},
      });
      const result = parseMutationReport(reportPath);
      expect(result).not.toBeNull();
      expect(result!.score).toBe(0);
      expect(result!.killed).toBe(0);
      expect(result!.total).toBe(0);
    } finally {
      tmp.cleanup();
    }
  });

  it('validMutants 为 0（忽略、编译错误、运行时错误占满所有变异体）时 score=100', () => {
    const tmp = createTempDir();
    try {
      const reportPath = writeReport(tmp.root, {
        schemaVersion: '1.0',
        testRunner: 'vitest-runner',
        metrics: {
          mutationScore: 100,
          mutationScoreBasedOnCoveredCode: 100,
          killed: 0,
          survived: 0,
          timeout: 0,
          noCoverage: 0,
          compileErrors: 5,
          runtimeErrors: 3,
          ignored: 2,
          totalDetected: 0,
          totalUndetected: 0,
          totalMutants: 10,
        },
      });
      const result = parseMutationReport(reportPath);
      expect(result).not.toBeNull();
      expect(result!.score).toBe(100);
      expect(result!.ignored).toBe(2);
      expect(result!.compileError).toBe(5);
      expect(result!.runtimeError).toBe(3);
    } finally {
      tmp.cleanup();
    }
  });
});
