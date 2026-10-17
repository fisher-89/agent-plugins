import { describe, expect, it } from 'vite-plus/test';

import { buildExplorePrompt } from './explore-stance';

/**
 * stance 前导模板为模块私有常量：断言以结构性事实承载——
 * - 前导以固定开篇句起头（模板第一句，模板归属本包）；
 * - 输出 = 前导 + 分隔线 + 用户输入原文（拼接非替换）；
 * - 前导在输出中恰出现一次；
 * - stance 按 `recordName` 的 `draft-` 前缀分两形（命名指令形 / 笔记定位形）。
 */

const PREAMBLE_HEAD = '你在探索模式下工作';
const SEPARATOR = '\n\n---\n\n';
/** 占位前缀锚点（与后端 `EXPLORE_PLACEHOLDER_PREFIX` 双源互链——前缀判别行为的可观测投影） */
const DRAFT_NAME_PREFIX = 'draft-';
/** 立场五条条目（两形公共段逐项锚） */
const STANCE_ITEMS = [
  '好奇而非说教',
  '多开线索而非审问',
  '善用 ASCII 图解',
  '随线索转向',
  '落地调研',
];
/** 两行笔记约定（两形公共段逐项锚） */
const NOTE_RULE_ITEMS = [
  '笔记首行写',
  '# <标题>',
  '笔记第二行写',
  'workflow_type: <requirement|test-only>',
  '默认追加',
];

/** 拆出输出尾部（用户输入原文）与其前的前导半边。 */
function splitPrompt(input: string, recordName = 'api-retry'): { head: string; tail: string } {
  const output = buildExplorePrompt(input, recordName);
  expect(output.length).toBeGreaterThanOrEqual(input.length);
  const tail = output.slice(output.length - input.length);
  const head = output.slice(0, output.length - input.length);
  return { head, tail };
}

// ---------------------------------------------------------------------------
// 两形分形：命名指令形（占位）/ 笔记定位形（已定名）
// ---------------------------------------------------------------------------

describe('buildExplorePrompt：命名指令形（draft- 占位记录）', () => {
  it('占位名输入：含 kebab 最终名指引与 create_explore_note(name, content) 定名叙事', () => {
    const output = buildExplorePrompt('调研一下重试策略', 'draft-1758123456789');

    expect(output.startsWith(PREAMBLE_HEAD)).toBe(true);
    expect(output).toContain('## 笔记定名与落盘');
    expect(output).toContain('kebab-case 最终名');
    expect(output).toContain('create_explore_note(name, content)');
    expect(output).toContain('name` 即最终名');
  });

  it('命名指令形：返回路径供 edit 续用、优先专用工具而非 bash', () => {
    const output = buildExplorePrompt('继续调研', 'draft-1758123456789');

    expect(output).toContain('返回笔记路径');
    expect(output).toContain('此后用 `edit` 修改该路径');
    expect(output).toContain('优先使用 `create_explore_note` 而非 bash 造文件');
  });

  it('命名指令形：MUST NOT 出现任何文件路径插值（openspec/explores 零出现）', () => {
    const output = buildExplorePrompt('继续调研', 'draft-1758123456789');

    expect(output).not.toContain('openspec/explores');
    expect(output).not.toContain('.md');
  });
});

describe('buildExplorePrompt：笔记定位形（已定名记录）', () => {
  it('已定名输入：插值记录名与笔记路径 openspec/explores/api-retry.md', () => {
    const output = buildExplorePrompt('继续追这条线索', 'api-retry');

    expect(output).toContain('## 笔记定位');
    expect(output).toContain('openspec/explores/api-retry.md');
    expect(output).toContain('记录名 = api-retry');
  });

  it('笔记定位形：edit 修改指引与文件缺失同名补建句', () => {
    const output = buildExplorePrompt('继续追这条线索', 'api-retry');

    expect(output).toContain('修改笔记用 `edit`');
    expect(output).toContain('文件缺失');
    expect(output).toContain('create_explore_note(name="api-retry", content=…)');
    expect(output).toContain('同名补建');
  });
});

describe('buildExplorePrompt：两形公共段共享', () => {
  const forms: Array<[string, string]> = [
    ['命名指令形', 'draft-1758123456789'],
    ['笔记定位形', 'api-retry'],
  ];

  it.each(forms)('%s：开篇句与立场五条均在（公共段单函数复用）', (_label, recordName) => {
    const output = buildExplorePrompt('输入', recordName);

    expect(output.startsWith(PREAMBLE_HEAD)).toBe(true);
    expect(output).toContain('## 立场');
    for (const item of STANCE_ITEMS) {
      expect(output).toContain(item);
    }
  });

  it.each(forms)('%s：两行笔记约定与中文回答句均在', (_label, recordName) => {
    const output = buildExplorePrompt('输入', recordName);

    for (const item of NOTE_RULE_ITEMS) {
      expect(output).toContain(item);
    }
    expect(output).toContain('回答与提问一律用中文');
  });
});

// ---------------------------------------------------------------------------
// 前缀判别精确性（draft- 前缀 = 命名指令形判别单点）
// ---------------------------------------------------------------------------

describe('buildExplorePrompt：前缀判别精确性', () => {
  it('中缀含 draft（my-draft-thing）→ 笔记定位形（非前缀起头不命中判别）', () => {
    const output = buildExplorePrompt('输入', 'my-draft-thing');

    expect(output).toContain('## 笔记定位');
    expect(output).not.toContain('## 笔记定名与落盘');
    expect(output).toContain('openspec/explores/my-draft-thing.md');
  });

  it('恰前缀空尾（draft-）→ 命名指令形（startsWith 判别的边界形态）', () => {
    const output = buildExplorePrompt('输入', DRAFT_NAME_PREFIX);

    expect(output).toContain('## 笔记定名与落盘');
    expect(output).not.toContain('openspec/explores');
  });

  it('占位前缀锚点：draft-<毫秒> 命名形态命中命名指令形（与后端 EXPLORE_PLACEHOLDER_PREFIX 双源互链）', () => {
    const naming = buildExplorePrompt('输入', 'draft-999');
    const locating = buildExplorePrompt('输入', 'draftx-999');

    expect(naming).toContain('## 笔记定名与落盘');
    expect(locating).toContain('## 笔记定位');
  });
});

// ---------------------------------------------------------------------------
// 前导拼接语义（userInput 边界回归）
// ---------------------------------------------------------------------------

describe('buildExplorePrompt：stance 前导拼接', () => {
  it('常规输入：输出以前导模板开头、以用户输入原文结尾、前导恰出现一次（前导唯一性回归）', () => {
    const { head, tail } = splitPrompt('调研一下重试策略');

    expect(head.startsWith(PREAMBLE_HEAD)).toBe(true);
    expect(head.endsWith(SEPARATOR)).toBe(true);
    expect(tail).toBe('调研一下重试策略');
    expect(
      buildExplorePrompt('调研一下重试策略', 'api-retry').split(PREAMBLE_HEAD).length - 1,
    ).toBe(1);
  });

  it('空串输入：前导 + 空尾，不抛错', () => {
    const { head, tail } = splitPrompt('');

    expect(head.startsWith(PREAMBLE_HEAD)).toBe(true);
    expect(tail).toBe('');
    expect(() => buildExplorePrompt('', 'api-retry')).not.toThrow();
  });

  it('输入含换行/引号/反引号/插值形貌/emoji：原样保留在尾部，前导不被改写', () => {
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

  it('前导半边对同形记录的所有输入恒等：输出恰为同一前导与输入的拼接', () => {
    const headOf = (input: string, name = 'api-retry') => splitPrompt(input, name).head;

    expect(headOf('甲')).toBe(headOf(''));
    expect(headOf('乙', 'draft-1')).toBe(headOf('', 'draft-1'));
    expect(buildExplorePrompt('任意输入', 'api-retry')).toBe(`${headOf('')}任意输入`);
  });
});

// ---------------------------------------------------------------------------
// 非法形态记录名判别（防御面：判别器不做 kebab 校验）
// ---------------------------------------------------------------------------

describe('buildExplorePrompt：非法形态记录名判别', () => {
  it('大写 / 下划线 / 路径形态非 draft- 前缀 → 一律笔记定位形且不抛错（判别器不做 kebab 校验）', () => {
    const names = ['Api-Retry', 'api_retry', 'a/b', '../escape', 'a:b'];

    for (const name of names) {
      expect(() => buildExplorePrompt('输入', name)).not.toThrow();
      const output = buildExplorePrompt('输入', name);
      expect(output).toContain('## 笔记定位');
      expect(output).toContain(`openspec/explores/${name}.md`);
      expect(output.endsWith('输入')).toBe(true);
    }
  });

  it('draft- 前缀的非法 kebab 形态仍命中命名指令形（判别仅按前缀，校验归 create 执行体）', () => {
    // 大写 DRAFT- 不以小写 draft- 起头 → 定位形（判别大小写敏感）
    const upper = buildExplorePrompt('输入', 'DRAFT-X');
    expect(upper).toContain('## 笔记定位');

    // 小写 draft- 前缀但非法 kebab（下划线段）仍命中命名指令形
    const lower = buildExplorePrompt('输入', 'draft-_bad');
    expect(lower).toContain('## 笔记定名与落盘');
    expect(lower).not.toContain('openspec/explores');
  });

  it('空 recordName：定位形、插值空名、不抛错（生产不可达防御面留痕）', () => {
    expect(() => buildExplorePrompt('输入', '')).not.toThrow();
    const output = buildExplorePrompt('输入', '');

    expect(output).toContain('## 笔记定位');
    expect(output).toContain('openspec/explores/.md');
    expect(output.endsWith('输入')).toBe(true);
  });
});
