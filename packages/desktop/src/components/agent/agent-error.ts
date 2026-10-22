import type { AgentEvent, TurnSummary } from '../../types/dto';

/** 会话失败横幅的展示载荷：友好标题 / 可操作提示 / 原始错误（折叠诊断） */
export interface AgentFailure {
  title: string;
  hint: string;
  raw: string;
}

interface ErrorCopy {
  title: string;
  hint: string;
}

interface ErrorRule extends ErrorCopy {
  match: (text: string) => boolean;
}

const FALLBACK_COPY: ErrorCopy = {
  title: 'Agent 运行失败',
  hint: '展开下方原始错误查看详情；若为配置问题，请检查「Agent 管理」中的 provider。',
};

/** 已知底层错误文案表（顺序即优先级；text 已小写化） */
const ERROR_RULES: ErrorRule[] = [
  {
    match: (text) => text.includes('invalid uri character'),
    title: 'Provider 地址无效',
    hint: 'base_url 含有非法字符（常见为开头/结尾的空格或制表符）。请到「Agent 管理」编辑该 provider，修正地址后重试。',
  },
  {
    match: (text) => text.includes('invalid content type') && text.includes('text/html'),
    title: '端点返回了网页而不是 API 响应',
    hint: '请检查 provider 的 base_url 是否指向正确的 API 根路径；网关登录页、404 页面或反向代理错误页都会导致该问题。',
  },
  {
    match: (text) =>
      /\b401\b/.test(text) || text.includes('authentication') || text.includes('unauthorized'),
    title: 'Provider 认证失败',
    hint: '请检查 provider 的 api_key 是否正确、是否有权限访问该模型。',
  },
  {
    match: (text) =>
      /\b402\b/.test(text) || text.includes('insufficient balance') || text.includes('quota'),
    title: '账户额度不足',
    hint: 'Provider 账户余额或配额不足，请充值或更换凭据后重试。',
  },
  {
    match: (text) => /\b429\b/.test(text) || text.includes('rate limit'),
    title: '请求过于频繁',
    hint: 'Provider 触发了限流，请稍后重试。',
  },
  {
    match: (text) => /\b404\b/.test(text) || text.includes('not found'),
    title: '模型或端点不存在',
    hint: '请检查 provider 的 base_url 与模型名是否匹配该端点。',
  },
  {
    match: (text) => text.includes('max_tokens'),
    title: '输出上限被 Provider 拒绝',
    hint: '请检查 provider 的 max_tokens 是否超过模型上限；不确定时留空。',
  },
  {
    match: (text) => text.includes('timeout') || text.includes('timed out'),
    title: '模型请求超时',
    hint: '请检查网络、代理与 provider 端点可达性后重试。',
  },
];

/** 已知底层错误的友好文案（模块私有：只被 latestAgentFailure 消费） */
function describeAgentError(raw: string): ErrorCopy {
  const text = raw.toLowerCase();
  const rule = ERROR_RULES.find((entry) => entry.match(text));
  return rule === undefined ? FALLBACK_COPY : { title: rule.title, hint: rule.hint };
}

/** 逆序找最后一条指定事件（避免全量反转副本） */
function lastTurnDone(events: AgentEvent[]): Extract<AgentEvent, { kind: 'turnDone' }> | null {
  for (let index = events.length - 1; index >= 0; index -= 1) {
    const event = events[index];
    if (event.kind === 'turnDone') return event;
  }
  return null;
}

/** 反序列化 payload.error（非对象 / 非字符串 / 空白一律缺席） */
function readPayloadError(payload: unknown): string | null {
  if (typeof payload !== 'object' || payload === null) return null;
  const error = (payload as { error?: unknown }).error;
  if (typeof error !== 'string') return null;
  const trimmed = error.trim();
  return trimmed === '' ? null : trimmed;
}

/** 失败成因：最新轮开始后的最后一条 api_error 载荷（已恢复的 api_retry 不算） */
function lastApiErrorCause(events: AgentEvent[], since: number): string | null {
  for (let index = events.length - 1; index >= 0; index -= 1) {
    const event = events[index];
    if (event.kind !== 'systemNotice') continue;
    if (event.subtype !== 'api_error') continue;
    if (event.timestampMs < since) continue;
    const cause = readPayloadError(event.payload);
    if (cause !== null) return cause;
  }
  return null;
}

/**
 * 最新一轮失败投影：成功 / 停止 / 失败后已重试成功均返回 null。
 *
 * 轮行（turns）是库态权威；流刚结束、终态行尚未到达时以最后一条失败
 * `turnDone` 兜底（且只在该 done 不早于最新轮开始时才认账，避免上一轮的
 * 失败泄漏到新轮）。
 */
export function latestAgentFailure(
  events: AgentEvent[],
  turns: TurnSummary[],
): AgentFailure | null {
  const latestTurn = turns.at(-1) ?? null;
  const done = lastTurnDone(events);
  const doneAfterStart =
    done !== null && (latestTurn === null || done.timestampMs >= latestTurn.startedAt);
  const latestFailed =
    latestTurn === null ? done?.isError === true : latestTurn.status === 'failed';
  const runningJustFailed =
    latestTurn?.status === 'running' && doneAfterStart && done?.isError === true;
  if (!latestFailed && !runningJustFailed) return null;

  const since = latestTurn?.startedAt ?? 0;
  const turnCause = latestTurn?.error?.trim() ?? '';
  const doneCause = done?.isError === true ? done.subtype.trim() : '';
  const raw = lastApiErrorCause(events, since) ?? (turnCause !== '' ? turnCause : doneCause);
  return { ...describeAgentError(raw), raw };
}
