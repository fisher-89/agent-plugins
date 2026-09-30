import type { WriteProtection, WriteProtectionFile } from '../../../types/generated/bindings';

/** 单条规则行：glob + reason（reason 为 null 时占位） */
function RuleRow({ rule }: { rule: WriteProtectionFile }): React.JSX.Element {
  return (
    <li
      className="flex flex-wrap items-baseline gap-2 py-0.5"
      data-testid="config-write-protection-rule"
    >
      <span className="break-all font-mono text-xs">{rule.glob ?? '—'}</span>
      {rule.reason !== null && <span className="text-muted-foreground">{rule.reason}</span>}
    </li>
  );
}

/**
 * write_protection 分区：逐条 glob 规则 + reason；protection / files 未设或
 * 空数组时呈弱化「未配置」占位（面板在、内容空）。
 */
export function WriteProtectionSection({
  protection,
}: {
  protection: WriteProtection | null;
}): React.JSX.Element {
  const files = protection?.files ?? null;
  return (
    <section
      className="rounded-lg border border-border bg-card px-4 py-3.5"
      data-testid="config-write-protection"
    >
      <h2 className="m-0 mb-3 text-[15px]">写入保护</h2>
      {files === null || files.length === 0 ? (
        <div className="text-muted-foreground">未配置</div>
      ) : (
        <ul className="m-0 list-none p-0">
          {files.map((rule, index) => (
            <RuleRow key={index} rule={rule} />
          ))}
        </ul>
      )}
    </section>
  );
}
