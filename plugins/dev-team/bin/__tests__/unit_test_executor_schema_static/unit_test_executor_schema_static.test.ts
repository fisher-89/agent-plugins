/**
 * 集成测试: unit-test-executor.md 静态 schema 校验
 *
 * 覆盖 AC-1 ~ AC-4：嵌套 coverage 对象、移除扁平字段与 HTML 引用、
 * 失败详情合并到 test_cases、integration_test 无 failures[]。
 *
 * @see openspec/changes/simplify-test-report-schema/test-design.md
 */

import * as fs from 'node:fs';
import * as path from 'node:path';

import { describe, expect, it } from 'vite-plus/test';

// ---------------------------------------------------------------------------
// 路径
// ---------------------------------------------------------------------------

/** vite-plus CWD 为 plugins/dev-team/bin/ */
const projectRoot = path.resolve(process.cwd(), '../../..');
const executorPath = path.resolve(projectRoot, 'plugins/dev-team/agents/unit-test-executor.md');

function readAgent(filePath: string): string {
  return fs.readFileSync(filePath, 'utf-8');
}

function extractSection(content: string, heading: string, nextHeading?: string): string {
  const start = content.indexOf(heading);
  if (start === -1) return '';
  const afterStart = content.slice(start + heading.length);
  if (nextHeading) {
    const end = afterStart.indexOf(nextHeading);
    return end === -1 ? afterStart : afterStart.slice(0, end);
  }
  return afterStart;
}

function extractJsonBlock(section: string): string {
  const match = section.match(/```json\r?\n([\s\S]*?)```/);
  return match?.[1] ?? '';
}

// ---------------------------------------------------------------------------
// AC-1: 嵌套 coverage 报告模板
// ---------------------------------------------------------------------------

describe('unit-test-executor.md — 嵌套 coverage 报告模板 (AC-1)', () => {
  it('Step 7 JSON 示例应含嵌套 coverage 对象（pass、measured、thresholds、by_framework、overrides）', () => {
    const content = readAgent(executorPath);
    const reportSection = extractSection(content, '### 7. Report writing', '## Constraints');
    const jsonBlock = extractJsonBlock(reportSection);
    expect(jsonBlock).toContain('"coverage"');
    expect(jsonBlock).toMatch(/"pass"\s*:/);
    expect(jsonBlock).toMatch(/"measured"\s*:/);
    expect(jsonBlock).toMatch(/"thresholds"\s*:/);
    expect(jsonBlock).toMatch(/"by_framework"\s*:/);
    expect(jsonBlock).toMatch(/"overrides"\s*:/);
  });

  it('报告 JSON 示例不应含扁平 coverage 字段（coverage_thresholds、coverage_pass 等）', () => {
    const content = readAgent(executorPath);
    const reportSection = extractSection(content, '### 7. Report writing', '## Constraints');
    const jsonBlock = extractJsonBlock(reportSection);
    expect(jsonBlock).not.toContain('coverage_thresholds');
    expect(jsonBlock).not.toContain('coverage_pass');
    expect(jsonBlock).not.toContain('coverage_by_framework');
    expect(jsonBlock).not.toContain('coverage_overrides');
    expect(jsonBlock).not.toContain('html_reports');
  });

  it('by_framework 条目不应含 html_report 字段', () => {
    const content = readAgent(executorPath);
    const reportSection = extractSection(content, '### 7. Report writing', '## Constraints');
    expect(reportSection).not.toMatch(/"html_report"\s*:/);
  });

  it('Process 步骤 2/3/5 不应引用 html_reports、html_report、index.html', () => {
    const content = readAgent(executorPath);
    const processSection = extractSection(content, '## Process', '## Constraints');
    const steps235 = processSection.split('### 6.')[0] ?? processSection;
    expect(steps235).not.toMatch(/html_reports/);
    expect(steps235).not.toMatch(/html_report/);
    expect(steps235).not.toMatch(/index\.html/);
  });
});

// ---------------------------------------------------------------------------
// AC-2: no-coverage 示例
// ---------------------------------------------------------------------------

describe('unit-test-executor.md — no-coverage 示例 (AC-2)', () => {
  it('no-coverage JSON 块应仅含 "coverage": null，不含其他 coverage 相关顶层字段', () => {
    const content = readAgent(executorPath);
    const noCoverageSection =
      content.split('When no coverage was generated')[1]?.split('When no integration')[0] ?? '';
    const jsonBlock = extractJsonBlock(noCoverageSection);
    expect(jsonBlock).toContain('"coverage": null');
    expect(jsonBlock).not.toContain('coverage_thresholds');
    expect(jsonBlock).not.toContain('coverage_pass');
    expect(jsonBlock).not.toContain('coverage_by_framework');
    expect(jsonBlock).not.toContain('html_reports');
  });
});

// ---------------------------------------------------------------------------
// AC-3: 失败详情在 test_cases
// ---------------------------------------------------------------------------

describe('unit-test-executor.md — 失败详情合并到 test_cases (AC-3)', () => {
  it('主报告 test_cases[] 中 status: "failed" 条目应含 line、error_type、error_message、stack_trace', () => {
    const content = readAgent(executorPath);
    const reportSection = extractSection(content, '### 7. Report writing', '## Constraints');
    expect(reportSection).toMatch(/"status"\s*:\s*"failed"/);
    expect(reportSection).toMatch(/"line"\s*:/);
    expect(reportSection).toMatch(/"error_type"\s*:/);
    expect(reportSection).toMatch(/"error_message"\s*:/);
    expect(reportSection).toMatch(/"stack_trace"\s*:/);
  });

  it('报告 JSON 示例与 Step 6/7 说明不应含顶层 failures 数组', () => {
    const content = readAgent(executorPath);
    const reportSection = extractSection(content, '### 7. Report writing', '## Constraints');
    const jsonBlock = extractJsonBlock(reportSection);
    expect(jsonBlock).not.toMatch(/^\s*"failures"\s*:/m);
  });

  it('Step 6 提取说明不应含 failures[]，失败详情应写入 test_cases', () => {
    const content = readAgent(executorPath);
    const step6 = extractSection(
      content,
      '### 6. Execute integration tests',
      '### 7. Report writing',
    );
    expect(step6).not.toMatch(/`failures\[\]`/);
    expect(step6).toMatch(/test_cases/);
  });
});

// ---------------------------------------------------------------------------
// AC-4: integration_test schema
// ---------------------------------------------------------------------------

describe('unit-test-executor.md — integration_test 子报告 schema (AC-4)', () => {
  it('integration_test 示例应含 test_cases[] 失败条目 error 字段，不含 failures[]', () => {
    const content = readAgent(executorPath);
    const reportSection = extractSection(content, '### 7. Report writing', '## Constraints');
    expect(reportSection).toMatch(/"integration_test"[\s\S]*"error_type"/);
    expect(reportSection).not.toMatch(/"integration_test"[\s\S]*"failures"\s*:/);
  });
});
