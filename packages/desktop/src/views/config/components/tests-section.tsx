import type { TestSuite } from '../../../types/generated/bindings';

/** 字符串列表行值：未设 / 空数组 / 多元素三形态可分（None 对位占位不崩） */
function listValue(items: string[] | null): string {
  if (items === null) {
    return '未配置';
  }
  return items.length === 0 ? '（空列表）' : items.join(', ');
}

/** 阈值值呈现：`number | null` 出线口径以 `??` 防御（specta 裸 f64 先例同型） */
function thresholdText(value: number | null): string {
  return `${value ?? '—'}`;
}

/** 阈值项：值 + defaultedPaths 对位时附「未设（默认 N）」弱化标注 */
function Threshold({
  label,
  path,
  value,
  defaultedPaths,
}: {
  label: string;
  path: string;
  value: number | null;
  defaultedPaths: ReadonlySet<string>;
}): React.JSX.Element {
  return (
    <span className="whitespace-nowrap">
      {`${label} `}
      <span className="tabular-nums">{thresholdText(value)}</span>
      {defaultedPaths.has(path) && (
        <em className="ml-1 text-xs not-italic text-muted-foreground">{`未设（默认 ${thresholdText(value)}）`}</em>
      )}
    </span>
  );
}

/** suite 明细行：标签 + 单值（字符串；未设弱化占位） */
function SuiteRow({ label, value }: { label: string; value: string | null }): React.JSX.Element {
  return (
    <div className="flex items-baseline gap-2 py-0.5">
      <span className="w-16 shrink-0 font-mono text-xs text-muted-foreground">{label}</span>
      {value === null ? (
        <span className="text-muted-foreground">未配置</span>
      ) : (
        <span className="min-w-0 break-all font-mono text-xs">{value}</span>
      )}
    </div>
  );
}

/** suite 列表行：标签 + 字符串列表（includes / excludes） */
function SuiteListRow({
  label,
  items,
}: {
  label: string;
  items: string[] | null;
}): React.JSX.Element {
  return (
    <div className="flex items-baseline gap-2 py-0.5">
      <span className="w-16 shrink-0 font-mono text-xs text-muted-foreground">{label}</span>
      <span
        className={items === null ? 'text-muted-foreground' : 'min-w-0 break-all font-mono text-xs'}
      >
        {listValue(items)}
      </span>
    </div>
  );
}

/** suite 卡片 coverage 行：三阈值（值 + 「未设（默认 N）」标注按 path 对位） */
function CoverageRow({
  base,
  coverage,
  defaultedPaths,
}: {
  base: string;
  coverage: TestSuite['coverage'];
  defaultedPaths: ReadonlySet<string>;
}): React.JSX.Element {
  return (
    <div className="flex flex-wrap items-baseline gap-x-4 gap-y-1 py-0.5">
      <span className="w-16 shrink-0 font-mono text-xs text-muted-foreground">coverage</span>
      <Threshold
        defaultedPaths={defaultedPaths}
        label="lines"
        path={`${base}.coverage.lines`}
        value={coverage.lines}
      />
      <Threshold
        defaultedPaths={defaultedPaths}
        label="branches"
        path={`${base}.coverage.branches`}
        value={coverage.branches}
      />
      <Threshold
        defaultedPaths={defaultedPaths}
        label="functions"
        path={`${base}.coverage.functions`}
        value={coverage.functions}
      />
    </div>
  );
}

/** suite 卡片 mutation 行：cwd 内联 + score 阈值（标注按 path 对位） */
function MutationRow({
  base,
  mutation,
  defaultedPaths,
}: {
  base: string;
  mutation: TestSuite['mutation'];
  defaultedPaths: ReadonlySet<string>;
}): React.JSX.Element {
  return (
    <div className="flex flex-wrap items-baseline gap-x-4 gap-y-1 py-0.5">
      <span className="w-16 shrink-0 font-mono text-xs text-muted-foreground">mutation</span>
      <SuiteRowInline label="cwd" value={mutation.cwd} />
      <Threshold
        defaultedPaths={defaultedPaths}
        label="score"
        path={`${base}.mutation.score`}
        value={mutation.score}
      />
    </div>
  );
}

/** 单 suite 卡片：全字段呈现（root / framework / cwd / config / includes /
 * excludes / coverage 三阈值 / mutation cwd + score），阈值标注按 path 对位 */
function SuiteCard({
  suite,
  index,
  defaultedPaths,
}: {
  suite: TestSuite;
  index: number;
  defaultedPaths: ReadonlySet<string>;
}): React.JSX.Element {
  const base = `tests[${index}]`;
  return (
    <div
      className="rounded-md border border-border px-3 py-2"
      data-suite-root={suite.root}
      data-testid="config-suite"
    >
      <div className="mb-1 flex flex-wrap items-baseline gap-2">
        <span className="font-mono text-xs text-muted-foreground">{suite.framework}</span>
        <span className="min-w-0 break-all font-mono text-sm">{suite.root}</span>
        {defaultedPaths.has(`${base}.cwd`) && (
          <em className="text-xs not-italic text-muted-foreground">未设（默认 .）</em>
        )}
      </div>
      <SuiteRow label="cwd" value={suite.cwd} />
      <SuiteRow label="config" value={suite.config} />
      <SuiteListRow label="includes" items={suite.includes} />
      <SuiteListRow label="excludes" items={suite.excludes} />
      <CoverageRow base={base} coverage={suite.coverage} defaultedPaths={defaultedPaths} />
      <MutationRow base={base} defaultedPaths={defaultedPaths} mutation={suite.mutation} />
    </div>
  );
}

/** mutation 组内 cwd 内联呈现（与 Threshold 同行，不另起卡片行） */
function SuiteRowInline({
  label,
  value,
}: {
  label: string;
  value: string | null;
}): React.JSX.Element {
  return (
    <span className="whitespace-nowrap">
      {`${label} `}
      <span className="font-mono text-xs">{value ?? '未配置'}</span>
    </span>
  );
}

/**
 * tests 面板：逐 suite 卡片呈现全字段；未设阈值按 defaultedPaths（path 对位
 * `tests[i].coverage.lines` 等）标注「未设（默认 N）」，与文件显式设值形态
 * 可区分；空数组呈弱化空行。
 */
export function TestsSection({
  suites,
  defaultedPaths,
}: {
  suites: TestSuite[];
  defaultedPaths: ReadonlySet<string>;
}): React.JSX.Element {
  return (
    <section
      className="rounded-lg border border-border bg-card px-4 py-3.5"
      data-testid="config-tests"
    >
      <h2 className="m-0 mb-3 text-[15px]">测试 suite</h2>
      {suites.length === 0 ? (
        <div className="text-muted-foreground">当前工作区未配置测试 suite。</div>
      ) : (
        <div className="flex flex-col gap-3">
          {suites.map((suite, index) => (
            <SuiteCard
              defaultedPaths={defaultedPaths}
              index={index}
              key={`${suite.root}:${suite.framework}:${index}`}
              suite={suite}
            />
          ))}
        </div>
      )}
    </section>
  );
}
