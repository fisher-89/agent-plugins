//! `preamble` 的单元测试（AC-2）：AGENT.md 读取语义——在场时正文逐字 + 外层
//! `<agent_md path="…">` 包装、缺席或读取失败一律 `None`、32KB 截断（char 边界
//! 对齐，只作用于正文）、读取侧无缓存（何时读取归组合根的首轮 / 补拍）。文件为
//! 进程边界依赖白名单：tempfile tempdir 真实文件系统承载 fixture（写 / 改 / 删均
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

/// 注入期望形态：正文逐字 + 外层 `<agent_md path="…">` 包装（路径 = AGENT.md
/// 绝对路径；与 `load` 的同形口径）。
fn wrapped(root: &Path, body: &str) -> String {
    let path = root.join("AGENT.md");
    let path = path.to_str().expect("临时路径为 UTF-8");
    format!("<agent_md path=\"{path}\">\n{body}\n</agent_md>")
}

// ---------------------------------------------------------------------------
// 正向：在场时正文逐字（外层带 `<agent_md>` 包装）
// ---------------------------------------------------------------------------

#[test]
fn agent_md在场正文逐字且外层带agent_md包装() {
    let dir = tempdir("verbatim");
    let content = "# 工作区约定 🎉\n\n- 提交信息用中文\n- 表格对齐\n第二段落\n";
    write_agent_md(dir.path(), content);

    let preamble = load(dir.path()).expect("在场必返回 Some");

    assert_eq!(
        preamble,
        wrapped(dir.path(), content),
        "正文逐字节一致（含中文 / emoji / 换行），外层为 `<agent_md path=…>` 包装"
    );
}

// ---------------------------------------------------------------------------
// 异常：缺席 / 读取失败一律 None（严格无兜底）
// ---------------------------------------------------------------------------

#[test]
fn agent_md缺席返回none且同目录claudemd不作兜底() {
    let dir = tempdir("absent");

    // root 无 AGENT.md：缺席即 None
    assert!(load(dir.path()).is_none(), "缺席 AGENT.md 必须返回 None");

    // 严格无兜底：同目录放 CLAUDE.md 仍 None（读侧不回退其它文件名）
    std::fs::write(dir.path().join("CLAUDE.md"), "# claude 侧提示词").expect("写 CLAUDE.md 失败");
    assert!(
        load(dir.path()).is_none(),
        "同目录 CLAUDE.md 不得兜底（严格单文件语义）"
    );
}

#[test]
fn agent_md读取失败一律none_路径为目录与root不存在同口径() {
    let dir = tempdir("read-fail");

    // AGENT.md 实为目录：读取失败（非 NotFound IO 错误同列）→ None 不炸
    std::fs::create_dir(dir.path().join("AGENT.md")).expect("以目录占位 AGENT.md 失败");
    assert!(
        load(dir.path()).is_none(),
        "AGENT.md 为目录（读取失败）必须返回 None（尽力增强不炸 run）"
    );

    // root 本身不存在：join 出的路径读取失败 → None
    let missing_root = dir.path().join("no-such-root");
    assert!(load(&missing_root).is_none(), "root 不存在必须返回 None");
}

// ---------------------------------------------------------------------------
// 边界：32KB 截断（恰阈值 / 超 1 起 char 边界）
// ---------------------------------------------------------------------------

#[test]
fn agent_md恰32kb不截断全文返回无留痕() {
    let dir = tempdir("exact-32k");
    // 恰 32 * 1024 字节（ASCII 单字节，len == 字符数）
    let content = "a".repeat(32 * 1024);
    write_agent_md(dir.path(), &content);

    let preamble = load(dir.path()).expect("在场必返回 Some");

    assert_eq!(
        preamble,
        wrapped(dir.path(), &content),
        "恰阈值全文返回（正文全文 + 包装，不截断）"
    );
    assert!(
        !preamble.contains("已截断"),
        "无截断留痕（体量上限内不注入痕迹）"
    );
}

#[test]
fn agent_md超32kb截前缀且多字节字符不劈半() {
    let dir = tempdir("over-32k");

    // 中文重载：'a' * 32767 + '中'(3 字节) → 总 32770 字节，32KB 边界（32768）
    // 落在 '中' 的中段 → 截断回退至 char 边界 32767
    let cjk_content = format!("{}中中中", "a".repeat(32 * 1024 - 1));
    write_agent_md(dir.path(), &cjk_content);
    let cjk_preamble = load(dir.path()).expect("在场必返回 Some");
    assert_eq!(
        cjk_preamble,
        wrapped(dir.path(), &"a".repeat(32 * 1024 - 1)),
        "超限截前 32KB 内的最长 char 边界前缀（多字节字符整体让位，不劈半）"
    );

    // emoji 重载：'a' * 32766 + 🎉(4 字节) × 3 → 32768 落在首个 🎉 中段
    let emoji_content = format!("{}🎉🎉🎉", "a".repeat(32 * 1024 - 2));
    write_agent_md(dir.path(), &emoji_content);
    let emoji_preamble = load(dir.path()).expect("在场必返回 Some");
    assert_eq!(
        emoji_preamble,
        wrapped(dir.path(), &"a".repeat(32 * 1024 - 2)),
        "emoji 边界同口径：回退至完整字符起点"
    );

    // 前缀逐字语义：包装内正文恒为原文前缀（截断不包装改写）
    let body = emoji_preamble
        .strip_prefix("<agent_md path=\"")
        .and_then(|rest| rest.split_once("\">\n"))
        .and_then(|(_, rest)| rest.strip_suffix("\n</agent_md>"))
        .expect("包装形态可解析");
    assert!(emoji_content.starts_with(body), "截断产物恒为原文前缀");
}

// ---------------------------------------------------------------------------
// 边界：读取侧无缓存（两次调用之间改写即反映）
// ---------------------------------------------------------------------------

#[test]
fn agent_md两次load之间改写即反映_读取侧无缓存() {
    let dir = tempdir("reread");
    write_agent_md(dir.path(), "第一轮版本");

    let first = load(dir.path()).expect("首读 Some");
    assert_eq!(first, wrapped(dir.path(), "第一轮版本"));

    // 两次连续 load 之间真实改写文件：读取侧自身不缓存（会话内冻结归组合根）
    write_agent_md(dir.path(), "第二轮改后版本 🚀");

    let second = load(dir.path()).expect("次读 Some");
    assert_eq!(
        second,
        wrapped(dir.path(), "第二轮改后版本 🚀"),
        "读取侧无缓存：改动后下一次 load 即反映"
    );
    assert_ne!(first, second, "两次读返回值随文件而变");
}

// ---------------------------------------------------------------------------
// 严格单文件与读取失败（session-preamble-snapshot 补充面）
// ---------------------------------------------------------------------------

#[test]
fn agent_md三文件并存时只读agent_md_另两名不进入返回值() {
    let dir = tempdir("single-file");
    write_agent_md(dir.path(), "AGENT 侧规则");
    std::fs::write(dir.path().join("CLAUDE.md"), "CLAUDE 侧规则").expect("写 CLAUDE.md");
    std::fs::write(dir.path().join("AGENTS.md"), "AGENTS 侧规则").expect("写 AGENTS.md");

    let preamble = load(dir.path()).expect("AGENT.md 在场必 Some");
    assert_eq!(preamble, wrapped(dir.path(), "AGENT 侧规则"));
    assert!(!preamble.contains("CLAUDE 侧规则"), "CLAUDE.md 不进入快照");
    assert!(
        !preamble.contains("AGENTS 侧规则"),
        "AGENTS.md 不进入快照（复数形态不支持）"
    );
}

#[test]
fn agent_md仅agents_md在场仍none_零兜底回退() {
    let dir = tempdir("agents-only");
    std::fs::write(dir.path().join("AGENTS.md"), "复数形态规则").expect("写 AGENTS.md");
    assert!(
        load(dir.path()).is_none(),
        "仅 AGENTS.md 在场 → None（严格单文件，零兜底回退）"
    );
}

#[test]
fn agent_md非utf8字节返回none不报错() {
    let dir = tempdir("non-utf8");
    std::fs::write(dir.path().join("AGENT.md"), [0xff_u8, 0xfe, 0x00, 0x41])
        .expect("写非 UTF-8 字节");
    assert!(
        load(dir.path()).is_none(),
        "非 UTF-8 内容读取失败 → None（读取侧永不报错）"
    );
}

// ---------------------------------------------------------------------------
// freeze：快照正文合成单点（段序 / 空段语义 / 零二次截断）
// ---------------------------------------------------------------------------

use crate::sdk::preamble::freeze;

const KNOWN_AGENT_MD: &str = "<agent_md path=\"/ws/AGENT.md\">\n约定\n</agent_md>";

#[test]
fn freeze双段在场时按知识_agent_md序以单空行连接() {
    let body = freeze(Some("知识正文"), Some(KNOWN_AGENT_MD));
    assert_eq!(
        body,
        format!("知识正文\n\n{KNOWN_AGENT_MD}"),
        "段序 = 知识 → AGENT.md 段，段间恰一个空行"
    );
    assert_eq!(body.matches("\n\n").count(), 1, "段间分隔恰一处");
    assert!(
        body.find("知识正文").unwrap() < body.find(KNOWN_AGENT_MD).unwrap(),
        "知识段在首位"
    );
}

#[test]
fn freeze单侧在场两形态() {
    let only_agent_md = freeze(None, Some(KNOWN_AGENT_MD));
    assert_eq!(only_agent_md, KNOWN_AGENT_MD, "仅 AGENT.md 段：无空前导");
    assert!(!only_agent_md.starts_with('\n'), "首位无空行");

    let only_knowledge = freeze(Some("知识正文"), None);
    assert_eq!(only_knowledge, "知识正文", "仅知识段");
}

#[test]
fn freeze全空四形态均返回空串() {
    for (label, knowledge, agent_md) in [
        ("双 None", None, None),
        ("知识空串", Some(""), None),
        ("AGENT.md 空串", None, Some("")),
        ("双空串", Some(""), Some("")),
    ] {
        let body = freeze(knowledge, agent_md);
        assert_eq!(body, String::new(), "形态「{label}」→ 空串");
    }
}

#[test]
fn freeze不trim_段内首尾空白逐字保留() {
    // 期望串按 freeze 的合成口径逐字构造：两入参**原样**代入（含各自首尾 `\n`），
    // 段间只多出连接用的 `\n\n` ⇒ 段界处连续 4 个换行（上段尾 1 + 连接 2 + 下段首 1）
    let knowledge = "\n  A  \n";
    let agent_md = "\n<agent_md/>\n";
    let body = freeze(Some(knowledge), Some(agent_md));

    assert_eq!(
        body,
        format!("{knowledge}\n\n{agent_md}"),
        "段内空白逐字保留，仅在段间插入单空行（AGENT.md 段逐字语义）"
    );
    assert!(
        body.contains("\n  A  \n"),
        "知识段首尾空白未被剥离（不 trim）"
    );
    assert_eq!(
        body.matches("\n\n\n\n").count(),
        1,
        "段界连续 4 个换行（上段尾 1 + 连接 2 + 下段首 1），空白未被压缩"
    );
}

#[test]
fn freeze不二次截断_超长agent_md段长度逐字节守恒() {
    let long = "长".repeat(20000);
    let knowledge = "知识";
    let body = freeze(Some(knowledge), Some(&long));
    let expected = knowledge.len() + 2 + long.len();
    assert_eq!(body.len(), expected, "拼接长度逐字节守恒（无二次截断）");
    assert!(
        body.contains(&long),
        "40KB 级入参原样在产物内（32KB 只在 load 内发生）"
    );
}

#[test]
fn freeze知识段不截断_超32kb知识原样在首位() {
    let knowledge = "知".repeat(20000);
    let body = freeze(Some(&knowledge), None);
    assert!(
        body.starts_with(&knowledge),
        "知识段 > 32KB 原样在首位（截断只作用于 AGENT.md 段）"
    );
    assert_eq!(body.len(), knowledge.len(), "长度守恒");
}

#[test]
fn freeze纯函数性_同入参两次相等且任一入参变化即结果变化() {
    let first = freeze(Some("知识"), Some(KNOWN_AGENT_MD));
    let second = freeze(Some("知识"), Some(KNOWN_AGENT_MD));
    assert_eq!(first, second, "同入参两次调用逐字相等（无内部缓存）");

    assert_ne!(
        freeze(Some("知识二"), Some(KNOWN_AGENT_MD)),
        first,
        "知识变即结果变"
    );
    assert_ne!(
        freeze(Some("知识"), Some("<agent_md/>")),
        first,
        "AGENT.md 段变即结果变"
    );
    assert_ne!(freeze(Some("知识"), None), first, "段缺席即结果变");
}

// ---------------------------------------------------------------------------
// 源码自证：async 退役与 32KB 截断单点
// ---------------------------------------------------------------------------

#[test]
fn preamble源文件自证_async退役与截断单点恰一处() {
    let own = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/sdk/preamble.rs"),
    )
    .expect("读取 preamble.rs");

    // 判据一：async 退役（同步读取，调用方是同步的组合根）
    assert!(
        !own.contains("async fn load"),
        "load 的 async 已退役（同步读取，调用方是同步的组合根）"
    );
    assert!(
        !own.contains("tokio::fs"),
        "零 async 文件读取（std::fs 单点）"
    );

    // 判据二：32KB 常量单一定义（不散落副本）
    assert_eq!(
        own.matches("const MAX_PREAMBLE_BYTES").count(),
        1,
        "截断常量单一定义"
    );

    // 函数体切片口径：自 `fn <name>(` 起至首个行首收口 `\n}\n`（不含 `///` doc 段）
    fn body_slice<'a>(source: &'a str, name: &str) -> &'a str {
        let marker = format!("fn {name}(");
        let start = source
            .find(&marker)
            .unwrap_or_else(|| panic!("preamble.rs 内缺 {marker}"));
        let rest = &source[start..];
        let end = rest.find("\n}\n").expect("函数体行首收口");
        &rest[..end]
    }

    // 判据三：常量的消费命中全部落在 `fn truncate` 函数体切片内（定义行以外零散落）
    let truncate_start = own.find("fn truncate(").expect("truncate 定义在位");
    let truncate_body = body_slice(&own, "truncate");
    let truncate_end = truncate_start + truncate_body.len();
    let definition_at =
        own.find("const MAX_PREAMBLE_BYTES").expect("常量定义在位") + "const ".len();
    assert!(
        truncate_body.matches("MAX_PREAMBLE_BYTES").count() >= 1,
        "truncate 是常量的唯一消费函数"
    );
    for (offset, _) in own.match_indices("MAX_PREAMBLE_BYTES") {
        assert!(
            offset == definition_at || (truncate_start..truncate_end).contains(&offset),
            "常量命中只允许「定义行」与「fn truncate 切片内」两处，实见偏移 {offset}"
        );
    }

    // 判据四：freeze 零二次截断；load 内截断调用唯一；truncate 定义唯一
    let freeze_body = body_slice(&own, "freeze");
    assert_eq!(
        freeze_body.matches("MAX_PREAMBLE_BYTES").count(),
        0,
        "freeze 不碰截断常量（32KB 只在 load 内发生）"
    );
    assert_eq!(
        freeze_body.matches("truncate(").count(),
        0,
        "freeze 零二次截断（不复制截断逻辑）"
    );
    assert_eq!(
        body_slice(&own, "load").matches("truncate(").count(),
        1,
        "load 内截断调用恰一处"
    );
    assert_eq!(
        own.matches("fn truncate(").count(),
        1,
        "truncate 单点定义（freeze 不复制截断逻辑）"
    );
}
