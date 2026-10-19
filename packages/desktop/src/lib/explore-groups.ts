import type { ExploreRecord } from '../types/dto';

/**
 * 探索清单分组与组内排序（纯函数；与 `lib/explore-title.ts` /
 * `lib/explore-stance.ts` 同族——零 IO、零 invoke、零 React 依赖，入参只读）。
 *
 * 分组判据取记录 `promotedTo`：`null` = 进行中（草稿态探索，含 `draft-*` 未定名
 * 占位记录）；非空 = 已转变更（已 promote）。打标即事实，不查 change 记录。
 *
 * 排序键取 `createdAt`（行内展示的同一字段）而非 `updatedAt`——title /
 * workflow_type 回填与 promote 打标都会刷新 `updatedAt`，取它会导致「看着没动的
 * 时间跳位置」；同刻以 `id` 倒序兜底（`id` 唯一 → 全序 → 输出确定）。
 *
 * 显式按 `createdAt` 排而不读后端返回顺序：后端 `list_explore_records` 契约为
 * `id` 降序，而 `id` 是可复用计数（幸存行 max+1）——删记录后新建的探索可拿到更小
 * `id`，`id` 序与建档时间序错位。`filter` 产出即新数组，`sort` 只动新数组，入参
 * 数组与记录字段调用前后零触碰。
 */
export function groupExploreRecords(records: ExploreRecord[]): {
  active: ExploreRecord[];
  promoted: ExploreRecord[];
} {
  return {
    active: records
      .filter((record) => record.promotedTo === null)
      .sort((a, b) => b.createdAt - a.createdAt || b.id - a.id),
    promoted: records
      .filter((record) => record.promotedTo !== null)
      .sort((a, b) => b.createdAt - a.createdAt || b.id - a.id),
  };
}
