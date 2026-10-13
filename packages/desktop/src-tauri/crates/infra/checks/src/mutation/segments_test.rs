//! `segments.rs` 的单元测试（test-design「segments.rs -> segments_test.rs」
//! 节）：分段切分（估算超预算触发 k 段字典序均分、段并集 = 全清单、零交叠）
//! + 自适应修正（实测速率调段大小——慢于初值变小、快于初值变大）+ 补丁切片
//! （`--in-diff` 按 `diff --git` 文件段头切片、`+++ b/` 兜底、引用路径解码）。
//!
//! Mock策略（test-design 本节 Mock 表）：无——清单 / 补丁文本 / 速率观测值
//! 均为显式入参，纯函数直测零 mock 零 spawn。

use super::super::{
    EST_MUTANTS_PER_FILE, SEGMENT_BUDGET_MS, SEGMENT_GUARD_MS,
};
use super::{
    adaptive_chunk_size, estimate_duration_ms, segment_count, slice_patch, static_chunk_size,
};
/// 静态估算耗时：`EST_MUTANTS_PER_FILE × 文件数 × 保守速率（2s）`——公式面
/// 锚定（20s/文件）。
#[test]
fn 静态估算耗时公式面锚定() {
    assert_eq!(estimate_duration_ms(0), 0, "零文件零耗时");
    assert_eq!(
        estimate_duration_ms(1),
        EST_MUTANTS_PER_FILE as u64 * 2000,
        "单文件 = 10 mutants × 2s = 20s"
    );
    assert_eq!(estimate_duration_ms(15), 300_000, "15 文件 = 300s（恰等段预算刻度）");
    // 饱和运算防溢出（超大文件数不 panic——饱和至 u64::MAX）
    assert_eq!(
        estimate_duration_ms(usize::MAX),
        u64::MAX,
        "usize::MAX 饱和至 u64::MAX 不 panic"
    );
}

/// 段数触发口径：估算 ≤ 段预算不切（k=1）、超 1 即切（k = ceil(估算/预算)）；
/// 恰等不切（阈值边界）。
#[test]
fn 段数触发口径_恰等不切超一即切() {
    // 15 文件 = 300s 恰等 → 不切
    assert_eq!(segment_count(15), 1, "估算恰等段预算不切（阈值边界）");
    // 16 文件 = 320s → ceil(320/300) = 2
    assert_eq!(segment_count(16), 2, "超 1 即切");
    // 30 文件 = 600s → 2；31 文件 = 620s → 3
    assert_eq!(segment_count(30), 2);
    assert_eq!(segment_count(31), 3);
    // 零文件 / 单文件恒 1（单文件超预算接受——段是软目标）
    assert_eq!(segment_count(0), 1);
    assert_eq!(segment_count(1), 1, "单文件超预算接受（不切单文件）");
    // 超大清单不溢出
    assert!(segment_count(usize::MAX) > 1, "超大清单触发多段不溢出");
}

/// 首段静态初值：`ceil(n / k)` 字典序均分；零文件 → 0。
#[test]
fn 首段静态初值均分() {
    assert_eq!(static_chunk_size(0), 0, "零文件零块");
    assert_eq!(static_chunk_size(1), 1, "单文件单块");
    assert_eq!(static_chunk_size(16), 8, "16 文件 / 2 段 = 8");
    assert_eq!(static_chunk_size(31), 11, "31 文件 / 3 段 → ceil = 11");
    // 均分后并集覆盖全清单（k 段 × 块大小 ≥ n）
    for n in [1usize, 2, 15, 16, 31, 100, 1000] {
        let size = static_chunk_size(n);
        let k = segment_count(n);
        assert!(size * k >= n, "n={n}：{size} × {k} ≥ {n}（并集覆盖全清单）");
        assert!(size >= 1, "n={n} 块非空");
    }
}

/// 自适应块大小：`段预算 / (速率 × 单文件密度)` 折算——速率越慢段越小、
/// 越快段越大；下限 1（单文件超预算接受）、上限 remaining；非法速率（NaN /
/// Inf / 零 / 负）退 remaining（静态初值续用面）。
#[test]
fn 自适应块大小_修正方向与上下限() {
    // 慢速率（5 s/mutant → 每文件 50s → 300000/50000 = 6）
    assert_eq!(adaptive_chunk_size(5.0, 100), 6, "慢于保守初值 → 段变小（6 < 15 静态）");
    // 快速率（0.5 s/mutant → 每文件 5s → 300000/5000 = 60）
    assert_eq!(adaptive_chunk_size(0.5, 100), 60, "快于初值 → 段变大");
    // 修正方向：慢速率块 < 快速率块
    assert!(
        adaptive_chunk_size(5.0, 1000) < adaptive_chunk_size(0.5, 1000),
        "修正方向正确（慢小快大）"
    );
    // 下限 1（极慢速率折算 0 → clamp 1——单文件超预算接受）
    assert_eq!(adaptive_chunk_size(1_000_000.0, 10), 1, "下限 1");
    // 上限 remaining（极快速率 → 全剩余一桶）
    assert_eq!(adaptive_chunk_size(0.000_001, 7), 7, "上限 remaining");

    // remaining = 0 → 0
    assert_eq!(adaptive_chunk_size(1.0, 0), 0);

    // 非法速率 → remaining（静态初值续用——段报告未携耗时的回退面）
    for rate in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, 0.0, -1.0] {
        assert_eq!(
            adaptive_chunk_size(rate, 9),
            9,
            "非法速率 {rate} → remaining（回退面）"
        );
    }
}

/// 三段切分组合：估算超预算 → k 段、字典序均分、段并集 = 全清单、段间零交叠
///、顺序稳定（模拟 run_suite 的 cursor 推进）。
#[test]
fn 组合_三段切分并集覆盖零交叠顺序稳定() {
    let files: Vec<String> = (0..45).map(|index| format!("src/f{index:03}.rs")).collect();
    let n = files.len();
    let k = segment_count(n);
    assert_eq!(k, 3, "45 文件 = 900s → 3 段");

    let mut cursor = 0usize;
    let mut chunks: Vec<&[String]> = Vec::new();
    let mut rate: Option<f64> = None;
    while cursor < n {
        let size = match rate {
            Some(rate) => adaptive_chunk_size(rate, n - cursor),
            None => static_chunk_size(n),
        }
        .clamp(1, n - cursor);
        chunks.push(&files[cursor..cursor + size]);
        cursor += size;
        // 模拟首段实测速率慢于初值 → 后续段变小
        rate = Some(3.0);
    }
    // 并集 = 全清单、零交叠、顺序稳定
    let flattened: Vec<&String> = chunks.iter().copied().flatten().collect();
    assert_eq!(flattened, files.iter().collect::<Vec<_>>(), "并集 = 全清单且顺序稳定");
    let mut ranges_sum = 0usize;
    for chunk in &chunks {
        ranges_sum += chunk.len();
        assert!(!chunk.is_empty(), "段非空");
    }
    assert_eq!(ranges_sum, n, "零交叠（长度和 = n）");
}

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

    let sliced = slice_patch(patch, &["src/b.rs".to_owned()]);
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
    let both = slice_patch(patch, &["src/a.rs".to_owned(), "src/b.rs".to_owned()]);
    assert_eq!(both.matches("diff --git").count(), 2, "双文件段齐备");
}

/// 补丁切片边界：空 patch / 畸形 patch（无 `diff --git` 头 / 半截 hunk）→
/// 切片不 panic（行级精度交 cargo-mutants）；文件清单空 → 空串。
#[test]
fn 补丁切片_畸形与空入参不panic() {
    assert_eq!(slice_patch("", &["src/a.rs".to_owned()]), "", "空 patch 切片空串");
    assert_eq!(
        slice_patch("diff --git a/x b/x\n+任意", &[]),
        "",
        "空清单切片空串"
    );
    // 无段头的裸 hunk（异常形态）→ 丢弃不 panic
    assert_eq!(
        slice_patch("@@ -1,2 +1,2 @@\n-old\n+new\n", &["x.rs".to_owned()]),
        "",
        "无段头裸 hunk 丢弃"
    );
    // 半截 hunk（段头在、hunk 截断）→ 原样归属不 panic
    let truncated = "diff --git a/x.rs b/x.rs\n--- a/x.rs\n+++ b/x.rs\n@@ -1,2 +1,2 @@\n-old";
    assert_eq!(
        slice_patch(truncated, &["x.rs".to_owned()]),
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
        slice_patch(space, &["src/my file.rs".to_owned()]).contains("+含空格"),
        "空格路径切片（rfind 兜住）"
    );

    let unicode = "diff --git a/src/文件 🚀.rs b/src/文件 🚀.rs\n\
                   --- a/src/文件 🚀.rs\n\
                   +++ b/src/文件 🚀.rs\n\
                   @@ -1,1 +1,2 @@\n\
                   +中文 emoji 行\n";
    assert!(
        slice_patch(unicode, &["src/文件 🚀.rs".to_owned()]).contains("+中文 emoji 行"),
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
        slice_patch(quoted, &["src/文.txt".to_owned()]),
        "",
        "git 原生引用形态段丢弃不 panic（路径不可辨面）"
    );

    // `+++ b/` 前缀在、路径本体 C 引用的混合形态 → 八进制解码兜底命中
    let hybrid = "diff --git \"a/zzz\" \"b/zzz\"\n\
                  --- \"a/zzz\"\n\
                  +++ b/\"src/\\346\\226\\207.txt\"\n\
                  @@ -1,1 +1,2 @@\n\
                  +解码行\n";
    let sliced = slice_patch(hybrid, &["src/文.txt".to_owned()]);
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
        slice_patch(deleted, &["gone.rs".to_owned()]).contains("-删除行"),
        "删除面段头可辨 → 照常入段（行级精度归 cargo-mutants）"
    );
}

/// 段预算 / 软护栏常量锚定（E9 值锚定——D9 不进 config schema）。
#[test]
fn 段预算与软护栏常量锚定() {
    assert_eq!(SEGMENT_BUDGET_MS, 300_000, "段预算 5 分钟（软目标）");
    assert_eq!(SEGMENT_GUARD_MS, 600_000, "软护栏 10 分钟（仅观察留痕）");
    assert_eq!(EST_MUTANTS_PER_FILE, 10, "首段静态单文件 mutant 密度");
}
