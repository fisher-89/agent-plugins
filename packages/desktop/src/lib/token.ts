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
