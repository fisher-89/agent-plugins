import type { TurnSummary } from '../types/dto';
import type { AgentUIMessage } from './agent-adapter';

/**
 * 对话透镜的呈现层纯函数：可见性判据单点 + 吸附层几何。
 * 零 IO、零 React、零 DOM 依赖（入参只读，产出新数组 / 新对象）。
 *
 * 落 `src/lib/**` 而非 `components/agent/` 的两条硬理由：
 * 1. `knip` 的 `project` 排除 `*.test.*`——组件内为测试开 `export` 会被判 unused；
 * 2. `src/lib/**` 是架构模型的封闭排除面（前端模型只对 `components/` / `views/`
 *    取元素，指向 hooks / lib / types 的 import 不建边）⇒ 此处新增文件模型零漂移。
 */

/** 吸附层一条占位块的几何 */
export interface UserTurnSpan {
  /** 过滤后列表索引 */
  index: number;
  /** 虚拟器测量起点（已含 `paddingStart`），与虚拟项 `translateY` 同基准 */
  start: number;
  /** 该轮跨度：下一条 user 消息 `start` 减本条 `start`，末轮为 `totalSize − start` */
  span: number;
}

/** 工具部件判据：`tool-<name>` 命名，`dynamic-tool` 不在信封内。
 * 与 `agent-blocks.tsx` 的 `isToolPart` 同一口径——在此复写而非 import，
 * 避免 `src/lib/**` 反向依赖 `components/`。 */
function isToolPartType(type: string): boolean {
  return type.startsWith('tool-') && type !== 'dynamic-tool';
}

/** 可见部件：`text` / `reasoning` / `tool-<name>` 三类计入；
 * `data-*` 与 `step-start` 不计（实时态的空壳消息由此不产空气泡）。
 * 两个生产消费点：`filterConversation`（过滤判据）与 `ChatBubbleBase`
 * 的空部件状态行判据。 */
export function hasVisibleParts(message: AgentUIMessage): boolean {
  return message.parts.some(
    (part) => part.type === 'text' || part.type === 'reasoning' || isToolPartType(part.type),
  );
}

/** 轮状态（唯一来源：该轮 assistant 消息的 `metadata.record.status`） */
function turnStatusOf(message: AgentUIMessage): TurnSummary['status'] | null {
  return message.metadata?.record?.status ?? null;
}

/**
 * 对话透镜可见性单点（system 剔除 → 可见部件 → 空内容轮按 `metadata.record.status`
 * 判定 `failed` / `stopped` 保留、`completed` 或轮行缺席剔除）。判据语义与分页层
 * 退役前的内联实现逐字等价，顺序保持。MUST NOT 在别处重复实现第二份。
 * 虚拟窗口的项数与吸附层几何均以本函数产出为唯一真相。
 */
export function filterConversation(all: AgentUIMessage[]): AgentUIMessage[] {
  return all.filter(
    (message) =>
      message.role !== 'system' &&
      (hasVisibleParts(message) ||
        turnStatusOf(message) === 'failed' ||
        turnStatusOf(message) === 'stopped'),
  );
}

/**
 * 吸附层占位块几何：仅为 `role === 'user'` 的索引产条（升序）。
 * `span` 即 sticky 的被夹边界——下一轮提问容器到达其底边即把上一条顶走，零缓冲。
 * `measurements[index]` 缺席（越界 / 尚未测量）时该条跳过；末轮退化到 `totalSize`。
 */
export function userTurnSpans(
  messages: AgentUIMessage[],
  measurements: readonly { start: number }[],
  totalSize: number,
): UserTurnSpan[] {
  const starts: { index: number; start: number }[] = [];
  messages.forEach((message, index) => {
    if (message.role !== 'user') return;
    const start = measurements[index]?.start;
    if (start !== undefined) starts.push({ index, start });
  });
  return starts.map(({ index, start }, position) => {
    const end = starts[position + 1]?.start ?? totalSize;
    return { index, start, span: Math.max(0, end - start) };
  });
}

/**
 * 会话身份（贴底复位键）：取首条携 `metadata.record.sessionId` 的消息的会话 id。
 * 同一会话的追加与流式增长不改变返回值，换会话必变。
 * 全体缺席（流式首轮未收口 / 收口重放）返回 `null` = 未知——`null` 不参与
 * 「换会话」比较，故首轮 `record` 落库（`null → 已知`）不会把上滑用户拽回底部。
 */
export function conversationIdentity(messages: AgentUIMessage[]): string | null {
  for (const message of messages) {
    const record = message.metadata?.record;
    if (record !== undefined && record !== null) return record.sessionId;
  }
  return null;
}
