import { ChevronRightIcon } from 'lucide-react';
import { useMemo } from 'react';

import { Button } from '@/components/ui/button';
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from '@/components/ui/collapsible';
import { ScrollArea } from '@/components/ui/scroll-area';

import type { ArchiElement } from '../../../types/generated/bindings';
import { shortName } from '../lib/fqn';

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
        const hasChildren = children.length > 0;
        return (
          <li className="m-0" key={fqn}>
            <Collapsible defaultOpen>
              <div className="w-full flex">
                <CollapsibleTrigger
                  disabled={!hasChildren}
                  render={({ onClick }, state) => (
                    <Button
                      variant="ghost"
                      size="icon-sm"
                      disabled={!hasChildren}
                      aria-label={state.open ? `收起 ${fqn}` : `展开 ${fqn}`}
                      onClick={onClick}
                    >
                      <ChevronRightIcon
                        className={`transition-transform ${state.open ? 'rotate-90' : ''} ${hasChildren ? '' : 'opacity-0'}`}
                      />
                    </Button>
                  )}
                />
                <Button
                  variant="ghost"
                  size="sm"
                  className={`flex-1 justify-start ${focused === fqn ? 'bg-muted' : ''}`}
                  data-testid="archi-tree-node"
                  data-fqn={fqn}
                  title={fqn}
                  onClick={() => onFocus(fqn)}
                >
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
    <section className="min-h-0 w-72 shrink-0 flex flex-col" data-testid="archi-element-tree">
      <h2 className="m-0 mb-1.5 text-[13px] shrink-0 font-medium text-muted-foreground">元素</h2>
      {tree.roots.length === 0 ? (
        <div className="text-[13px] text-muted-foreground" data-testid="archi-tree-empty">
          模型内暂无元素
        </div>
      ) : (
        <ScrollArea className="flex-1 min-h-0 pr-2">
          <TreeNodes
            fqns={tree.roots}
            byFqn={byFqn}
            tree={tree}
            focused={focused}
            onFocus={onFocus}
          />
        </ScrollArea>
      )}
    </section>
  );
}
