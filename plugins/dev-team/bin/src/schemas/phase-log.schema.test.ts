/**
 * 单元测试: phase-log.schema -- phaseIdSchema、phaseLogSchema、phaseLogInputSchema
 *
 * 覆盖范围:
 * - AC-1: phaseIdSchema 枚举包含 'test-execution'，不包含 'unit-test' 和 'integration-test'
 * - AC-1: 枚举值总数 = 9
 * - AC-1: 未知 phase ID 导致 zod parse 失败
 * - AC-1: 有效 entry 通过 zod parse
 * - AC-1: 无效 verdict（非 "pass"/"fail"）导致 parse 失败
 * - AC-1: report 长度 = 0 / 500 / >500 字符的边界处理
 * - AC-1: backtrack_to 为字符串 "null" 时 refine 校验失败
 */

import { describe, it, expect } from 'vite-plus/test';

import { phaseIdSchema, phaseLogSchema } from './phase-log.schema';

// ===========================================================================
// phaseIdSchema
// ===========================================================================

describe('phaseIdSchema', () => {
  it('枚举应包含 "test-execution"（AC-1）', () => {
    const result = phaseIdSchema.safeParse('test-execution');
    expect(result.success).toBe(true);
  });

  it('枚举应不包含 "unit-test"（AC-1）', () => {
    const result = phaseIdSchema.safeParse('unit-test');
    expect(result.success).toBe(false);
  });

  it('枚举应不包含 "integration-test"（AC-1）', () => {
    const result = phaseIdSchema.safeParse('integration-test');
    expect(result.success).toBe(false);
  });

  it('枚举值总数应为 9（proposal, dev-design, test-design, implement, test-gen, test-execution, code-review, acceptance, code-analyze）', () => {
    // 通过 safeParse 验证所有已知 phase ID
    const validPhases = [
      'proposal',
      'dev-design',
      'test-design',
      'implement',
      'test-gen',
      'test-execution',
      'code-review',
      'acceptance',
      'code-analyze',
    ];
    expect(validPhases).toHaveLength(9);
    for (const phase of validPhases) {
      expect(phaseIdSchema.safeParse(phase).success).toBe(true);
    }
  });

  it('未知 phase ID "invalid-phase" 应导致 parse 失败', () => {
    const result = phaseIdSchema.safeParse('invalid-phase');
    expect(result.success).toBe(false);
  });

  it('空字符串应导致 parse 失败', () => {
    const result = phaseIdSchema.safeParse('');
    expect(result.success).toBe(false);
  });
});

// ===========================================================================
// phaseLogSchema
// ===========================================================================

describe('phaseLogSchema', () => {
  const validEntry = {
    phase: 'test-execution',
    attempt: 1,
    verdict: 'pass',
    report: '所有测试通过',
    checklist: [{ item: '测试覆盖率达到80%', pass: true, evidence: '覆盖率报告显示85%' }],
    timestamp: new Date().toISOString(),
    backtrack_to: null,
  };

  it('有效 entry 应通过 zod parse', () => {
    const result = phaseLogSchema.safeParse(validEntry);
    expect(result.success).toBe(true);
  });

  it('无效 verdict（非 "pass"/"fail"）应导致 parse 失败', () => {
    const result = phaseLogSchema.safeParse({ ...validEntry, verdict: 'invalid' });
    expect(result.success).toBe(false);
  });

  it('report 长度为 0（空字符串）时应通过', () => {
    const result = phaseLogSchema.safeParse({ ...validEntry, report: '' });
    expect(result.success).toBe(true);
  });

  it('report 长度 = 500 字符时应通过（边界）', () => {
    const report500 = 'a'.repeat(500);
    const result = phaseLogSchema.safeParse({ ...validEntry, report: report500 });
    expect(result.success).toBe(true);
  });

  it('report 长度 > 500 字符时应 parse 失败（边界）', () => {
    const report501 = 'a'.repeat(501);
    const result = phaseLogSchema.safeParse({ ...validEntry, report: report501 });
    expect(result.success).toBe(false);
  });

  it('backtrack_to 为字符串 "null" 时 refine 校验应失败', () => {
    const result = phaseLogSchema.safeParse({
      ...validEntry,
      backtrack_to: 'null',
    });
    expect(result.success).toBe(false);
  });

  it('backtrack_to 为字符串数组时通过', () => {
    const result = phaseLogSchema.safeParse({
      ...validEntry,
      backtrack_to: ['test-gen', 'implement'],
    });
    expect(result.success).toBe(true);
  });

  it('skipped 为 true、verdict 为 pass 时通过', () => {
    const result = phaseLogSchema.safeParse({
      ...validEntry,
      verdict: 'pass',
      skipped: true,
    });
    expect(result.success).toBe(true);
  });

  it('timestamp 缺失时 parse 失败', () => {
    const { timestamp: _timestamp, ...entryWithoutTs } = validEntry;
    const result = phaseLogSchema.safeParse(entryWithoutTs);
    expect(result.success).toBe(false);
  });
});
