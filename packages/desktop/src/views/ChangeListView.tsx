import type { ChangeListState } from '../hooks/useChangeList';
import type { ChangeSummary, Inventory } from '../types/dto';

function InventoryBadge({ inventory }: { inventory: Inventory }) {
  return <span className={`badge badge-in${inventory}`}>{inventory}</span>;
}

function ChangeRow({
  summary,
  onSelect,
}: {
  summary: ChangeSummary;
  onSelect: (name: string) => void;
}) {
  return (
    <button className="change-row" onClick={() => onSelect(summary.name)}>
      <span className="name">{summary.name}</span>
      <InventoryBadge inventory={summary.inventory} />
      {summary.created !== null && <span className="created">{summary.created}</span>}
      {summary.unparsable && <span className="created">workflow.json 无法解析</span>}
    </button>
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
      {error !== null && <div className="error-note">列表加载失败：{error}</div>}
      {loading && <div className="muted">加载中…</div>}
      {!loading && data === null && !error && <div className="muted">暂无数据，点击刷新获取。</div>}
      {data !== null && (
        <>
          <section className="panel">
            <h2>
              进行中 <span className="muted">({data.active.length})</span>
            </h2>
            {data.active.length === 0 ? (
              <div className="muted">无进行中的 change。</div>
            ) : (
              data.active.map((summary) => (
                <ChangeRow key={summary.name} summary={summary} onSelect={onSelect} />
              ))
            )}
          </section>
          {data.archiveGroups.map((group) => (
            <section className="panel" key={group.month ?? 'unknown'}>
              <h2>
                {group.month ?? '未知时间'} <span className="muted">({group.changes.length})</span>
              </h2>
              {group.changes.map((summary) => (
                <ChangeRow key={summary.name} summary={summary} onSelect={onSelect} />
              ))}
            </section>
          ))}
          {data.active.length === 0 && data.archiveGroups.length === 0 && (
            <div className="muted">该 workspace 下未发现任何 change 目录。</div>
          )}
        </>
      )}
    </div>
  );
}
