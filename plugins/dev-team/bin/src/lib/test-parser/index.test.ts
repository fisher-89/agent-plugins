/**
 * Tests for lib/test-parser/index — parsePlanArtifacts
 */

import * as fs from 'fs';
import * as os from 'os';
import * as path from 'path';

import { describe, expect, it } from 'vite-plus/test';

import { parseGoOutput } from './go-parser';
import { parsePlanArtifacts } from './index';
import { parseJsOutput } from './js-parser';
import { parseTextOutput } from './text-parser';

function createReportDir(): { reportDir: string; cleanup: () => void } {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'parse-plan-'));
  return {
    reportDir: root,
    cleanup: () => fs.rmSync(root, { recursive: true, force: true }),
  };
}

const JS_RESULTS = JSON.stringify({
  testResults: [
    {
      name: 'foo.test.ts',
      assertionResults: [
        { title: 'ok', fullName: 'ok', status: 'passed', failureMessages: [] },
        { title: 'bad', fullName: 'bad', status: 'failed', failureMessages: ['x'] },
      ],
    },
  ],
});

const ISTANBUL = JSON.stringify({
  total: { lines: { pct: 88 }, branches: { pct: 70 }, functions: { pct: 80 } },
});

describe('parsePlanArtifacts — 八框架约定产物', () => {
  it('jest：results.json + coverage-summary.json → 解析成功且 coverage 非 null', () => {
    const { reportDir, cleanup } = createReportDir();
    try {
      fs.writeFileSync(path.join(reportDir, 'results.json'), JS_RESULTS, 'utf-8');
      fs.writeFileSync(path.join(reportDir, 'coverage-summary.json'), ISTANBUL, 'utf-8');
      const result = parsePlanArtifacts('jest', reportDir);
      expect(result.testCases.length).toBe(2);
      expect(result.coverage).not.toBeNull();
      expect(result.coverage!.lines).toBe(88);
    } finally {
      cleanup();
    }
  });

  it('vitest/vite-plus：同 jest 产物约定', () => {
    for (const fw of ['vitest', 'vite-plus'] as const) {
      const { reportDir, cleanup } = createReportDir();
      try {
        fs.writeFileSync(path.join(reportDir, 'results.json'), JS_RESULTS, 'utf-8');
        fs.writeFileSync(path.join(reportDir, 'coverage-summary.json'), ISTANBUL, 'utf-8');
        const result = parsePlanArtifacts(fw, reportDir);
        expect(result.passed).toBe(1);
        expect(result.coverage).not.toBeNull();
      } finally {
        cleanup();
      }
    }
  });

  it('bun：读 results.txt + lcov.info', () => {
    const { reportDir, cleanup } = createReportDir();
    try {
      fs.writeFileSync(path.join(reportDir, 'results.txt'), '1 [PASS] works\n', 'utf-8');
      fs.writeFileSync(
        path.join(reportDir, 'lcov.info'),
        'TN:\nSF:src/a.ts\nLF:10\nLH:8\nend_of_record\n',
        'utf-8',
      );
      const result = parsePlanArtifacts('bun', reportDir);
      expect(result.passed).toBe(1);
      expect(result.coverage).not.toBeNull();
    } finally {
      cleanup();
    }
  });

  it('go：读 results.ndjson + func-summary.txt', () => {
    const { reportDir, cleanup } = createReportDir();
    try {
      fs.writeFileSync(
        path.join(reportDir, 'results.ndjson'),
        `${JSON.stringify({ Action: 'pass', Test: 'TestFoo' })}\n`,
        'utf-8',
      );
      fs.writeFileSync(
        path.join(reportDir, 'func-summary.txt'),
        'total:\t(statements)\t90.0%\n',
        'utf-8',
      );
      const result = parsePlanArtifacts('go', reportDir);
      expect(result.passed).toBe(1);
      expect(result.coverage).not.toBeNull();
    } finally {
      cleanup();
    }
  });

  it('rust：读 results.txt + coverage-summary.json', () => {
    const { reportDir, cleanup } = createReportDir();
    try {
      fs.writeFileSync(
        path.join(reportDir, 'results.txt'),
        'test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out\n',
        'utf-8',
      );
      fs.writeFileSync(
        path.join(reportDir, 'coverage-summary.json'),
        JSON.stringify({ data: [{ totals: { lines: { percent: 91 } } }] }),
        'utf-8',
      );
      const result = parsePlanArtifacts('rust', reportDir);
      expect(result.passed).toBe(1);
      expect(result.coverage).not.toBeNull();
    } finally {
      cleanup();
    }
  });

  it('pytest：读 results.txt + coverage.json', () => {
    const { reportDir, cleanup } = createReportDir();
    try {
      fs.writeFileSync(
        path.join(reportDir, 'results.txt'),
        'tests/test_a.py::test_x PASSED\n',
        'utf-8',
      );
      fs.writeFileSync(
        path.join(reportDir, 'coverage.json'),
        JSON.stringify({ totals: { percent_covered: 77, num_statements: 10, covered_lines: 7 } }),
        'utf-8',
      );
      const result = parsePlanArtifacts('pytest', reportDir);
      expect(result.passed).toBe(1);
      expect(result.coverage).not.toBeNull();
    } finally {
      cleanup();
    }
  });

  it('node-test：读 results.txt', () => {
    const { reportDir, cleanup } = createReportDir();
    try {
      fs.writeFileSync(
        path.join(reportDir, 'results.txt'),
        '# pass 1\n# fail 0\n# skip 0\n',
        'utf-8',
      );
      const result = parsePlanArtifacts('node-test', reportDir);
      expect(result.passed).toBe(1);
    } finally {
      cleanup();
    }
  });

  it('reportDir 不存在 → 返回 error，coverage=null', () => {
    const result = parsePlanArtifacts('jest', path.join(os.tmpdir(), 'missing-plan-dir-xyz'));
    expect(result.error).toBeTruthy();
    expect(result.coverage).toBeNull();
    expect(result.testCases).toEqual([]);
  });

  it('未知 framework → 可诊断错误', () => {
    const { reportDir, cleanup } = createReportDir();
    try {
      const result = parsePlanArtifacts('unknown-fw', reportDir);
      expect(result.error).toMatch(/Unknown framework/);
      expect(result.coverage).toBeNull();
    } finally {
      cleanup();
    }
  });

  it('仅有 results 无 coverage 侧车 → coverage=null', () => {
    const { reportDir, cleanup } = createReportDir();
    try {
      fs.writeFileSync(path.join(reportDir, 'results.json'), JS_RESULTS, 'utf-8');
      const result = parsePlanArtifacts('vitest', reportDir);
      expect(result.testCases.length).toBe(2);
      expect(result.coverage).toBeNull();
    } finally {
      cleanup();
    }
  });

  it('不读取 suite cwd 下旧 coverage/ 路径', () => {
    const root = fs.mkdtempSync(path.join(os.tmpdir(), 'parse-plan-cwd-'));
    try {
      const reportDir = path.join(root, 'reports', 'test', 'vitest');
      fs.mkdirSync(reportDir, { recursive: true });
      fs.writeFileSync(path.join(reportDir, 'results.json'), JS_RESULTS, 'utf-8');
      const oldCov = path.join(root, 'coverage');
      fs.mkdirSync(oldCov, { recursive: true });
      fs.writeFileSync(path.join(oldCov, 'coverage-summary.json'), ISTANBUL, 'utf-8');
      const result = parsePlanArtifacts('vitest', reportDir);
      expect(result.coverage).toBeNull();
    } finally {
      fs.rmSync(root, { recursive: true, force: true });
    }
  });
});

describe('parse dispatch / 脏 stdout 边界（主路径 parsePlanArtifacts）', () => {
  it('既有 dispatch（vitest/jest/go/text）保持可用', () => {
    expect(parseJsOutput(JS_RESULTS).passed).toBe(1);
    expect(parseGoOutput(`${JSON.stringify({ Action: 'pass', Test: 'T' })}\n`).passed).toBe(1);
    expect(parseTextOutput('1 [PASS] a\n').passed).toBe(1);
  });

  it('未知 framework → parsePlanArtifacts 返回 error 且不抛', () => {
    const { reportDir, cleanup } = createReportDir();
    try {
      expect(() => parsePlanArtifacts('nope', reportDir)).not.toThrow();
      expect(parsePlanArtifacts('nope', reportDir).error).toBeTruthy();
    } finally {
      cleanup();
    }
  });

  it('jest/vitest 的内容为非法 JSON → 返回带 error 的结果', () => {
    const { reportDir, cleanup } = createReportDir();
    try {
      fs.writeFileSync(path.join(reportDir, 'results.json'), 'NOT JSON {{{', 'utf-8');
      const result = parsePlanArtifacts('jest', reportDir);
      expect(result.error).toBeTruthy();
    } finally {
      cleanup();
    }
  });

  it('results 为空字符串时返回空结果或 error', () => {
    const { reportDir, cleanup } = createReportDir();
    try {
      fs.writeFileSync(path.join(reportDir, 'results.json'), '', 'utf-8');
      const result = parsePlanArtifacts('vitest', reportDir);
      expect(result.testCases.length === 0 || result.error).toBeTruthy();
    } finally {
      cleanup();
    }
  });

  it('脏 stdout JSON 对 execute 主路径不再是唯一入口（主验收在 parsePlanArtifacts）', () => {
    const dirtyStdout = `>>> loading config\n${JS_RESULTS}`;
    // stdout JSON 带前缀噪声时纵向 JS 解析失败
    expect(parseJsOutput(dirtyStdout).error).toBeTruthy();

    // execute 主路径经 parsePlanArtifacts 读文件仍成功
    const { reportDir, cleanup } = createReportDir();
    try {
      fs.writeFileSync(path.join(reportDir, 'results.json'), JS_RESULTS, 'utf-8');
      const fileResult = parsePlanArtifacts('jest', reportDir);
      expect(fileResult.error).toBeUndefined();
      expect(fileResult.passed).toBe(1);
      expect(fileResult.failed).toBe(1);
    } finally {
      cleanup();
    }
  });
});
