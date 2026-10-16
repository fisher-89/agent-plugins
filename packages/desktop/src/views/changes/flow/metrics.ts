/**
 * 节点度量纯函数模块：毫秒耗时人读分级 + token 缩写 + 缺席判定单点。
 *
 * 缺席 / 0ms 判定收本模块单点（`startedAt <= 0` 等缺席口径 MUST NOT 散落调用
 * 侧魔数比较——flow-view delta 条款）；耗时分级核心 `formatDurationMs` 模块私有
 * （分级边界经两个导出标签函数覆盖）。
 */

/** 耗时分级（毫秒 → 人读串）：`< 60_000` 秒一位小数（`1.2s` / `59.9s`，999ms
 * → `1.0s` 边界留痕）；`< 3_600_000` 分 + 整秒余量（`3m20s`）；≥ 3_600_000 时
 * + 两位补零分（`1h05m`）。 */
function formatDurationMs(ms: number): string {
  if (ms < 60_000) {
    return `${(ms / 1000).toFixed(1)}s`;
  }
  if (ms < 3_600_000) {
    return `${Math.floor(ms / 60_000)}m${Math.floor((ms % 60_000) / 1000)}s`;
  }
  const hours = Math.floor(ms / 3_600_000);
  const minutes = Math.floor((ms % 3_600_000) / 60_000);
  return `${hours}h${String(minutes).padStart(2, '0')}m`;
}

/** 已收口步耗时段标签：缺席（旧库行 `startedAt <= 0` 或 `endedAt == null`）→
 * `"-"`；0ms 瞬步（`endedAt <= startedAt`）→ `null`（调用侧省显该段）；正值 →
 * 分级串。 */
export function settledDurationLabel(startedAt: number, endedAt: number | null): string | null {
  if (startedAt <= 0 || endedAt === null) return '-';
  if (endedAt <= startedAt) return null;
  return formatDurationMs(endedAt - startedAt);
}

/** running 走秒段标签：缺席（旧库行 `startedAt <= 0`）→ `"-"`；否则
 * `now - startedAt` 分级串（`now` 由组件 tick 注入，纯函数直测）。 */
export function runningDurationLabel(startedAt: number, now: number): string {
  if (startedAt <= 0) return '-';
  return formatDurationMs(now - startedAt);
}

/** token 数值缩写：`null` → `"—"`；`< 1000` 原样十进制；k / M 阶一位小数四舍
 * 五入去尾零（`12k` / `12.3k` / `1.5M` / `1M`；999_999 → `1000k`——k 阶封顶
 * 不跨界，阶界互斥取显示保真）。 */
export function formatTokens(value: number | null): string {
  if (value === null) return '—';
  if (value < 1000) return String(value);
  if (value < 1_000_000) return `${trimmed((value / 1000).toFixed(1))}k`;
  return `${trimmed((value / 1_000_000).toFixed(1))}M`;
}

/** 一位小数串去尾零与尾点（`"12.0"` → `"12"`、`"12.3"` → `"12.3"`）。 */
function trimmed(text: string): string {
  return text.endsWith('.0') ? text.slice(0, -2) : text;
}
