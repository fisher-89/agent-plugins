import { useMemo, useState } from 'react';
import { toast } from 'sonner';

import { FileViewer } from '@/components/file/file-viewer';
import { StandardDialog } from '@/components/standard/dailog';
import { Button } from '@/components/ui/button';
import { ScrollArea } from '@/components/ui/scroll-area';

import {
  commands,
  type ArchiApiCheck,
  type ArchiApiFinding,
} from '../../../types/generated/bindings';
import { useArchiApiCheck } from '../hooks/use-archi-api-check';
import type { CanvasMode } from './element-drawer';

/** `sha256:<hex>` → 短形（与生成物头注释同形；DTO 出的始终是全量摘要）。 */
function shortDigest(digest: string): string {
  const [, hex] = digest.split(':');
  return (hex ?? digest).slice(0, 12);
}

/** 生成物差异的三组（次序即声明序：内容不符 → 孤儿 → 缺失）。 */
const ARTIFACT_GROUPS: { kind: 'mismatched' | 'orphaned' | 'missing'; label: string }[] = [
  { kind: 'mismatched', label: '不符' },
  { kind: 'orphaned', label: '孤儿' },
  { kind: 'missing', label: '缺失' },
];

export interface ContractPanelProps {
  /** 观察根（`null` 时零 invoke） */
  root: string | null;
  /** 能力差异：`full` 只读 / `incremental` 可生成 */
  mode: CanvasMode;
  /** 契约判据（父级同一份漂移报告，零新取数） */
  findings: ArchiApiFinding[];
  /** 条目点击的聚焦语义（与元素树 / 文件树同款） */
  onFocus: (fqn: string) => void;
}

/** 生成物差异的一条（路径行 + 「打开」走通用读面）。 */
function ArtifactRow({
  root,
  path,
  kind,
}: {
  root: string | null;
  path: string;
  kind: string;
}): React.JSX.Element {
  return (
    <div
      className="flex items-center gap-1 rounded-md px-1 py-0.5 hover:bg-muted"
      data-axis="artifacts"
      data-kind={kind}
      data-testid="archi-contract-item"
    >
      <span className="min-w-0 flex-1 truncate font-mono text-[11px] text-muted-foreground">
        {path}
      </span>
      {root !== null && (
        <FileViewer
          root={root}
          files={[{ path }]}
          trigger={
            <Button variant="ghost" size="sm" title={path}>
              打开
            </Button>
          }
        />
      )}
    </div>
  );
}

/** 判据条目（`element` 非空才可聚焦；空归属只呈文本）。 */
function FindingRow({
  finding,
  onFocus,
}: {
  finding: ArchiApiFinding;
  onFocus: (fqn: string) => void;
}): React.JSX.Element {
  const element = finding.element;
  return (
    <div
      className={`flex flex-col gap-0.5 rounded-md px-1 py-0.5 ${element === null ? '' : 'cursor-pointer hover:bg-muted'}`}
      data-axis="findings"
      data-kind={finding.kind}
      data-testid="archi-contract-item"
      onClick={element === null ? undefined : () => onFocus(element)}
      title={finding.message}
    >
      <span className="text-[11px] text-muted-foreground">
        {finding.kind}
        {element === null ? '' : ` · ${element}`}
        {finding.target === null ? '' : ` · ${finding.target}`}
      </span>
      <span className="wrap-break-word text-xs">{finding.message}</span>
    </div>
  );
}

/** 生成确认正文（三件事写死：写入份数 / 保留孤儿 / 非原子）。 */
function generatePrompt(check: ArchiApiCheck): React.JSX.Element {
  return (
    <div className="flex flex-col gap-1 text-[13px]">
      <div>
        将写入 {check.mismatched.length + check.missing.length} 份（不符 {check.mismatched.length} +
        缺失 {check.missing.length}）
      </div>
      <div>保留 {check.orphaned.length} 份孤儿生成物不做处理</div>
      <div className="text-muted-foreground">写盘非原子：中途失败时失败点之前已写，不回滚</div>
    </div>
  );
}

/**
 * 面板头部：标题 + 「检查」+（`incremental`）二次确认的「生成」。生成入口在
 * `check` 未回时也在场但禁用——入口缺席会让「可生成」被误读成「不能生成」。
 */
function ContractHeader({
  check,
  mode,
  busy,
  onCheck,
  onGenerate,
}: {
  check: ArchiApiCheck | null;
  mode: CanvasMode;
  busy: boolean;
  onCheck: () => void;
  onGenerate: () => void;
}): React.JSX.Element {
  return (
    <header className="flex shrink-0 items-center justify-between gap-1">
      <h2 className="m-0 text-[13px] font-medium text-muted-foreground">契约</h2>
      <div className="flex items-center gap-1">
        <Button
          data-testid="archi-contract-check"
          disabled={busy}
          onClick={onCheck}
          size="sm"
          variant="outline"
        >
          检查
        </Button>
        {mode === 'incremental' &&
          (check === null ? (
            // 载入中 / 未载入：生成入口在场但禁用（确认正文的份数无从取值，MUST NOT 缺席）
            <Button data-testid="archi-contract-generate" disabled size="sm" variant="default">
              生成
            </Button>
          ) : (
            <StandardDialog
              title="生成契约生成物"
              content={generatePrompt(check)}
              onSubmit={(close) => {
                close();
                onGenerate();
              }}
              trigger={
                <Button
                  data-testid="archi-contract-generate"
                  disabled={busy}
                  size="sm"
                  variant="default"
                >
                  生成
                </Button>
              }
            />
          ))}
      </div>
    </header>
  );
}

/** 上轴：生成物差异（三类分组 + 各自计数；合计只在本轴出现）。 */
function ArtifactsAxis({
  check,
  root,
}: {
  check: ArchiApiCheck | null;
  root: string | null;
}): React.JSX.Element {
  const total =
    check === null ? 0 : check.mismatched.length + check.orphaned.length + check.missing.length;
  return (
    <div className="flex flex-col gap-1" data-testid="archi-contract-axis-artifacts">
      <h3 className="m-0 text-[13px] font-medium text-muted-foreground">生成物差异（{total}）</h3>
      {check === null
        ? null
        : ARTIFACT_GROUPS.map((group) => {
            const paths = check[group.kind];
            return (
              <div className="flex flex-col" key={group.kind}>
                <div className="text-[11px] text-muted-foreground">
                  {group.label} {paths.length}
                </div>
                {paths.map((path) => (
                  <ArtifactRow key={path} kind={group.kind} path={path} root={root} />
                ))}
              </div>
            );
          })}
    </div>
  );
}

/** 下轴：契约判据（chip 由数据派生、零前端词表；计数只在本轴）。 */
function FindingsAxis({
  findings,
  onFocus,
}: {
  findings: ArchiApiFinding[];
  onFocus: (fqn: string) => void;
}): React.JSX.Element {
  const [category, setCategory] = useState<string | null>(null);
  const categories = useMemo(() => {
    const counts = new Map<string, number>();
    for (const finding of findings) {
      counts.set(finding.category, (counts.get(finding.category) ?? 0) + 1);
    }
    return [...counts.entries()].sort(([left], [right]) => left.localeCompare(right));
  }, [findings]);
  const visible =
    category === null ? findings : findings.filter((finding) => finding.category === category);
  return (
    <div className="mt-2 flex flex-col gap-1" data-testid="archi-contract-axis-findings">
      <h3 className="m-0 text-[13px] font-medium text-muted-foreground">
        判据（{findings.length}）
      </h3>
      <div className="flex flex-wrap gap-1">
        {categories.map(([name, count]) => (
          <button
            key={name}
            type="button"
            aria-pressed={category === name}
            className={`cursor-pointer rounded-md border px-2 py-0.5 text-[11px] ${
              category === name
                ? 'border-primary/40 bg-muted font-medium text-foreground'
                : 'border-transparent bg-transparent text-muted-foreground hover:text-foreground'
            }`}
            data-category={name}
            data-testid="archi-contract-chip"
            onClick={() => setCategory(category === name ? null : name)}
          >
            {name} {count}
          </button>
        ))}
      </div>
      {visible.length === 0 ? (
        <div className="text-[13px] text-muted-foreground">无契约判据</div>
      ) : (
        visible.map((finding, index) => (
          <FindingRow
            // 判据无稳定 id（同 kind / element / target 可多条）：序即身份
            key={`${finding.kind}:${finding.target ?? ''}:${index}`}
            finding={finding}
            onFocus={onFocus}
          />
        ))
      )}
    </div>
  );
}

/** 面板状态词（五态里「呈文字」的四个；`check` 失败不渲状态词，成因只由错误块承载）。 */
function statusText(
  root: string | null,
  error: string | null,
  loading: boolean,
  check: ArchiApiCheck | null,
  findings: ArchiApiFinding[],
): string | null {
  if (root === null) return '无工作区根';
  if (error !== null) return null;
  if (loading || check === null) return '解析中…';
  if (!check.enabled) return '契约面未启用（应生成集此时只含 `INDEX.toml`）';
  const artifacts = check.mismatched.length + check.orphaned.length + check.missing.length;
  if (artifacts === 0 && findings.length === 0) return '生成物最新，判据 0 条';
  return null;
}

/**
 * 左栏契约视图：两轴并列（上轴 = 生成物新鲜度，可机器修；下轴 = 契约 vs 代码，要人或
 * agent 修），**MUST NOT** 出现跨轴合并总数（两轴可修复者不同）。chip 由 `findings` 的
 * `category` 派生（零前端词表、零计数类别不渲 chip）；条目点击 = 聚焦（与元素树同语义）；
 * 路径行经 `FileViewer` + 通用 `read_file` 打开。生成仅 `incremental` 且有二次确认。
 */
export function ContractPanel({
  root,
  mode,
  findings,
  onFocus,
}: ContractPanelProps): React.JSX.Element {
  const { check, loading, error, refresh } = useArchiApiCheck(root);
  const [generating, setGenerating] = useState(false);
  const [generateError, setGenerateError] = useState<string | null>(null);

  const runGenerate = (): void => {
    if (root === null) return;
    setGenerating(true);
    setGenerateError(null);
    commands
      .archiApiGenerate(root)
      .then((outcome) => {
        toast(`写入 ${outcome.written.length} 份 / 未变 ${outcome.unchanged.length} 份`);
        setGenerating(false);
        refresh();
      })
      .catch((reason: unknown) => {
        setGenerating(false);
        setGenerateError(String(reason));
      });
  };

  const status = statusText(root, error, loading, check, findings);
  return (
    <section className="min-h-0 flex flex-1 flex-col gap-1.5" data-testid="archi-contract-panel">
      <ContractHeader
        check={check}
        mode={mode}
        busy={loading || generating}
        onCheck={refresh}
        onGenerate={runGenerate}
      />
      {mode === 'full' && (
        <div className="text-[11px] text-muted-foreground" data-testid="archi-contract-readonly">
          生成需开 change（或让左侧会话执行 `pnpm api:export`）
        </div>
      )}
      {check !== null && (
        <div className="shrink-0 text-[11px] text-muted-foreground">
          契约面 {check.meta.toolVersion} · 映射 {shortDigest(check.meta.commonDigest)}
        </div>
      )}
      {generateError !== null && (
        <div
          className="break-all rounded-md bg-fail-bg px-2 py-1 text-xs text-fail"
          data-source="generate"
          data-testid="archi-contract-error"
        >
          生成失败：{generateError}（非原子：失败点之前已写，不回滚）
        </div>
      )}
      {error !== null && (
        <div
          className="break-all rounded-md bg-fail-bg px-2 py-1 text-xs text-fail"
          data-source="check"
          data-testid="archi-contract-error"
        >
          检查失败：{error}（画布照常渲染）
        </div>
      )}
      {status !== null && (
        <div className="text-[13px] text-muted-foreground" data-testid="archi-contract-status">
          {status}
        </div>
      )}
      <ScrollArea className="min-h-0 flex-1 pr-2">
        <ArtifactsAxis check={check} root={root} />
        <FindingsAxis findings={findings} onFocus={onFocus} />
      </ScrollArea>
    </section>
  );
}
