import { ChevronRightIcon } from 'lucide-react';
/**
 * 左元素树：层级由 **FQN 最长前缀**派生（父 = 最长已声明前缀，非 TOML 表嵌套），
 * 节点状态点取节点标记词汇（`nodeDot`）。停摆态零节点（由父级全或无停摆承载），
 * 空模型亦零节点。
 */
import { useMemo } from 'react';

import { Button } from '@/components/ui/button';
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from '@/components/ui/collapsible';

import type { ArchiElement } from '../../../types/generated/bindings';
import { shortName } from '../lib/fqn';
import { nodeDot } from '../lib/marks';

export interface ElementTreeProps {
  /** 图元素（FQN 唯一） */
  elements: ArchiElement[];
  /** 当前聚焦元素（1-hop 中心） */
  focused: string | null;
  /** 聚焦切换（纯前端，零取数） */
  onFocus: (fqn: string) => void;
}

/** 父 FQN（最长已声明前缀；顶层返回 null）。 */
function parentOf(fqn: string, known: ReadonlySet<string>): string | null {
  const segments = fqn.split('.');
  while (segments.length > 1) {
    segments.pop();
    const candidate = segments.join('.');
    if (known.has(candidate)) return candidate;
  }
  return null;
}

interface TreeShape {
  roots: string[];
  children: Map<string, string[]>;
}

/** 前缀派生树（元素按 FQN 字典序，兄弟节点稳定序）。 */
function buildTree(elements: ArchiElement[]): TreeShape {
  const known = new Set(elements.map((element) => element.fqn));
  const roots: string[] = [];
  const children = new Map<string, string[]>();
  for (const element of [...elements].sort((left, right) => left.fqn.localeCompare(right.fqn))) {
    const parent = parentOf(element.fqn, known);
    if (parent === null) {
      roots.push(element.fqn);
      continue;
    }
    const siblings = children.get(parent);
    if (siblings === undefined) children.set(parent, [element.fqn]);
    else siblings.push(element.fqn);
  }
  return { roots, children };
}

function TreeNodes({
  fqns,
  byFqn,
  tree,
  focused,
  onFocus,
}: {
  fqns: string[];
  byFqn: ReadonlyMap<string, ArchiElement>;
  tree: TreeShape;
  focused: string | null;
  onFocus: (fqn: string) => void;
}): React.JSX.Element {
  return (
    <ul className="m-0 list-none p-0">
      {fqns.map((fqn) => {
        const element = byFqn.get(fqn);
        if (element === undefined) return null;
        const children = tree.children.get(fqn) ?? [];
        return (
          <li className="m-0" key={fqn}>
            {children.length > 0 ? (
              <Collapsible defaultOpen>
                <div className="flex">
                  <CollapsibleTrigger
                    render={({ onClick }, state) => (
                      <ChevronRightIcon
                        className={`transition-transform ${state.open ? 'rotate-90' : ''}`}
                        aria-label={state.open ? `收起 ${fqn}` : `展开 ${fqn}`}
                        onClick={onClick}
                      />
                    )}
                  />
                  <Button
                    variant="ghost"
                    size="sm"
                    className={`flex-1 justify-start ${focused === fqn ? 'bg-muted font-medium' : ''}`}
                    data-testid="archi-tree-node"
                    data-fqn={fqn}
                    title={fqn}
                    onClick={() => onFocus(fqn)}
                  >
                    <span data-testid="archi-node-mark">{nodeDot(element.mark)}</span>
                    {shortName(fqn)}
                  </Button>
                </div>
                <CollapsibleContent className="mt-1 ml-5 style-lyra:ml-4">
                  <TreeNodes
                    fqns={children}
                    byFqn={byFqn}
                    tree={tree}
                    focused={focused}
                    onFocus={onFocus}
                  />
                </CollapsibleContent>
              </Collapsible>
            ) : (
              <Button
                variant="ghost"
                size="sm"
                className={`w-full justify-start ${focused === fqn ? 'bg-muted font-medium' : ''}`}
                data-testid="archi-tree-node"
                data-fqn={fqn}
                title={fqn}
                onClick={() => onFocus(fqn)}
              >
                <span data-testid="archi-node-mark">{nodeDot(element.mark)}</span>
                {shortName(fqn)}
              </Button>
            )}
          </li>
        );
      })}
    </ul>
  );
}

export function ElementTree({ elements, focused, onFocus }: ElementTreeProps): React.JSX.Element {
  const tree = useMemo(() => buildTree(elements), [elements]);
  const byFqn = useMemo(
    () => new Map(elements.map((element) => [element.fqn, element])),
    [elements],
  );

  return (
    <section className="min-h-0 w-52 shrink-0 overflow-auto pr-2" data-testid="archi-element-tree">
      <h2 className="m-0 mb-1.5 text-[13px] font-medium text-muted-foreground">元素</h2>
      {tree.roots.length === 0 ? (
        <div className="text-[13px] text-muted-foreground" data-testid="archi-tree-empty">
          模型内暂无元素
        </div>
      ) : (
        <TreeNodes
          fqns={tree.roots}
          byFqn={byFqn}
          tree={tree}
          focused={focused}
          onFocus={onFocus}
        />
      )}
    </section>
  );
}
