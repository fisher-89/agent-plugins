import { cn } from '@/lib/utils';

/* shadcn Table vendored 内部化：仅保留本 app 用到的组件族，
   TableCaption / TableFooter 按 D11/knip 纪律不保留。单元格保留
   break-all（filelog 长路径换行，原 .filelog-table 语义）。 */

function Table({ className, ...props }: React.ComponentProps<'table'>): React.JSX.Element {
  return (
    <div data-slot="table-container" className="relative w-full overflow-x-auto">
      <table data-slot="table" className={cn('w-full text-sm', className)} {...props} />
    </div>
  );
}

function TableHeader({ className, ...props }: React.ComponentProps<'thead'>): React.JSX.Element {
  return <thead data-slot="table-header" className={cn('[&_tr]:border-b', className)} {...props} />;
}

function TableBody({ className, ...props }: React.ComponentProps<'tbody'>): React.JSX.Element {
  return (
    <tbody
      data-slot="table-body"
      className={cn('[&_tr:last-child]:border-0', className)}
      {...props}
    />
  );
}

function TableRow({ className, ...props }: React.ComponentProps<'tr'>): React.JSX.Element {
  return <tr data-slot="table-row" className={cn('border-b', className)} {...props} />;
}

function TableHead({ className, ...props }: React.ComponentProps<'th'>): React.JSX.Element {
  return (
    <th
      data-slot="table-head"
      className={cn(
        'h-10 px-2 text-left align-middle font-medium whitespace-nowrap text-muted-foreground',
        className,
      )}
      {...props}
    />
  );
}

function TableCell({ className, ...props }: React.ComponentProps<'td'>): React.JSX.Element {
  return (
    <td data-slot="table-cell" className={cn('p-2 align-middle break-all', className)} {...props} />
  );
}

export { Table, TableHeader, TableBody, TableRow, TableHead, TableCell };
