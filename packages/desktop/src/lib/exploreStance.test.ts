import { describe, expect, it } from 'vite-plus/test';

import { buildExplorePrompt } from './exploreStance';

/**
 * stance 前导模板为模块私有常量：断言以结构性事实承载——
 * - 前导以固定开篇句起头（模板第一句，模板归属本包）；
 * - 输出 = 前导 + 分隔线 + 用户输入原文（拼接非替换）；
 * - 前导在输出中恰出现一次。
 */
const PREAMBLE_HEAD = '你在探索模式下工作';
const SEPARATOR = '\n\n---\n\n';

/** 拆出输出尾部（用户输入原文）与其前的前导半边。 */
function splitPrompt(input: string): { head: string; tail: string } {
  const output = buildExplorePrompt(input);
  expect(output.length).toBeGreaterThanOrEqual(input.length);
  const tail = output.slice(output.length - input.length);
  const head = output.slice(0, output.length - input.length);
  return { head, tail };
}

describe('buildExplorePrompt：stance 前导拼接（AC-9）', () => {
  it('常规输入：输出以前导模板开头、以用户输入原文结尾、前导恰出现一次', () => {
    const { head, tail } = splitPrompt('调研一下重试策略');

    expect(head.startsWith(PREAMBLE_HEAD)).toBe(true);
    expect(head.endsWith(SEPARATOR)).toBe(true);
    expect(head).toContain('## 笔记落盘');
    expect(tail).toBe('调研一下重试策略');
    expect(buildExplorePrompt('调研一下重试策略').split(PREAMBLE_HEAD).length - 1).toBe(1);
  });

  it('空串输入：前导 + 空尾，不抛错', () => {
    const { head, tail } = splitPrompt('');

    expect(head.startsWith(PREAMBLE_HEAD)).toBe(true);
    expect(tail).toBe('');
    expect(() => buildExplorePrompt('')).not.toThrow();
  });

  it('输入含换行/引号/反引号/插值形貌/emoji：原样保留在尾部，不被模板截断或改写', () => {
    const input = '两行\n"引号" `反引` ${not_interp} 🎉 中文';
    const { head, tail } = splitPrompt(input);

    expect(tail).toBe(input);
    expect(head.startsWith(PREAMBLE_HEAD)).toBe(true);
    expect(head).not.toContain('${not_interp}');
  });

  it('超长输入（>1000 字符）：完整保留无截断', () => {
    const input = '长'.repeat(1200);
    const { head, tail } = splitPrompt(input);

    expect(tail).toBe(input);
    expect(tail.length).toBe(1200);
    expect(head.startsWith(PREAMBLE_HEAD)).toBe(true);
  });

  it('输入本身含前导文本片段：拼接非替换，输出前导仍只出现一次且片段原文入尾', () => {
    const input = `${PREAMBLE_HEAD}。这句话是用户输入的引用`;
    const output = buildExplorePrompt(input);
    const { head, tail } = splitPrompt(input);

    expect(tail).toBe(input);
    expect(head.startsWith(PREAMBLE_HEAD)).toBe(true);
    // 前导一次 + 输入回显一次 = 恰两次（若为替换语义则不会出现第二次回显）
    expect(output.split(PREAMBLE_HEAD).length - 1).toBe(2);
  });

  it('前导半边对所有输入恒等：输出恰为同一前导与输入的拼接', () => {
    const headOf = (input: string) => splitPrompt(input).head;

    expect(headOf('甲')).toBe(headOf(''));
    expect(headOf('乙')).toBe(headOf(''));
    expect(buildExplorePrompt('任意输入')).toBe(`${headOf('')}任意输入`);
  });
});
