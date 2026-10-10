/** 首行标题形态：`#` + 空白 + 非空标题（`## …` 等更深层级不匹配）。 */
const TITLE_LINE = /^#\s+(.+?)\s*$/;

/**
 * 取笔记首行的 `# <标题>`：命中且标题 trim 后非空 → 标题原文（已去首尾空白）；
 * 内容为空 / 无首行 / 首行非一级标题 / 标题为空 → `null`（调用方据此保持
 * `record.title` 不变、零写入、零报错）。
 */
export function extractMarkdownTitle(content: string): string | null {
  const firstLine = content.split(/\r?\n/, 1)[0] ?? '';
  const match = TITLE_LINE.exec(firstLine);
  if (match === null) return null;
  const title = match[1].trim();
  return title.length > 0 ? title : null;
}
