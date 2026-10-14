import { cn } from 'cn';

import { Button } from '@/components/ui/button';
import {
  Pagination,
  PaginationContent,
  PaginationEllipsis,
  PaginationItem,
} from '@/components/ui/pagination';

interface StandardPaginationProps {
  /** 当前页码（1 起） */
  page: number;
  /** 总页数（≥1，调用方由 total 推导） */
  totalPages: number;
  /** 过滤集全量计数；在场才呈摘要行（缺省不呈） */
  total?: number;
  onPageChange: (page: number) => void;
  disabled?: boolean;
  'data-testid'?: string;
  className?: string;
}

type PageWindowCell = number | 'gap';

/** 页码窗口：`totalPages ≤ 7` 全列，否则 `[1, …, p-1, p, p+1, …, N]`。 */
function pageWindow(page: number, totalPages: number): PageWindowCell[] {
  if (totalPages <= 7) {
    return Array.from({ length: totalPages }, (_, index) => index + 1);
  }
  const cells: PageWindowCell[] = [1];
  if (page > 3) cells.push('gap');
  for (let item = Math.max(2, page - 1); item <= Math.min(totalPages - 1, page + 1); item++) {
    cells.push(item);
  }
  if (page < totalPages - 2) cells.push('gap');
  cells.push(totalPages);
  return cells;
}

/**
 * 通用分页器（页码 + 上一页 / 下一页 + 可选摘要行）：页界与计数由调用方推导
 * 传入，本组件零取数零状态。
 */
export function StandardPagination({
  page,
  totalPages,
  total,
  onPageChange,
  disabled = false,
  'data-testid': testId,
  className,
}: StandardPaginationProps): React.JSX.Element {
  return (
    <Pagination className={className} data-testid={testId}>
      <PaginationContent>
        <PaginationItem>
          <Button
            aria-label="上一页"
            data-testid="page-prev"
            disabled={disabled || page <= 1}
            variant="ghost"
            onClick={() => onPageChange(page - 1)}
          >
            上一页
          </Button>
        </PaginationItem>
        {pageWindow(page, totalPages).map((cell, index) =>
          cell === 'gap' ? (
            <PaginationItem key={`gap-${String(index)}`}>
              <PaginationEllipsis />
            </PaginationItem>
          ) : (
            <PaginationItem key={cell}>
              <Button
                aria-current={cell === page ? 'page' : undefined}
                aria-label={`第 ${cell} 页`}
                className={cn(cell !== page && 'bg-muted text-muted-foreground')}
                data-page={cell}
                data-testid="page-link"
                disabled={disabled}
                variant="ghost"
                onClick={() => onPageChange(cell)}
              >
                {cell}
              </Button>
            </PaginationItem>
          ),
        )}
        {total !== undefined && (
          <PaginationItem>
            <span className="px-2 text-xs text-muted-foreground">
              第 {page} / {totalPages} 页（共 {total} 条）
            </span>
          </PaginationItem>
        )}
        <PaginationItem>
          <Button
            aria-label="下一页"
            data-testid="page-next"
            disabled={disabled || page >= totalPages}
            variant="ghost"
            onClick={() => onPageChange(page + 1)}
          >
            下一页
          </Button>
        </PaginationItem>
      </PaginationContent>
    </Pagination>
  );
}
