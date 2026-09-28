/**
 * 流程图布局常量与坐标纯函数（9 列手工坐标，布局即语义；不引入 dagre / elk）。
 *
 * 坐标约定（design 数据模型）：
 * - 列容器为画布绝对坐标：x = colIndex × COL_W，y = 0；容器宽 COL_W，
 *   高 = COLUMN_HEADER_H + 列内节点数 × ROW_H + COLUMN_PAD_Y（高度由
 *   ChangeFlowGraph 按列内事件数机械换算，不在此函数职责内）。
 * - 事件节点为相对父列坐标（react-flow subflow：parentId + extent 'parent'）：
 *   x = COLUMN_PAD_X，y = COLUMN_HEADER_H + order × ROW_H。
 *   与 AC-1 公式 x = 列索引 × COL_W、y = 执行序 × ROW_H 一一对应，
 *   COLUMN_HEADER_H 即列容器自身的常量偏移项。
 */
import type { FlowColumn, FlowNode } from './types';

/** 固定 9 站流水线前端镜像（与 Rust queries::detail 的 PIPELINE_PHASES 同序，改动需双侧同步） */
export const PIPELINE_PHASES: readonly string[] = [
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

export const COL_W = 260;
const COL_W_GAP = 20;
export const ROW_H = 128;
export const COLUMN_HEADER_H = 48;
export const COLUMN_PAD_X = 18;
export const COLUMN_PAD_Y = 16;

/** react-flow 就绪坐标：列容器为画布绝对坐标，事件节点为相对父列坐标 */
export function nodePosition(node: FlowColumn | FlowNode): { x: number; y: number } {
  if ('kind' in node) {
    return { x: COLUMN_PAD_X, y: COLUMN_HEADER_H + node.order * ROW_H };
  }
  return { x: node.colIndex * (COL_W + COL_W_GAP), y: 0 };
}
