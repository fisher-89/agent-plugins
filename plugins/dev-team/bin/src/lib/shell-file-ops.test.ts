/**
 * 单元测试: lib/shell-file-ops.ts — shell 命令 → FileOp 三态提取（自研正则提取层）
 *
 * 三张表:
 * - FP 回归表: 箭头函数 / 引号与 heredoc 正文中的命令名 / 未展开 $var 不产生操作
 *   （move-files-write-into-workflow-module 工作流实况事故的逐条回归）
 * - 必捕获基线表: 真实 bash / PowerShell 写、删除、移动、git 三态操作逐项锁定——
 *   修误报不得丢真实写入是本套件的另一半使命
 * - 掩盖边界表: quoteSpans / heredocSpans / maskSpans 的 fail-open 与守卫边界
 *
 * 断言为精确 op 集合（"op:path" 排序去重后 toEqual，多一条少一条都失败）。
 * 去重对齐折叠层语义（extractFileOps 对同一目标可产生重复 op，如 bash/PS 双正则
 * 同时命中 `echo x > f`，由下游 foldFileOps 去重）。无 mock、无文件系统。
 */

import { describe, expect, it } from 'vite-plus/test';

import { extractFileOps } from './shell-file-ops';

/** 提取结果规整为排序去重的 "op:path" 集合，便于精确断言。 */
function opsOf(cmd: string): string[] {
  return [...new Set(extractFileOps(cmd).map((o) => `${o.op}:${o.path}`))].sort();
}

interface Case {
  name: string;
  cmd: string;
  expected: string[];
}

// ===========================================================================
// FP 回归表 — 实况事故逐条回归（引号/heredoc/箭头/$var 不产生操作）
// ===========================================================================

describe('extractFileOps — FP 回归（箭头函数 / 引号与 heredoc / 未展开变量）', () => {
  const cases: Case[] = [
    {
      name: 'JS 箭头函数体标识符不记为 write（=> path.join 实况）',
      cmd: 'node -e "const p = (...segs) => path.join(proj, ...segs);"',
      expected: [],
    },
    {
      name: '箭头函数 k/x.type/pl.id 形态不记为 write（报告校验脚本实况）',
      cmd: 'node -e "checks.every(k => k in r) && rows.some(x => x.type === \'e\') && plans.every(pl => pl.id);"',
      expected: [],
    },
    {
      name: 'printf 格式串中的 => label 不记为 write（gitignore-diag 实况）',
      cmd: 'printf \'%s => gitignore-diag: %s\' "$f" "$(grep -c x "$f")"',
      expected: [],
    },
    {
      name: '>= 比较不记为 write（现状锁定）',
      cmd: 'node -e "if (a >= b) return;"',
      expected: [],
    },
    {
      name: 'heredoc 驱动脚本行只记 header 写目标，正文 rm/引号碎片全消失（12 条垃圾 delete 实况）',
      cmd:
        "cat > driver.mjs <<'EOF'\n" +
        "event('Bash rm .claude/y.md src/b.ts (mixed delete)', ev({ tool_name: 'Bash', tool_input: { command: 'rm .claude/y.md src/b.ts' } }));\n" +
        'EOF',
      expected: ['write:driver.mjs'],
    },
    {
      name: '引号字符串内的 git restore 不产生 revert（echo 建议文本）',
      cmd: "echo 'please git restore src/a.ts'",
      expected: [],
    },
    {
      name: 'echo 的合成事件 JSON 内的 rm 不产生 delete（fixture 回放实况）',
      cmd: 'echo \'{"tool_name":"Bash","tool_input":{"command":"rm src/a.ts src/b.ts"}}\' | node hooks.cjs',
      expected: [],
    },
    {
      name: '未展开 $TMP 不入 delete；绝对路径 op 由记录器出根守卫丢弃（提取层保留）',
      cmd: 'rm -rf "$TMP" /tmp/smoke-path.txt',
      expected: ['delete:/tmp/smoke-path.txt'],
    },
    {
      name: 'dotnet 写入的 $var 首参不记为 write',
      cmd: "[System.IO.File]::WriteAllText($out, '{}')",
      expected: [],
    },
    {
      name: '-> 两端文本不记为 write（既有守卫不回退）',
      cmd: 'git log --oneline -> openspec/changes/x/workflow.json',
      expected: [],
    },
  ];

  for (const c of cases) {
    it(c.name, () => {
      expect(opsOf(c.cmd)).toEqual(c.expected);
    });
  }
});

// ===========================================================================
// 必捕获基线表 — 真实操作逐项锁定（修误报不得丢真写）
// ===========================================================================

describe('extractFileOps — 必捕获基线（真实 bash / PowerShell / git 操作）', () => {
  const cases: Case[] = [
    // bash 重定向族
    { name: '单重定向 >', cmd: 'echo hi > out.log', expected: ['write:out.log'] },
    { name: '追加重定向 >>', cmd: 'echo hi >> out.log', expected: ['write:out.log'] },
    { name: 'noclobber 重定向 >|', cmd: 'echo hi >| out.log', expected: ['write:out.log'] },
    { name: '>& 命名目标', cmd: 'echo hi >& out.log', expected: ['write:out.log'] },
    { name: 'fd 数字后重定向 2>', cmd: 'cmd 2> err.log', expected: ['write:err.log'] },
    { name: '无空格 >&outfile', cmd: 'cmd >&outfile', expected: ['write:outfile'] },
    { name: '>&2.log 非纯数字目标仍捕获', cmd: 'cmd >&2.log', expected: ['write:2.log'] },
    { name: '行首重定向', cmd: '> /tmp/x', expected: ['write:/tmp/x'] },
    { name: 'tee', cmd: 'tee out.log', expected: ['write:out.log'] },
    { name: '管道 tee -a', cmd: 'echo hi | tee -a out.log', expected: ['write:out.log'] },
    {
      name: 'heredoc header 行的重定向目标仍捕获',
      cmd: 'cat > out.log <<EOF\nhello\nEOF',
      expected: ['write:out.log'],
    },
    {
      name: 'marker 在前的 heredoc（cat <<EOF > X）',
      cmd: 'cat <<EOF > out.log\nhello\nEOF',
      expected: ['write:out.log'],
    },
    // fd 复制不产生 op
    { name: '2>&1 不产生 op', cmd: 'cmd 2>&1', expected: [] },
    { name: '1>&2 不产生 op', cmd: 'cmd 1>&2', expected: [] },
    { name: '>&2 不产生 op', cmd: 'cmd >&2', expected: [] },
    // PowerShell 族
    {
      name: 'Set-Content -Path',
      cmd: 'Set-Content -Path config.json',
      expected: ['write:config.json'],
    },
    {
      name: 'Out-File -FilePath',
      cmd: 'Out-File -FilePath out.json',
      expected: ['write:out.json'],
    },
    { name: 'Add-Content', cmd: 'Add-Content log.txt', expected: ['write:log.txt'] },
    { name: 'Export-Csv', cmd: 'Export-Csv data.csv', expected: ['write:data.csv'] },
    { name: 'Export-CliXml', cmd: 'Export-CliXml data.xml', expected: ['write:data.xml'] },
    { name: 'Tee-Object -FilePath', cmd: 'Tee-Object -FilePath t.log', expected: ['write:t.log'] },
    { name: 'PS 全流重定向 *>', cmd: 'Get-Process *> all.log', expected: ['write:all.log'] },
    {
      name: 'dotnet WriteAllText 带引号含空格首参',
      cmd: '[System.IO.File]::WriteAllText("a b.json", "{}")',
      expected: ['write:a b.json'],
    },
    {
      name: 'dotnet WriteAllText 单引号首参',
      cmd: "[System.IO.File]::WriteAllText('x.json', '{}')",
      expected: ['write:x.json'],
    },
    // 删除族
    { name: 'rm', cmd: 'rm src/old.ts', expected: ['delete:src/old.ts'] },
    { name: 'rm -rf', cmd: 'rm -rf build', expected: ['delete:build'] },
    { name: 'Remove-Item', cmd: 'Remove-Item tmp.txt', expected: ['delete:tmp.txt'] },
    { name: 'unlink', cmd: 'unlink x', expected: ['delete:x'] },
    { name: 'del', cmd: 'del y.txt', expected: ['delete:y.txt'] },
    { name: 'rd', cmd: 'rd z', expected: ['delete:z'] },
    { name: 'git rm', cmd: 'git rm staged.ts', expected: ['delete:staged.ts'] },
    // 移动双条目
    {
      name: 'mv 双条目 delete(old)+write(new)',
      cmd: 'mv src/a.ts src/b.ts',
      expected: ['delete:src/a.ts', 'write:src/b.ts'],
    },
    // git 三态
    {
      name: 'git restore → revert',
      cmd: 'git restore src/foo.ts',
      expected: ['revert:src/foo.ts'],
    },
    {
      name: 'git restore --source= → write',
      cmd: 'git restore --source=HEAD~1 src/foo.ts',
      expected: ['write:src/foo.ts'],
    },
    {
      name: 'git checkout -- → revert',
      cmd: 'git checkout -- src/foo.ts',
      expected: ['revert:src/foo.ts'],
    },
    {
      name: 'git checkout <commit> -- → write',
      cmd: 'git checkout abc123 -- src/foo.ts',
      expected: ['write:src/foo.ts'],
    },
    {
      name: 'git checkout $REF -- → 仍 write（commitish 不过滤）',
      cmd: 'git checkout $REF -- src/foo.ts',
      expected: ['write:src/foo.ts'],
    },
    // 现状基线锁定
    { name: '无空格 x>path 现状不捕获（基线锁定）', cmd: 'echo x>path', expected: [] },
  ];

  for (const c of cases) {
    it(c.name, () => {
      expect(opsOf(c.cmd)).toEqual(c.expected);
    });
  }
});

// ===========================================================================
// 掩盖边界表 — fail-open 与守卫边界
// ===========================================================================

describe('extractFileOps — 引号/heredoc 掩盖边界', () => {
  const cases: Case[] = [
    {
      name: '未闭合单引号 fail-open：其前的真实 rm 仍捕获',
      cmd: "rm src/real.ts; echo 'unclosed",
      expected: ['delete:src/real.ts'],
    },
    {
      name: '未闭合双引号 fail-open：其前的真实 rm 仍捕获',
      cmd: 'rm src/real.ts; echo "unclosed',
      expected: ['delete:src/real.ts'],
    },
    {
      name: '双引号内的单引号（"it\'s"）不破坏掩盖',
      cmd: 'echo "it\'s here" > f',
      expected: ['write:f'],
    },
    {
      name: "单引号内的双引号（'a\"b'）不破坏掩盖",
      cmd: "echo 'a\"b' > f",
      expected: ['write:f'],
    },
    {
      name: 'heredoc 无终止行 fail-open：正文按现状提取（rm x 仍捕获）',
      cmd: 'cat > f <<EOF\nrm x',
      expected: ['delete:x', 'write:f'],
    },
    {
      name: '<<- 缩进终止行识别，正文掩盖',
      cmd: 'cat <<-EOF\n\trm x\n\tEOF',
      expected: [],
    },
    {
      name: '<<< herestring 不是 heredoc，其后的真实重定向仍捕获',
      cmd: 'cat <<< "data" > f',
      expected: ['write:f'],
    },
    {
      name: '引号内的 <<EOF 不是 opener，其后的真实重定向仍捕获',
      cmd: 'echo "<<EOF" > f',
      expected: ['write:f'],
    },
    {
      name: '<< 与 marker 间有空格（1 << 2）不形成 opener',
      cmd: 'node -e "x = 1 << 2"',
      expected: [],
    },
    {
      name: '紧贴 marker 的 <<2 单行无换行不形成正文，行为不变',
      cmd: 'node -e "x = 1 <<2; y = 3"',
      expected: [],
    },
    {
      name: 'heredoc 正文中的引号 rm 与重定向全部掩盖（不变式：仅 header 写目标）',
      cmd: "cat > driver.mjs <<'EOF'\necho 'rm src/fake.ts' > inner.log\nEOF",
      expected: ['write:driver.mjs'],
    },
    {
      name: "真实命令的引号参数保持今天的 tokenize 语义（rm 'a b.txt' → a + b.txt）",
      cmd: "rm 'a b.txt'",
      expected: ['delete:a', 'delete:b.txt'],
    },
    {
      name: 'git restore 引号路径照常 revert（保护依赖此路径）',
      cmd: "git restore 'openspec/changes/x/design.md'",
      expected: ['revert:openspec/changes/x/design.md'],
    },
    {
      name: '重定向引号目标保持今天的部分捕获语义',
      cmd: "echo x > 'a b.txt'",
      expected: ['write:a'],
    },
    // $ 过滤六处
    { name: 'rm $FILE 过滤', cmd: 'rm $FILE', expected: [] },
    { name: 'mv $A b 不再构成双条目', cmd: 'mv $A b', expected: [] },
    { name: 'git rm $X 过滤', cmd: 'git rm $X', expected: [] },
    {
      name: 'git restore --source=$C 路径不过滤',
      cmd: 'git restore --source=$C src/a.ts',
      expected: ['write:src/a.ts'],
    },
  ];

  for (const c of cases) {
    it(c.name, () => {
      expect(opsOf(c.cmd)).toEqual(c.expected);
    });
  }
});
