/**
 * 单元测试: parse-node-test-coverage.mjs — node:test 覆盖率表格解析
 *
 * @see openspec/changes/add-node-go-pytest-frameworks/test-design.md
 *
 * 用法: node --test parse-node-test-coverage.test.mjs
 */

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import * as fs from 'node:fs';
import * as os from 'node:os';
import * as path from 'node:path';
import { fileURLToPath } from 'node:url';

const scriptPath = fileURLToPath(new URL('./parse-node-test-coverage.mjs', import.meta.url));

/** node:test --experimental-test-coverage 典型 stdout 表格样本 */
const VALID_STDOUT_FIXTURE = [
  'ℹ tests 2',
  'ℹ suites 0',
  'ℹ pass 2',
  'ℹ fail 0',
  'ℹ cancelled 0',
  'ℹ skipped 0',
  'ℹ todo 0',
  'ℹ duration_ms 42',
  '',
  '----------|---------|----------|---------|----------|',
  'File      | % Stmts | % Branch | % Funcs | % Lines |',
  '----------|---------|----------|---------|----------|',
  'All files |   85.71 |    50.00 |   66.67 |   80.00 |',
  '----------|---------|----------|---------|----------|',
].join('\n');

function runParser(args, cwd) {
  return spawnSync(process.execPath, [scriptPath, ...args], {
    cwd,
    encoding: 'utf8',
    shell: false,
  });
}

function withTempDir(fn) {
  const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'parse-node-test-'));
  try {
    return fn(tmpDir);
  } finally {
    fs.rmSync(tmpDir, { recursive: true, force: true });
  }
}

// ---------------------------------------------------------------------------
// parseNodeTestCoverage — 正向 (AC-6)
// ---------------------------------------------------------------------------

describe('parseNodeTestCoverage — 正向', () => {
  it('有效 node:test stdout fixture 应解析 lines/branches/functions 并写入 istanbul 兼容 coverage-summary.json', () => {
    withTempDir((tmpDir) => {
      const inputPath = path.join(tmpDir, 'node-test-output.txt');
      const outputPath = path.join(tmpDir, 'coverage-summary.json');
      fs.writeFileSync(inputPath, VALID_STDOUT_FIXTURE, 'utf8');

      const result = runParser([inputPath, outputPath], tmpDir);
      assert.equal(result.status, 0, result.stderr);

      const summary = JSON.parse(fs.readFileSync(outputPath, 'utf8'));
      assert.ok(summary.total);
      assert.equal(summary.total.lines.pct, 80.0);
      assert.equal(summary.total.branches.pct, 50.0);
      assert.equal(summary.total.functions.pct, 66.67);
    });
  });

  it('输出 JSON 结构应与 istanbul coverage-summary.json 字段兼容（lines.pct、branches.pct、functions.pct）', () => {
    withTempDir((tmpDir) => {
      const inputPath = path.join(tmpDir, 'out.txt');
      const outputPath = path.join(tmpDir, 'summary.json');
      fs.writeFileSync(inputPath, 'all files | 95.00 | 90.00 | 80.00 | 100.00 |\n', 'utf8');

      const result = runParser([inputPath, outputPath], tmpDir);
      assert.equal(result.status, 0);

      const summary = JSON.parse(fs.readFileSync(outputPath, 'utf8'));
      assert.equal(typeof summary.total.lines.pct, 'number');
      assert.equal(typeof summary.total.branches.pct, 'number');
      assert.equal(typeof summary.total.functions.pct, 'number');
    });
  });
});

// ---------------------------------------------------------------------------
// parseNodeTestCoverage — 异常 (AC-6)
// ---------------------------------------------------------------------------

describe('parseNodeTestCoverage — 异常', () => {
  it('输入文件不存在时应以非零退出码失败', () => {
    withTempDir((tmpDir) => {
      const outputPath = path.join(tmpDir, 'summary.json');
      const result = runParser([path.join(tmpDir, 'missing.txt'), outputPath], tmpDir);
      assert.notEqual(result.status, 0);
    });
  });

  it('输入为空文件或非表格文本时应失败（与实现 D8 一致）', () => {
    withTempDir((tmpDir) => {
      const inputPath = path.join(tmpDir, 'empty.txt');
      const outputPath = path.join(tmpDir, 'summary.json');
      fs.writeFileSync(inputPath, '', 'utf8');

      const result = runParser([inputPath, outputPath], tmpDir);
      assert.notEqual(result.status, 0);
    });
  });
});

// ---------------------------------------------------------------------------
// parseNodeTestCoverage — 边界 (AC-6)
// ---------------------------------------------------------------------------

describe('parseNodeTestCoverage — 边界', () => {
  it('输入仅含表头无数据行时行为确定（0 或失败，与实现一致）', () => {
    withTempDir((tmpDir) => {
      const inputPath = path.join(tmpDir, 'header-only.txt');
      const outputPath = path.join(tmpDir, 'summary.json');
      fs.writeFileSync(
        inputPath,
        'File      | % Stmts | % Branch | % Funcs |\n----------|---------|----------|--------|\n',
        'utf8',
      );

      const result = runParser([inputPath, outputPath], tmpDir);
      // TODO: 实现完成后确认是 exit 1 还是 0 值降级
      assert.ok(result.status === 0 || result.status === 1);
    });
  });

  it('缺失列数不足4列时应失败（regex 需要 Stmts/Branch/Funcs/Lines 四列）', () => {
    withTempDir((tmpDir) => {
      const inputPath = path.join(tmpDir, 'partial.txt');
      const outputPath = path.join(tmpDir, 'summary.json');
      fs.writeFileSync(inputPath, 'all files | 75.00 |\n', 'utf8');

      const result = runParser([inputPath, outputPath], tmpDir);
      assert.notEqual(result.status, 0);
    });
  });
});

// ---------------------------------------------------------------------------
// parseNodeTestCoverage — CLI 参数 (AC-6)
// ---------------------------------------------------------------------------

describe('parseNodeTestCoverage — CLI 参数', () => {
  it('缺少输出路径参数时应以非零退出码失败', () => {
    withTempDir((tmpDir) => {
      const inputPath = path.join(tmpDir, 'out.txt');
      fs.writeFileSync(inputPath, VALID_STDOUT_FIXTURE, 'utf8');

      const result = runParser([inputPath], tmpDir);
      assert.notEqual(result.status, 0);
    });
  });

  it('输入路径为空字符串时应失败', () => {
    withTempDir((tmpDir) => {
      const outputPath = path.join(tmpDir, 'summary.json');
      const result = runParser(['', outputPath], tmpDir);
      assert.notEqual(result.status, 0);
    });
  });
});
