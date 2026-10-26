/**
 * 未知形态的宽松解析原语（纯函数；零 IO、零 React 依赖）。
 *
 * 载荷来自 IPC / 线上替身，运行时形态不受 TS 类型约束：呈现层各消费点
 * （引用列表、搜索卡、AskUserQuestion 静态卡）都需要「非对象当空对象、
 * 非字符串当 `null`」的同一条收敛口径，故收敛在此单点，MUST NOT 就地下沉
 * 各自副本。
 */

/**
 * 宽松对象化：非对象 / 数组一律空对象（免断言的未知形态收敛）。
 *
 * 数组并入「非对象」是刻意收紧——数组的索引键不是载荷字段名，按对象读取只会
 * 拿到 `undefined`；统一归空对象使消费点的缺省分支（省略该行 / 回退 JSON）
 * 由同一条判据触发。
 */
export function asRecord(value: unknown): Record<string, unknown> {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) return {};
  const record: Record<string, unknown> = {};
  for (const [key, item] of Object.entries(value)) record[key] = item;
  return record;
}

/** 宽松取字符串字段（缺席 / 类型不符 → `null`） */
export function textOf(value: unknown): string | null {
  return typeof value === 'string' ? value : null;
}
