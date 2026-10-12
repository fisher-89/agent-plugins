/** 类型标记行形态：`workflow_type:` + 空白 + 单个非空值（值内不含空白）。 */
const WORKFLOW_TYPE_LINE = /^workflow_type:\s*(\S+)\s*$/;

/**
 * 取笔记约定标记行 `workflow_type: <requirement|test-only>`（stance 要求写在
 * 首行标题之下）：逐行扫描**首个**命中行，值 case-insensitive 命中两合法值时
 * 返回 canonical 小写值；无标记行 / 首命中行的值非法 → `null`（调用方据此保持
 * `record.workflowType` 不变、零写入、零报错）。合法值权威仍是后端
 * `phase_table`，此处只作约定标记的前端解析。
 */
export function extractWorkflowType(content: string): 'requirement' | 'test-only' | null {
  for (const line of content.split(/\r?\n/)) {
    const match = WORKFLOW_TYPE_LINE.exec(line);
    if (match === null) continue;
    const value = match[1].toLowerCase();
    if (value === 'requirement' || value === 'test-only') return value;
    return null;
  }
  return null;
}
