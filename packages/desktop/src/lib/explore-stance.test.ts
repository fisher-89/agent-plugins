import { describe, expect, it } from 'vite-plus/test';

import { buildExplorePrompt } from './explore-stance';

/**
 * stance 前导模板为模块私有常量：断言以结构性事实承载——
 * - 前导以固定开篇句起头（模板第一句，模板归属本包）；
 * - 输出 = 前导 + 分隔线 + 用户输入原文（拼接非替换）；
 * - 前导在输出中恰出现一次。
 */
const PREAMBLE_HEAD = '你在探索模式下工作';
/** 当前探索记录 name（stance 按 topic 插值；基设固定值） */
const TOPIC = 'api-retry';
const SEPARATOR = '\n\n---\n\n';

/** 拆出输出尾部（用户输入原文）与其前的前导半边。 */
function splitPrompt(input: string): { head: string; tail: string } {
  const output = buildExplorePrompt(input, TOPIC);
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
    expect(buildExplorePrompt('调研一下重试策略', TOPIC).split(PREAMBLE_HEAD).length - 1).toBe(1);
  });

  it('空串输入：前导 + 空尾，不抛错', () => {
    const { head, tail } = splitPrompt('');

    expect(head.startsWith(PREAMBLE_HEAD)).toBe(true);
    expect(tail).toBe('');
    expect(() => buildExplorePrompt('', TOPIC)).not.toThrow();
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
    const output = buildExplorePrompt(input, TOPIC);
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
    expect(buildExplorePrompt('任意输入', TOPIC)).toBe(`${headOf('')}任意输入`);
  });
});

describe('buildExplorePrompt：name 注入与首行标题约定（AC-4 模板注入半边）', () => {
  it('正向：输出含 topic（文件名 MUST 等于 topic 指令）与「笔记首行写 # <标题>」约定', () => {
    const output = buildExplorePrompt('调研一下', 'api-retry');

    expect(output).toContain('api-retry');
    expect(output).toContain('文件名 MUST 等于 api-retry');
    expect(output).toContain('笔记首行写');
    expect(output).toContain('# <标题>');
  });

  it('正向：两参拼接 = 前导（含 name 注入段）+ 分隔线 + 用户输入原文（拼接非替换）', () => {
    const input = '继续追这条线索';
    const head = buildExplorePrompt('', 'layout-design');
    const output = buildExplorePrompt(input, 'layout-design');

    expect(output).toBe(`${head}${input}`);
    expect(output.endsWith(input)).toBe(true);
    expect(head.endsWith(SEPARATOR)).toBe(true);
    expect(output.split(PREAMBLE_HEAD).length - 1).toBe(1);
  });

  it('边界：topic 含空格 / 中文 / 超长 / emoji 原样注入不截断、不抛错', () => {
    const topics = ['has space', '中文话题', '长'.repeat(1200), 'emoji-🚀-topic', ''];

    for (const topic of topics) {
      expect(() => buildExplorePrompt('输入', topic)).not.toThrow();
      const head = buildExplorePrompt('', topic);
      expect(head).toContain(`文件名 MUST 等于 ${topic}`);
    }
  });

  it('边界：空 topic 不抛错，约定段照常输出（name 兜底由调用方 / agent 承接）', () => {
    const output = buildExplorePrompt('输入', '');

    expect(output.startsWith(PREAMBLE_HEAD)).toBe(true);
    expect(output).toContain('## 笔记落盘');
    expect(output).toContain('MUST NOT 自拟其它文件名');
    expect(output.endsWith('输入')).toBe(true);
  });
});

describe('buildExplorePrompt：workflow_type 标记行约定（AC-8 stance 半边）', () => {
  it('正向：输出含第二行 `workflow_type: <requirement|test-only>` 标记行约定（与首行标题约定同构）', () => {
    const output = buildExplorePrompt('调研一下', TOPIC);

    expect(output).toContain('笔记第二行写');
    expect(output).toContain('workflow_type: <requirement|test-only>');
    // 与首行标题约定同构（同段落内的并列条目、同「笔记第 N 行写 …」句式）
    expect(output).toContain('笔记首行写');
    expect(output).toContain('# <标题>');
  });

  it('正向：输出含判断标准（目标是补测试 / 生成测试用例 → test-only，否则 requirement）', () => {
    const output = buildExplorePrompt('调研一下', TOPIC);

    expect(output).toContain('目标是补测试');
    expect(output).toContain('生成测试用例');
    expect(output).toContain('test-only');
    expect(output).toContain('否则 requirement');
    // 两合法值形态均在约定内（值集与后端 phase_table 同宽）
    expect(output).toContain('requirement');
  });

  it('边界：首行标题约定与第二行类型约定并存且顺序正确（首行在前、第二行在后）', () => {
    const output = buildExplorePrompt('输入', TOPIC);

    const titleIndex = output.indexOf('笔记首行写');
    const workflowTypeIndex = output.indexOf('笔记第二行写');

    expect(titleIndex).toBeGreaterThanOrEqual(0);
    expect(workflowTypeIndex).toBeGreaterThanOrEqual(0);
    expect(titleIndex).toBeLessThan(workflowTypeIndex);
    // 两条约定均在「## 笔记落盘」段内（段落归属不漂移）
    const sectionIndex = output.indexOf('## 笔记落盘');
    expect(sectionIndex).toBeGreaterThanOrEqual(0);
    expect(sectionIndex).toBeLessThan(titleIndex);
  });

  it('边界：文件名约定段与两条标记行约定并存（段内条目齐备且不互相吞并）', () => {
    const output = buildExplorePrompt('输入', TOPIC);

    for (const clause of [
      `文件名 MUST 等于 ${TOPIC}`,
      '笔记首行写',
      '笔记第二行写',
      '默认追加',
      '若文件尚不存在',
    ]) {
      expect(output).toContain(clause);
    }
  });

  it('异常：空 userInput / 空 topic 下类型约定段照常输出，不崩（既有模板拼接持衡）', () => {
    for (const topic of ['', 'has space', '中文话题', 'emoji-🚀-topic']) {
      expect(() => buildExplorePrompt('', topic)).not.toThrow();
      const output = buildExplorePrompt('', topic);

      expect(output).toContain('workflow_type: <requirement|test-only>');
      expect(output).toContain('目标是补测试');
      expect(output.startsWith(PREAMBLE_HEAD)).toBe(true);
    }
  });
});
