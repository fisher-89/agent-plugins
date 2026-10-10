import { describe, expect, it } from 'vite-plus/test';

import { extractMarkdownTitle } from './explore-title';

/**
 * `extractMarkdownTitle` 纯函数（零依赖零 mock）：首行 `# <标题>` 解析——命中
 * 且标题 trim 后非空返回标题原文（已去首尾空白）；空内容 / 无首行 / 首行非一级
 * 标题 / 标题为空 → `null`（调用方据此保持 record.title 不变、零写入零报错）。
 */
describe('extractMarkdownTitle：首行一级标题解析（AC-5 标题解析半边）', () => {
  it('正向：`# 标题` + 正文 → 返回标题原文（去前导 # 与空白）', () => {
    expect(extractMarkdownTitle('# 接口重试策略\n\n正文第一段')).toBe('接口重试策略');
    expect(extractMarkdownTitle('# 标题')).toBe('标题');
    expect(extractMarkdownTitle('#   多余空白标题   \n正文')).toBe('多余空白标题');
  });

  it('边界：标题含空格 / 中文 / emoji / 特殊字符原样返回（零改写）', () => {
    expect(extractMarkdownTitle('# 带 空格 的标题 → 保留\n正文')).toBe('带 空格 的标题 → 保留');
    expect(extractMarkdownTitle('# emoji 🚀 标题\n正文')).toBe('emoji 🚀 标题');
    expect(extractMarkdownTitle('# a"b`c\\d\n正文')).toBe('a"b`c\\d');
    expect(extractMarkdownTitle('# tab\t与 制表符\n正文')).toBe('tab\t与 制表符');
  });

  it('边界：正文起首（非 `# `）→ null（无标题行不更新，D5）', () => {
    expect(extractMarkdownTitle('正文起首，没有标题行')).toBeNull();
    expect(extractMarkdownTitle('#无空格的井号行')).toBeNull();
    expect(extractMarkdownTitle('  # 缩进标题')).toBeNull();
  });

  it('边界：空白内容（空串 / 纯空白 / 纯换行）→ null', () => {
    expect(extractMarkdownTitle('')).toBeNull();
    expect(extractMarkdownTitle('   ')).toBeNull();
    expect(extractMarkdownTitle('\n\n正文')).toBeNull();
    expect(extractMarkdownTitle('\t\r\n')).toBeNull();
  });

  it('边界：首行非一级标题（## / ###）→ null（仅一级标题被解析）', () => {
    expect(extractMarkdownTitle('## 二级标题\n正文')).toBeNull();
    expect(extractMarkdownTitle('### 三级标题\n正文')).toBeNull();
    expect(extractMarkdownTitle('# #围栏式\n正文')).toBe('#围栏式');
  });

  it('边界：`#` 后仅空白 → null（空标题不产出），且不产生空白标题', () => {
    expect(extractMarkdownTitle('# \n正文')).toBeNull();
    expect(extractMarkdownTitle('#\n正文')).toBeNull();
    expect(extractMarkdownTitle('#     \n正文')).toBeNull();
    expect(extractMarkdownTitle('#   ')).toBeNull();
  });

  it('边界：只解析首行；超长标题（>1000 字符）不截断', () => {
    const longTitle = '长'.repeat(1200);
    expect(extractMarkdownTitle(`${longTitle}\n正文`)).toBeNull();
    expect(extractMarkdownTitle(`# ${longTitle}\n正文`)).toBe(longTitle);
    expect(extractMarkdownTitle(`# 短标题\n# 第二行不是标题`)).toBe('短标题');
  });

  it('边界：CRLF 行尾首行解析兼容（`# 标题\\r\\n正文`）', () => {
    expect(extractMarkdownTitle('# 标题\r\n正文')).toBe('标题');
    expect(extractMarkdownTitle('# 标题\r\n')).toBe('标题');
    expect(extractMarkdownTitle('# 标题\r\n# 第二个标题')).toBe('标题');
  });

  it('边界：无首行（内容仅换行符起首）→ null（空首行非标题）', () => {
    expect(extractMarkdownTitle('\n# 标题在后')).toBeNull();
  });
});
