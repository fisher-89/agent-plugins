//! `preamble` 的单元测试（AC-2）：AGENT.md 读取语义——在场逐字、缺席或读取
//! 失败一律 `None`、32KB 截断（char 边界对齐）、每轮重读无缓存。文件为进程
//! 边界依赖白名单：tempfile tempdir 真实文件系统承载 fixture（写 / 改 / 删均
//! 为真实盘上操作，不 mock 文件）。

use std::path::Path;

use crate::sdk::preamble::load;

fn tempdir(tag: &str) -> tempfile::TempDir {
    tempfile::Builder::new()
        .prefix(&format!("sdk-preamble-test-{tag}-"))
        .tempdir()
        .expect("创建合成目录失败")
}

/// 在 root 下写 AGENT.md（真实盘上操作）。
fn write_agent_md(root: &Path, content: &str) {
    std::fs::write(root.join("AGENT.md"), content).expect("写 AGENT.md 失败");
}

// ---------------------------------------------------------------------------
// 正向：在场逐字
// ---------------------------------------------------------------------------

#[tokio::test]
async fn agent_md在场逐字返回_含中文emoji换行无包装无前后缀() {
    let dir = tempdir("verbatim");
    let content = "# 工作区约定 🎉\n\n- 提交信息用中文\n- 表格对齐\n第二段落\n";
    write_agent_md(dir.path(), content);

    let preamble = load(dir.path()).await.expect("在场必返回 Some");

    assert_eq!(
        preamble.as_str(),
        content,
        "逐字语义：与文件内容逐字节一致（无包装、无前缀后缀）"
    );
}

// ---------------------------------------------------------------------------
// 异常：缺席 / 读取失败一律 None（严格无兜底）
// ---------------------------------------------------------------------------

#[tokio::test]
async fn agent_md缺席返回none且同目录claudemd不作兜底() {
    let dir = tempdir("absent");

    // root 无 AGENT.md：缺席即 None
    assert!(
        load(dir.path()).await.is_none(),
        "缺席 AGENT.md 必须返回 None"
    );

    // 严格无兜底：同目录放 CLAUDE.md 仍 None（读侧不回退其它文件名）
    std::fs::write(dir.path().join("CLAUDE.md"), "# claude 侧提示词").expect("写 CLAUDE.md 失败");
    assert!(
        load(dir.path()).await.is_none(),
        "同目录 CLAUDE.md 不得兜底（严格单文件语义）"
    );
}

#[tokio::test]
async fn agent_md读取失败一律none_路径为目录与root不存在同口径() {
    let dir = tempdir("read-fail");

    // AGENT.md 实为目录：读取失败（非 NotFound IO 错误同列）→ None 不炸
    std::fs::create_dir(dir.path().join("AGENT.md")).expect("以目录占位 AGENT.md 失败");
    assert!(
        load(dir.path()).await.is_none(),
        "AGENT.md 为目录（读取失败）必须返回 None（尽力增强不炸 run）"
    );

    // root 本身不存在：join 出的路径读取失败 → None
    let missing_root = dir.path().join("no-such-root");
    assert!(
        load(&missing_root).await.is_none(),
        "root 不存在必须返回 None"
    );
}

// ---------------------------------------------------------------------------
// 边界：32KB 截断（恰阈值 / 超 1 起 char 边界）
// ---------------------------------------------------------------------------

#[tokio::test]
async fn agent_md恰32kb不截断全文返回无留痕() {
    let dir = tempdir("exact-32k");
    // 恰 32 * 1024 字节（ASCII 单字节，len == 字符数）
    let content = "a".repeat(32 * 1024);
    write_agent_md(dir.path(), &content);

    let preamble = load(dir.path()).await.expect("在场必返回 Some");

    assert_eq!(
        preamble.len(),
        32 * 1024,
        "恰阈值全文返回（不截断）"
    );
    assert_eq!(preamble, content, "内容逐字一致");
    assert!(
        !preamble.contains("已截断"),
        "无截断留痕（体量上限内不注入痕迹）"
    );
}

#[tokio::test]
async fn agent_md超32kb截前缀且多字节字符不劈半() {
    let dir = tempdir("over-32k");

    // 中文重载：'a' * 32767 + '中'(3 字节) → 总 32770 字节，32KB 边界（32768）
    // 落在 '中' 的中段 → 截断回退至 char 边界 32767
    let cjk_content = format!("{}中中中", "a".repeat(32 * 1024 - 1));
    write_agent_md(dir.path(), &cjk_content);
    let cjk_preamble = load(dir.path()).await.expect("在场必返回 Some");
    assert_eq!(
        cjk_preamble,
        "a".repeat(32 * 1024 - 1),
        "超限截前 32KB 内的最长 char 边界前缀（多字节字符整体让位，不劈半）"
    );

    // emoji 重载：'a' * 32766 + 🎉(4 字节) × 3 → 32768 落在首个 🎉 中段
    let emoji_content = format!("{}🎉🎉🎉", "a".repeat(32 * 1024 - 2));
    write_agent_md(dir.path(), &emoji_content);
    let emoji_preamble = load(dir.path()).await.expect("在场必返回 Some");
    assert_eq!(
        emoji_preamble,
        "a".repeat(32 * 1024 - 2),
        "emoji 边界同口径：回退至完整字符起点"
    );

    // 前缀逐字语义：截断产物是原文前缀（非包装改写）
    assert!(
        emoji_content.starts_with(emoji_preamble.as_str()),
        "截断产物恒为原文前缀"
    );
}

// ---------------------------------------------------------------------------
// 边界：每轮重读（无缓存语义）
// ---------------------------------------------------------------------------

#[tokio::test]
async fn agent_md每轮重读_两次调用之间改写即生效无缓存() {
    let dir = tempdir("reread");
    write_agent_md(dir.path(), "第一轮版本");

    let first = load(dir.path()).await.expect("首读 Some");
    assert_eq!(first.as_str(), "第一轮版本");

    // 两次连续 load 之间真实改写文件（bash 入工具面后 agent 可自改系统提示词）
    write_agent_md(dir.path(), "第二轮改后版本 🚀");

    let second = load(dir.path()).await.expect("次读 Some");
    assert_eq!(
        second.as_str(),
        "第二轮改后版本 🚀",
        "无缓存语义：改动下一轮即生效"
    );
    assert_ne!(first, second, "两次读返回值随文件而变");
}
