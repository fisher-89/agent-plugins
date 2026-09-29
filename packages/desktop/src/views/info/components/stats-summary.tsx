import type { CodeTotals } from '../../../types/generated/bindings';

/** 汇总项：标签在上、数值在下 */
function SummaryItem({ label, value }: { label: string; value: number }): React.JSX.Element {
  return (
    <span className="flex flex-col">
      <span className="text-xs text-muted-foreground">{label}</span>
      <span className="text-lg font-medium tabular-nums">{value.toLocaleString()}</span>
    </span>
  );
}

/** 汇总面：文件数 / 代码行 / 注释行 / 空行四项总量 */
export function StatsSummary({ totals }: { totals: CodeTotals }): React.JSX.Element {
  return (
    <div className="flex flex-wrap gap-x-8 gap-y-2" data-testid="info-summary">
      <SummaryItem label="文件数" value={totals.files} />
      <SummaryItem label="代码行" value={totals.code} />
      <SummaryItem label="注释行" value={totals.comments} />
      <SummaryItem label="空行" value={totals.blanks} />
    </div>
  );
}
