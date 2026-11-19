import type { ProjectConfig, WorkspaceConfig } from '../../../types/generated/bindings';

/** 弱化标注：字段未设、吃默认值（defaultedPaths 对位时出现，与文件显式设值可分） */
function DefaultedMark({ value }: { value: string }): React.JSX.Element {
  return (
    <em className="ml-2 text-xs not-italic text-muted-foreground">{`未设（默认 ${value}）`}</em>
  );
}

/** 单字段行：标签 + 值；未设（含 defaultApplied 吃默认的 None 位）呈弱化占位 */
function FieldRow({
  label,
  value,
  defaulted,
}: {
  label: string;
  value: string | null;
  defaulted: boolean;
}): React.JSX.Element {
  return (
    <div className="flex items-baseline gap-3 py-0.5">
      <span className="w-28 shrink-0 font-mono text-xs text-muted-foreground">{label}</span>
      {value !== null ? (
        <span className="min-w-0 break-all">
          {value}
          {defaulted && <DefaultedMark value={value} />}
        </span>
      ) : (
        <span className="text-muted-foreground">未配置</span>
      )}
    </div>
  );
}

/** rules 子字段组：proposal / tasks 字符串列表（空数组与多元素两形态均可呈现） */
function RulesGroup({
  label,
  items,
}: {
  label: string;
  items: string[] | null;
}): React.JSX.Element {
  return (
    <div className="py-0.5">
      <span className="font-mono text-xs">{`rules.${label}`}</span>
      {items === null ? (
        <span className="ml-2 text-muted-foreground">未配置</span>
      ) : items.length === 0 ? (
        <span className="ml-2 text-muted-foreground">（空列表）</span>
      ) : (
        <ul className="m-0 mt-1 list-disc pl-5">
          {items.map((item) => (
            <li key={item}>{item}</li>
          ))}
        </ul>
      )}
    </div>
  );
}

/** 依赖列表呈现文案：未设 → 「未设」；空数组 → 「（空列表）」；否则顿号并列 */
function depsText(deps: string[] | null): string {
  if (deps === null) {
    return '未设';
  }
  return deps.length === 0 ? '（空列表）' : deps.join('、');
}

/**
 * projects 子组：逐项目呈现 root + 已设的 language / depends_on；未设标注
 *「未设」（呈现面不隐藏已配置值）。空数组呈「（空列表）」；重复 root 允许，
 * index 作 key；只读（无编辑控件、无写回通道）。
 */
function ProjectsGroup({ items }: { items: ProjectConfig[] }): React.JSX.Element {
  return (
    <div className="flex items-baseline gap-3 py-0.5" data-testid="config-projects">
      <span className="w-28 shrink-0 font-mono text-xs text-muted-foreground">projects</span>
      {items.length === 0 ? (
        <span className="text-muted-foreground">（空列表）</span>
      ) : (
        <ul className="m-0 min-w-0 list-disc pl-5">
          {items.map((project, index) => (
            <li key={index} className="break-all">
              <span>{project.root}</span>
              <span className="ml-3 text-xs text-muted-foreground">{`language：${project.language ?? '未设'}`}</span>
              <span className="ml-3 text-xs text-muted-foreground">{`depends_on：${depsText(project.dependsOn)}`}</span>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}

/**
 * 基础配置分区：$schema / schema / context / static_analysis / projects / rules
 *（proposal / tasks 列表逐项）；schema 等吃默认字段按 defaultedPaths 对位标注
 *「未设（默认 N）」，与文件显式设值形态可区分。
 */
export function BasicConfigSection({
  config,
  defaultedPaths,
}: {
  config: WorkspaceConfig;
  defaultedPaths: ReadonlySet<string>;
}): React.JSX.Element {
  return (
    <section
      className="rounded-lg border border-border bg-card px-4 py-3.5"
      data-testid="config-basic"
    >
      <h2 className="m-0 mb-3 text-[15px]">基础配置</h2>
      <FieldRow defaulted={false} label="$schema" value={config.$schema} />
      <FieldRow defaulted={defaultedPaths.has('schema')} label="schema" value={config.schema} />
      <FieldRow defaulted={false} label="context" value={config.context} />
      <FieldRow defaulted={false} label="static_analysis" value={config.staticAnalysis} />
      <ProjectsGroup items={config.projects} />
      {config.rules === null ? (
        <FieldRow defaulted={false} label="rules" value={null} />
      ) : (
        <div className="flex items-baseline gap-3 py-0.5">
          <span className="w-28 shrink-0 font-mono text-xs text-muted-foreground">rules</span>
          <div className="min-w-0 flex-1">
            <RulesGroup label="proposal" items={config.rules.proposal} />
            <RulesGroup label="tasks" items={config.rules.tasks} />
          </div>
        </div>
      )}
    </section>
  );
}
