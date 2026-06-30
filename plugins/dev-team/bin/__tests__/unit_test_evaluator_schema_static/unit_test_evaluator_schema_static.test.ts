/**
 * 集成测试: unit-test-evaluator.md 静态消费逻辑校验
 *
 * 覆盖 AC-5 ~ AC-8：新 schema 字段路径、决策树输入源切换、
 * coverage null 跳过、findings 无 HTML 引用。
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
const evaluatorPath = path.resolve(projectRoot, 'plugins/dev-team/agents/unit-test-evaluator.md');

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

// ---------------------------------------------------------------------------
// AC-5: U1/U3/Step 1 新 schema
// ---------------------------------------------------------------------------

describe('unit-test-evaluator.md — U1/U3/Step 1 新 schema (AC-5)', () => {
  it('Static Checklist U1 必需字段应含 coverage（object 或 null）、test_cases', () => {
    const content = readAgent(evaluatorPath);
    const checklist = extractSection(content, '## Static Checklist', '## Input');
    expect(checklist).toMatch(/coverage/);
    expect(checklist).toMatch(/test_cases/);
    expect(checklist).not.toMatch(/coverage_thresholds/);
    expect(checklist).not.toMatch(/coverage_pass/);
    expect(checklist).not.toMatch(/coverage_by_framework/);
    expect(checklist).not.toMatch(/html_reports/);
    expect(checklist).not.toMatch(/\bfailures\b/);
  });

  it('U3 判定依据应引用 coverage.pass === true 或 coverage === null', () => {
    const content = readAgent(evaluatorPath);
    const checklist = extractSection(content, '## Static Checklist', '## Input');
    expect(checklist).toMatch(/coverage\.pass/);
    expect(checklist).toMatch(/coverage === null/);
  });

  it('Step 1 应要求 coverage 为嵌套对象（含 pass, measured, thresholds, by_framework, overrides）或 null', () => {
    const content = readAgent(evaluatorPath);
    const step1 = extractSection(content, '### Step 1:', '### Step 2:');
    expect(step1).toMatch(/coverage\.pass/);
    expect(step1).toMatch(/coverage\.measured/);
    expect(step1).toMatch(/coverage\.thresholds/);
    expect(step1).toMatch(/coverage\.by_framework/);
    expect(step1).toMatch(/coverage\.overrides/);
    expect(step1).not.toMatch(/coverage_thresholds/);
    expect(step1).not.toMatch(/coverage_pass/);
  });

  it('Step 3 覆盖率子检查应引用 coverage.pass、coverage.measured、coverage.thresholds、coverage.overrides', () => {
    const content = readAgent(evaluatorPath);
    const step3 = extractSection(content, '### Step 3:', '### Step 4:');
    expect(step3).toMatch(/coverage\.pass/);
    expect(step3).toMatch(/coverage\.measured/);
    expect(step3).toMatch(/coverage\.thresholds/);
    expect(step3).toMatch(/coverage\.overrides/);
  });
});

// ---------------------------------------------------------------------------
// AC-6: Step 4 决策树输入源
// ---------------------------------------------------------------------------

describe('unit-test-evaluator.md — Step 4 决策树输入 (AC-6)', () => {
  it('Step 4 应明确从 test_cases.filter(c => c.status === "failed") 读取 error_type、file、line', () => {
    const content = readAgent(evaluatorPath);
    const step4 = extractSection(content, '### Step 4:', '### Step 5:');
    expect(step4).toMatch(/test_cases\.filter/);
    expect(step4).toMatch(/status === "failed"/);
    expect(step4).toMatch(/error_type/);
    expect(step4).toMatch(/file/);
    expect(step4).toMatch(/line/);
  });

  it('决策树 5 类分类与优先级文本应保持（设计冲突 > 语法 > 逻辑 > 接口 > 无法判断）', () => {
    const content = readAgent(evaluatorPath);
    const step4 = extractSection(content, '### Step 4:', '### Step 5:');
    expect(step4).toMatch(/语法错误/);
    expect(step4).toMatch(/逻辑错误/);
    expect(step4).toMatch(/设计冲突/);
    expect(step4).toMatch(/接口签名不匹配/);
    expect(step4).toMatch(/无法判断/);
    expect(step4).toMatch(
      /Design conflict \(4\) > Syntax error \(1\) > Logic error \(2\) > Interface mismatch \(3\) > Coverage failure \(5\) > Unknown \(6\)/,
    );
  });
});

// ---------------------------------------------------------------------------
// AC-7: coverage null 跳过
// ---------------------------------------------------------------------------

describe('unit-test-evaluator.md — coverage null 跳过 (AC-7)', () => {
  it('Step 3 含 coverage === null 时 evidence「覆盖率检查未配置或生成失败，跳过」', () => {
    const content = readAgent(evaluatorPath);
    const step3 = extractSection(content, '### Step 3:', '### Step 4:');
    expect(step3).toMatch(/coverage === null/);
    expect(step3).toContain('覆盖率检查未配置或生成失败，跳过');
  });

  it('Constraints 含 coverage === null 时覆盖率检查始终 pass', () => {
    const content = readAgent(evaluatorPath);
    const constraints = extractSection(content, '## Constraints', undefined);
    expect(constraints).toMatch(/coverage === null/);
    expect(constraints).toMatch(/pass the coverage check/);
  });
});

// ---------------------------------------------------------------------------
// AC-8: Step 5 findings 无 HTML
// ---------------------------------------------------------------------------

describe('unit-test-evaluator.md — Step 5 findings 无 HTML (AC-8)', () => {
  it('Step 5 findings 模板应引用 coverage.measured、coverage.pass、coverage.by_framework[].measured', () => {
    const content = readAgent(evaluatorPath);
    const step5 = extractSection(content, '### Step 5:', '### Step 6:');
    expect(step5).toMatch(/coverage\.measured/);
    expect(step5).toMatch(/coverage\.pass/);
    expect(step5).toMatch(/coverage\.by_framework/);
    expect(step5).not.toMatch(/html_reports/);
    expect(step5).not.toMatch(/html_report/);
    expect(step5).not.toMatch(/index\.html/);
  });

  it('全文不应含 coverage_pass、coverage_thresholds、html_reports（grep 断言）', () => {
    const content = readAgent(evaluatorPath);
    expect(content).not.toMatch(/\bcoverage_pass\b/);
    expect(content).not.toMatch(/\bcoverage_thresholds\b/);
    expect(content).not.toMatch(/\bhtml_reports\b/);
  });
});
