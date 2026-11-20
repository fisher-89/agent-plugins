//! `config_knowledge` 与静态骨架 `config-knowledge.md` 的单元测试：词表两面派生
//!（`config::known_keys` 八键 / `config::TestFramework::ALL` 八值）的在场与连接
//! 形态、使用指引三句、骨架自证（零占位符 / 体量上限 / 零盘面真值 / 写权只留事实
//! 陈述）与降级语义。
//!
//! 零 mock（沿 test-design「Mock策略」表）：词表取真实函数，骨架取
//! `include_str!` 编译期内嵌的同一份字节（禁测试内复制副本——替身会抹掉「词表漂移 /
//! 骨架写坏」的判据）。零 IO：不落盘、不建 tempdir。

use config::{known_keys, TestFramework};

use crate::config_knowledge::{build, knowledge};

/// 静态骨架字节（与生产 `include_str!` 同一路径同一份文件）。
const SKELETON: &str = include_str!("config-knowledge.md");

/// 唯一入口产物（每条用例自取，零共享可变状态）。
fn text() -> String {
    knowledge().expect("真实骨架应拼装成功（词表两面派生全在场）")
}

/// 词表连接形态单点的连接串（已知键 / 框架枚举同用 `" / "`）。
fn keys_joined() -> String {
    known_keys().join(" / ")
}

fn frameworks_joined() -> String {
    TestFramework::ALL
        .iter()
        .map(|framework| framework.as_str())
        .collect::<Vec<&str>>()
        .join(" / ")
}

/// 派生段（骨架之后的部分）：拼装以 `skeleton.trim_end()` 起手，故尾部即派生段。
fn derived() -> String {
    let full = text();
    let head = SKELETON.trim_end();
    assert!(
        full.starts_with(head),
        "输出必须以 SKELETON.trim_end() 起手（骨架零改写、零截断）"
    );
    full[head.len()..].to_owned()
}

// ---------------------------------------------------------------------------
// `knowledge`：骨架 + 派生段
// ---------------------------------------------------------------------------

/// 正向：返回 `Some`，骨架字节逐字起手，派生段接其后（两段以单空行分隔）。
#[test]
fn 知识输出以骨架字节起手且派生段随后() {
    let full = text();
    assert!(
        full.starts_with(SKELETON.trim_end()),
        "骨架逐字保留（零改写 / 零截断）"
    );
    let tail = derived();
    assert!(
        tail.starts_with("\n\n"),
        "两段之间以单空行分隔，实际尾部起手 {tail:.8?}"
    );
    assert!(
        tail.contains("工作区配置顶层已知键："),
        "派生段含已知键引导语"
    );
    assert!(
        tail.contains("tests[].framework 枚举："),
        "派生段含框架枚举引导语"
    );
}

/// 正向（矩阵 8 键）：八键逐键在场，语料取 `known_keys()` 实值（不手抄字面量）。
#[test]
fn 派生段八键逐键在场() {
    let tail = derived();
    for key in known_keys() {
        assert!(
            tail.contains(key),
            "派生段缺顶层已知键 {key}（词表由 known_keys 派生，新增键未同步即红）"
        );
    }
    assert_eq!(known_keys().len(), 8, "顶层已知键封闭八项");
}

/// 正向（矩阵 8 值）：八值逐值在场（`as_str()` 实值）。
#[test]
fn 派生段八值逐值在场() {
    let tail = derived();
    for framework in TestFramework::ALL {
        assert!(
            tail.contains(framework.as_str()),
            "派生段缺框架枚举值 {}",
            framework.as_str()
        );
    }
    assert_eq!(TestFramework::ALL.len(), 8, "框架枚举封闭八值");
}

/// 正向：词表连接形态为单点分隔「 / 」——整段连接串逐字在场（与 archi 知识同形）。
#[test]
fn 派生段词表以单点分隔连接() {
    let tail = derived();
    assert!(
        tail.contains(&keys_joined()),
        "已知键应按 known_keys 序以 \" / \" 连接，期望 {}",
        keys_joined()
    );
    assert!(
        tail.contains(&frameworks_joined()),
        "框架枚举应按 ALL 序以 \" / \" 连接，期望 {}",
        frameworks_joined()
    );
}

/// 正向：使用指引三句在场（两模式 / 候选须为完整文件面 / `is_error` 后重交完整候选）。
#[test]
fn 使用指引三句逐句在场() {
    let full = text();
    for anchor in [
        "只读模式",
        "校验模式",
        "整份文件面",
        "重交完整候选",
        "snake_case",
    ] {
        assert!(full.contains(anchor), "知识文本缺使用指引关键词 {anchor}");
    }
}

// ---------------------------------------------------------------------------
// `knowledge`：边界（体量 / 占位符 / 零盘面真值 / 零写保护复述）
// ---------------------------------------------------------------------------

/// 边界：体量上限 8 000 字节（与 `AGENT.md` 合体后不挤压上下文）。
#[test]
fn 知识体量不超八千字节() {
    let full = text();
    assert!(
        full.len() < 8_000,
        "知识与 AGENT.md 合体后不挤压上下文，实测 {} 字节",
        full.len()
    );
}

/// 边界：全文本零 `<>` 占位符（骨架自证）。
#[test]
fn 知识零尖括号占位符() {
    assert_eq!(text().matches("<>").count(), 0, "派生后仍须零占位符");
}

/// 边界：零盘面真值——不含本仓项目根、不含阈值实际值形态、不含写保护 glob 字面量。
#[test]
fn 知识零盘面真值() {
    let full = text();
    for leak in [
        "packages/desktop",
        "lines: 80",
        "tests/golden",
        "Cargo.lock",
        "vite-plus.config.ts",
    ] {
        assert!(
            !full.contains(leak),
            "知识不得携带盘面真值 {leak}（取数时机归 propose_config 只读模式）"
        );
    }
}

/// 边界：零写保护规则复述——只留「config.json 是 agent 不可写的文件」这一事实句，
/// 不复述内置锁定规则的键名表（不构成第二真相源）。
#[test]
fn 知识零写保护规则复述() {
    let full = text();
    assert!(
        full.contains("永远不可写") || full.contains("不可写"),
        "写权事实句应在场"
    );
    // 路径规则的键面引用（`write_protection.files[].glob` 出现在路径段）不算复述，
    // 判据只看「是否抄了锁定规则的键名表 / glob 字面量 / 规则实现概念」。
    for rule_key in ["ProtectRules", "内置锁定规则", "tests/golden", "Cargo.lock"] {
        assert!(
            !full.contains(rule_key),
            "写保护规则复述 {rule_key} 不得进知识（判据归 desktop-write-protection）"
        );
    }
}

// ---------------------------------------------------------------------------
// `build`：降级面与形态边界
// ---------------------------------------------------------------------------

/// 异常（矩阵 2 形态）：空 / 纯空白骨架 → `Err`，文案含「骨架为空」语义。
#[test]
fn build骨架为空或纯空白均err() {
    for skeleton in ["", "   \n\t "] {
        let error = build(skeleton).expect_err("空骨架必须 Err");
        assert!(
            error.contains("骨架为空"),
            "文案应含「骨架为空」语义，实际 {error}"
        );
    }
}

/// 异常：含 `<>` 占位符 → `Err`，文案含占位符成因（占位符即「词表未派生」形态）。
#[test]
fn build占位符骨架err且文案含成因() {
    let error = build("路径：<>").expect_err("占位符必须 Err");
    assert!(error.contains("<>"), "文案应指认占位符成因，实际 {error}");
}

/// 边界：超长骨架（100 000 字符）→ `Ok` 且派生段仍追加在尾部（无截断实现）。
#[test]
fn build超长骨架ok且派生段仍在尾部() {
    let skeleton = "长".repeat(100_000);
    let full = build(&skeleton).expect("超长骨架应成功");
    assert!(full.starts_with(&skeleton), "骨架原样保留");
    assert_eq!(
        full.len(),
        skeleton.len() + derived().len(),
        "派生段整段追加"
    );
    assert!(
        full.ends_with(&derived()),
        "派生段仍在尾部（与真实骨架同源同形）"
    );
}

/// 边界（矩阵 4 形态）：特殊字符骨架原样保留在输出中（零转义加工）。
#[test]
fn build特殊字符骨架原样保留() {
    for skeleton in [
        "🙂 表情",
        "多行\n文本",
        "含 \0 空字节",
        "引号 \" 与 ' 单引号",
    ] {
        let full = build(skeleton).expect("特殊字符骨架应成功");
        assert!(
            full.starts_with(skeleton.trim_end()),
            "骨架 {skeleton:?} 零转义加工"
        );
    }
}

/// 正向：骨架尾随空白归一——`trim_end()` 后起手，不重复保留尾换行。
#[test]
fn build尾随空白归一三形态() {
    for skeleton in ["骨架", "骨架\n", "骨架\n\n  \t"] {
        let full = build(skeleton).expect("尾随空白形态应成功");
        assert!(
            full.starts_with("骨架\n\n工作区配置顶层已知键："),
            "尾随空白形态 {skeleton:?} 应归一为单空行分隔，实际 {full:.24?}"
        );
    }
}

/// 异常防护：坏骨架的降级传导——`build` 的 `Err` 面与真实 `knowledge()` 的 `Some`
/// 构成双向判据（装配层只取 `Option`，不消费 `Err`）。
#[test]
fn 降级语义ok为none即丢preamble位() {
    assert!(build("<>").is_err(), "坏骨架 build 必须 Err");
    assert!(
        knowledge().is_some(),
        "生产骨架恒喂仓内字节 → 恒 Some（降级只在骨架被写坏时发生）"
    );
    assert_eq!(
        knowledge().as_deref(),
        Some(text().as_str()),
        "两次调用逐字相等（纯函数、零状态）"
    );
}

// ---------------------------------------------------------------------------
// 骨架资产自证（`config-knowledge.md`）
// ---------------------------------------------------------------------------

/// 正向（矩阵 8 键）：八键名逐键以「字段名 + 语义」形态出现在骨架内（键名以
/// `known_keys()` 遍历断言，骨架侧零手抄词表）。
#[test]
fn 骨架八键逐键以字段条目形态在场() {
    for key in known_keys() {
        assert!(
            SKELETON.contains(&format!("- `{key}`：")),
            "骨架缺字段条目 `- {key}：`（顶层字段段须逐键给语义）"
        );
    }
}

/// 正向：`propose_config` 两模式与「完整候选」指引在骨架内（派生段只放词表，
/// 两处不重复持有）。
#[test]
fn 骨架使用指引与完整候选口径在案() {
    for anchor in [
        "propose_config({})",
        "propose_config({ candidate })",
        "整份文件面",
        "重交完整候选",
        "不接受片段",
    ] {
        assert!(SKELETON.contains(anchor), "骨架缺使用指引锚 {anchor}");
    }
}

/// 正向：`defaultApplied` 口径在案（正常形态 / 清零噪音 = 给有默认值的字段写真值，
/// 不是给 Option 写 `null`）。
#[test]
fn 骨架default_applied口径在案() {
    for anchor in ["defaultApplied", "正常形态", "清零噪音", "MUST NOT 给"] {
        assert!(SKELETON.contains(anchor), "骨架缺默认值口径锚 {anchor}");
    }
}

/// 正向：软规则与零消费事实在案（`language` 开放字符串 / `depends_on` 平级边 /
/// `context` 与 `rules` 当前零工程消费）。
#[test]
fn 骨架软规则与零消费事实在案() {
    for anchor in [
        "开放字符串",
        "平级边",
        "安装顺序",
        "零工程消费",
        "depends_on",
        "language",
    ] {
        assert!(SKELETON.contains(anchor), "骨架缺软规则锚 {anchor}");
    }
}

/// 边界：写权只作事实陈述（零内置锁定规则的键名 / glob 复述）。
#[test]
fn 骨架写权仅事实陈述() {
    assert!(
        SKELETON.contains("不可写") || SKELETON.contains("永远不可写"),
        "写权事实句在场"
    );
    assert!(
        !SKELETON.contains("tests/golden") && !SKELETON.contains("Cargo.lock"),
        "骨架零写保护 glob 字面量（不构成第二真相源）"
    );
}

/// 边界：骨架自证——零 `<>` 占位符且整体体量 < 8 000 字节（与 `knowledge()` 上限同源）。
#[test]
fn 骨架自证零占位符与体量上限() {
    assert_eq!(SKELETON.matches("<>").count(), 0, "骨架零占位符");
    assert!(
        SKELETON.len() < 8_000,
        "骨架体量应在上限内，实测 {} 字节",
        SKELETON.len()
    );
}

/// 异常：内存副本注入 `<>`（如 `"路径：<>"`）→ `build(副本)` `Err`（负向判据由
/// 内存副本驱动，不改仓内文件）。
#[test]
fn 骨架副本注入占位符即被抓() {
    let mutated = format!("{SKELETON}\n路径：<>");
    let error = build(&mutated).expect_err("注入占位符的副本必须 Err");
    assert!(error.contains("<>"), "文案指认占位符，实际 {error}");
}
