import {
  ArchiveIcon,
  ChevronRightIcon,
  FolderCodeIcon,
  LayersIcon,
  NetworkIcon,
} from 'lucide-react';
import { useMemo } from 'react';

import {
  Button,
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
  ScrollArea,
} from '@/components/ui';
import type { ArchiElement } from '@/types/generated/bindings';

import { shortName } from '../../lib/fqn';

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

type ElementTreeNode = ArchiElement & {
  children: ElementTreeNode[];
};

/** 取 / 建某 FQN 的子节点槽（引用稳定，先建子后填父也成立）。 */
function childrenSlot(map: Map<string, ElementTreeNode[]>, fqn: string): ElementTreeNode[] {
  const existing = map.get(fqn);
  if (existing !== undefined) return existing;
  const created: ElementTreeNode[] = [];
  map.set(fqn, created);
  return created;
}

/** 前缀派生树（元素按 FQN 字典序，兄弟节点稳定序）。 */
function buildTree(elements: ArchiElement[]): ElementTreeNode[] {
  const known = new Set(elements.map((element) => element.fqn));
  const roots: ElementTreeNode[] = [];
  const allChildrenMap = new Map<string, ElementTreeNode[]>();
  for (const element of [...elements].sort((left, right) => left.fqn.localeCompare(right.fqn))) {
    const treeNode: ElementTreeNode = {
      ...element,
      children: childrenSlot(allChildrenMap, element.fqn),
    };
    const parent = parentOf(element.fqn, known);
    if (parent === null) {
      roots.push(treeNode);
    } else {
      childrenSlot(allChildrenMap, parent).push(treeNode);
    }
  }
  return roots;
}

function TreeNodes({
  nodes,
  focused,
  onFocus,
}: {
  nodes: ElementTreeNode[];
  focused: string | null;
  onFocus: (fqn: string) => void;
}): React.JSX.Element {
  return (
    <ul className="m-0 list-none p-0">
      {nodes.map(({ fqn, kind, children }) => {
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
                  {kind === 'package' && <ArchiveIcon className="mr-1" />}
                  {kind === 'domain' && <NetworkIcon className="mr-1" />}
                  {kind === 'module' && <FolderCodeIcon className="mr-1" />}
                  {kind === 'composite' && <LayersIcon className="mr-1" />}
                  {shortName(fqn)}
                </Button>
              </div>
              <CollapsibleContent className="ml-5 style-lyra:ml-4">
                <TreeNodes nodes={children} focused={focused} onFocus={onFocus} />
              </CollapsibleContent>
            </Collapsible>
          </li>
        );
      })}
    </ul>
  );
}

export function ElementTree({ elements, focused, onFocus }: ElementTreeProps): React.JSX.Element {
  const roots = useMemo(() => buildTree(elements), [elements]);

  return (
    <ScrollArea className="h-full" data-testid="archi-element-tree">
      {roots.length === 0 ? (
        <div className="text-[13px] text-muted-foreground" data-testid="archi-tree-empty">
          模型内暂无元素
        </div>
      ) : (
        <TreeNodes nodes={roots} focused={focused} onFocus={onFocus} />
      )}
    </ScrollArea>
  );
}
