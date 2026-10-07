//! `identity` 的单元测试（test-design「identity.rs -> identity_test.rs」节）：
//! workspace 身份段清洗算法行为平移（自 store_test「派生单点纯函数可读段
//! 清洗_截断_非法字符_尾点空格与空回退」整体迁移——算法与参数逐字平移，
//! 断言零丢失）；哈希半边以 sha2 独立对拍 + 固定期望向量钉死跨实现稳定。
//!
//! 同源锚注记：db 文件名（store 半边）与 worktree 落位（vcs 半边）的同源
//! 消费对拍经各自 crate 的委托等值行承载（crate 图：core 无 store / vcs
//! 依赖边——跨 crate 对拍在消费侧），本节钉单点形态与确定性。

use sha2::{Digest, Sha256};

use super::identity::workspace_identity_segment;

/// 独立对拍摘要：SHA-256(canonical_root UTF-8) 前 16 字节 32 位小写 hex
///（不经被测函数重算——hash 半边的第二来源）。
fn expected_hash(canonical_root: &str) -> String {
    let digest = Sha256::digest(canonical_root.as_bytes());
    digest[..16]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// 取 segment 的可读段（`{可读段}-{32 位 hash}` 的连字符前半；可读段可自含
/// 连字符——按尾段 hash 长度剥离，纯哈希回退时原样返回）。
fn readable_of(segment: &str) -> &str {
    match segment.len().checked_sub(33) {
        Some(idx) if segment.as_bytes()[idx] == b'-' => &segment[..idx],
        _ => segment,
    }
}

/// 常规路径基本形态：`{末段目录名}-{32 位小写 hex}`；hash 与独立对拍逐字
/// 一致，另以固定期望向量钉死跨实现稳定（平台间 hash 面恒定——hash 输入是
/// 完整 canonical root 串）。
#[test]
fn 基本形态_末段目录名与sha256前16字节小写hex拼接() {
    let root = "C:\\ws\\demo-alpha";
    let segment = workspace_identity_segment(root);

    // 固定期望向量（sha256("C:\ws\demo-alpha") 前 16 字节）：跨实现稳定锚
    assert_eq!(segment, "demo-alpha-087cea553ac3436f3b2eb2824a9b1faa");

    // 独立对拍：hash 半边与 sha2 第二来源逐字一致
    let hash = expected_hash(root);
    assert_eq!(hash.len(), 32, "hash 恒 32 位小写 hex");
    assert!(
        hash.chars()
            .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c)),
        "hash 为小写 hex: {hash}"
    );
    assert!(
        segment.ends_with(&hash),
        "segment 尾段 = SHA-256(canonical root) 前 16 字节，实际: {segment}"
    );

    // POSIX 形态根：末段目录名同式（向量第二点）
    let posix = "/home/u/repo";
    assert_eq!(
        workspace_identity_segment(posix),
        format!("repo-{}", expected_hash(posix)),
        "POSIX 根形态同式（可读段 = 末段目录名）"
    );
}

/// 非法字符清洗：`/ \ : * ? " < > |` 与控制符逐字置换 `_`；产物不含任何
/// OS 非法字符（文件名 / 目录名共用安全面）。
#[test]
fn 非法字符逐字置换下划线且产物零非法字符() {
    // 末段目录名含全部非法字符族（路径分隔先行由 file_name 剥离；其余逐字置换）
    let root = "C:\\ws\\c:d*e?f\"g<h>i|j";
    let segment = workspace_identity_segment(root);
    let readable = readable_of(&segment);
    assert_eq!(readable, "c_d_e_f_g_h_i_j", "非法字符逐字置换 _");

    // 控制符同置换（不可见字符不进产物）
    let control_root = "C:\\ws\u{07}bell";
    let segment = workspace_identity_segment(control_root);
    assert!(
        segment.starts_with("ws_bell-"),
        "控制符置换 _（不悬挂不进产物）: {segment}"
    );

    for root in [
        "C:\\ws\\c:d*e?f\"g<h>i|j",
        "C:\\ws\u{07}bell",
        "C:\\ws\\slash/name",
    ] {
        let segment = workspace_identity_segment(root);
        assert!(
            !segment.chars().any(|c| {
                matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') || c.is_control()
            }),
            "产物不含任何 OS 非法字符（{root} -> {segment}）"
        );
    }
}

/// char 截断：可读段超 24 字符按 char boundary 截断恰 24（多字节 emoji 不
/// 悬挂不 panic——半个字符不产出）。
#[test]
fn 超长可读段按char截断恰24且多字节不悬挂() {
    // ASCII 超 24 → 前 24 字符
    let long = "C:\\ws\\a-very-long-workspace-directory-name";
    let segment = workspace_identity_segment(long);
    let readable = readable_of(&segment);
    assert_eq!(
        readable, "a-very-long-workspace-di",
        "超 24 字符截断为前 24 字符"
    );

    // 恰 24 个四字节 emoji：char 计数 24 不截断（byte 长度 96 超 24——char
    // boundary 口径，非 byte 截断）
    let emoji_root = "C:\\ws\\😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀";
    let segment = workspace_identity_segment(emoji_root);
    let readable = readable_of(&segment);
    assert_eq!(readable.chars().count(), 24, "char boundary 截断 24 字符");
    assert!(readable.chars().all(|c| c == '😀'), "截断不产生半个字符");

    // 25 个 emoji → 恰取前 24（char 截断面）
    let emoji_25 = "C:\\ws\\😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀";
    let segment = workspace_identity_segment(emoji_25);
    let readable = readable_of(&segment);
    assert_eq!(readable.chars().count(), 24, "25 emoji 截 24");
}

/// 尾部清洗与空回退：尾部 `.` 与空格去除（Windows 保留语义）；清洗后为空
/// → 纯 32 位 hex 无 `-` 连接。
#[test]
fn 尾部点空格去除且空回退纯哈希名() {
    let dotted = "C:\\ws\\trailing...  ";
    let segment = workspace_identity_segment(dotted);
    let readable = readable_of(&segment);
    assert_eq!(readable, "trailing", "尾部 `.` 与空格去除");

    // 清洗后为空 → 纯哈希（固定向量 + 形态断言双锚）
    let empty = "C:\\ws\\...";
    let segment = workspace_identity_segment(empty);
    assert_eq!(
        segment, "1b51c6e8e3bca14384443df7b989175c",
        "空回退固定向量"
    );
    assert_eq!(segment.len(), 32, "回退名 32 位 hex");
    assert!(
        !segment.contains('-'),
        "空可读段回退纯哈希名（无连字符）: {segment}"
    );
    assert_eq!(segment, expected_hash(empty), "回退名 = 独立对拍 hash");
    assert!(
        segment
            .chars()
            .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c)),
        "回退名为纯 32 位小写 hex: {segment}"
    );
}

/// 确定性：同根两次调用逐字相等（跨重启可复现）；异根产物必不同。
#[test]
fn 同根确定可复现且异根必不同() {
    for root in [
        "C:\\ws\\demo-alpha",
        "C:\\ws\\...",
        "/home/u/repo",
        "C:\\ws\\😀😀😀",
    ] {
        assert_eq!(
            workspace_identity_segment(root),
            workspace_identity_segment(root),
            "同根两次派生逐字相等（{root}）"
        );
    }
    // 异根必不同（可读段同、hash 异；可读段异更显式不同）
    let a = workspace_identity_segment("C:\\ws\\alpha");
    let b = workspace_identity_segment("C:\\ws\\beta");
    assert_ne!(a, b, "异根产物必不同");
    let long_a = workspace_identity_segment("C:\\ws\\same-long-readab-1");
    let long_b = workspace_identity_segment("C:\\ws\\same-long-readab-2");
    assert_ne!(long_a, long_b, "可读段截断后同形仍以 hash 区分（异根不撞）");
}

/// 同源锚（单点形态钉）：segment 为单分量文件名词汇（db 文件名 stem 与
/// worktree 子目录名共用形态）——不含路径分隔符、非空、长度有界；db 文件
/// 名消费半边（`{segment}.redb` 委托等值）经 store_test 同名回归行承载、
/// worktree 落位消费半边经 vcs lib_test 落位形态行承载（跨 crate 对拍归
/// 消费侧）。
#[test]
fn 同源锚_单分量文件名词汇面供双消费者共用() {
    for root in [
        "C:\\ws\\demo-alpha",
        "C:\\ws\\...",
        "C:\\ws\\c:d*e?f\"g<h>i|j",
        "C:\\ws\\a-very-long-workspace-directory-name",
        "/home/u/repo",
    ] {
        let segment = workspace_identity_segment(root);
        assert!(!segment.is_empty(), "segment 非空（{root}）");
        assert!(
            !segment.contains(['/', '\\', ':']),
            "segment 不含路径分隔符（db stem / 目录名单分量词汇面）: {segment}"
        );
        // 长度有界：可读段 ≤24 + '-' + 32 hex = 57 字符上界（纯哈希 32）
        assert!(
            segment.chars().count() <= 57,
            "segment 长度有界（清洗预算），实际: {}",
            segment.chars().count()
        );
    }
}
