import { cva, type VariantProps } from 'class-variance-authority';

import { cn } from '@/lib/utils';

/* shadcn Badge vendored 内部化：变体集收敛为本 app 实际用量——
   代际 inv0/inv1/inv2 与 kind 直接取 Tailwind 调色板（固定深色下调参：`*-500/15` 底 + `*-300` 字，
   保留原灰/黄/蓝/紫色相身份），pass/fail/active 走 CSS 入口 token（design D10/D11）。 */
const badgeVariants = cva(
  'inline-block rounded px-[7px] py-px text-xs font-semibold leading-[18px]',
  {
    variants: {
      variant: {
        inv0: 'bg-gray-500/15 text-gray-300',
        inv1: 'bg-yellow-500/15 text-yellow-300',
        inv2: 'bg-blue-500/15 text-blue-300',
        pass: 'bg-pass-bg text-pass',
        fail: 'bg-fail-bg text-fail',
        kind: 'bg-violet-500/15 text-violet-300',
        active: 'bg-primary text-primary-foreground',
      },
    },
    defaultVariants: {
      variant: 'inv0',
    },
  },
);

function Badge({
  className,
  variant,
  ...props
}: React.ComponentProps<'span'> & VariantProps<typeof badgeVariants>): React.JSX.Element {
  return (
    <span data-slot="badge" className={cn(badgeVariants({ variant }), className)} {...props} />
  );
}

export { Badge };
