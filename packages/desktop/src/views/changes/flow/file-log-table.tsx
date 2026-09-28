/**
 * file_log 条目表体（op / scope / attempt / path / at 五列）：
 * workflow 独立面板与抽屉文件表节共用；空数组渲染「（空）」占位（无表格），
 * attempt / at 空缺渲染「—」。
 */
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from '@/components/ui/table';

import type { FileLogEntry } from './types';

export function FileLogTable({ entries }: { entries: FileLogEntry[] }): React.JSX.Element {
  if (entries.length === 0) {
    return <div className="text-muted-foreground">（空）</div>;
  }
  return (
    <Table data-testid="filelog-table">
      <TableHeader>
        <TableRow>
          <TableHead>op</TableHead>
          <TableHead>scope</TableHead>
          <TableHead>attempt</TableHead>
          <TableHead>path</TableHead>
          <TableHead>at</TableHead>
        </TableRow>
      </TableHeader>
      <TableBody>
        {entries.map((entry, index) => (
          <TableRow key={index}>
            <TableCell>{entry.op}</TableCell>
            <TableCell>{entry.scope}</TableCell>
            <TableCell>{entry.attempt ?? '—'}</TableCell>
            <TableCell>{entry.path}</TableCell>
            <TableCell>{entry.at ?? '—'}</TableCell>
          </TableRow>
        ))}
      </TableBody>
    </Table>
  );
}
