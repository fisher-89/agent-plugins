use super::super::{EST_MUTANTS_PER_FILE, PER_MUTANT_TIMEOUT_SECS, SEGMENT_SPAWN_TIMEOUT_MS};
use super::{estimate_duration_sec, estimate_mutants, run_spawn_timeout_ms, slice_patch};

/// 补丁切片正向：多文件 patch → 段补丁只含段内文件 hunks（`diff --git` 段头
/// 匹配）；前导文本（无段头）丢弃不 panic。
#[test]
fn 补丁切片_段内文件hunks与前导文本丢弃() {
    let patch = "warning: 装饰性前导文本（非 diff 段）\n\
                 diff --git a/src/a.rs b/src/a.rs\n\
                 index 1111111..2222222 100644\n\
                 --- a/src/a.rs\n\
                 +++ b/src/a.rs\n\
                 @@ -1,3 +1,4 @@\n\
                 fn a() {}\n\
                 +新增行 a\n\
                 diff --git a/src/b.rs b/src/b.rs\n\
                 index 3333333..4444444 100644\n\
                 --- a/src/b.rs\n\
                 +++ b/src/b.rs\n\
                 @@ -2,2 +2,3 @@\n\
                 fn b() {}\n\
                 +新增行 b\n";

    let sliced = slice_patch(patch, &["src/b.rs".to_owned()], "");
    assert!(
        sliced.contains("diff --git a/src/b.rs b/src/b.rs"),
        "段补丁含段内文件段头: {sliced}"
    );
    assert!(sliced.contains("+新增行 b"), "段内 hunk 在案");
    assert!(
        !sliced.contains("a/src/a.rs") && !sliced.contains("+新增行 a"),
        "段外文件 hunks 不入段补丁: {sliced}"
    );
    assert!(
        !sliced.contains("装饰性前导文本"),
        "段头前前导文本丢弃（异常形态不 panic）"
    );

    // 多文件段：并集口径
    let both = slice_patch(patch, &["src/a.rs".to_owned(), "src/b.rs".to_owned()], "");
    assert_eq!(both.matches("diff --git").count(), 2, "双文件段齐备");
}

/// 补丁切片边界：空 patch / 畸形 patch（无 `diff --git` 头 / 半截 hunk）→
/// 切片不 panic（行级精度交 cargo-mutants）；文件清单空 → 空串。
#[test]
fn 补丁切片_畸形与空入参不panic() {
    assert_eq!(
        slice_patch("", &["src/a.rs".to_owned()], ""),
        "",
        "空 patch 切片空串"
    );
    assert_eq!(
        slice_patch("diff --git a/x b/x\n+任意", &[], ""),
        "",
        "空清单切片空串"
    );
    // 无段头的裸 hunk（异常形态）→ 丢弃不 panic
    assert_eq!(
        slice_patch("@@ -1,2 +1,2 @@\n-old\n+new\n", &["x.rs".to_owned()], ""),
        "",
        "无段头裸 hunk 丢弃"
    );
    // 半截 hunk（段头在、hunk 截断）→ 原样归属不 panic
    let truncated = "diff --git a/x.rs b/x.rs\n--- a/x.rs\n+++ b/x.rs\n@@ -1,2 +1,2 @@\n-old";
    assert_eq!(
        slice_patch(truncated, &["x.rs".to_owned()], ""),
        truncated,
        "半截 hunk 原样归属（行级精度交 cargo-mutants）"
    );
}

/// 特殊文件名切片：路径含空格 / 中文 / emoji / 深层路径 → 段头匹配与清单
/// 切片不破坏（`rfind(" b/")` 兜住空格、`+++ b/` 兜底引用形态）。
#[test]
fn 补丁切片_特殊文件名不破坏() {
    let space = "diff --git a/src/my file.rs b/my file.rs\n\
                 --- a/src/my file.rs\n\
                 +++ b/src/my file.rs\n\
                 @@ -1,1 +1,2 @@\n\
                 +含空格\n";
    assert!(
        slice_patch(space, &["src/my file.rs".to_owned()], "").contains("+含空格"),
        "空格路径切片（rfind 兜住）"
    );

    let unicode = "diff --git a/src/文件 🚀.rs b/src/文件 🚀.rs\n\
                   --- a/src/文件 🚀.rs\n\
                   +++ b/src/文件 🚀.rs\n\
                   @@ -1,1 +1,2 @@\n\
                   +中文 emoji 行\n";
    assert!(
        slice_patch(unicode, &["src/文件 🚀.rs".to_owned()], "").contains("+中文 emoji 行"),
        "中文 / emoji 路径切片"
    );

    // git 原生引用形态（quotePath 默认——非 ASCII 路径整行引用）：段头与
    // `+++` 行俱引用 → 路径不可辨 → 段丢弃不 panic（设计：段头不可辨的段丢
    // 弃，行级精度交 cargo-mutants）
    let quoted = "diff --git \"a/src/\\346\\226\\207.txt\" \"b/src/\\346\\226\\207.txt\"\n\
                  index 1111111..2222222 100644\n\
                  --- \"a/src/\\346\\226\\207.txt\"\n\
                  +++ \"b/src/\\346\\226\\207.txt\"\n\
                  @@ -1,1 +1,2 @@\n\
                  +解码行\n";
    assert_eq!(
        slice_patch(quoted, &["src/文.txt".to_owned()], ""),
        "",
        "git 原生引用形态段丢弃不 panic（路径不可辨面）"
    );

    // `+++ b/` 前缀在、路径本体 C 引用的混合形态 → 八进制解码兜底命中
    let hybrid = "diff --git \"a/zzz\" \"b/zzz\"\n\
                  --- \"a/zzz\"\n\
                  +++ b/\"src/\\346\\226\\207.txt\"\n\
                  @@ -1,1 +1,2 @@\n\
                  +解码行\n";
    let sliced = slice_patch(hybrid, &["src/文.txt".to_owned()], "");
    assert!(
        sliced.contains("+解码行"),
        "+++ b/ 前缀 + C 引用路径八进制解码兜底命中: {sliced}"
    );

    // 删除面（+++ /dev/null）：段头 a/ 侧可辨（gone.rs 命中清单）→ 删除段照
    // 常入段（行级 diff 归 cargo-mutants；`/dev/null` 判定只在段头不可辨的
    // +++ 兜底面生效）
    let deleted = "diff --git a/gone.rs b/gone.rs\n\
                   deleted file mode 100644\n\
                   --- a/gone.rs\n\
                   +++ /dev/null\n\
                   @@ -1,1 +0,0 @@\n\
                   -删除行\n";
    assert!(
        slice_patch(deleted, &["gone.rs".to_owned()], "").contains("-删除行"),
        "删除面段头可辨 → 照常入段（行级精度归 cargo-mutants）"
    );
}

/// 段补丁路径重写：清单匹配恒项目根相对、落盘段补丁剥 suite 根前缀——
/// `diff --git a/<p> b/<p>` / `--- a/<p>` / `+++ b/<p>` 三类行剥
/// `<strip_dir>/`，hunk 体与 `/dev/null` 侧逐字保留；前缀空串 = suite 锚项目
/// 根（cargo mutants 以 spawn cwd 相对路径匹配 `--in-diff`，Stryker mutate 清
/// 单 relative_to_cwd 同约定）。
#[test]
fn 补丁切片_suite根前缀剥离与空串透传() {
    let patch = "diff --git a/app/src/lib.rs b/app/src/lib.rs\n\
                 index 1111111..2222222 100644\n\
                 --- a/app/src/lib.rs\n\
                 +++ b/app/src/lib.rs\n\
                 @@ -1,2 +1,3 @@\n\
                 fn a() {}\n\
                 +新增行\n";

    // suite 根前缀剥离：三类路径行剥 `app/`，hunk 体逐字保留
    let stripped = slice_patch(patch, &["app/src/lib.rs".to_owned()], "app");
    assert_eq!(
        stripped,
        "diff --git a/src/lib.rs b/src/lib.rs\n\
         index 1111111..2222222 100644\n\
         --- a/src/lib.rs\n\
         +++ b/src/lib.rs\n\
         @@ -1,2 +1,3 @@\n\
         fn a() {}\n\
         +新增行\n",
        "三类路径行剥 suite 根前缀，其余行逐字保留"
    );

    // 前缀空串（suite 锚项目根）→ 逐字透传（路径已相对执行目录）
    assert_eq!(
        slice_patch(patch, &["app/src/lib.rs".to_owned()], ""),
        patch,
        "strip_dir 空串全段透传"
    );

    // 删除面：`+++ /dev/null` 侧不剥、`--- a/` 侧照剥
    let deleted = "diff --git a/app/src/gone.rs b/app/src/gone.rs\n\
                   deleted file mode 100644\n\
                   --- a/app/src/gone.rs\n\
                   +++ /dev/null\n\
                   @@ -1,1 +0,0 @@\n\
                   -删除行\n";
    assert_eq!(
        slice_patch(deleted, &["app/src/gone.rs".to_owned()], "app"),
        "diff --git a/src/gone.rs b/src/gone.rs\n\
         deleted file mode 100644\n\
         --- a/src/gone.rs\n\
         +++ /dev/null\n\
         @@ -1,1 +0,0 @@\n\
         -删除行\n",
        "删除面剥 a/ 侧、/dev/null 侧保留"
    );

    // 新增面：`--- /dev/null` 侧不剥、`+++ b/` 侧照剥
    let added = "diff --git a/app/src/new.rs b/app/src/new.rs\n\
                 new file mode 100644\n\
                 --- /dev/null\n\
                 +++ b/app/src/new.rs\n\
                 @@ -0,0 +1,1 @@\n\
                 +新增行\n";
    assert_eq!(
        slice_patch(added, &["app/src/new.rs".to_owned()], "app"),
        "diff --git a/src/new.rs b/src/new.rs\n\
         new file mode 100644\n\
         --- /dev/null\n\
         +++ b/src/new.rs\n\
         @@ -0,0 +1,1 @@\n\
         +新增行\n",
        "新增面剥 b/ 侧、/dev/null 侧保留"
    );

    // 前缀不匹配的路径（跨 suite 前缀误入清单的防御面）→ 原样保留不误剥
    let other = "diff --git a/other/src/lib.rs b/other/src/lib.rs\n\
                 --- a/other/src/lib.rs\n\
                 +++ b/other/src/lib.rs\n\
                 @@ -1,1 +1,2 @@\n\
                 +新增行\n";
    assert_eq!(
        slice_patch(other, &["other/src/lib.rs".to_owned()], "app"),
        other,
        "非 suite 根前缀路径不误剥"
    );
}

// ---------------------------------------------------------------------------
// 整体估算（不切段）：耗时 = 固定开销 + 突变体数 × 单突变体墙钟；spawn 超时 =
// 估算 × 2（保守倍数），下限为固定护栏
// ---------------------------------------------------------------------------

/// 正向：耗时估算只含突变体数线性项（无固定开销项——换仓库不失效）。
#[test]
fn 整体耗时估算_只含线性项() {
    assert_eq!(
        estimate_duration_sec(0),
        0.0,
        "零突变体零耗时（无固定开销项）"
    );
    assert_eq!(
        estimate_duration_sec(100),
        100.0 * super::SEC_PER_MUTANT,
        "线性项 = 突变体数 × 单突变体墙钟"
    );
    // 单调递增（突变体越多估得越久）
    assert!(estimate_duration_sec(200) > estimate_duration_sec(100));
}

/// 边界：突变体数兜底估算 = 文件数 × 单文件密度（分母口径必须是文件数）。
#[test]
fn 突变体数兜底估算_文件数乘密度() {
    assert_eq!(estimate_mutants(0), 0, "零文件零突变体");
    assert_eq!(estimate_mutants(1), EST_MUTANTS_PER_FILE, "单文件 = 密度");
    assert_eq!(estimate_mutants(14), 14 * EST_MUTANTS_PER_FILE);
    // 饱和不 panic（超大文件数）
    assert_eq!(estimate_mutants(usize::MAX), u64::MAX, "饱和至 u64::MAX");
}

/// 正向：spawn 超时 = 估算耗时 × 安全倍数，且不低于固定护栏下限。
#[test]
fn 整体spawn超时_估算乘安全倍数且不低于下限() {
    // 大突变体数下安全倍数生效
    let many = 10_000u64;
    let expected = (estimate_duration_sec(many) * super::TIMEOUT_SAFETY_FACTOR * 1000.0) as u64;
    assert_eq!(run_spawn_timeout_ms(many), expected);
    assert!(
        run_spawn_timeout_ms(many) > many * super::SEC_PER_MUTANT as u64 * 1000,
        "超时必须是估算的倍数"
    );
    // 小值 / 零值退回下限
    assert_eq!(
        run_spawn_timeout_ms(0),
        SEGMENT_SPAWN_TIMEOUT_MS,
        "零突变体回下限"
    );
    assert_eq!(
        run_spawn_timeout_ms(1),
        SEGMENT_SPAWN_TIMEOUT_MS,
        "单突变体回下限"
    );
}

/// 常量锚定：单突变体墙钟取实测上界（8.5 实测 → 10），per-mutant 超时沿用。
#[test]
fn 速率常数取实测上界() {
    assert_eq!(super::SEC_PER_MUTANT, 10.0, "实测 8.5 s/mutant 取整上界");
    assert_eq!(PER_MUTANT_TIMEOUT_SECS, 300, "per-mutant 超时不变");
    assert_eq!(EST_MUTANTS_PER_FILE, 22, "单文件密度按实测 21.6 取上界");
}
