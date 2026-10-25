//! `shell` 的单元测试（AC-3 / AC-4 / AC-5 / AC-6）：命令文本写入面提取（双变体
//! 等长掩码 + token 纪律 + 三态 + 档 B 五族 + 批量还原三规则）。
//!
//! 断言口径：
//! - 「提取」用例只锚定 `Vec<FileOp>` 的形状 / 顺序 / 落点（命中判定归
//!   `protect/mod.rs`，本文件不判命中）；
//! - 零 mock：命令字符串即被测 API 显式入参，纯文本函数零 IO / 零子进程 / 零
//!   时钟；`cwd` 以真实 `tempdir` 路径传入（不读盘）；
//! - 精度双向（AC-5）：误拦方向与漏拦方向各自断言，MUST NOT 只锚单向。

use std::path::Path;

use foundation::layout;

use super::rules;
use super::shell::{self, FileOp, OpKind, OpTarget};

// ---------------------------------------------------------------------------
// 装置：操作落点构造（被测类型直构）+ 断言辅助
// ---------------------------------------------------------------------------

fn write_path(path: &str) -> FileOp {
    FileOp {
        kind: OpKind::Write,
        target: OpTarget::Path(path.to_owned()),
    }
}

fn delete_path(path: &str) -> FileOp {
    FileOp {
        kind: OpKind::Delete,
        target: OpTarget::Path(path.to_owned()),
    }
}

fn revert_path(path: &str) -> FileOp {
    FileOp {
        kind: OpKind::Revert,
        target: OpTarget::Path(path.to_owned()),
    }
}

fn delete_tree(path: &str) -> FileOp {
    FileOp {
        kind: OpKind::Delete,
        target: OpTarget::Tree(path.to_owned()),
    }
}

/// 落点文本（`Path` 与 `Tree` 同形取材，便于逐条断言）。
fn target_text(op: &FileOp) -> &str {
    match &op.target {
        OpTarget::Path(path) | OpTarget::Tree(path) => path,
    }
}

/// 去重后的 (操作态, 落点) 序列（族间重复提取路径归一，如重定向族与 PS 方言
/// 对同一 `>` 各记一条）。
fn unique(ops: &[FileOp]) -> Vec<(OpKind, String)> {
    let mut deduped: Vec<(OpKind, String)> = Vec::new();
    for op in ops {
        let pair = (op.kind, target_text(op).to_owned());
        if !deduped.contains(&pair) {
            deduped.push(pair);
        }
    }
    deduped
}

fn tempdir(tag: &str) -> tempfile::TempDir {
    tempfile::Builder::new()
        .prefix(&format!("sdk-protect-shell-{tag}-"))
        .tempdir()
        .expect("创建合成目录失败")
}

/// 归一文本（与 `rules::normalize` 同口径，用于拼 cwd 相对/绝对路径）。
fn normalized(path: &Path) -> String {
    rules::normalize(&path.to_string_lossy())
}

/// 域目录相对路径（由 layout 派生，脚本内零目录名字面量）。
fn domain_relative() -> String {
    rules::normalize(&format!("{}/x.md", layout::domain_dir_name()))
}

/// changes 子树相对路径（由 layout 派生）。
fn changes_relative() -> String {
    format!(
        "{}/x/proposal.md",
        rules::normalize(
            &layout::resolve(Path::new(""))
                .changes_root
                .to_string_lossy()
        )
    )
}

/// 批量还原判定入口（`BulkGlobs` 是被测 API 的显式注入依赖，取真实派生值）。
fn bulk_hit(command: &str, cwd: &Path) -> Option<String> {
    shell::bulk_revert_detail(command, &rules::bulk_globs(), cwd)
}

/// 掩码不变量：等长 + 逐位「原文或掩码填充」（两变体 char 索引 1:1 互通的
/// 机械保证）。
fn assert_mask_invariant(original: &str, masked: &str) {
    let source: Vec<char> = original.chars().collect();
    let product: Vec<char> = masked.chars().collect();
    assert_eq!(
        source.len(),
        product.len(),
        "掩码输出与原文等长（char 计），原文: {original:?}"
    );
    for (index, (source_char, masked_char)) in source.iter().zip(product.iter()).enumerate() {
        assert!(
            masked_char == source_char || *masked_char == '\u{0}',
            "第 {index} 位只能是原文或掩码填充，原文: {original:?}，产物: {masked:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// 掩码（精度唯一来源）
// ---------------------------------------------------------------------------

#[test]
fn 掩码变体a只掩heredoc体而保留引号() {
    let command = "cat <<EOF\nrm -rf /\nEOF\necho tail";
    let masked = shell::mask_heredocs(command);
    assert!(
        !masked.contains("rm -rf /"),
        "heredoc 体内的删除命令被逐字符掩掉，产物: {masked:?}"
    );
    assert!(
        masked.starts_with("cat <<EOF\n"),
        "开标记行保留，产物: {masked:?}"
    );
    assert!(
        masked.ends_with("\necho tail"),
        "结束 marker 行之后内容保留，产物: {masked:?}"
    );

    let quoted = "echo \"a b\"";
    assert_eq!(
        shell::mask_heredocs(quoted),
        quoted,
        "变体 A 只掩 heredoc 体：引号与内容逐字符保留"
    );
}

#[test]
fn 掩码变体a等长且逐位只替换不位移() {
    let commands = [
        "echo foo > notes.txt",
        "cat <<EOF\nbody\nEOF",
        "cat <<A\nx\nA\ncat <<B\ny\nB",
        "echo 中文🚀 > 目标.txt",
    ];
    for command in commands {
        assert_mask_invariant(command, &shell::mask_heredocs(command));
    }
}

#[test]
fn 掩码变体a多heredoc与缩进结束符与未闭合() {
    let double = shell::mask_heredocs("cat <<A\nx\nA\ncat <<B\ny\nB");
    assert!(
        !double.contains('x') && !double.contains('y'),
        "两段 heredoc 体均被掩，产物: {double:?}"
    );

    let dashed = shell::mask_heredocs("cat <<-EOF\nbody\n\tEOF");
    assert!(
        !dashed.contains("body"),
        "`<<-` 与缩进结束符形态同掩，产物: {dashed:?}"
    );

    let unclosed = "cat <<EOF";
    assert_eq!(
        shell::mask_heredocs(unclosed),
        unclosed,
        "未闭合 heredoc 候选丢弃（fail-open）且不 panic"
    );
}

#[test]
fn 掩码引号内的heredoc候选不越界() {
    let command = "echo \"<<EOF\"";
    assert_eq!(
        shell::mask_heredocs(command),
        command,
        "引号内的 `<<` 不构成开标记（等长 / 不 panic）"
    );
    assert_eq!(shell::mask_heredocs(""), "", "空串 → 空串");
}

#[test]
fn 掩码变体b掩引号串而变体a保留() {
    let command = "echo \"rm -rf /\" > x";
    let masked_b = shell::mask_quoted_and_heredoc(command);
    let masked_a = shell::mask_heredocs(command);
    assert!(
        !masked_b.contains("rm -rf /"),
        "变体 B 掩引号串（命令名定位不到 `rm`），产物: {masked_b:?}"
    );
    assert!(
        masked_a.contains("rm -rf /"),
        "变体 A 保留引号内容（写入正则运行面），产物: {masked_a:?}"
    );

    // 引号区之外两变体逐字符一致
    let source: Vec<char> = command.chars().collect();
    let a: Vec<char> = masked_a.chars().collect();
    let b: Vec<char> = masked_b.chars().collect();
    for (index, (source_char, b_char)) in source.iter().zip(b.iter()).enumerate() {
        if *b_char != '\u{0}' {
            assert_eq!(*b_char, a[index], "第 {index} 位（引号区之外）两变体一致");
            assert_eq!(*b_char, *source_char, "第 {index} 位未被掩码即原文");
        }
    }
}

#[test]
fn 掩码变体b引号族与畸形形态等长不panic() {
    let cases = [
        "echo 'rm -rf /'",
        "echo \"rm -rf /\"",
        "echo `rm -rf /`",
        "Set-Content 'p' 'x'",
        "echo \"a",
        "echo \"a \\\"b\\\" c\"",
    ];
    for command in cases {
        assert_mask_invariant(command, &shell::mask_quoted_and_heredoc(command));
    }

    // 未闭合引号：候选放弃（fail-open，其后按未掩码处理）
    let unclosed = "echo \"a b";
    assert_eq!(
        shell::mask_quoted_and_heredoc(unclosed),
        unclosed,
        "未闭合引号不掩码（fail-open）"
    );
    // 反引号不在掩码引号面（`'` / `"` 两族），命令替换形态按未掩码处理
    let backtick = "echo `rm -rf /`";
    assert_eq!(
        shell::mask_quoted_and_heredoc(backtick),
        backtick,
        "反引号串不在掩码面（既定口径，不 panic）"
    );
}

#[test]
fn 两变体与原文等长互通() {
    let long = format!("echo {} > p", "y".repeat(1_000));
    let commands = [
        "echo a > b",
        "cat <<EOF\nrm -rf /\nEOF",
        "rm -rf 'a b.txt'",
        "echo 中文🚀 > 目标.txt",
        "sed -i 's/中文/🚀/' p",
        long.as_str(),
    ];
    for command in commands {
        let source = command.chars().count();
        assert_eq!(
            shell::mask_heredocs(command).chars().count(),
            source,
            "变体 A 等长，命令: {command:?}"
        );
        assert_eq!(
            shell::mask_quoted_and_heredoc(command).chars().count(),
            source,
            "变体 B 等长（索引 1:1 互通），命令: {command:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// token 纪律
// ---------------------------------------------------------------------------

#[test]
fn 切分_按空白切分并剥包裹引号剔空() {
    // 口径：split_whitespace 先切、再 trim 首尾引号（不解析引号内的空白边界）
    assert_eq!(shell::tokenize("\"a b\" c"), vec!["a", "b", "c"]);
    assert_eq!(shell::tokenize("'x y' z"), vec!["x", "y", "z"]);
    assert_eq!(
        shell::tokenize("  a\t b\n c  "),
        vec!["a", "b", "c"],
        "多空白 / tab / 换行全剔空"
    );
}

#[test]
fn 切分_不做变量过滤且空串为空表() {
    assert_eq!(
        shell::tokenize("git clean $TARGET"),
        vec!["git", "clean", "$TARGET"],
        "`$` token 原样保留（`git clean` 零路径判定依赖原始 token 计数）"
    );
    assert!(shell::tokenize("").is_empty());
    assert!(shell::tokenize("   \t\n ").is_empty());
}

#[test]
fn 切分_边界字符不panic() {
    assert_eq!(
        shell::tokenize("'中文 路径' 🚀"),
        vec!["中文", "路径", "🚀"],
        "引号内中文 / emoji / 空格原样保留"
    );
    assert_eq!(
        shell::tokenize("'未闭合"),
        vec!["未闭合"],
        "未闭合引号剥到末尾不 panic"
    );
    assert_eq!(
        shell::tokenize(&"t".repeat(10_000)).len(),
        1,
        "10 KB 单 token 不 panic"
    );
}

#[test]
fn flag判定两向() {
    for token in ["-n", "--dry-run", "-i.bak", "-s", "-"] {
        assert!(shell::is_flag(token), "`-` 起手即 flag: {token}");
    }
    for token in ["a", "openspec", "./a", "", "中文", "🚀"] {
        assert!(!shell::is_flag(token), "非 `-` 起手不是 flag: {token:?}");
    }
}

// ---------------------------------------------------------------------------
// extract_file_ops：重定向族 / fd 复制 / 箭头守卫 / 数据面
// ---------------------------------------------------------------------------

#[test]
fn 重定向族五种形态均落到写入落点() {
    for command in [
        "echo {} > p",
        "echo {} >> p",
        "echo {} >| p",
        "echo {} | tee p",
        "echo {} &> p",
    ] {
        let ops = shell::extract_file_ops(command);
        assert!(!ops.is_empty(), "重定向族应产出写入 op: {command:?}");
        assert_eq!(
            unique(&ops),
            vec![(OpKind::Write, "p".to_owned())],
            "目标位唯一且为写入，命令: {command:?}"
        );
    }
}

#[test]
fn 描述符复制不产生文件操作() {
    for command in ["foo 2>&1", "foo 1>&2", "foo 3>&2", "foo >&2"] {
        assert!(
            shell::extract_file_ops(command).is_empty(),
            "纯数字目标 = 描述符复制（非文件），命令: {command:?}"
        );
    }
}

#[test]
fn 箭头与等号形态不误判为写入() {
    for command in ["echo a->b", "node -e \"x => p\"", "echo a => p"] {
        assert!(
            shell::extract_file_ops(command).is_empty(),
            "`>` 前缀守卫（只排 `-` 与 `=`）使箭头 / 等号不误判，命令: {command:?}"
        );
    }
}

#[test]
fn heredoc体与引号内文本按数据面处理() {
    let heredoc = shell::extract_file_ops("cat <<EOF > notes.txt\nrm -rf /\n> ignored.txt\nEOF");
    assert!(!heredoc.is_empty(), "头部重定向仍产出写入 op");
    assert!(
        heredoc
            .iter()
            .all(|op| op.kind == OpKind::Write && target_text(op) == "notes.txt"),
        "体内 `rm` 与 `> ignored.txt` 不算（只余头部目标），实际: {heredoc:?}"
    );

    let quoted = shell::extract_file_ops("echo \"rm -rf /\" > notes.txt");
    assert!(
        quoted.iter().all(|op| op.kind == OpKind::Write),
        "引号内 `rm` 不产出删除 op，实际: {quoted:?}"
    );
    assert!(
        quoted.iter().any(|op| target_text(op) == "notes.txt"),
        "引号外的真实目标照旧提取"
    );
}

#[test]
fn 变量落点丢弃而字面量落点保留() {
    for command in ["echo x > $OUT", "rm $TARGET", "echo x > ${OUT}"] {
        assert!(
            shell::extract_file_ops(command).is_empty(),
            "`$` 落点不可知即丢弃该 op，命令: {command:?}"
        );
    }
    assert_eq!(
        shell::extract_file_ops("cp $SRC dst"),
        vec![write_path("dst")],
        "末参字面量保留（源被丢）"
    );
}

// ---------------------------------------------------------------------------
// extract_file_ops：删除族 / 多目标 / 移动族
// ---------------------------------------------------------------------------

#[test]
fn 删除族七命令名均产出删除落点() {
    for command in [
        "rm p",
        "rm -rf p",
        "rmdir p",
        "unlink p",
        "del p",
        "rd /s /q p",
        "Remove-Item -Recurse p",
        "git rm p",
    ] {
        let ops = shell::extract_file_ops(command);
        assert!(
            ops.contains(&delete_path("p")),
            "含删除落点 p（flag 跳过），命令: {command:?}，实际: {ops:?}"
        );
    }
}

#[test]
fn 删除族多目标保序且重复不折叠() {
    assert_eq!(
        shell::extract_file_ops("rm a b"),
        vec![delete_path("a"), delete_path("b")],
        "多目标保序"
    );
    assert_eq!(
        shell::extract_file_ops("rm p p"),
        vec![delete_path("p"), delete_path("p")],
        "重复目标不去重"
    );
    assert!(
        shell::extract_file_ops("rm -f").is_empty(),
        "只有 flag 时零 op"
    );
    assert_eq!(
        shell::extract_file_ops("rm a && rm b"),
        vec![delete_path("a"), delete_path("b")],
        "链式命令保序"
    );
}

#[test]
fn 移动族产出旧删加新写两条() {
    for command in [
        "mv a b",
        "ren a b",
        "Rename-Item a b",
        "Move-Item a b",
        "mv -f a b",
    ] {
        assert_eq!(
            shell::extract_file_ops(command),
            vec![delete_path("a"), write_path("b")],
            "旧路径删除 + 新路径写入，命令: {command:?}"
        );
    }
}

#[test]
fn 移动族单参或零参不猜目标() {
    for command in ["mv p", "mv"] {
        assert!(
            shell::extract_file_ops(command).is_empty(),
            "非恰两字面路径 token 不产出 op，命令: {command:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// extract_file_ops：PowerShell 方言
// ---------------------------------------------------------------------------

#[test]
fn ps重定向与六cmdlet均产出写入落点() {
    for command in ["2> p", "*> p"] {
        let ops = shell::extract_file_ops(command);
        assert!(!ops.is_empty(), "PS 重定向应产出写入 op: {command:?}");
        assert_eq!(
            unique(&ops),
            vec![(OpKind::Write, "p".to_owned())],
            "重定向目标唯一，命令: {command:?}"
        );
    }
    for command in [
        "Set-Content p",
        "Out-File p",
        "Add-Content p",
        "Export-Csv p",
        "Export-CliXml p",
        "Tee-Object p",
        "Set-Content -Path p",
    ] {
        assert_eq!(
            shell::extract_file_ops(command),
            vec![write_path("p")],
            "cmdlet 目标位（可带 -Path）取到 p，命令: {command:?}"
        );
    }
}

#[test]
fn dotnet静态写方法取首参而读方法与变量首参不产出() {
    for command in [
        "[System.IO.File]::WriteAllText(\"p\", \"x\")",
        "[System.IO.File]::WriteAllLines(\"p\", $x)",
        "[System.IO.File]::WriteAllBytes(\"p\", $b)",
        "[System.IO.File]::AppendAllText(\"p\", \"x\")",
    ] {
        assert_eq!(
            shell::extract_file_ops(command),
            vec![write_path("p")],
            "四个写方法首参即落点，命令: {command:?}"
        );
    }
    assert!(
        shell::extract_file_ops("[System.IO.File]::WriteAllText($p, \"x\")").is_empty(),
        "首参为变量（`$`）即丢弃"
    );
    assert!(
        shell::extract_file_ops("[System.IO.File]::ReadAllText(\"p\")").is_empty(),
        "读方法不在写入面"
    );
}

// ---------------------------------------------------------------------------
// extract_file_ops：档 B 五族
// ---------------------------------------------------------------------------

#[test]
fn cp族目标位取末个字面token() {
    for command in ["cp a p", "cp -r a p", "cp a b p", "Copy-Item -Recurse a p"] {
        assert_eq!(
            shell::extract_file_ops(command),
            vec![write_path("p")],
            "cp 族目标位 = 末个非 flag 非 `$` token，命令: {command:?}"
        );
    }
}

#[test]
fn dd只取of等号而if等号为源() {
    assert_eq!(
        shell::extract_file_ops("dd if=/tmp/x of=p"),
        vec![write_path("p")],
        "`of=` 即落点"
    );
    let reverse = shell::extract_file_ops("dd if=p of=/tmp/x");
    assert_eq!(reverse, vec![write_path("/tmp/x")], "`if=` 显式跳过");
    assert!(
        !reverse.iter().any(|op| target_text(op) == "p"),
        "源路径 p 零 op"
    );
}

#[test]
fn sed与perl仅原地改写形态产出写入落点() {
    for command in [
        "sed -i 's/a/b/' p",
        "sed -i.bak 's/a/b/' p",
        "perl -i -pe 's/a/b/' p",
    ] {
        assert_eq!(
            shell::extract_file_ops(command),
            vec![write_path("p")],
            "原地改写取末个非 flag token，命令: {command:?}"
        );
    }
    for command in ["sed 's/a/b/' p", "perl -pe 's/a/b/' p"] {
        assert!(
            shell::extract_file_ops(command).is_empty(),
            "无 `-i` 的纯读 MUST NOT 产出 op（不误拦），命令: {command:?}"
        );
    }
}

#[test]
fn truncate取size取值之后的路径() {
    assert_eq!(
        shell::extract_file_ops("truncate -s 0 p"),
        vec![write_path("p")]
    );
    assert_eq!(
        shell::extract_file_ops("truncate --size=0 p"),
        vec![write_path("p")]
    );
    assert!(
        shell::extract_file_ops("truncate p").is_empty(),
        "无 `-s` / `--size=` 不产出 op"
    );
}

#[test]
fn find仅delete形态取搜索根为树落点() {
    assert_eq!(
        shell::extract_file_ops("find . -delete"),
        vec![delete_tree(".")],
        "搜索根 `.` 走子树落点"
    );
    assert_eq!(
        shell::extract_file_ops("find src -name '*.rs' -delete"),
        vec![delete_tree("src")],
        "首个非 flag token 即搜索根"
    );
    assert!(
        shell::extract_file_ops("find . -print").is_empty(),
        "无 `-delete` 不产出 op"
    );
}

// ---------------------------------------------------------------------------
// extract_file_ops：畸形 / 非 ASCII / 超长
// ---------------------------------------------------------------------------

#[test]
fn 畸形输入不panic且产出有限() {
    for command in ["", "   ", "-rf", ";;&&"] {
        assert!(
            shell::extract_file_ops(command).is_empty(),
            "畸形输入零 op，命令: {command:?}"
        );
    }
    assert!(
        shell::extract_file_ops("echo \"a").is_empty(),
        "未闭合引号不产出 op 且不 panic"
    );
    let unclosed_heredoc = shell::extract_file_ops("cat <<EOF > p");
    assert!(
        unclosed_heredoc
            .iter()
            .all(|op| op.kind == OpKind::Write && target_text(op) == "p"),
        "未闭合 heredoc 候选丢弃后仍只余头部重定向，实际: {unclosed_heredoc:?}"
    );
}

#[test]
fn 非ascii命令文本索引不错位() {
    let leading = shell::extract_file_ops("echo 中文路径🚀 > p");
    assert!(!leading.is_empty());
    assert!(
        leading
            .iter()
            .all(|op| op.kind == OpKind::Write && target_text(op) == "p"),
        "`>` 之后取对（char 索引不错位），实际: {leading:?}"
    );

    assert!(
        shell::extract_file_ops("echo 🚀 > 中文.txt").contains(&write_path("中文.txt")),
        "非 ASCII 落点原文保留"
    );
}

#[test]
fn 超长命令尾部写入仍被提取() {
    let command = format!("echo {} > p", "y".repeat(100_000));
    let ops = shell::extract_file_ops(&command);
    assert!(
        ops.contains(&write_path("p")),
        "100 KB 命令尾部重定向仍提取（不设长度上限）"
    );
}

// ---------------------------------------------------------------------------
// extract_git_ops
// ---------------------------------------------------------------------------

#[test]
fn git_restore的source形态为写而入形态为撤销() {
    assert_eq!(
        shell::extract_git_ops("git restore --source=HEAD~1 p"),
        vec![write_path("p")],
        "`--source=` 形态是从仓库取回内容 → 写入面"
    );
    assert_eq!(
        shell::extract_git_ops("git restore -s HEAD p"),
        vec![write_path("p")],
        "`-s <ref>` 分离值形态同 write（ref 不作路径）"
    );
    assert_eq!(
        shell::extract_git_ops("git restore p"),
        vec![revert_path("p")],
        "无 source 即撤销动作"
    );
}

#[test]
fn git_checkout两态与无分隔符零op() {
    assert_eq!(
        shell::extract_git_ops("git checkout main -- p"),
        vec![write_path("p")],
        "`--` 前有 commit-ish → write"
    );
    assert_eq!(
        shell::extract_git_ops("git checkout -- p"),
        vec![revert_path("p")],
        "裸 `--` → revert"
    );
    assert!(
        shell::extract_git_ops("git checkout main").is_empty(),
        "无 `--` 即切分支（不产出路径 op）"
    );
}

#[test]
fn git_rm为删除而纯读子命令零op() {
    assert!(
        shell::extract_git_ops("git rm p").contains(&delete_path("p")),
        "`git rm` → 删除落点"
    );
    for command in ["git status", "git diff p", "git log --oneline"] {
        assert!(
            shell::extract_git_ops(command).is_empty(),
            "纯读子命令零 op，命令: {command:?}"
        );
    }
}

#[test]
fn 引号内git不算命令而掩码后真实命令仍可定位() {
    assert!(
        shell::extract_git_ops("echo \"git restore p\"").is_empty(),
        "引号内文本不是命令（变体 B 掩码后定位不到命令名）"
    );
    assert!(
        shell::extract_git_ops("printf 'git rm p'").is_empty(),
        "单引号内文本同（数据面）"
    );
    assert!(
        shell::extract_git_ops("echo \"git rm p\" && git rm p").contains(&delete_path("p")),
        "引号外的真实命令仍被定位"
    );
}

// ---------------------------------------------------------------------------
// bulk_revert_detail：git stash
// ---------------------------------------------------------------------------

#[test]
fn 批量还原_git_stash族命中() {
    let cwd = tempdir("bulk-stash");
    for command in [
        "git stash",
        "git stash push",
        "git stash pop",
        "git stash apply",
        "git stash drop",
    ] {
        let detail = bulk_hit(command, cwd.path()).expect("stash 变更态一律命中");
        assert!(
            detail.contains("stash"),
            "中间事实串含 stash 语义，命令: {command:?}，实际: {detail}"
        );
    }
}

#[test]
fn 批量还原_git_stash查看形态放行() {
    let cwd = tempdir("bulk-stash-readonly");
    for command in ["git stash list", "git stash show", "git stash list -p"] {
        assert_eq!(
            bulk_hit(command, cwd.path()),
            None,
            "只读查看形态放行，命令: {command:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// bulk_revert_detail：git clean
// ---------------------------------------------------------------------------

#[test]
fn 批量还原_git_clean无路径参数即命中() {
    let cwd = tempdir("bulk-clean-bare");
    for command in ["git clean", "git clean -fd", "git clean -fdx"] {
        let detail = bulk_hit(command, cwd.path()).expect("无路径参数即拒");
        assert!(
            detail.contains("clean"),
            "中间事实串含 clean 语义，命令: {command:?}，实际: {detail}"
        );
    }
}

#[test]
fn 批量还原_git_clean预览形态放行() {
    let cwd = tempdir("bulk-clean-dry");
    for command in ["git clean -n", "git clean --dry-run", "git clean -nd"] {
        assert_eq!(
            bulk_hit(command, cwd.path()),
            None,
            "预览形态放行，命令: {command:?}"
        );
    }
}

#[test]
fn 批量还原_git_clean路径参数按子树相交判定() {
    let cwd = tempdir("bulk-clean-path");
    let anchor = normalized(cwd.path());

    let domain = layout::domain_dir_name();
    let hit = bulk_hit(&format!("git clean -fd {domain}"), cwd.path()).expect("域目录子树命中");
    assert!(
        hit.contains(&format!("{anchor}/{domain}")),
        "事实串携带解析后的落点，实际: {hit}"
    );

    let dot = bulk_hit("git clean -fd .", cwd.path()).expect("`.` 归一后为受保护前缀祖先");
    assert!(dot.contains(&anchor), "`.` 锚定 cwd，实际: {dot}");

    assert_eq!(
        bulk_hit("git clean -fd src", cwd.path()),
        None,
        "源码子树参数放行（不误拦）"
    );
}

#[test]
fn 批量还原_同一命令按各自cwd解析落点() {
    let first = tempdir("bulk-clean-cwd-a");
    let second = tempdir("bulk-clean-cwd-b");
    let command = format!("git clean -fd {}", layout::domain_dir_name());

    let first_detail = bulk_hit(&command, first.path()).expect("cwd-A 域目录子树命中");
    let second_detail =
        bulk_hit(&command, second.path()).expect("cwd-B 同样命中（`**/` 规则不锚定具体 root）");
    assert!(
        first_detail.contains(&normalized(first.path())),
        "落点以 cwd-A 为基座，实际: {first_detail}"
    );
    assert!(
        second_detail.contains(&normalized(second.path())),
        "落点以 cwd-B 为基座（内部零 IO，纯文本解析），实际: {second_detail}"
    );
    assert_ne!(first_detail, second_detail, "两 cwd 的事实串不同");
}

// ---------------------------------------------------------------------------
// bulk_revert_detail：restore / checkout --（revert 目标）
// ---------------------------------------------------------------------------

#[test]
fn 批量还原_revert目标命中changes子树() {
    let cwd = tempdir("bulk-revert-changes");
    let changes = changes_relative();

    let restore =
        bulk_hit(&format!("git restore {changes}"), cwd.path()).expect("restore 命中 changes 子树");
    assert!(
        restore.contains("git restore"),
        "事实串标注 restore 语义，实际: {restore}"
    );

    assert!(
        bulk_hit(&format!("git checkout -- {changes}"), cwd.path()).is_some(),
        "checkout -- 的 revert 目标同判"
    );
    assert_eq!(
        bulk_hit(&format!("git checkout main -- {changes}"), cwd.path()),
        None,
        "带 commit-ish 的 checkout 是 write 态（不入批量还原判定），实际行为锁定"
    );
}

#[test]
fn 批量还原_源码目标放行() {
    let cwd = tempdir("bulk-revert-source");
    for command in [
        "git restore src/a.rs",
        "git checkout -- packages/desktop/src/a.ts",
    ] {
        assert_eq!(
            bulk_hit(command, cwd.path()),
            None,
            "规范撤销动作（源码目标）放行，命令: {command:?}"
        );
    }
}

#[test]
fn 批量还原_引号内与畸形与超长零命中() {
    let cwd = tempdir("bulk-malformed");
    for command in ["echo \"git stash\"", "echo 'git clean -fd'", "", "   "] {
        assert_eq!(
            bulk_hit(command, cwd.path()),
            None,
            "数据面 / 空命令零命中，命令: {command:?}"
        );
    }

    let long = format!("echo {}", "y".repeat(100_000));
    assert_eq!(
        bulk_hit(&long, cwd.path()),
        None,
        "100 KB 无命中内容的命令不 panic 且零命中"
    );
}

#[test]
fn 批量还原_多命中取首个事实串() {
    let cwd = tempdir("bulk-first-hit");
    let detail = bulk_hit("git stash && git clean -fd", cwd.path()).expect("首条规则命中");
    assert!(
        detail.contains("stash"),
        "首个命中（stash 一路）即返回，实际: {detail}"
    );
    assert!(
        !detail.contains("clean"),
        "不拼接第二段事实串，实际: {detail}"
    );
}

// ---------------------------------------------------------------------------
// 类型面（FileOp / OpKind / OpTarget）
// ---------------------------------------------------------------------------

#[test]
fn 类型面_file_op两态与枚举变体逐项可辨() {
    let write = write_path("a");
    let tree = delete_tree("a");
    assert_eq!(write.clone(), write, "clone 等价且 PartialEq 自反");
    assert_ne!(write, tree, "操作态 / 落点不同即不等");
    assert_eq!(
        delete_path("a"),
        FileOp {
            kind: OpKind::Delete,
            target: OpTarget::Path("a".to_owned()),
        }
    );

    let kinds = [OpKind::Write, OpKind::Delete, OpKind::Revert];
    assert_eq!(kinds.len(), 3, "操作态恰三值（封闭集）");
    for kind in kinds {
        let copied = kind;
        assert_eq!(copied, kind, "Copy + PartialEq 逐值自反");
    }

    let targets = [
        OpTarget::Path("p".to_owned()),
        OpTarget::Tree("p".to_owned()),
    ];
    assert_eq!(targets.len(), 2, "落点恰两值（Path / Tree）");
    assert!(
        format!("{:?}", targets[1]).contains("Tree"),
        "Debug 渲染含变体名，实际: {:?}",
        targets[1]
    );
    assert!(
        format!("{:?}", write).contains("Write"),
        "Debug 渲染含操作态变体名，实际: {write:?}"
    );
}

/// 域目录命中文档路径的形态锚（`domain_relative` 由 layout 派生，供断言拼接用）。
#[test]
fn 域目录相对路径由layout派生() {
    let relative = domain_relative();
    assert!(
        relative.starts_with(layout::domain_dir_name()),
        "派生路径以域目录名起手，实际: {relative}"
    );
}
