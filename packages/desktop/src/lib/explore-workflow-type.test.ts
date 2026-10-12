import { describe, expect, it } from 'vite-plus/test';

import { extractWorkflowType } from './explore-workflow-type';

/**
 * `extractWorkflowType` 单测（desktop-test-only-flow AC-8 前端解析半边）：
 * 逐行扫描首个约定标记行 `^workflow_type:\s*(\S+)\s*$`，值 case-insensitive
 * 命中 `{requirement, test-only}` → canonical 小写值；无标记行 / 首个命中行的
 * 值非法 → `null`（调用方据此保持记录不变、零写入、零报错）。纯函数零 mock。
 */

describe('extractWorkflowType：约定标记行解析（AC-8）', () => {
  it('正向：两合法值逐值解析为 canonical 小写值', () => {
    expect(extractWorkflowType('workflow_type: test-only')).toBe('test-only');
    expect(extractWorkflowType('workflow_type: requirement')).toBe('requirement');
  });

  it('正向：非首行命中——逐行扫描首个匹配行返回，前序行不干扰', () => {
    const content = ['# 我的标题', '', '## 背景', 'workflow_type: test-only', '', '正文段落'].join(
      '\n',
    );

    expect(extractWorkflowType(content)).toBe('test-only');
    // 标记行前的散行内容含冒号亦不误判（只认标记行形态）
    expect(extractWorkflowType('## 说明: 补充\nworkflow_type: requirement')).toBe('requirement');
  });

  it('正向：值大小写不敏感 + 规范小写出线', () => {
    expect(extractWorkflowType('workflow_type: TEST-ONLY')).toBe('test-only');
    expect(extractWorkflowType('workflow_type: Test-Only')).toBe('test-only');
    expect(extractWorkflowType('workflow_type: REQUIREMENT')).toBe('requirement');
    expect(extractWorkflowType('workflow_type: Requirement')).toBe('requirement');
  });

  it('正向：值首尾空白 / 行首多余空白（分隔符容忍）', () => {
    expect(extractWorkflowType('workflow_type:   test-only  ')).toBe('test-only');
    expect(extractWorkflowType('# 标题\r\nworkflow_type: test-only\r\n正文')).toBe('test-only');
  });

  it('正向：首个命中行决定结果（后续合规行不覆盖）', () => {
    const content = ['workflow_type: requirement', 'workflow_type: test-only'].join('\n');

    expect(extractWorkflowType(content)).toBe('requirement');
  });

  it('异常：无标记行 / 非法值 → null（不抛错、不臆测类型）', () => {
    expect(extractWorkflowType('# 标题\n\n只有正文，没有约定标记行')).toBeNull();
    expect(extractWorkflowType('workflow_type: bug-fix')).toBeNull();
    expect(extractWorkflowType('workflow_type: refactor')).toBeNull();
    expect(extractWorkflowType('workflow_type: test_only')).toBeNull();
    expect(extractWorkflowType('workflow_type: requirement-workflow')).toBeNull();
  });

  it('异常：值缺失（标记行后为空 / 只有空白）→ null，解析失败不报错', () => {
    expect(extractWorkflowType('workflow_type:')).toBeNull();
    expect(extractWorkflowType('workflow_type:   ')).toBeNull();
    expect(extractWorkflowType('workflow_type:\t')).toBeNull();
    expect(() => extractWorkflowType('workflow_type:')).not.toThrow();
  });

  it('异常：首个命中行值非法即返回 null（不再扫描后续合规行）', () => {
    const content = ['workflow_type: bug-fix', 'workflow_type: test-only'].join('\n');

    expect(extractWorkflowType(content)).toBeNull();
  });

  it('边界：空串 / 纯空白 / 仅换行 → null', () => {
    expect(extractWorkflowType('')).toBeNull();
    expect(extractWorkflowType('   ')).toBeNull();
    expect(extractWorkflowType('\n')).toBeNull();
    expect(extractWorkflowType('\n\n\n')).toBeNull();
    expect(extractWorkflowType('\r\n\r\n')).toBeNull();
    expect(extractWorkflowType('\t \t')).toBeNull();
  });

  it('边界：非约定行（标题行 / 相邻词形）→ null（不臆测标记）', () => {
    expect(extractWorkflowType('# 标题')).toBeNull();
    expect(extractWorkflowType('## 标题')).toBeNull();
    // 键为字面小写 workflow_type；变体拼写不是约定标记行
    expect(extractWorkflowType('Workflow_Type: TEST-ONLY')).toBeNull();
    expect(extractWorkflowType('workflow-type: test-only')).toBeNull();
    // 前缀带字面（非行首）亦非标记行（正则锚定行首）
    expect(extractWorkflowType('> workflow_type: test-only')).toBeNull();
    expect(extractWorkflowType('  workflow_type: test-only')).toBeNull();
    // 行尾携额外内容（值含空白）不命中
    expect(extractWorkflowType('workflow_type: test-only 说明')).toBeNull();
    expect(extractWorkflowType('workflow_type: test-only # 注释')).toBeNull();
  });

  it('边界：超长内容（>1000 字符）不截断——尾部标记行照常命中', () => {
    const filler = '长'.repeat(1200);
    const content = `# 标题\n${filler}\nworkflow_type: test-only`;
    expect(content.length).toBeGreaterThan(1000);

    expect(extractWorkflowType(content)).toBe('test-only');

    const withoutMarker = `# 标题\n${filler}`;
    expect(extractWorkflowType(withoutMarker)).toBeNull();
  });

  it('边界：特殊字符（换行 / NUL / emoji / 制表符）不崩、不误命中', () => {
    const content = [
      '# 标题 🚀',
      '正文含 NUL \u0000 与 emoji 🎉',
      '\t制表符开头行',
      'workflow_type: test-only',
    ].join('\n');

    expect(extractWorkflowType(content)).toBe('test-only');
    expect(extractWorkflowType('\u0000')).toBeNull();
    expect(extractWorkflowType('workflow_type:\u0000test-only')).toBeNull();
    expect(extractWorkflowType('🚀 workflow_type: requirement')).toBeNull();
  });

  it('边界：返回值恒为三态闭集（合法两值或 null）', () => {
    const samples = [
      'workflow_type: test-only',
      'workflow_type: requirement',
      'workflow_type: TEST-ONLY',
      'workflow_type: bug-fix',
      '',
    ];

    for (const sample of samples) {
      const value = extractWorkflowType(sample);
      expect(['requirement', 'test-only', null]).toContain(value);
    }
  });
});
