//! `domain.rs` 的 `derive_verdict`（verdict 推导单点）单元测试：三态归约、
//! 单元素 / 大列表边界、「只依赖 pass 位」的推导面，以及单点性源码自证。
//!
//! 布尔归约纯函数，输入为内存 `ChecklistItem` 构造；零进程边界依赖（零文件 /
//! 网络 / DB / 子进程 / 环境变量），故不设 Mock。

use crate::model::domain::{derive_verdict, ChecklistItem, Verdict};

/// 单项 fixture（`item` / `evidence` 为旁证面，只有 `pass` 参与推导）。
fn item(pass: bool) -> ChecklistItem {
    ChecklistItem {
        item: "检查项".to_owned(),
        pass,
        evidence: "依据".to_owned(),
    }
}

// ---------------------------------------------------------------------------
// 正向：三态
// ---------------------------------------------------------------------------

#[test]
fn 三态归约_全pass过含fail败空表过() {
    // 全 pass（2 项）→ Pass
    assert_eq!(
        derive_verdict(&[item(true), item(true)]),
        Verdict::Pass,
        "全 pass → Pass"
    );
    // 含 1 项 fail → Fail
    assert_eq!(
        derive_verdict(&[item(true), item(false)]),
        Verdict::Fail,
        "含任一 fail → Fail"
    );
    // 空表 → Pass（与既有 every 语义一致：空表即通过）
    assert_eq!(derive_verdict(&[]), Verdict::Pass, "空表 → Pass");
}

// ---------------------------------------------------------------------------
// 边界：单元素 / 末项 / 大列表
// ---------------------------------------------------------------------------

#[test]
fn 单元素边界_两臂各一例() {
    assert_eq!(
        derive_verdict(&[item(true)]),
        Verdict::Pass,
        "单元素 pass → Pass"
    );
    assert_eq!(
        derive_verdict(&[item(false)]),
        Verdict::Fail,
        "单元素 fail → Fail"
    );
}

#[test]
fn 末项决定结论_大列表不因多数通过而放行() {
    let mut mostly_pass: Vec<ChecklistItem> = (0..99).map(|_| item(true)).collect();
    mostly_pass.push(item(false));
    assert_eq!(
        derive_verdict(&mostly_pass),
        Verdict::Fail,
        "99 pass + 末项 fail → Fail（多数通过不构成通过）"
    );

    let thousand: Vec<ChecklistItem> = (0..1000).map(|_| item(true)).collect();
    assert_eq!(derive_verdict(&thousand), Verdict::Pass, "1000 项不 panic");
}

// ---------------------------------------------------------------------------
// 边界：只依赖 pass 位
// ---------------------------------------------------------------------------

#[test]
fn 只依赖pass位_文本面零影响() {
    let shapes = [
        ChecklistItem {
            item: String::new(),
            pass: false,
            evidence: String::new(),
        },
        ChecklistItem {
            item: "🚀含 emoji".to_owned(),
            pass: false,
            evidence: String::new(),
        },
        ChecklistItem {
            item: "含\n换行".to_owned(),
            pass: false,
            evidence: String::new(),
        },
        ChecklistItem {
            item: "长".repeat(5000),
            pass: false,
            evidence: String::new(),
        },
    ];
    for (index, shape) in shapes.into_iter().enumerate() {
        assert_eq!(
            derive_verdict(&[shape]),
            Verdict::Fail,
            "形态 {index}：item / evidence 内容不参与推导（只看 pass 位）"
        );
    }

    // 对照臂：同文本形态但 pass = true → Pass（判据力自证）
    let passing = ChecklistItem {
        item: "长".repeat(5000),
        pass: true,
        evidence: String::new(),
    };
    assert_eq!(derive_verdict(&[passing]), Verdict::Pass);
}

// ---------------------------------------------------------------------------
// 异常：单点性（源码自证）
// ---------------------------------------------------------------------------

#[test]
fn 单点性_派生实现恰一份且落账侧零第二份实现体() {
    fn collect(relative: &str) -> Vec<std::path::PathBuf> {
        let mut found = Vec::new();
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(relative);
        let mut stack = vec![root];
        while let Some(dir) = stack.pop() {
            let Ok(entries) = std::fs::read_dir(&dir) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                } else if path.extension().is_some_and(|ext| ext == "rs") {
                    found.push(path);
                }
            }
        }
        found
    }

    let mut definition_sites: Vec<String> = Vec::new();
    for path in collect("src") {
        // 测试文件本体不计入产品定义面（其内为断言用字面量，非实现）
        let display = path.to_string_lossy().replace('\\', "/");
        if display.ends_with("_test.rs") {
            continue;
        }
        let text = std::fs::read_to_string(&path).expect("读取产品源文件");
        if text.contains("fn derive_verdict") {
            definition_sites.push(display);
        }
    }
    assert_eq!(
        definition_sites.len(),
        1,
        "`fn derive_verdict` 恰一处定义，实际: {definition_sites:?}"
    );
    assert!(
        definition_sites[0].ends_with("model/domain.rs"),
        "唯一定义位于 model/domain.rs，实际: {}",
        definition_sites[0]
    );

    // 落账侧只出现 use 行与调用处：零第二份实现体（无 `fn derive_verdict` 定义）
    let phase_log = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/write/phase_log.rs"),
    )
    .expect("读取 phase_log.rs");
    assert!(
        !phase_log.contains("fn derive_verdict"),
        "phase_log.rs 内不得残留私有实现"
    );
    assert!(
        phase_log.contains("use crate::model::derive_verdict")
            || phase_log.contains("derive_verdict, "),
        "phase_log.rs 经 model 导入推导单点，实际导入面缺失"
    );
    assert_eq!(
        phase_log
            .matches("derive_verdict(&input.checklist)")
            .count(),
        1,
        "phase_log.rs 恰一处调用"
    );
}
