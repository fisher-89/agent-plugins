import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';

import type { ChangeListState } from '../../hooks/useChangeList';
import type { ChangeList, ChangeSummary, Inventory } from '../../types/dto';

// Tailwind 无法静态识别模板串类名：`badge-in${inventory}` 收敛为显式 variant 映射（spec 硬性要求）
const INVENTORY_VARIANT: Record<Inventory, 'inv0' | 'inv1' | 'inv2'> = {
  v0: 'inv0',
  v1: 'inv1',
  v2: 'inv2',
};

function InventoryBadge({ inventory }: { inventory: Inventory }) {
  return <Badge variant={INVENTORY_VARIANT[inventory]}>{inventory}</Badge>;
}

function ChangeRow({
  summary,
  onSelect,
}: {
  summary: ChangeSummary;
  onSelect: (name: string) => void;
}) {
  return (
    <Button
      className="h-auto w-full justify-start gap-2.5 whitespace-normal rounded-none border-0 border-b bg-transparent px-1 py-2 text-left font-normal text-inherit hover:bg-transparent hover:text-primary last:border-b-0"
      onClick={() => onSelect(summary.name)}
      data-testid="change-row"
    >
      <span className="font-semibold">{summary.name}</span>
      <InventoryBadge inventory={summary.inventory} />
      {summary.created !== null && (
        <span className="text-xs text-muted-foreground" data-testid="created">
          {summary.created}
        </span>
      )}
      {summary.unparsable && (
        <span className="text-xs text-muted-foreground">workflow.json 无法解析</span>
      )}
    </Button>
  );
}

/** archive 按月分组列表（"未知时间"组置尾） */
function ArchiveGroups({
  groups,
  onSelect,
}: {
  groups: ChangeList['archiveGroups'];
  onSelect: (name: string) => void;
}) {
  return (
    <>
      {groups.map((group) => (
        <section
          className="mb-4 rounded-lg border border-border bg-card px-4 py-3.5"
          key={group.month ?? 'unknown'}
        >
          <h2 className="m-0 mb-2.5 text-[15px]">
            {group.month ?? '未知时间'}{' '}
            <span className="text-muted-foreground">({group.changes.length})</span>
          </h2>
          {group.changes.map((summary) => (
            <ChangeRow key={summary.name} summary={summary} onSelect={onSelect} />
          ))}
        </section>
      ))}
    </>
  );
}

/** change 列表视图：active 列表 + archive 按月分组（"未知时间"组置尾）、代际徽标、点击进详情 */
export function ChangeListView({
  state,
  onSelect,
}: {
  state: ChangeListState;
  onSelect: (name: string) => void;
}) {
  const { data, loading, error } = state;
  return (
    <div>
      {error !== null && (
        <div
          className="mb-3 break-all rounded-md bg-fail-bg px-3 py-2 text-fail"
          data-testid="error-note"
        >
          列表加载失败：{error}
        </div>
      )}
      {loading && <div className="text-muted-foreground">加载中…</div>}
      {!loading && data === null && !error && (
        <div className="text-muted-foreground">暂无数据，点击刷新获取。</div>
      )}
      {data !== null && (
        <>
          <section className="mb-4 rounded-lg border border-border bg-card px-4 py-3.5">
            <h2 className="m-0 mb-2.5 text-[15px]">
              进行中 <span className="text-muted-foreground">({data.active.length})</span>
            </h2>
            {data.active.length === 0 ? (
              <div className="text-muted-foreground">无进行中的 change。</div>
            ) : (
              data.active.map((summary) => (
                <ChangeRow key={summary.name} summary={summary} onSelect={onSelect} />
              ))
            )}
          </section>
          <ArchiveGroups groups={data.archiveGroups} onSelect={onSelect} />
          {data.active.length === 0 && data.archiveGroups.length === 0 && (
            <div className="text-muted-foreground">该 workspace 下未发现任何 change 目录。</div>
          )}
        </>
      )}
    </div>
  );
}
