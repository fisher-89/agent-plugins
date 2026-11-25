//! `edit` 写回原语族的单元测试：4 × `apply_*`（源文本进、改写后的完整源文本出）与
//! 4 × `plan_*`（读盘定位 + 纯函数 + 待落盘产物）。覆盖三态语义（可清空位 / 封闭枚举
//! 位）、表头计数回填、自证链（渲染 → 重解析取计数 → 回填 → 重解析自证 → 回读）、
//! 删除 fail-closed、关系原语的落点（既有关系的声明文件 / 新增关系的 `from` 文件）与
//! 多处声明 fail-closed、`expect` 乐观锁，以及 crate 根出口面与「零写盘 / 零哈希拼装」
//! 两条源码锚。
//!
//! 零 mock：真实 `toml_edit`（排版与 decor 保留即被测能力）+ 真实 `TomlParser`（自证与
//! 回读判据与解析层同源）+ 真实 `ModelGraph`（层级 / 邻接判据单点，不自写前缀扫描）+
//! 真实 `drift::file_hash_of`；临时目录由 std 自建（`TempDir` RAII + `AtomicU64` 唯一
//! 命名，沿 `drift/mod_test.rs` 先例）。私有 helper（`backfill_header_counts` /
//! `finish` / `check_create` / `check_target` / `read_source` / `check_expect` /
//! `relation_hits` / `check_relation_unique`）经 `super::…` 直调（子模块可见父模块私有
//! 项），不为测试新增导出。

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use foundation::layout::archi_graph_model_dir;
use toml_edit::DocumentMut;

use crate::dto::{ApiStatus, ElementKind, GateTier};
use crate::model::{Element, Model};
use crate::parse::{self, SourceParser, TomlParser};
use crate::{
    apply_element_delete, apply_element_spec, apply_relation_delete, apply_relation_upsert,
    file_hash, file_hash_of, plan_element_delete, plan_element_upsert, plan_relation_delete,
    plan_relation_upsert, DeleteSet, EditError, EditErrorKind, ElementSpec, ModelWrite,
};

use super::{
    backfill_header_counts, check_create, check_expect, check_relation_unique, check_target,
    finish, messages, read_source, relation_hits,
};

/// 单元素删除集的退化形（`DeleteSet` 恰含一个 FQN，删除纯函数唯一形态）。
fn one(fqn: &str) -> DeleteSet {
    DeleteSet::of(fqn)
}

// ---------------------------------------------------------------------------
// 真实临时目录（本 crate 零 dev-dependency，故用 std 自建）
// ---------------------------------------------------------------------------

static COUNTER: AtomicU64 = AtomicU64::new(0);

struct TempDir(PathBuf);

impl TempDir {
    fn new(tag: &str) -> Self {
        let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "archi-edit-test-{}-{}-{}",
            std::process::id(),
            unique,
            tag
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("创建临时目录失败");
        TempDir(dir)
    }

    fn root(&self) -> &Path {
        &self.0
    }

    /// 落一个模型文件并返回其绝对路径。
    fn write_model(&self, name: &str, content: &str) -> PathBuf {
        let dir = archi_graph_model_dir(&self.0);
        fs::create_dir_all(&dir).expect("创建模型声明目录失败");
        let path = dir.join(name);
        fs::write(&path, content).expect("写模型文件失败");
        path
    }

    /// 全量字节快照（相对路径 → 字节；用于「零写盘」对拍）。
    fn snapshot(&self) -> Vec<(String, Vec<u8>)> {
        let mut entries: Vec<(String, Vec<u8>)> = Vec::new();
        let mut frontier: Vec<PathBuf> = vec![self.0.clone()];
        while let Some(dir) = frontier.pop() {
            let Ok(read) = fs::read_dir(&dir) else {
                continue;
            };
            for entry in read.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    frontier.push(path);
                    continue;
                }
                let relative = path
                    .strip_prefix(&self.0)
                    .expect("相对路径")
                    .to_string_lossy()
                    .replace('\\', "/");
                entries.push((relative, fs::read(&path).expect("读文件")));
            }
        }
        entries.sort();
        entries
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

// ---------------------------------------------------------------------------
// 回读 / 断言 helper
// ---------------------------------------------------------------------------

/// 解析并断言零诊断（自证判据与解析层同源）。
fn model_of(source: &str) -> Model {
    let outcome = TomlParser::parse(source);
    assert!(
        outcome.diagnostics.is_empty(),
        "源应可解析：{:?}",
        outcome.diagnostics
    );
    outcome.model
}

/// 回读某元素（缺席即测试自身缺陷）。
fn element_of(source: &str, fqn: &str) -> Element {
    model_of(source)
        .element(fqn)
        .cloned()
        .unwrap_or_else(|| panic!("回读应含元素 {fqn}：\n{source}"))
}

/// 回读某关系（缺席即测试自身缺陷）。
fn relation_of<'a>(model: &'a Model, from: &str, to: &str) -> &'a crate::model::Relation {
    model
        .relations
        .iter()
        .find(|relation| relation.from == from && relation.to == to)
        .unwrap_or_else(|| panic!("回读应含关系 {from} → {to}"))
}

/// 某键的整行（`key = …`，容忍键与 `=` 之间的空白；缺席即 `None`）。
fn key_line<'a>(source: &'a str, key: &str) -> Option<&'a str> {
    source.lines().find(|line| {
        line.trim_start()
            .strip_prefix(key)
            .is_some_and(|rest| rest.trim_start().starts_with('='))
    })
}

/// 按行 diff 的变化行下标（行数必须不变）。
fn changed_lines(before: &str, after: &str) -> Vec<usize> {
    let left: Vec<&str> = before.lines().collect();
    let right: Vec<&str> = after.lines().collect();
    assert_eq!(left.len(), right.len(), "行数应不变（就地改值）");
    left.iter()
        .zip(right.iter())
        .enumerate()
        .filter(|(_, (old, new))| old != new)
        .map(|(index, _)| index)
        .collect()
}

/// 表头注释块内第一处 `（N 元素 / M 关系）` 的两个数字。
fn header_counts(source: &str) -> Option<(usize, usize)> {
    for line in source.lines() {
        let Some(open) = line.find('（') else {
            continue;
        };
        let body = &line[open + '（'.len_utf8()..];
        let Some(split) = body.find(" 元素 / ") else {
            continue;
        };
        let elements = body[..split].parse::<usize>().ok()?;
        let tail = &body[split + " 元素 / ".len()..];
        let Some(end) = tail.find(" 关系）") else {
            continue;
        };
        let relations = tail[..end].parse::<usize>().ok()?;
        return Some((elements, relations));
    }
    None
}

/// 判据 + 事实（两入口对拍用）。
fn verdict(failure: &EditError) -> (EditErrorKind, &str) {
    (failure.kind, failure.facts.as_str())
}

/// gate 退化调用（改档 = `ElementSpec { gate }` 的一次调用）。
fn gate_spec(fqn: &str, gate: GateTier) -> ElementSpec {
    ElementSpec {
        fqn: fqn.to_owned(),
        gate: Some(gate),
        ..Default::default()
    }
}

fn spec_entry(fqn: &str, change: Option<Option<&str>>) -> ElementSpec {
    ElementSpec {
        fqn: fqn.to_owned(),
        entry: change.map(|value| value.map(str::to_owned)),
        ..Default::default()
    }
}

fn spec_doc(fqn: &str, change: Option<Option<&str>>) -> ElementSpec {
    ElementSpec {
        fqn: fqn.to_owned(),
        doc: change.map(|value| value.map(str::to_owned)),
        ..Default::default()
    }
}

fn spec_summary(fqn: &str, change: Option<Option<&str>>) -> ElementSpec {
    ElementSpec {
        fqn: fqn.to_owned(),
        summary: change.map(|value| value.map(str::to_owned)),
        ..Default::default()
    }
}

fn read_entry(element: &Element) -> Option<String> {
    element.entry.clone()
}

fn read_doc(element: &Element) -> Option<String> {
    element.doc.clone()
}

fn read_summary(element: &Element) -> Option<String> {
    element.summary.clone()
}

// ---------------------------------------------------------------------------
// 盘面 fixture
// ---------------------------------------------------------------------------

const CORE_SOURCE: &str = concat!(
    "# 核心包（注释须逐字保留）\n",
    "[element.\"App\"]\n",
    "kind = \"package\"\n",
    "\n",
    "# 计费域\n",
    "[element.\"App.Billing\"]\n",
    "kind = \"module\"\n",
    "gate = \"off\"\n",
    "\n",
    "[element.\"App.Auth\"]\n",
    "kind = \"domain\"\n",
    "gate = \"warn\"\n",
);

/// 表头计数准确的源（3 元素 / 1 关系）：改成本值即零字节差异。
const HEADER_SOURCE: &str = concat!(
    "# 01 · 核心（3 元素 / 1 关系）\n",
    "[element.\"App\"]\n",
    "kind = \"package\"\n",
    "\n",
    "[element.\"App.Auth\"]\n",
    "kind = \"domain\"\n",
    "gate = \"off\"\n",
    "\n",
    "[element.\"App.Billing\"]\n",
    "kind = \"module\"\n",
    "gate = \"warn\"\n",
    "\n",
    "[[relation]]\n",
    "from = \"App.Auth\"\n",
    "to = \"App.Billing\"\n",
);

/// 六个可清空位齐备（`entry` / `doc` / `summary`）。
const FIELDS_SOURCE: &str = concat!(
    "[element.\"App.Auth\"]\n",
    "kind = \"domain\"\n",
    "gate = \"warn\"\n",
    "entry = \"src/auth/token.ts\"\n",
    "doc = \"docs/auth.md\"\n",
    "summary = \"认证\"\n",
);

/// 集合位齐备（`path` / `metadata`）。
const COLLECTION_SOURCE: &str = concat!(
    "[element.\"App.Auth\"]\n",
    "kind = \"domain\"\n",
    "path = [\"src/auth\"]\n",
    "metadata = { owner = \"core\" }\n",
);

/// 三元素一关系（关系不碰被删元素，删除面可成功）。
const DELETE_SOURCE: &str = concat!(
    "# 01 · 核心（3 元素 / 1 关系）\n",
    "[element.\"App\"]\n",
    "kind = \"package\"\n",
    "\n",
    "[element.\"App.Auth\"]\n",
    "kind = \"domain\"\n",
    "\n",
    "[element.\"App.Billing\"]\n",
    "kind = \"module\"\n",
    "\n",
    "[[relation]]\n",
    "from = \"App.Billing\"\n",
    "to = \"App\"\n",
);

/// 三条关系（含空行与一条 `desc`）：更新 / 删除只动命中条。
const RELATIONS_SOURCE: &str = concat!(
    "[element.\"A\"]\n",
    "kind = \"module\"\n",
    "\n",
    "[element.\"B\"]\n",
    "kind = \"module\"\n",
    "\n",
    "[element.\"C\"]\n",
    "kind = \"module\"\n",
    "\n",
    "[[relation]]\n",
    "from = \"A\"\n",
    "to = \"B\"\n",
    "desc = \"旧说明\"\n",
    "\n",
    "[[relation]]\n",
    "from = \"B\"\n",
    "to = \"C\"\n",
    "\n",
    "[[relation]]\n",
    "from = \"A\"\n",
    "to = \"C\"\n",
);

/// 一条带 `apis` 的关系（三态判据面）。
const APIS_SOURCE: &str = concat!(
    "[element.\"A\"]\n",
    "kind = \"module\"\n",
    "\n",
    "[element.\"B\"]\n",
    "kind = \"module\"\n",
    "\n",
    "[[relation]]\n",
    "from = \"A\"\n",
    "to = \"B\"\n",
    "apis = [\"open\", \"close\"]\n",
);

/// 两元素两关系 + 准确表头（关系删除计数随动面）。
const RELATION_PLAN_SOURCE: &str = concat!(
    "# 01 · 核心（2 元素 / 2 关系）\n",
    "[element.\"A\"]\n",
    "kind = \"module\"\n",
    "\n",
    "[element.\"B\"]\n",
    "kind = \"module\"\n",
    "\n",
    "[[relation]]\n",
    "from = \"A\"\n",
    "to = \"B\"\n",
    "\n",
    "[[relation]]\n",
    "from = \"B\"\n",
    "to = \"A\"\n",
);

/// 简单两元素一关系模型（plan 面通用起点）。
const TWO_ELEMENTS_SOURCE: &str = concat!(
    "[element.\"A\"]\n",
    "kind = \"module\"\n",
    "\n",
    "[element.\"B\"]\n",
    "kind = \"module\"\n",
    "\n",
    "[[relation]]\n",
    "from = \"A\"\n",
    "to = \"B\"\n",
);

/// parser 面结构诊断源（`package` 至多一条包根 ⇒ `invalidValue`）。
const DIAGNOSTIC_SOURCE: &str = "[element.\"App\"]\nkind = \"package\"\npath = [\"a\", \"b\"]\n";

/// 跨文件关系的 `from` 侧文件（复刻本仓 `Desktop → Desktop.Components.Ui` 的形态：
/// `from` 元素在 `01-app.toml`、该关系声明在 `06-frontend.toml`）。
const SPLIT_FROM_SOURCE: &str = concat!(
    "# 01 · app（1 元素 / 0 关系）\n",
    "[element.\"Desktop\"]\n",
    "kind = \"package\"\n",
);

/// 跨文件关系的声明侧文件（表头计数准确；两条关系都是祖先 → 后代，无结构诊断）。
const SPLIT_RELATION_SOURCE: &str = concat!(
    "# 06 · frontend（2 元素 / 2 关系）\n",
    "[element.\"Desktop.Components\"]\n",
    "kind = \"module\"\n",
    "\n",
    "[element.\"Desktop.Components.Ui\"]\n",
    "kind = \"module\"\n",
    "\n",
    "[[relation]]\n",
    "from = \"Desktop\"\n",
    "to = \"Desktop.Components.Ui\"\n",
    "desc = \"旧说明\"\n",
    "\n",
    "[[relation]]\n",
    "from = \"Desktop\"\n",
    "to = \"Desktop.Components\"\n",
);

/// 同一源内 `(from, to)` 两处声明（文件面 `RelationDuplicated` 的起点）。
const DUPLICATED_IN_SOURCE: &str = concat!(
    "[element.\"A\"]\n",
    "kind = \"module\"\n",
    "\n",
    "[element.\"B\"]\n",
    "kind = \"module\"\n",
    "\n",
    "[[relation]]\n",
    "from = \"A\"\n",
    "to = \"B\"\n",
    "desc = \"第一处\"\n",
    "\n",
    "[[relation]]\n",
    "from = \"A\"\n",
    "to = \"B\"\n",
    "desc = \"第二处\"\n",
);

/// 模型面重复声明的元素侧文件（`01-app.toml`）。
const DUPLICATED_APP_SOURCE: &str = concat!(
    "[element.\"A\"]\n",
    "kind = \"module\"\n",
    "\n",
    "[element.\"B\"]\n",
    "kind = \"module\"\n",
    "\n",
    "[[relation]]\n",
    "from = \"A\"\n",
    "to = \"B\"\n",
);

/// 模型面重复声明的第二处（`06-frontend.toml`：跨文件同一对再声明一次）。
const DUPLICATED_FRONTEND_SOURCE: &str = "[[relation]]\nfrom = \"A\"\nto = \"B\"\n";

/// 关系身份键的元素侧 + 同对第一处（`01-app.toml`）。
const IDENTITY_APP_SOURCE: &str = concat!(
    "[element.\"A\"]\n",
    "kind = \"module\"\n",
    "\n",
    "[element.\"B\"]\n",
    "kind = \"module\"\n",
    "\n",
    "[element.\"C\"]\n",
    "kind = \"module\"\n",
    "\n",
    "[[relation]]\n",
    "from = \"A\"\n",
    "to = \"B\"\n",
);

/// 关系身份键的跨文件第二处 + 两个近似形态（同 `from` 不同 `to` / 同 `to` 不同 `from`）。
const IDENTITY_FRONTEND_SOURCE: &str = concat!(
    "[[relation]]\n",
    "from = \"A\"\n",
    "to = \"B\"\n",
    "\n",
    "[[relation]]\n",
    "from = \"A\"\n",
    "to = \"C\"\n",
    "\n",
    "[[relation]]\n",
    "from = \"C\"\n",
    "to = \"B\"\n",
);

// ---------------------------------------------------------------------------
// `apply_element_spec`：正向
// ---------------------------------------------------------------------------

/// 正向（矩阵：三档）：改档只动目标行，其余元素 / 注释逐字保留、读回为新档位。
#[test]
fn 改档只动目标行且其余逐字保留() {
    let before = model_of(CORE_SOURCE);
    let cases = [
        (GateTier::Off, "App.Auth"),
        (GateTier::Warn, "App.Billing"),
        (GateTier::Strict, "App.Auth"),
    ];
    for (gate, fqn) in cases {
        let rendered = apply_element_spec(CORE_SOURCE, &gate_spec(fqn, gate))
            .unwrap_or_else(|failure| panic!("{fqn} 改到 {} 应成功：{failure:?}", gate.as_str()));

        let changed = changed_lines(CORE_SOURCE, &rendered);
        assert_eq!(changed.len(), 1, "{fqn} 按行 diff 恰 1 行：{changed:?}");
        let target = rendered.lines().nth(changed[0]).expect("目标行");
        assert!(
            target.contains(&format!("gate = \"{}\"", gate.as_str())),
            "目标行为新档位：{target}"
        );
        assert!(
            rendered.contains("# 核心包（注释须逐字保留）") && rendered.contains("# 计费域"),
            "注释逐字保留：\n{rendered}"
        );

        for element in &before.elements {
            let reread = element_of(&rendered, &element.fqn);
            if element.fqn == fqn {
                assert_eq!(reread.gate, gate, "{fqn} 读回新档位");
            } else {
                assert_eq!(
                    reread.gate, element.gate,
                    "非目标元素档位不变：{}",
                    element.fqn
                );
            }
        }
    }
}

/// 正向：把 `gate` 改成其当前值 ⇒ 与源逐字节相等（回填 no-op + decor 沿用）。
#[test]
fn gate改成当前值产出零字节差异() {
    for (fqn, gate) in [("App.Auth", GateTier::Off), ("App.Billing", GateTier::Warn)] {
        let rendered = apply_element_spec(HEADER_SOURCE, &gate_spec(fqn, gate))
            .unwrap_or_else(|failure| panic!("{fqn} 改成当前值应成功：{failure:?}"));
        assert_eq!(rendered, HEADER_SOURCE, "{fqn} 改成当前值应与源逐字节相等");
    }
}

/// 可清空位的一臂：键名 + 「构造带该位三态的 spec」+ 「回读该位」。
type ClearableArm = (
    &'static str,
    fn(&str, Option<Option<&str>>) -> ElementSpec,
    fn(&Element) -> Option<String>,
);

/// 正向（矩阵：3 位 × 3 态）：`entry` / `doc` / `summary` 的三态逐位可辨。
#[test]
fn 三态逐位可辨() {
    let cases: [ClearableArm; 3] = [
        ("entry", spec_entry, read_entry),
        ("doc", spec_doc, read_doc),
        ("summary", spec_summary, read_summary),
    ];
    for (key, build, read) in cases {
        const FQN: &str = "App.Auth";

        let untouched = apply_element_spec(FIELDS_SOURCE, &build(FQN, None)).expect("不动臂应成功");
        assert_eq!(untouched, FIELDS_SOURCE, "{key} 的 None 臂整份零差异");
        assert_eq!(key_line(&untouched, key), key_line(FIELDS_SOURCE, key));

        let cleared =
            apply_element_spec(FIELDS_SOURCE, &build(FQN, Some(None))).expect("清空臂应成功");
        assert!(
            key_line(&cleared, key).is_none(),
            "{key} 清空臂删键：\n{cleared}"
        );
        assert_eq!(read(&element_of(&cleared, FQN)), None, "{key} 回读 None");

        let written = apply_element_spec(FIELDS_SOURCE, &build(FQN, Some(Some("新值"))))
            .expect("设值臂应成功");
        assert_eq!(
            read(&element_of(&written, FQN)),
            Some("新值".to_owned()),
            "{key} 回读新值"
        );
    }
}

/// 正向（矩阵：2 位 × 3 态）：集合位清空即删键，MUST NOT 写出空数组 / 空表。
#[test]
fn 集合位清空即删键不写空集合() {
    const FQN: &str = "App.Auth";

    let untouched = apply_element_spec(
        COLLECTION_SOURCE,
        &ElementSpec {
            fqn: FQN.to_owned(),
            ..Default::default()
        },
    )
    .expect("不动臂应成功");
    assert_eq!(untouched, COLLECTION_SOURCE, "两位的 None 臂整份零差异");

    let cleared = apply_element_spec(
        COLLECTION_SOURCE,
        &ElementSpec {
            fqn: FQN.to_owned(),
            path: Some(Vec::new()),
            metadata: Some(BTreeMap::new()),
            ..Default::default()
        },
    )
    .expect("清空臂应成功");
    assert!(
        key_line(&cleared, "path").is_none(),
        "path 键删净：\n{cleared}"
    );
    assert!(
        key_line(&cleared, "metadata").is_none(),
        "metadata 键删净：\n{cleared}"
    );
    assert!(!cleared.contains("path = []"), "零空数组形态");
    assert!(!cleared.contains("metadata = {"), "零空表形态");
    let reread = element_of(&cleared, FQN);
    assert!(reread.path.is_empty() && reread.metadata.is_empty());

    let mut metadata = BTreeMap::new();
    metadata.insert("owner".to_owned(), "auth".to_owned());
    let written = apply_element_spec(
        COLLECTION_SOURCE,
        &ElementSpec {
            fqn: FQN.to_owned(),
            path: Some(vec!["src/auth".to_owned(), "src/token".to_owned()]),
            metadata: Some(metadata),
            ..Default::default()
        },
    )
    .expect("设值臂应成功");
    let reread = element_of(&written, FQN);
    assert_eq!(
        reread.path,
        vec!["src/auth".to_owned(), "src/token".to_owned()]
    );
    assert_eq!(
        reread.metadata.get("owner").map(String::as_str),
        Some("auth"),
        "内联表逐项回读"
    );
    assert!(written.contains("path = ["), "写内联数组：\n{written}");
}

/// 正向（矩阵：`gate` / `api` 两值）：封闭枚举位显式化缺省不是清空——插入键、无删键路径。
#[test]
fn 封闭枚举位显式化缺省不是清空() {
    let source = concat!("[element.\"App.Auth\"]\n", "kind = \"domain\"\n");

    let inserted = apply_element_spec(source, &gate_spec("App.Auth", GateTier::Off))
        .expect("显式化缺省应成功");
    assert_eq!(
        key_line(&inserted, "gate"),
        Some("gate = \"off\""),
        "显式化缺省即插入该行：\n{inserted}"
    );
    assert_eq!(element_of(&inserted, "App.Auth").gate, GateTier::Off);

    let api_written = apply_element_spec(
        source,
        &ElementSpec {
            fqn: "App.Auth".to_owned(),
            api: Some(ApiStatus::None),
            ..Default::default()
        },
    )
    .expect("写 api 应成功");
    assert_eq!(key_line(&api_written, "api"), Some("api = \"none\""));
    assert_eq!(element_of(&api_written, "App.Auth").api, ApiStatus::None);

    let closed = apply_element_spec(
        &inserted,
        &ElementSpec {
            fqn: "App.Auth".to_owned(),
            ..Default::default()
        },
    )
    .expect("封闭枚举位不动臂应成功");
    assert_eq!(closed, inserted, "封闭枚举位无清空臂：None 即不动");

    let strict = apply_element_spec(source, &gate_spec("App.Auth", GateTier::Strict))
        .expect("设 strict 应成功");
    let reverted = apply_element_spec(&strict, &gate_spec("App.Auth", GateTier::Off))
        .expect("改回 off 应成功");
    assert_eq!(
        key_line(&reverted, "gate"),
        Some("gate = \"off\""),
        "改回缺省档位后行仍在（幂等可逆）"
    );
}

/// 正向：新建元素按 `kind` / `gate` / `api` / `entry` / `path` / `doc` / `summary` /
/// `metadata` 规范字段序整表写出，逐字段回读相符。
#[test]
fn 新建元素按规范字段序整表写出() {
    let source = concat!("# 核心包\n", "[element.\"App\"]\n", "kind = \"package\"\n");
    let mut metadata = BTreeMap::new();
    metadata.insert("owner".to_owned(), "core".to_owned());
    let spec = ElementSpec {
        fqn: "App.New".to_owned(),
        kind: Some(ElementKind::Module),
        gate: Some(GateTier::Warn),
        api: Some(ApiStatus::Declared),
        entry: Some(Some("src/new/index.ts".to_owned())),
        path: Some(vec!["src/new".to_owned()]),
        doc: Some(Some("docs/new.md".to_owned())),
        summary: Some(Some("新模块".to_owned())),
        metadata: Some(metadata),
        relations: Vec::new(),
        file: None,
        create: false,
        expect: None,
    };
    let rendered = apply_element_spec(source, &spec).expect("新建应成功");
    let header = rendered
        .find("[element.\"App.New\"]")
        .expect("新增单层表头");
    assert!(header > 0, "新表追加在既有元素之后：\n{rendered}");

    let body = &rendered[header..];
    let order = [
        "kind", "gate", "api", "entry", "path", "doc", "summary", "metadata",
    ];
    let mut previous = 0;
    for key in order {
        let index = body
            .find(&format!("\n{key} = "))
            .unwrap_or_else(|| panic!("{key} 应在新建表内：\n{body}"));
        assert!(index >= previous, "字段序固定，{key} 越位：\n{body}");
        previous = index;
    }

    let reread = element_of(&rendered, "App.New");
    assert_eq!(reread.kind, ElementKind::Module);
    assert_eq!(reread.gate, GateTier::Warn);
    assert_eq!(reread.api, ApiStatus::Declared);
    assert_eq!(reread.entry.as_deref(), Some("src/new/index.ts"));
    assert_eq!(reread.path, vec!["src/new".to_owned()]);
    assert_eq!(reread.doc.as_deref(), Some("docs/new.md"));
    assert_eq!(reread.summary.as_deref(), Some("新模块"));
    assert_eq!(
        reread.metadata.get("owner").map(String::as_str),
        Some("core")
    );
}

/// 正向（矩阵：3 形态）：写面零重排——既有键沿用原 decor，新键追加表尾，
/// `entry` / `api` 的相对序逐字保留。
#[test]
fn 写面零重排且沿用原decor() {
    let decorated = concat!(
        "[element.\"App.Auth\"]\n",
        "kind = \"domain\"\n",
        "  gate   =   \"warn\"   # 档位说明\n",
    );
    let rendered = apply_element_spec(decorated, &gate_spec("App.Auth", GateTier::Strict))
        .expect("改档应成功");
    assert_eq!(
        key_line(&rendered, "gate"),
        Some("  gate   =   \"strict\"   # 档位说明"),
        "沿用原 decor（缩进 / 行内注释逐字）：\n{rendered}"
    );

    let appendable = concat!(
        "[element.\"App.Auth\"]\n",
        "kind = \"domain\"\n",
        "gate = \"warn\"\n",
        "path = [\"src/auth\"]\n",
    );
    let rendered = apply_element_spec(
        appendable,
        &ElementSpec {
            fqn: "App.Auth".to_owned(),
            summary: Some(Some("认证".to_owned())),
            ..Default::default()
        },
    )
    .expect("追加新键应成功");
    let summary_index = rendered.find("summary = ").expect("新键在场");
    assert!(
        summary_index > rendered.find("path = ").expect("path 在案"),
        "新键追加到元素表尾（不按规范序插队）：\n{rendered}"
    );

    let before_api = concat!(
        "[element.\"App.Auth\"]\n",
        "kind = \"domain\"\n",
        "gate = \"warn\"\n",
        "entry = \"./packages/desktop/src/auth/token.ts\"\n",
        "api = \"declared\"\n",
    );
    let after_api = concat!(
        "[element.\"App.Auth\"]\n",
        "kind = \"domain\"\n",
        "gate = \"warn\"\n",
        "api = \"declared\"\n",
        "entry = \"./packages/desktop/src/auth/token.ts\"\n",
    );
    for source in [before_api, after_api] {
        let rendered =
            apply_element_spec(source, &gate_spec("App.Auth", GateTier::Off)).expect("改档应成功");
        assert!(
            rendered.contains("entry = \"./packages/desktop/src/auth/token.ts\""),
            "`entry` 行逐字保留（`./` 前缀不归一、不重排）：\n{rendered}"
        );
        let entry_line = rendered
            .lines()
            .position(|line| line.starts_with("entry = "))
            .expect("entry 行在案");
        let api_line = rendered
            .lines()
            .position(|line| line.starts_with("api = "))
            .expect("api 行在案");
        assert_eq!(
            entry_line < api_line,
            source.find("entry = ").expect("源含 entry") < source.find("api = ").expect("源含 api"),
            "相对序逐字保留（写面零重排）：\n{rendered}"
        );
        assert_eq!(
            element_of(&rendered, "App.Auth").entry.as_deref(),
            Some("./packages/desktop/src/auth/token.ts"),
            "读回 `entry` 逐字相等"
        );
    }

    let plain = concat!(
        "[element.\"App.Auth\"]\n",
        "kind = \"domain\"\n",
        "gate = \"warn\"\n",
        "path = [\"src/auth\"]\n",
    );
    let rendered =
        apply_element_spec(plain, &gate_spec("App.Auth", GateTier::Strict)).expect("改档应成功");
    assert!(
        !rendered.contains("entry"),
        "无 `entry` 的源改档后仍无该键（不臆造字段）：\n{rendered}"
    );
}

// ---------------------------------------------------------------------------
// `apply_element_spec`：异常（零文本产出即 `Err` 的返回类型保证）
// ---------------------------------------------------------------------------

/// 异常：元素在案且给了 `kind`（新建意图）⇒ `ElementAlreadyExists`。
#[test]
fn 在案元素给kind即拒() {
    let spec = ElementSpec {
        fqn: "App.Auth".to_owned(),
        kind: Some(ElementKind::Domain),
        ..Default::default()
    };
    let failure = apply_element_spec(CORE_SOURCE, &spec).expect_err("在案 + kind 应拒");
    assert_eq!(failure.kind, EditErrorKind::ElementAlreadyExists);
    assert!(
        failure.facts.contains("App.Auth"),
        "facts 点名元素：{}",
        failure.facts
    );
    assert!(
        failure.facts.contains("省去 kind"),
        "facts 给出路：{}",
        failure.facts
    );
    assert_eq!(failure.message(), format!("元素已在案：{}", failure.facts));
}

/// 异常：元素缺席且缺 `kind`（含全缺省 `ElementSpec`）⇒ `ElementKindMissing`。
#[test]
fn 缺席元素缺kind即拒() {
    let spec = ElementSpec {
        fqn: "App.Missing".to_owned(),
        ..Default::default()
    };
    let failure = apply_element_spec(CORE_SOURCE, &spec).expect_err("缺席 + 缺 kind 应拒");
    assert_eq!(failure.kind, EditErrorKind::ElementKindMissing);
    assert!(
        failure.facts.contains("App.Missing") && failure.facts.contains("不在案"),
        "facts 点名 FQN 不在案：{}",
        failure.facts
    );
}

/// 异常：元素声明非表形态 ⇒ `ElementNotTable`。
#[test]
fn 元素声明非表形态即拒() {
    let source = "element.\"App\" = \"x\"\n";
    let failure = apply_element_spec(
        source,
        &ElementSpec {
            fqn: "App".to_owned(),
            ..Default::default()
        },
    )
    .expect_err("非表形态应拒");
    assert_eq!(failure.kind, EditErrorKind::ElementNotTable);
    assert!(
        failure.facts.contains("App") && failure.facts.contains("无法写回"),
        "facts 含 FQN 与「无法写回」：{}",
        failure.facts
    );
}

/// 异常：源不可解析 ⇒ `SourceParseFailed`（facts 为 parser 原文，零加工）。
#[test]
fn 坏源解析失败零文本() {
    let failure = apply_element_spec(
        "[element.\"App]\nkind = \"package\"\n",
        &ElementSpec {
            fqn: "App".to_owned(),
            ..Default::default()
        },
    )
    .expect_err("坏源应 Err");
    assert_eq!(failure.kind, EditErrorKind::SourceParseFailed);
    assert!(!failure.facts.is_empty(), "facts 为 parser 原文");
}

/// 异常：越界值只由重解析自证拒绝（写面零第二份值域判据）。
#[test]
fn 越界值只由重解析自证拒绝() {
    let failure = apply_element_spec(
        "[element.\"App\"]\nkind = \"package\"\n",
        &ElementSpec {
            fqn: "App".to_owned(),
            path: Some(vec!["a".to_owned(), "b".to_owned()]),
            ..Default::default()
        },
    )
    .expect_err("package 两条 path 应被自证拒绝");
    assert_eq!(failure.kind, EditErrorKind::RenderSelfCheck);
    assert!(
        failure.facts.contains("重解析出 1 条诊断"),
        "facts 含诊断条数：{}",
        failure.facts
    );
    assert!(
        failure.facts.contains("invalidValue"),
        "拒绝理由是 parser 诊断（非写面自写规则）：{}",
        failure.facts
    );
}

/// 异常（矩阵：7 形态 + 1 对照）：新建 FQN 七形态皆拒；既有元素的同形态 FQN 不重做校验。
#[test]
fn 新建fqn七形态皆拒() {
    let rejects = [
        "",
        "App Auth",
        "App\"Auth",
        "App[",
        "App]",
        "App#x",
        "App\n.Auth",
    ];
    for fqn in rejects {
        let spec = ElementSpec {
            fqn: fqn.to_owned(),
            kind: Some(ElementKind::Module),
            ..Default::default()
        };
        let Err(failure) = apply_element_spec(CORE_SOURCE, &spec) else {
            panic!("{fqn:?} 应被拒（写面要把它渲染成键）")
        };
        assert_eq!(
            failure.kind,
            EditErrorKind::FqnInvalid,
            "{fqn:?} 应 FqnInvalid"
        );
        assert!(
            failure.facts.contains(&format!("{fqn:?}")),
            "facts 含形态：{}",
            failure.facts
        );
    }

    let existing = "[element.\"App.🚀Auth\"]\nkind = \"module\"\ngate = \"off\"\n";
    let rendered = apply_element_spec(existing, &gate_spec("App.🚀Auth", GateTier::Warn))
        .expect("既有元素只由 parser 判形态（合法键即写回）");
    assert_eq!(element_of(&rendered, "App.🚀Auth").gate, GateTier::Warn);
}

/// 边界：`apply_*` 不读 `file` 与 `expect`（两者是 plan 层旋钮）。
#[test]
fn apply不读file与expect() {
    let plain = gate_spec("App.Auth", GateTier::Strict);
    let with_knobs = ElementSpec {
        file: Some("99-other.toml".to_owned()),
        expect: Some("sha256:错值".to_owned()),
        ..plain.clone()
    };
    assert_eq!(
        apply_element_spec(CORE_SOURCE, &plain).expect("plain 臂应成功"),
        apply_element_spec(CORE_SOURCE, &with_knobs).expect("带旋钮臂应成功"),
        "两臂产出逐字节相等（apply 不读落点与锁）"
    );
    assert_eq!(with_knobs.expect.as_deref(), Some("sha256:错值"));
}

/// 边界（矩阵：空源 / 超长与 emoji FQN / 排版五形态 / 500 元素规模）。
#[test]
fn 特殊字符与排版与规模边界() {
    let minimal = apply_element_spec(
        "",
        &ElementSpec {
            fqn: "App".to_owned(),
            kind: Some(ElementKind::Package),
            ..Default::default()
        },
    )
    .expect("空源新建应成功");
    assert!(
        minimal.contains("[element.") && minimal.contains("kind = \"package\""),
        "最小文件：\n{minimal}"
    );
    assert_eq!(model_of(&minimal).elements.len(), 1);

    for fqn in ["X".repeat(1200), "App.🚀Auth".to_owned()] {
        let spec = ElementSpec {
            fqn: fqn.clone(),
            kind: Some(ElementKind::Module),
            ..Default::default()
        };
        let rendered = apply_element_spec(CORE_SOURCE, &spec)
            .unwrap_or_else(|failure| panic!("特殊 FQN {fqn:?} 应可写回：{failure:?}"));
        assert_eq!(
            element_of(&rendered, &fqn).kind,
            ElementKind::Module,
            "读回 {fqn:?}"
        );
    }

    let spaced = concat!(
        "[element.\"App\"]\n",
        "  kind   =   \"package\"   # 缩进与行内注释\n",
        "\n",
        "\n",
        "[element.\"App.Billing\"]\n",
        "kind = \"module\"\n",
        "gate = \"off\"\n",
    );
    let rendered = apply_element_spec(spaced, &gate_spec("App.Billing", GateTier::Strict))
        .expect("改档应成功");
    assert!(
        rendered.contains("  kind   =   \"package\"   # 缩进与行内注释"),
        "缩进与行内注释逐字保留：\n{rendered}"
    );
    assert_eq!(
        rendered.matches('\n').count(),
        spaced.matches('\n').count(),
        "空行不增删"
    );
    assert_eq!(element_of(&rendered, "App.Billing").gate, GateTier::Strict);

    let no_trailing = "[element.\"App\"]\nkind = \"package\"";
    let rendered =
        apply_element_spec(no_trailing, &gate_spec("App", GateTier::Warn)).expect("改档应成功");
    assert_eq!(element_of(&rendered, "App").gate, GateTier::Warn);
    assert!(
        rendered.starts_with("[element.\"App\"]\nkind = \"package\"\n"),
        "无尾换行源的正文逐字保留：{rendered:?}"
    );

    let crlf = "[element.\"App\"]\r\nkind = \"package\"\r\ngate = \"off\"\r\n";
    let rendered =
        apply_element_spec(crlf, &gate_spec("App", GateTier::Strict)).expect("CRLF 源改档应成功");
    assert_eq!(
        element_of(&rendered, "App").gate,
        GateTier::Strict,
        "CRLF 下值正确"
    );

    let mut large = String::new();
    for index in 0..500 {
        large.push_str(&format!(
            "[element.\"App.Mod{index:03}\"]\nkind = \"module\"\ngate = \"off\"\n\n"
        ));
    }
    let rendered = apply_element_spec(&large, &gate_spec("App.Mod250", GateTier::Strict))
        .expect("规模面改档应成功");
    assert_eq!(changed_lines(&large, &rendered).len(), 1, "只有目标行变化");
    assert_eq!(
        element_of(&rendered, "App.Mod249").gate,
        GateTier::Off,
        "邻行不变"
    );
}

// ---------------------------------------------------------------------------
// `apply_element_delete`
// ---------------------------------------------------------------------------

/// 正向：删中间元素 ⇒ 该表离场、其余元素逐字、表头计数随动下降。
#[test]
fn 删除叶元素其余逐字且计数随动() {
    let rendered = apply_element_delete(DELETE_SOURCE, &one("App.Auth")).expect("删除应成功");
    assert!(
        !rendered.contains("[element.\"App.Auth\"]"),
        "目标元素离场：\n{rendered}"
    );
    assert!(
        rendered.contains("[element.\"App\"]\nkind = \"package\""),
        "首元素逐字保留：\n{rendered}"
    );
    assert!(
        rendered.contains("[element.\"App.Billing\"]\nkind = \"module\""),
        "尾元素逐字保留：\n{rendered}"
    );
    assert_eq!(header_counts(&rendered), Some((2, 1)), "表头计数随动下降");
    let model = model_of(&rendered);
    assert_eq!(model.elements.len(), 2);
    assert_eq!(model.relations.len(), 1);
}

/// 异常（矩阵：空源 / 只有关系）⇒ `ElementTableMissing`。
#[test]
fn 无element表无从定位() {
    for source in ["", "[[relation]]\nfrom = \"A\"\nto = \"B\"\n"] {
        let failure = apply_element_delete(source, &one("A")).expect_err("无 element 表应 Err");
        assert_eq!(failure.kind, EditErrorKind::ElementTableMissing);
        assert!(
            failure.facts.contains("无从定位元素"),
            "facts 说明逻辑落点缺席：{}",
            failure.facts
        );
    }
}

/// 异常：元素缺席 ⇒ `ElementNotFound`，facts 列该文件现有元素。
#[test]
fn 元素缺席列出该文件现有元素() {
    let failure =
        apply_element_delete(CORE_SOURCE, &one("App.Missing")).expect_err("元素缺席应 Err");
    assert_eq!(failure.kind, EditErrorKind::ElementNotFound);
    for existing in ["App", "App.Billing", "App.Auth"] {
        assert!(
            failure.facts.contains(existing),
            "facts 附现有元素 {existing}：{}",
            failure.facts
        );
    }
    assert!(
        failure.facts.contains(" / "),
        "现有元素以 ` / ` 连接：{}",
        failure.facts
    );
}

/// 边界（矩阵：删 `App.A` / 删 `App` 两臂）：删除按完整 FQN 精确匹配，前缀近似不误删。
#[test]
fn 删除按完整fqn精确匹配() {
    let source = concat!(
        "[element.\"App\"]\n",
        "kind = \"package\"\n",
        "\n",
        "[element.\"App.AB\"]\n",
        "kind = \"module\"\n",
        "\n",
        "[element.\"App.A\"]\n",
        "kind = \"module\"\n",
        "\n",
        "[element.\"App.A.B\"]\n",
        "kind = \"module\"\n",
    );

    let rendered = apply_element_delete(source, &one("App.A")).expect("删 App.A 应成功");
    assert!(!rendered.contains("[element.\"App.A\"]"), "命中条离场");
    assert!(rendered.contains("[element.\"App.AB\"]"), "前缀近似不误删");
    assert!(rendered.contains("[element.\"App.A.B\"]"), "后代元素不误删");

    let rendered = apply_element_delete(source, &one("App")).expect("删 App 应成功");
    assert!(!rendered.contains("[element.\"App\"]"));
    assert!(
        rendered.contains("[element.\"App.A.B\"]"),
        "删祖先不连带后代"
    );
}

/// 边界（矩阵：唯一元素 / emoji / 超长 FQN）：删净后产出仍可解析。
#[test]
fn 删净唯一元素仍可解析() {
    let long = "X".repeat(1200);
    let cases = [
        (
            "App".to_owned(),
            "[element.\"App\"]\nkind = \"package\"\n".to_owned(),
        ),
        (
            "App.🚀Auth".to_owned(),
            "[element.\"App.🚀Auth\"]\nkind = \"module\"\n".to_owned(),
        ),
        (
            long.clone(),
            format!("[element.\"{long}\"]\nkind = \"module\"\n"),
        ),
    ];
    for (fqn, source) in cases {
        let rendered = apply_element_delete(&source, &one(&fqn))
            .unwrap_or_else(|failure| panic!("删除 {fqn:?} 应成功：{failure:?}"));
        assert_eq!(model_of(&rendered).elements.len(), 0, "元素数 0 且可解析");
    }
}

// ---------------------------------------------------------------------------
// `apply_relation_upsert`
// ---------------------------------------------------------------------------

/// 正向：既有 `(from, to)` 改 `desc` ⇒ 条数不变，其余条目（含空行与注释）逐字保留。
#[test]
fn 关系命中走更新条数不变() {
    let rendered = apply_relation_upsert(RELATIONS_SOURCE, "A", "B", Some(Some("新说明")), None)
        .expect("更新应成功");
    let model = model_of(&rendered);
    assert_eq!(model.relations.len(), 3, "条数不变");
    assert_eq!(
        relation_of(&model, "A", "B").description.as_deref(),
        Some("新说明")
    );
    assert_eq!(
        relation_of(&model, "B", "C").description,
        None,
        "其余条目不动"
    );
    assert!(
        rendered.contains("[[relation]]\nfrom = \"B\"\nto = \"C\"\n"),
        "其余条目逐字保留：\n{rendered}"
    );
}

/// 正向（矩阵：4 形态）：`desc` 三态逐位可辨，空串仍可解析。
#[test]
fn 关系desc三态逐位可辨() {
    let untouched =
        apply_relation_upsert(RELATIONS_SOURCE, "A", "B", None, None).expect("不动臂应成功");
    assert_eq!(untouched, RELATIONS_SOURCE, "desc None 即整份零差异");

    let cleared =
        apply_relation_upsert(RELATIONS_SOURCE, "A", "B", Some(None), None).expect("清空臂应成功");
    assert!(
        key_line(&cleared, "desc").is_none(),
        "desc 键消失：\n{cleared}"
    );
    assert_eq!(
        relation_of(&model_of(&cleared), "A", "B").description,
        None,
        "回读 None"
    );

    let written = apply_relation_upsert(RELATIONS_SOURCE, "A", "B", Some(Some("说明")), None)
        .expect("设值臂应成功");
    assert_eq!(
        relation_of(&model_of(&written), "A", "B")
            .description
            .as_deref(),
        Some("说明")
    );

    let empty = apply_relation_upsert(RELATIONS_SOURCE, "A", "B", Some(Some("")), None)
        .expect("空串应可写回");
    assert_eq!(
        relation_of(&model_of(&empty), "A", "B")
            .description
            .as_deref(),
        Some("")
    );
}

/// 正向（矩阵：3 态）：`apis` 清空即删键（MUST NOT 写空数组），设值逐项回读。
#[test]
fn 关系apis清空即删键() {
    let untouched = apply_relation_upsert(APIS_SOURCE, "A", "B", None, None).expect("不动臂应成功");
    assert_eq!(untouched, APIS_SOURCE, "apis None 即整份零差异");

    let empty: Vec<String> = Vec::new();
    let cleared = apply_relation_upsert(APIS_SOURCE, "A", "B", None, Some(empty.as_slice()))
        .expect("清空臂应成功");
    assert!(!cleared.contains("apis"), "apis 键消失：\n{cleared}");
    assert!(
        relation_of(&model_of(&cleared), "A", "B").apis.is_empty(),
        "回读为空"
    );

    let names = vec!["open".to_owned(), "close".to_owned()];
    let written = apply_relation_upsert(APIS_SOURCE, "A", "B", None, Some(names.as_slice()))
        .expect("设值臂应成功");
    assert_eq!(
        relation_of(&model_of(&written), "A", "B").apis,
        names,
        "原文序逐项回读"
    );
}

/// 正向（矩阵：追加 / 零关系文件）：缺席即追加条目；`relation` 键缺席即新建条目数组。
#[test]
fn 关系缺席追加且缺键即建数组() {
    let rendered = apply_relation_upsert(RELATIONS_SOURCE, "A", "A", Some(Some("自述")), None)
        .expect("追加应成功");
    let model = model_of(&rendered);
    assert_eq!(model.relations.len(), 4, "追加一条");
    assert_eq!(
        relation_of(&model, "A", "A").description.as_deref(),
        Some("自述")
    );
    assert_eq!(
        rendered.matches("[[relation]]").count(),
        4,
        "每条一个表头（不合并）"
    );

    let block = &rendered[rendered.rfind("[[relation]]").expect("新条目在尾")..];
    let from_index = block.find("from = ").expect("from 在场");
    let to_index = block.find("to = ").expect("to 在场");
    let desc_index = block.find("desc = ").expect("desc 在场");
    assert!(
        from_index < to_index && to_index < desc_index,
        "键序 from / to / desc：\n{block}"
    );

    let zero_relation = concat!(
        "[element.\"A\"]\n",
        "kind = \"module\"\n",
        "\n",
        "[element.\"B\"]\n",
        "kind = \"module\"\n",
    );
    let rendered =
        apply_relation_upsert(zero_relation, "A", "B", None, None).expect("缺 relation 键即建数组");
    assert!(
        rendered.contains("[[relation]]"),
        "新建条目数组：\n{rendered}"
    );
    assert_eq!(model_of(&rendered).relations.len(), 1);
}

/// 异常（矩阵：3 值）：越界 `apis` 值只由重解析自证拒绝。
#[test]
fn apis越界值只由自证拒绝() {
    let cases = [
        vec!["A".to_owned(), "A".to_owned()],
        vec![String::new()],
        vec!["a::b".to_owned()],
    ];
    for names in cases {
        let failure = apply_relation_upsert(APIS_SOURCE, "A", "B", None, Some(names.as_slice()))
            .expect_err("越界 apis 应被自证拒绝");
        assert_eq!(failure.kind, EditErrorKind::RenderSelfCheck);
        assert!(
            failure.facts.contains("invalidValue"),
            "拒绝理由来自 parser 诊断：{}",
            failure.facts
        );
    }
}

/// 异常：源不可解析 ⇒ `SourceParseFailed`（零文本）。
#[test]
fn 关系写入坏源零文本() {
    let failure =
        apply_relation_upsert("[element.\"A\"\nkind = \"module\"\n", "A", "B", None, None)
            .expect_err("坏源应 Err");
    assert_eq!(failure.kind, EditErrorKind::SourceParseFailed);
}

/// 边界（矩阵：多条中间条 / 特殊字符端点）：更新只动命中条。
#[test]
fn 多条目更新只动命中条() {
    let rendered = apply_relation_upsert(RELATIONS_SOURCE, "B", "C", Some(Some("中间条")), None)
        .expect("更新中间条应成功");
    let model = model_of(&rendered);
    assert_eq!(model.relations.len(), 3);
    assert_eq!(
        relation_of(&model, "B", "C").description.as_deref(),
        Some("中间条")
    );
    assert_eq!(
        relation_of(&model, "A", "B").description.as_deref(),
        Some("旧说明"),
        "其余条目保持原值"
    );

    let emoji = "App.🚀Auth";
    let long = "Y".repeat(1200);
    let source = format!(
        "[element.\"{emoji}\"]\nkind = \"module\"\n\n\
         [element.\"{long}\"]\nkind = \"module\"\n\n\
         [[relation]]\nfrom = \"{emoji}\"\nto = \"{long}\"\n"
    );
    let rendered = apply_relation_upsert(&source, emoji, &long, Some(Some("跨语言")), None)
        .expect("特殊端点应可写回");
    assert_eq!(
        relation_of(&model_of(&rendered), emoji, &long)
            .description
            .as_deref(),
        Some("跨语言")
    );
}

// ---------------------------------------------------------------------------
// `apply_relation_delete`
// ---------------------------------------------------------------------------

/// 正向：三条关系中删中间一条 ⇒ 该条目离场，其余条目与元素逐字保留。
#[test]
fn 关系删除只动命中条() {
    let rendered = apply_relation_delete(RELATIONS_SOURCE, "B", "C").expect("删除应成功");
    let model = model_of(&rendered);
    assert_eq!(model.relations.len(), 2, "关系回读条数 −1");
    assert!(
        !model
            .relations
            .iter()
            .any(|relation| relation.from == "B" && relation.to == "C"),
        "命中条离场"
    );
    assert_eq!(model.elements.len(), 3, "元素逐字保留");
    assert!(
        rendered.contains("[element.\"C\"]\nkind = \"module\""),
        "元素表不重排：\n{rendered}"
    );
}

/// 异常：关系缺席 ⇒ `RelationNotFound`，facts 列该文件现有 `from → to` 对。
#[test]
fn 关系缺席列出该文件现有对() {
    let failure = apply_relation_delete(RELATIONS_SOURCE, "C", "A").expect_err("关系缺席应 Err");
    assert_eq!(failure.kind, EditErrorKind::RelationNotFound);
    assert!(
        failure.facts.contains("C → A"),
        "facts 点名目标对：{}",
        failure.facts
    );
    for pair in ["A → B", "B → C", "A → C"] {
        assert!(
            failure.facts.contains(pair),
            "facts 列现有对 {pair}：{}",
            failure.facts
        );
    }
}

/// 边界（矩阵：删净 / 同 from 不同 to / 同 to 不同 from）：`(from, to)` 双等是唯一口径。
#[test]
fn 关系删除双等匹配与删净() {
    let only = concat!(
        "[element.\"A\"]\n",
        "kind = \"module\"\n",
        "\n",
        "[element.\"B\"]\n",
        "kind = \"module\"\n",
        "\n",
        "[[relation]]\n",
        "from = \"A\"\n",
        "to = \"B\"\n",
    );
    let rendered = apply_relation_delete(only, "A", "B").expect("删净应成功");
    assert_eq!(model_of(&rendered).relations.len(), 0, "关系数归零且可解析");

    let rendered = apply_relation_delete(RELATIONS_SOURCE, "A", "B").expect("删命中条应成功");
    let model = model_of(&rendered);
    assert_eq!(model.relations.len(), 2);
    assert!(
        model
            .relations
            .iter()
            .any(|relation| relation.from == "A" && relation.to == "C"),
        "同 from 的另一条保留"
    );

    let rendered = apply_relation_delete(RELATIONS_SOURCE, "A", "C").expect("删命中条应成功");
    let model = model_of(&rendered);
    assert!(
        model
            .relations
            .iter()
            .any(|relation| relation.from == "B" && relation.to == "C"),
        "同 to 的另一条保留"
    );
    assert_eq!(model.relations.len(), 2);
}

/// 异常（矩阵：2 入口）：同一源内 `(from, to)` 两处声明 ⇒ 两函数均
/// `RelationDuplicated`（文件面 facts 点名处数与「同一源内多处声明」），零文本产出。
#[test]
fn 同源重复声明fail_closed() {
    let upsert_failure =
        apply_relation_upsert(DUPLICATED_IN_SOURCE, "A", "B", Some(Some("改")), None)
            .expect_err("同源两处 upsert 应拒");
    let delete_failure =
        apply_relation_delete(DUPLICATED_IN_SOURCE, "A", "B").expect_err("同源两处 delete 应拒");

    for failure in [&upsert_failure, &delete_failure] {
        assert_eq!(failure.kind, EditErrorKind::RelationDuplicated);
        assert!(
            failure.facts.contains("A → B 在同一源内多处声明（2 处）"),
            "文件面 facts 点名目标对与处数：{}",
            failure.facts
        );
        assert!(
            failure.facts.contains("先删除重复条目"),
            "facts 给出下一步：{}",
            failure.facts
        );
        let message = failure.message();
        assert!(
            message.starts_with("关系多处声明："),
            "判据句与文案族同源：{message}"
        );
        assert!(!message.contains("改档"), "判据句操作中立：{message}");
    }

    // 对照臂：同一源内该对只有一处 ⇒ 走各自规则（不报 RelationDuplicated）。
    let rendered = apply_relation_delete(RELATIONS_SOURCE, "A", "B").expect("唯一命中应删成功");
    assert_eq!(model_of(&rendered).relations.len(), 2, "只删该条");
}

/// 边界（矩阵：既有 / 新增 / 删除三臂）：重解析的关系条数分别不变 / +1 / −1——
/// 「更新而非追加副本」与「只删一条」的结构性判据（不符即 `ReadBackMismatch`）。
#[test]
fn 关系回读条数三臂各成立() {
    let base = concat!(
        "# 01 · 核心（3 元素 / 2 关系）\n",
        "[element.\"A\"]\n",
        "kind = \"module\"\n",
        "\n",
        "[element.\"B\"]\n",
        "kind = \"module\"\n",
        "\n",
        "[element.\"C\"]\n",
        "kind = \"module\"\n",
        "\n",
        "[[relation]]\n",
        "from = \"A\"\n",
        "to = \"B\"\n",
        "\n",
        "[[relation]]\n",
        "from = \"A\"\n",
        "to = \"C\"\n",
    );
    let before = model_of(base).relations.len();
    assert_eq!(before, 2);

    let updated =
        apply_relation_upsert(base, "A", "B", Some(Some("改")), None).expect("更新应成功");
    assert_eq!(
        model_of(&updated).relations.len(),
        before,
        "既有 ⇒ 条数不变（更新而非追加副本）"
    );

    let appended = apply_relation_upsert(base, "B", "C", None, None).expect("追加应成功");
    assert_eq!(
        model_of(&appended).relations.len(),
        before + 1,
        "新增 ⇒ 条数 +1"
    );

    let deleted = apply_relation_delete(base, "A", "C").expect("删除应成功");
    assert_eq!(
        model_of(&deleted).relations.len(),
        before - 1,
        "删除 ⇒ 条数 −1"
    );
}

// ---------------------------------------------------------------------------
// `plan_element_upsert`
// ---------------------------------------------------------------------------

/// 正向：两文件模型（目标元素在 `02-auth.toml`）⇒ 定位该文件、待落盘全文含新档位、
/// 调用前后盘面字节全等（`plan_*` 零写盘）。
#[test]
fn 定位目标文件并零写盘() {
    let temp = TempDir::new("plan-locate");
    temp.write_model("01-core.toml", "[element.\"App\"]\nkind = \"package\"\n");
    let auth_path = temp.write_model(
        "02-auth.toml",
        "[element.\"App.Auth\"]\nkind = \"domain\"\ngate = \"off\"\n",
    );
    let before = temp.snapshot();

    let plan = plan_element_upsert(temp.root(), &gate_spec("App.Auth", GateTier::Strict))
        .expect("定位应成功");
    assert_eq!(plan.write.file, auth_path, "定位到声明所在文件的绝对路径");
    assert!(
        plan.write.content.contains("gate = \"strict\""),
        "待落盘全文含新档位：\n{}",
        plan.write.content
    );
    assert!(
        plan.write.hash.starts_with("sha256:"),
        "hash 形态：{}",
        plan.write.hash
    );
    assert_eq!(temp.snapshot(), before, "plan 零写盘");
}

/// 正向：主题元素缺席 + 显式 `file`（目标文件尚不存在）⇒ 可落新文件且不落盘。
#[test]
fn 新建可落新文件且不落盘() {
    let temp = TempDir::new("plan-new-file");
    temp.write_model("01-core.toml", "[element.\"App\"]\nkind = \"package\"\n");
    let spec = ElementSpec {
        fqn: "App.New".to_owned(),
        kind: Some(ElementKind::Module),
        file: Some("07-new.toml".to_owned()),
        create: true,
        ..Default::default()
    };
    let plan = plan_element_upsert(temp.root(), &spec).expect("新建应成功");
    assert_eq!(
        plan.write.file,
        archi_graph_model_dir(temp.root()).join("07-new.toml"),
        "落点为模型声明目录下该文件"
    );
    assert!(
        plan.write.content.contains("[element.\"App.New\"]"),
        "待落盘全文含新表：\n{}",
        plan.write.content
    );
    assert!(!plan.write.file.exists(), "plan 零写盘（新文件仍不存在）");
}

/// 正向（组合：plan → apply → drift::file_hash_of / file_hash）：写后哈希可作下一次
/// `expect`。
#[test]
fn 写后哈希可作下一次expect() {
    let temp = TempDir::new("hash-chain");
    temp.write_model(
        "01-core.toml",
        "[element.\"App\"]\nkind = \"package\"\ngate = \"off\"\n",
    );
    let plan =
        plan_element_upsert(temp.root(), &gate_spec("App", GateTier::Warn)).expect("计划应成功");
    assert_eq!(
        plan.write.hash,
        file_hash_of(&plan.write.file, &plan.write.content),
        "hash 为写后内容哈希"
    );

    fs::write(&plan.write.file, &plan.write.content).expect("模拟壳层落盘");
    assert_eq!(
        plan.write.hash,
        file_hash(&plan.write.file),
        "落盘后 file_hash 逐字相等"
    );

    let next = plan_element_upsert(
        temp.root(),
        &ElementSpec {
            fqn: "App".to_owned(),
            gate: Some(GateTier::Strict),
            expect: Some(plan.write.hash.clone()),
            ..Default::default()
        },
    )
    .expect("写后哈希作 expect 应 Ok");
    assert!(next.write.content.contains("gate = \"strict\""));
}

/// 异常：既有元素给不同 `file` ⇒ `TargetInvalid`（迁文件属独立语义）。
#[test]
fn 既有元素不接受改落点() {
    let temp = TempDir::new("plan-move");
    temp.write_model("01-core.toml", "[element.\"App\"]\nkind = \"package\"\n");
    let spec = ElementSpec {
        fqn: "App".to_owned(),
        gate: Some(GateTier::Warn),
        file: Some("02-other.toml".to_owned()),
        ..Default::default()
    };
    let failure = plan_element_upsert(temp.root(), &spec).expect_err("改落点应拒");
    assert_eq!(failure.kind, EditErrorKind::TargetInvalid);
    assert!(
        failure.facts.contains("01-core.toml")
            && failure.facts.contains("02-other.toml")
            && failure.facts.contains("迁文件属独立语义"),
        "facts 点名现声明文件与给定值：{}",
        failure.facts
    );
}

/// 正向 + 异常（矩阵：5 臂）：落点四情形各点名成因。
#[test]
fn 落点四情形各点名成因() {
    let temp = TempDir::new("plan-target-arms");
    temp.write_model(
        "01-core.toml",
        "[element.\"App\"]
kind = \"package\"
",
    );

    let landed = plan_element_upsert(
        temp.root(),
        &ElementSpec {
            fqn: "App.New".to_owned(),
            kind: Some(ElementKind::Module),
            ..Default::default()
        },
    )
    .expect("省略 file 落父元素文件");
    assert_eq!(
        landed.write.file,
        temp.root()
            .join(".dev-team/architecture/graph/model/01-core.toml"),
        "新建省略 file ⇒ 落父元素声明文件"
    );

    let failure = plan_element_upsert(
        temp.root(),
        &ElementSpec {
            fqn: "Store".to_owned(),
            kind: Some(ElementKind::Package),
            ..Default::default()
        },
    )
    .expect_err("单段 FQN 省略 file 应拒");
    assert_eq!(failure.kind, EditErrorKind::TargetInvalid);
    assert!(
        failure.facts.contains("单段 FQN"),
        "facts 点名需给 file：{}",
        failure.facts
    );

    for file in ["a/b.toml", "/abs.toml", "x.json"] {
        let failure = plan_element_upsert(
            temp.root(),
            &ElementSpec {
                fqn: "App.New".to_owned(),
                kind: Some(ElementKind::Module),
                file: Some(file.to_owned()),
                ..Default::default()
            },
        )
        .expect_err("非模型声明目录一级应拒");
        assert_eq!(failure.kind, EditErrorKind::TargetInvalid);
        assert!(
            failure.facts.contains("不在模型声明目录一级"),
            "{file} 的 facts：{}",
            failure.facts
        );
    }

    let bare = TempDir::new("plan-target-bare");
    let failure = plan_element_upsert(
        bare.root(),
        &ElementSpec {
            fqn: "App".to_owned(),
            kind: Some(ElementKind::Package),
            file: Some("01-x.toml".to_owned()),
            ..Default::default()
        },
    )
    .expect_err("模型声明目录缺席应拒");
    assert_eq!(failure.kind, EditErrorKind::TargetInvalid);
    assert!(
        failure.facts.contains("不存在") && failure.facts.contains("不代建目录"),
        "facts 点名目录缺席：{}",
        failure.facts
    );
}

/// 异常（矩阵：父缺席 / 单段跳过）：新建查直接父元素，单段 FQN 跳过父判据。
#[test]
fn 新建查直接父且单段跳过() {
    let temp = TempDir::new("plan-parent");
    temp.write_model("01-core.toml", "[element.\"App\"]\nkind = \"package\"\n");

    let failure = plan_element_upsert(
        temp.root(),
        &ElementSpec {
            fqn: "App.A.B".to_owned(),
            kind: Some(ElementKind::Module),
            file: Some("02-more.toml".to_owned()),
            ..Default::default()
        },
    )
    .expect_err("父缺席应拒");
    assert_eq!(failure.kind, EditErrorKind::ParentMissing);
    assert!(
        failure.facts.contains("App.A") && failure.facts.contains("祖先须显式声明"),
        "facts 点名父 FQN：{}",
        failure.facts
    );

    let plan = plan_element_upsert(
        temp.root(),
        &ElementSpec {
            fqn: "Store".to_owned(),
            kind: Some(ElementKind::Package),
            file: Some("02-more.toml".to_owned()),
            create: true,
            ..Default::default()
        },
    )
    .expect("单段 FQN 跳过父判据");
    assert!(
        model_of(&plan.write.content).element("Store").is_some(),
        "新表写回该文件：
{}",
        plan.write.content
    );
}

/// 异常（矩阵：4 形态）：`expect` 乐观锁四形态逐条成立。
#[test]
fn expect四形态逐条成立() {
    let temp = TempDir::new("plan-expect");
    let path = temp.write_model(
        "01-core.toml",
        "[element.\"App\"]\nkind = \"package\"\ngate = \"off\"\n",
    );
    let current = file_hash(&path);

    plan_element_upsert(
        temp.root(),
        &ElementSpec {
            fqn: "App".to_owned(),
            gate: Some(GateTier::Warn),
            expect: Some(current),
            ..Default::default()
        },
    )
    .expect("当前值匹配应 Ok");

    let failure = plan_element_upsert(
        temp.root(),
        &ElementSpec {
            fqn: "App".to_owned(),
            gate: Some(GateTier::Warn),
            expect: Some("sha256:错值".to_owned()),
            ..Default::default()
        },
    )
    .expect_err("错值应 Err");
    assert_eq!(failure.kind, EditErrorKind::ExpectMismatch);
    assert!(
        failure.facts.contains("期望 sha256:错值")
            && failure.facts.contains("实际 sha256:")
            && failure.facts.contains("重读后再试"),
        "facts 含期望与实际：{}",
        failure.facts
    );

    let failure = plan_element_upsert(
        temp.root(),
        &ElementSpec {
            fqn: "App.New".to_owned(),
            kind: Some(ElementKind::Module),
            file: Some("07-new.toml".to_owned()),
            create: true,
            expect: Some("sha256:任意".to_owned()),
            ..Default::default()
        },
    )
    .expect_err("文件缺席 + expect 应 Err");
    assert_eq!(failure.kind, EditErrorKind::ExpectMismatch);
    assert!(
        failure.facts.contains("不存在") && failure.facts.contains("期望 sha256:任意"),
        "facts 点名文件缺席：{}",
        failure.facts
    );

    let plan = plan_element_upsert(temp.root(), &gate_spec("App", GateTier::Warn))
        .expect("None 不校验且照常产出");
    assert!(plan.write.content.contains("gate = \"warn\""));
}

/// 异常（矩阵：2 形态）：新建与在案的判据次序固定。
#[test]
fn 新建与在案的判据次序固定() {
    let temp = TempDir::new("plan-order");
    temp.write_model("01-core.toml", "[element.\"App\"]\nkind = \"package\"\n");

    let failure = plan_element_upsert(
        temp.root(),
        &ElementSpec {
            fqn: "App.New".to_owned(),
            file: Some("a/b.toml".to_owned()),
            ..Default::default()
        },
    )
    .expect_err("缺 kind 应早于落点判据");
    assert_eq!(
        failure.kind,
        EditErrorKind::ElementKindMissing,
        "判据句为新建缺 kind：{}",
        failure.message()
    );

    let failure = plan_element_upsert(
        temp.root(),
        &ElementSpec {
            fqn: "App".to_owned(),
            kind: Some(ElementKind::Package),
            file: Some("02-other.toml".to_owned()),
            ..Default::default()
        },
    )
    .expect_err("在案应早于落点判据");
    assert_eq!(failure.kind, EditErrorKind::ElementAlreadyExists);
}

/// 异常：目标文件读不出（目录形态占位）⇒ `SourceReadFailed`，facts 为
/// `{path}（{error}）` 语序落位形态。
#[test]
fn 源读取失败点名路径() {
    let temp = TempDir::new("plan-read-fail");
    let dir = archi_graph_model_dir(temp.root());
    fs::create_dir_all(dir.join("02-dir.toml")).expect("建同名目录（读不出的目标文件）");

    let failure = plan_element_upsert(
        temp.root(),
        &ElementSpec {
            fqn: "App".to_owned(),
            kind: Some(ElementKind::Package),
            file: Some("02-dir.toml".to_owned()),
            create: true,
            ..Default::default()
        },
    )
    .expect_err("读源失败应 Err");
    assert_eq!(failure.kind, EditErrorKind::SourceReadFailed);
    assert!(
        failure.facts.contains("02-dir.toml"),
        "facts 点名路径：{}",
        failure.facts
    );
    assert!(
        failure.facts.contains("（") && failure.facts.ends_with('）'),
        "语序落位 `{{path}}（{{error}}）`：{}",
        failure.facts
    );
}

/// 异常（矩阵：3 形态）：两入口共用同一判据措辞（`kind` 与 facts 同源）。
#[test]
fn 两入口共用同一判据措辞() {
    let temp = TempDir::new("plan-same-wording");
    temp.write_model("01-core.toml", CORE_SOURCE);

    let invalid_fqn = ElementSpec {
        fqn: "App Bad".to_owned(),
        kind: Some(ElementKind::Module),
        ..Default::default()
    };
    let from_plan = plan_element_upsert(temp.root(), &invalid_fqn).expect_err("plan 应拒");
    let from_apply = apply_element_spec(CORE_SOURCE, &invalid_fqn).expect_err("apply 应拒");
    assert_eq!(
        verdict(&from_plan),
        verdict(&from_apply),
        "FQN 形态一份措辞"
    );

    let missing_kind = ElementSpec {
        fqn: "App.Missing".to_owned(),
        ..Default::default()
    };
    let from_plan = plan_element_upsert(temp.root(), &missing_kind).expect_err("plan 应拒");
    let from_apply = apply_element_spec(CORE_SOURCE, &missing_kind).expect_err("apply 应拒");
    assert_eq!(
        verdict(&from_plan),
        verdict(&from_apply),
        "缺 kind 一份措辞"
    );

    let declared = ElementSpec {
        fqn: "App.Auth".to_owned(),
        kind: Some(ElementKind::Domain),
        ..Default::default()
    };
    let from_plan = plan_element_upsert(temp.root(), &declared).expect_err("plan 应拒");
    let from_apply = apply_element_spec(CORE_SOURCE, &declared).expect_err("apply 应拒");
    assert_eq!(from_plan.kind, from_apply.kind, "在案判据同一变体");
    assert!(
        from_plan.facts.contains("已声明于") && from_plan.facts.contains("省去 kind"),
        "plan 额外带声明文件事实：{}",
        from_plan.facts
    );
    assert!(
        from_apply.facts.contains("省去 kind"),
        "两入口同一出路措辞：{}",
        from_apply.facts
    );
}

/// 边界：空白 / 不存在的 `root` 一律 `Err` 不 panic；空模型目录 + 单段 FQN 新建 ⇒ `Ok`；
/// 既有 emoji 元素 ⇒ `Ok`。
#[test]
fn 空白与不存在root均err() {
    let spec = ElementSpec {
        fqn: "App".to_owned(),
        kind: Some(ElementKind::Package),
        file: Some("01-core.toml".to_owned()),
        create: true,
        ..Default::default()
    };
    assert!(
        plan_element_upsert(Path::new(""), &spec).is_err(),
        "空白 root 应 Err"
    );
    assert!(
        plan_element_upsert(Path::new("/nonexistent/archi/root"), &spec).is_err(),
        "不存在 root 应 Err"
    );
    assert!(
        plan_element_delete(Path::new(""), "App", false, None).is_err(),
        "删除面空白 root 应 Err"
    );

    let empty = TempDir::new("plan-empty-models");
    fs::create_dir_all(archi_graph_model_dir(empty.root())).expect("建空模型目录");
    let plan = plan_element_upsert(empty.root(), &spec).expect("空模型目录 + 单段 FQN 应 Ok");
    assert!(
        model_of(&plan.write.content).element("App").is_some(),
        "空模型目录下新建成功：
{}",
        plan.write.content
    );

    let emoji = TempDir::new("plan-emoji");
    emoji.write_model(
        "01-core.toml",
        "[element.\"App.🚀Auth\"]\nkind = \"module\"\ngate = \"off\"\n",
    );
    let plan = plan_element_upsert(emoji.root(), &gate_spec("App.🚀Auth", GateTier::Warn))
        .expect("既有 emoji 元素应 Ok");
    assert!(plan.write.content.contains("gate = \"warn\""));
}

/// 异常（矩阵：4 入口）：结构诊断前置——四个 `plan_*` 全 `Err(ModelHasDiagnostics)`。
#[test]
fn 结构错模型四拒() {
    let temp = TempDir::new("plan-diagnostics");
    temp.write_model("01-bad.toml", DIAGNOSTIC_SOURCE);

    let failures = [
        plan_element_upsert(temp.root(), &gate_spec("App", GateTier::Strict))
            .expect_err("元素 upsert 应拒"),
        plan_element_delete(temp.root(), "App", false, None).expect_err("元素删除应拒"),
        plan_relation_upsert(temp.root(), "App", "App", None, None, None)
            .expect_err("关系 upsert 应拒"),
        plan_relation_delete(temp.root(), "App", "App", None).expect_err("关系删除应拒"),
    ];
    for failure in &failures {
        assert_eq!(failure.kind, EditErrorKind::ModelHasDiagnostics);
        assert_eq!(
            failure.kind.criterion(),
            "模型存在结构诊断，拒绝写回",
            "判据句操作中立"
        );
        assert!(
            failure.facts.contains("invalidValue"),
            "facts 含诊断种类：{}",
            failure.facts
        );
        assert!(
            failure.facts.contains("先修好模型"),
            "facts 给出路：{}",
            failure.facts
        );
    }
}

// ---------------------------------------------------------------------------
// `plan_element_delete`
// ---------------------------------------------------------------------------

/// 正向：无从属叶元素 ⇒ `Ok`，content 不含该元素、其余逐字、表头计数下降、hash 自洽。
#[test]
fn 删除叶元素成功且计数随动() {
    let temp = TempDir::new("plan-delete-leaf");
    temp.write_model("01-core.toml", DELETE_SOURCE);
    let plan = plan_element_delete(temp.root(), "App.Auth", false, None).expect("叶元素删除应 Ok");
    assert!(
        !plan.write.content.contains("[element.\"App.Auth\"]"),
        "目标元素离场：\n{}",
        plan.write.content
    );
    assert!(
        plan.write
            .content
            .contains("[element.\"App.Billing\"]\nkind = \"module\""),
        "其余元素逐字保留"
    );
    assert_eq!(
        header_counts(&plan.write.content),
        Some((2, 1)),
        "表头计数随动下降"
    );
    assert_eq!(
        plan.write.hash,
        file_hash_of(&plan.write.file, &plan.write.content),
        "hash 与 content 自洽"
    );
}

/// 异常：元素有已声明的直接子元素 ⇒ `DeleteHasChildren`，facts 列出子元素 FQN。
#[test]
fn 有子元素拒绝删除() {
    let temp = TempDir::new("plan-delete-children");
    temp.write_model(
        "01-core.toml",
        concat!(
            "[element.\"App\"]\n",
            "kind = \"package\"\n",
            "\n",
            "[element.\"App.Auth\"]\n",
            "kind = \"domain\"\n",
            "\n",
            "[element.\"App.Auth.Token\"]\n",
            "kind = \"module\"\n",
        ),
    );
    let failure = plan_element_delete(temp.root(), "App", false, None).expect_err("有子元素应拒");
    assert_eq!(failure.kind, EditErrorKind::DeleteHasChildren);
    assert!(
        failure.facts.contains("App.Auth"),
        "facts 点名列出子元素：{}",
        failure.facts
    );
}

/// 异常：元素仍被出边引用 ⇒ `DeleteHasRelations`，facts 点名关系对。
#[test]
fn 出边引用拒绝删除() {
    let temp = TempDir::new("plan-delete-out-edge");
    temp.write_model("01-core.toml", TWO_ELEMENTS_SOURCE);
    let failure = plan_element_delete(temp.root(), "A", false, None).expect_err("出边引用应拒");
    assert_eq!(failure.kind, EditErrorKind::DeleteHasRelations);
    assert!(
        failure.facts.contains("A → B"),
        "facts 点名关系对：{}",
        failure.facts
    );
}

/// 异常：元素仍被入边引用 ⇒ `DeleteHasRelations`（出 / 入两臂同源）。
#[test]
fn 入边引用拒绝删除() {
    let temp = TempDir::new("plan-delete-in-edge");
    temp.write_model("01-core.toml", TWO_ELEMENTS_SOURCE);
    let failure = plan_element_delete(temp.root(), "B", false, None).expect_err("入边引用应拒");
    assert_eq!(failure.kind, EditErrorKind::DeleteHasRelations);
    assert!(
        failure.facts.contains("A → B"),
        "facts 点名关系对：{}",
        failure.facts
    );
}

/// 异常：元素缺席 ⇒ `ElementNotFound`，facts 为「`{fqn}（模型共 N 个元素）`」。
#[test]
fn 模型面缺席报元素总数() {
    let temp = TempDir::new("plan-delete-absent");
    temp.write_model("01-core.toml", TWO_ELEMENTS_SOURCE);
    let failure =
        plan_element_delete(temp.root(), "App.Missing", false, None).expect_err("元素缺席应拒");
    assert_eq!(failure.kind, EditErrorKind::ElementNotFound);
    assert!(
        failure.facts.contains("App.Missing（模型共 2 个元素）"),
        "facts 为模型面形态：{}",
        failure.facts
    );
}

/// 异常：删除面同样受乐观锁保护（零文本产出）。
#[test]
fn 删除面同样受乐观锁保护() {
    let temp = TempDir::new("plan-delete-expect");
    temp.write_model("01-core.toml", DELETE_SOURCE);
    let failure = plan_element_delete(temp.root(), "App.Auth", false, Some("sha256:错值"))
        .expect_err("expect 不匹配应拒");
    assert_eq!(failure.kind, EditErrorKind::ExpectMismatch);
    assert!(
        failure.facts.contains("期望 sha256:错值"),
        "{}",
        failure.facts
    );
}

/// 边界（矩阵：2 形态）：删除判据次序固定——结构诊断先于一切，子元素先于关系。
#[test]
fn 删除判据次序固定() {
    let diagnostic = TempDir::new("plan-delete-order-diagnostic");
    diagnostic.write_model(
        "01-bad.toml",
        concat!(
            "[element.\"App\"]\n",
            "kind = \"package\"\n",
            "path = [\"a\", \"b\"]\n",
            "\n",
            "[element.\"App.Auth\"]\n",
            "kind = \"domain\"\n",
        ),
    );
    let failure =
        plan_element_delete(diagnostic.root(), "App", false, None).expect_err("结构诊断先行");
    assert_eq!(failure.kind, EditErrorKind::ModelHasDiagnostics);

    let both = TempDir::new("plan-delete-order-both");
    both.write_model(
        "01-core.toml",
        concat!(
            "[element.\"App\"]\n",
            "kind = \"package\"\n",
            "\n",
            "[element.\"App.Auth\"]\n",
            "kind = \"domain\"\n",
            "\n",
            "[element.\"Other\"]\n",
            "kind = \"module\"\n",
            "\n",
            "[[relation]]\n",
            "from = \"Other\"\n",
            "to = \"App\"\n",
        ),
    );
    let failure = plan_element_delete(both.root(), "App", false, None).expect_err("子元素判据先");
    assert_eq!(failure.kind, EditErrorKind::DeleteHasChildren);
}

/// 边界：删除模型唯一元素 ⇒ `Ok`、回读元素数 0；其余文件字节零变化。
#[test]
fn 删除唯一元素后模型可解析() {
    let temp = TempDir::new("plan-delete-only");
    temp.write_model("01-only.toml", "[element.\"App\"]\nkind = \"package\"\n");
    let keep = temp.write_model("02-note.toml", "# 本文件暂无声明\n");
    let keep_before = fs::read(&keep).expect("读旁文件");

    let plan = plan_element_delete(temp.root(), "App", false, None).expect("删唯一元素应 Ok");
    assert_eq!(
        model_of(&plan.write.content).elements.len(),
        0,
        "回读元素数 0"
    );
    assert_eq!(
        fs::read(&keep).expect("读旁文件"),
        keep_before,
        "其余文件零变化"
    );
}

// ---------------------------------------------------------------------------
// `plan_relation_upsert` / `plan_relation_delete`
// ---------------------------------------------------------------------------

/// 正向：跨文件模型新增关系 ⇒ 落点 = `from` 元素声明文件，落盘后该关系的 `file` 命中它。
#[test]
fn 新关系落from元素所在文件() {
    let temp = TempDir::new("plan-relation-target");
    let core = temp.write_model("01-core.toml", "[element.\"App\"]\nkind = \"package\"\n");
    let auth = temp.write_model(
        "02-auth.toml",
        concat!(
            "[element.\"App.Auth\"]\n",
            "kind = \"domain\"\n",
            "\n",
            "[element.\"App.Token\"]\n",
            "kind = \"module\"\n",
        ),
    );
    let core_before = fs::read(&core).expect("读 core");

    let plan = plan_relation_upsert(
        temp.root(),
        "App.Auth",
        "App",
        Some(Some("入口")),
        None,
        None,
    )
    .expect("新增关系应 Ok");
    assert_eq!(plan.write.file, auth, "落点 = from 元素声明文件");
    fs::write(&plan.write.file, &plan.write.content).expect("模拟壳层落盘");

    let outcome = parse::parse_dir(temp.root());
    assert!(
        outcome.diagnostics.is_empty(),
        "落盘后零诊断：{:?}",
        outcome.diagnostics
    );
    let relation = outcome
        .model
        .relations
        .iter()
        .find(|relation| relation.from == "App.Auth" && relation.to == "App")
        .expect("关系在案");
    assert_eq!(
        relation.file, ".dev-team/architecture/graph/model/02-auth.toml",
        "modelFile 命中 from 所在文件"
    );
    assert_eq!(
        fs::read(&core).expect("读 core"),
        core_before,
        "非落点文件零变化"
    );
}

/// 正向：既有 `(from, to)` 改 `desc` ⇒ `Ok`、条数不变、`description` 为新值。
#[test]
fn 既有关系对走更新() {
    let temp = TempDir::new("plan-relation-update");
    temp.write_model("01-core.toml", RELATION_PLAN_SOURCE);
    let plan = plan_relation_upsert(temp.root(), "A", "B", Some(Some("新")), None, None)
        .expect("更新应 Ok");
    let model = model_of(&plan.write.content);
    assert_eq!(model.relations.len(), 2, "条数不变");
    assert_eq!(
        relation_of(&model, "A", "B").description.as_deref(),
        Some("新")
    );
}

/// 正向（组合：`relation_hits` 的 `file` → `apply_relation_upsert`）：`from` 元素声明在
/// `01-app.toml`、`(from, to)` 声明在 `06-frontend.toml`（`from` 元素文件 ≠ 关系文件）⇒
/// 落点 = 关系的声明文件，只改该条 `desc`，`from` 元素文件零变化、回读条数不变。
#[test]
fn 既有关系声明于非from文件时原地更新() {
    let temp = TempDir::new("plan-relation-split-upsert");
    let app = temp.write_model("01-app.toml", SPLIT_FROM_SOURCE);
    let frontend = temp.write_model("06-frontend.toml", SPLIT_RELATION_SOURCE);
    let app_before = fs::read(&app).expect("读 01-app");

    let plan = plan_relation_upsert(
        temp.root(),
        "Desktop",
        "Desktop.Components.Ui",
        Some(Some("新说明")),
        None,
        None,
    )
    .expect("既有关系更新应 Ok");

    assert_eq!(
        plan.write.file, frontend,
        "落点 = 关系声明文件（不是 from 元素文件）"
    );
    assert_eq!(
        plan.write.content,
        SPLIT_RELATION_SOURCE.replace("旧说明", "新说明"),
        "只改该条 desc，其余逐字"
    );
    assert_eq!(
        fs::read(&app).expect("读 01-app"),
        app_before,
        "from 元素文件零变化"
    );

    let model = model_of(&plan.write.content);
    assert_eq!(
        model.relations.len(),
        2,
        "回读关系条数不变（更新而非追加副本）"
    );
    assert_eq!(
        relation_of(&model, "Desktop", "Desktop.Components.Ui")
            .description
            .as_deref(),
        Some("新说明")
    );

    fs::write(&plan.write.file, &plan.write.content).expect("模拟壳层落盘");
    let outcome = parse::parse_dir(temp.root());
    assert!(
        outcome.diagnostics.is_empty(),
        "落盘后零诊断：{:?}",
        outcome.diagnostics
    );
    let relation = relation_of(&outcome.model, "Desktop", "Desktop.Components.Ui");
    assert!(
        relation.file.ends_with("06-frontend.toml"),
        "modelFile 仍是关系声明文件：{}",
        relation.file
    );
}

/// 异常（矩阵：3 形态）：端点缺席点名缺失端。
#[test]
fn 端点缺席点名缺失端() {
    let temp = TempDir::new("plan-relation-endpoints");
    temp.write_model("01-core.toml", TWO_ELEMENTS_SOURCE);

    for (from, to) in [("A", "Missing"), ("Missing", "B")] {
        let failure = plan_relation_upsert(temp.root(), from, to, None, None, None)
            .expect_err("端点缺席应拒");
        assert_eq!(failure.kind, EditErrorKind::RelationEndpointMissing);
        assert!(
            failure.facts.contains(&format!("{from} → {to}")) && failure.facts.contains("未声明"),
            "facts 点名目标对与缺失端：{}",
            failure.facts
        );
        assert!(
            failure.facts.contains("Missing"),
            "facts 点名缺失端 FQN：{}",
            failure.facts
        );
    }

    let failure = plan_relation_upsert(temp.root(), "Missing", "Also", None, None, None)
        .expect_err("双端点缺席应拒");
    assert!(
        failure.facts.contains("Missing") && failure.facts.contains("Also"),
        "facts 点名两端：{}",
        failure.facts
    );
}

/// 异常（矩阵：2 臂）：关系 upsert 的两道前置（`expect` / 结构诊断）均 `Err`。
#[test]
fn 关系upsert两前置均err() {
    let temp = TempDir::new("plan-relation-expect");
    temp.write_model("01-core.toml", TWO_ELEMENTS_SOURCE);
    let failure = plan_relation_upsert(temp.root(), "A", "B", None, None, Some("sha256:错值"))
        .expect_err("expect 不匹配应拒");
    assert_eq!(failure.kind, EditErrorKind::ExpectMismatch);

    let diagnostic = TempDir::new("plan-relation-diagnostic");
    diagnostic.write_model("01-bad.toml", DIAGNOSTIC_SOURCE);
    let failure = plan_relation_upsert(diagnostic.root(), "A", "B", None, None, None)
        .expect_err("结构诊断应拒");
    assert_eq!(failure.kind, EditErrorKind::ModelHasDiagnostics);
}

/// 边界（矩阵：2 臂）：零关系文件新建条目数组；`apis = Some(&[])` ⇒ 落盘文本零该键。
#[test]
fn 零关系文件新建数组与apis清空() {
    let temp = TempDir::new("plan-relation-zero");
    temp.write_model("01-core.toml", TWO_ELEMENTS_SOURCE);

    let plan = plan_relation_upsert(temp.root(), "B", "A", None, None, None)
        .expect("零关系文件新建数组应 Ok");
    assert!(
        plan.write.content.contains("[[relation]]"),
        "新建条目数组：\n{}",
        plan.write.content
    );

    let empty: Vec<String> = Vec::new();
    let plan = plan_relation_upsert(temp.root(), "B", "A", None, Some(empty.as_slice()), None)
        .expect("apis 清空臂应 Ok");
    assert!(
        !plan.write.content.contains("apis"),
        "apis 键不落盘（零空数组）：\n{}",
        plan.write.content
    );
}

/// 正向：在场关系删除 ⇒ 回读关系数 −1、表头计数随动、其余条目逐字。
#[test]
fn 关系删除成功计数随动() {
    let temp = TempDir::new("plan-relation-delete");
    temp.write_model("01-core.toml", RELATION_PLAN_SOURCE);
    let plan = plan_relation_delete(temp.root(), "A", "B", None).expect("删除应 Ok");
    let model = model_of(&plan.write.content);
    assert_eq!(model.relations.len(), 1, "关系数 −1");
    assert!(
        model
            .relations
            .iter()
            .any(|relation| relation.from == "B" && relation.to == "A"),
        "其余条目逐字保留"
    );
    assert_eq!(
        header_counts(&plan.write.content),
        Some((2, 1)),
        "表头计数随动"
    );
}

/// 异常（矩阵：2 形态）：关系缺席 ⇒ `RelationNotFound`；端点缺席先于关系缺席。
#[test]
fn 关系删除的双前置() {
    let temp = TempDir::new("plan-relation-delete-pre");
    temp.write_model("01-core.toml", TWO_ELEMENTS_SOURCE);

    let failure = plan_relation_delete(temp.root(), "A", "A", None).expect_err("关系缺席应拒");
    assert_eq!(failure.kind, EditErrorKind::RelationNotFound);
    assert!(
        failure.facts.contains("A → A") && failure.facts.contains("现有关系：A → B"),
        "模型面 facts 列该 from 元素的出边对（不读 from 文件后由 apply 报「该文件没有关系」）：{}",
        failure.facts
    );

    let failure =
        plan_relation_delete(temp.root(), "A", "Missing", None).expect_err("端点缺席应拒");
    assert_eq!(
        failure.kind,
        EditErrorKind::RelationEndpointMissing,
        "端点判据先于关系判据：{}",
        failure.message()
    );
}

/// 异常（矩阵：2 入口 + 1 对照）：同一 `(from, to)` 在 `01-app.toml` / `06-frontend.toml`
/// 各声明一次 ⇒ 两函数均 `RelationDuplicated`（模型面 facts 点名两文件与处数）、零文本
/// 产出；单处声明不报该错（走各自落点规则）。
#[test]
fn 模型面重复声明fail_closed() {
    let temp = TempDir::new("plan-relation-duplicated");
    temp.write_model("01-app.toml", DUPLICATED_APP_SOURCE);
    temp.write_model("06-frontend.toml", DUPLICATED_FRONTEND_SOURCE);
    let before = temp.snapshot();

    let upsert_failure = plan_relation_upsert(temp.root(), "A", "B", Some(Some("改")), None, None)
        .expect_err("模型面两处 upsert 应拒");
    let delete_failure =
        plan_relation_delete(temp.root(), "A", "B", None).expect_err("模型面两处 delete 应拒");

    for failure in [&upsert_failure, &delete_failure] {
        assert_eq!(failure.kind, EditErrorKind::RelationDuplicated);
        assert!(
            failure.facts.contains("A → B 在 "),
            "facts 点名目标对：{}",
            failure.facts
        );
        for file in ["01-app.toml", "06-frontend.toml"] {
            assert!(
                failure.facts.contains(file),
                "facts 点名声明文件 {file}：{}",
                failure.facts
            );
        }
        assert!(
            failure.facts.contains("多处声明（2 处）"),
            "facts 点名处数：{}",
            failure.facts
        );
        assert!(
            failure.facts.contains("先删除重复条目"),
            "facts 给出下一步：{}",
            failure.facts
        );
    }
    assert_eq!(temp.snapshot(), before, "两臂均零写盘（零文本产出）");

    let single = TempDir::new("plan-relation-single");
    let app = single.write_model("01-app.toml", DUPLICATED_APP_SOURCE);
    let plan = plan_relation_upsert(single.root(), "A", "B", Some(Some("唯一")), None, None)
        .expect("单处声明应 Ok");
    assert_eq!(plan.write.file, app, "走落点规则（= 关系声明文件）");
    assert_eq!(
        relation_of(&model_of(&plan.write.content), "A", "B")
            .description
            .as_deref(),
        Some("唯一")
    );
}

/// 边界（矩阵：2 形态）：删除落在**关系声明文件**（可以不是 `from` 元素文件），表头
/// 计数随动、其余条目逐字；对照臂——关系声明在 `from` 元素文件内时落点仍是该文件。
#[test]
fn 既有关系声明于非from文件时删除落声明文件() {
    let temp = TempDir::new("plan-relation-split-delete");
    let app = temp.write_model("01-app.toml", SPLIT_FROM_SOURCE);
    let frontend = temp.write_model("06-frontend.toml", SPLIT_RELATION_SOURCE);
    let app_before = fs::read(&app).expect("读 01-app");

    let plan = plan_relation_delete(temp.root(), "Desktop", "Desktop.Components.Ui", None)
        .expect("既有关系删除应 Ok");
    assert_eq!(plan.write.file, frontend, "落点 = 关系声明文件");
    assert_eq!(
        fs::read(&app).expect("读 01-app"),
        app_before,
        "from 元素文件零变化"
    );

    let model = model_of(&plan.write.content);
    assert_eq!(model.relations.len(), 1, "关系数 −1");
    assert!(
        !model
            .relations
            .iter()
            .any(|relation| relation.from == "Desktop" && relation.to == "Desktop.Components.Ui"),
        "命中条目离场"
    );
    assert!(
        model
            .relations
            .iter()
            .any(|relation| relation.from == "Desktop" && relation.to == "Desktop.Components"),
        "其余条目逐字保留"
    );
    assert_eq!(
        header_counts(&plan.write.content),
        Some((2, 1)),
        "表头计数随动"
    );
    assert!(
        model.element("Desktop.Components.Ui").is_some(),
        "元素表零改动"
    );

    let local = TempDir::new("plan-relation-same-file-delete");
    let core = local.write_model("01-core.toml", RELATION_PLAN_SOURCE);
    let plan = plan_relation_delete(local.root(), "A", "B", None).expect("同文件删除应 Ok");
    assert_eq!(
        plan.write.file, core,
        "落点 = 声明文件（与 from 元素同文件）"
    );
    assert_eq!(model_of(&plan.write.content).relations.len(), 1);
}

/// 边界（矩阵：4 形态）：关系身份键 = `from` / `to` **双等**（与 `diff::model_diff` /
/// `relation::relation_detail` 同口径），各命中条目携带其声明文件。
#[test]
fn 关系身份键双等且带落点() {
    let temp = TempDir::new("relation-hits");
    temp.write_model("01-app.toml", IDENTITY_APP_SOURCE);
    temp.write_model("06-frontend.toml", IDENTITY_FRONTEND_SOURCE);
    let model = parse::parse_dir(temp.root()).model;

    let pair = relation_hits(&model, "A", "B");
    assert_eq!(pair.len(), 2, "同一对跨两文件各成一命");
    let mut files: Vec<&str> = pair.iter().map(|relation| relation.file.as_str()).collect();
    files.sort_unstable();
    assert!(
        files[0].ends_with("01-app.toml"),
        "命中条目带各自声明文件：{files:?}"
    );
    assert!(
        files[1].ends_with("06-frontend.toml"),
        "命中条目带各自声明文件：{files:?}"
    );

    let same_from = relation_hits(&model, "A", "C");
    assert_eq!(same_from.len(), 1, "同 from 不同 to 只命中目标对");
    assert!(same_from[0].file.ends_with("06-frontend.toml"));

    let same_to = relation_hits(&model, "C", "B");
    assert_eq!(same_to.len(), 1, "同 to 不同 from 只命中目标对");

    assert!(relation_hits(&model, "B", "C").is_empty(), "零命中");
}

/// 异常（矩阵：2 / 1 / 0 三形态）：模型面唯一性判据与 `apply_*` 的文件面判据同一
/// `EditErrorKind`（不引第三处）。
#[test]
fn 模型面唯一性两臂() {
    let temp = TempDir::new("check-relation-unique");
    temp.write_model("01-app.toml", IDENTITY_APP_SOURCE);
    temp.write_model("06-frontend.toml", IDENTITY_FRONTEND_SOURCE);
    let model = parse::parse_dir(temp.root()).model;

    let duplicated =
        check_relation_unique(&relation_hits(&model, "A", "B")).expect_err("跨两文件两处应拒");
    assert_eq!(duplicated.kind, EditErrorKind::RelationDuplicated);
    assert!(
        duplicated.facts.contains("多处声明（2 处）"),
        "facts 点名处数：{}",
        duplicated.facts
    );

    let file_face =
        apply_relation_upsert(DUPLICATED_IN_SOURCE, "A", "B", None, None).expect_err("同源两处");
    assert_eq!(
        file_face.kind, duplicated.kind,
        "模型面与文件面同一判据（不引第三处）"
    );

    assert!(
        check_relation_unique(&relation_hits(&model, "A", "C")).is_ok(),
        "唯一命中不报"
    );
    assert!(
        check_relation_unique(&relation_hits(&model, "C", "B")).is_ok(),
        "唯一命中不报"
    );
    assert!(
        check_relation_unique(&relation_hits(&model, "B", "C")).is_ok(),
        "零命中不报"
    );
}

// ---------------------------------------------------------------------------
// 类型面
// ---------------------------------------------------------------------------

/// 正向：`ModelWrite` 三元组逐字段可读，`PartialEq` / `Eq` 按三字段成立。
#[test]
#[allow(non_snake_case)]
fn ModelWrite三元组可比() {
    let base = ModelWrite {
        file: PathBuf::from("/root/01-core.toml"),
        content: "a = 1\n".to_owned(),
        hash: "sha256:aa".to_owned(),
    };
    assert_eq!(base, base.clone(), "同三元组相等");
    assert!(!format!("{base:?}").is_empty(), "Debug 可用");

    let other_content = ModelWrite {
        content: "a = 2\n".to_owned(),
        ..base.clone()
    };
    assert_ne!(base, other_content, "content 差一字节即不等");
    let other_hash = ModelWrite {
        hash: "sha256:bb".to_owned(),
        ..base.clone()
    };
    assert_ne!(base, other_hash, "hash 差一即不等");
    let other_file = ModelWrite {
        file: PathBuf::from("/root/02-core.toml"),
        ..base.clone()
    };
    assert_ne!(base, other_file, "file 差一即不等");
}

/// 边界：`ElementSpec::default()` 全字段缺省且不 panic；`..Default::default()` 简写可
/// 构造壳层同形 spec。
#[test]
#[allow(non_snake_case)]
fn ElementSpec缺省可构造且不panic() {
    let spec = ElementSpec::default();
    assert!(spec.fqn.is_empty(), "fqn 缺省空串");
    assert!(spec.kind.is_none());
    assert!(spec.gate.is_none());
    assert!(spec.api.is_none());
    assert!(spec.entry.is_none());
    assert!(spec.path.is_none());
    assert!(spec.doc.is_none());
    assert!(spec.summary.is_none());
    assert!(spec.metadata.is_none());
    assert!(spec.file.is_none());
    assert!(spec.expect.is_none());

    let failure = apply_element_spec(CORE_SOURCE, &spec).expect_err("缺省 spec 应 Err");
    assert_eq!(
        failure.kind,
        EditErrorKind::FqnInvalid,
        "按判据次序先判 FQN 形态：{}",
        failure.message()
    );

    let shell_shaped = ElementSpec {
        fqn: "App.Auth".to_owned(),
        gate: Some(GateTier::Warn),
        ..Default::default()
    };
    assert_eq!(shell_shaped.gate, Some(GateTier::Warn));
    assert!(shell_shaped.entry.is_none(), "其余位沿用缺省");
}

// ---------------------------------------------------------------------------
// 表头回填（私有 helper）
// ---------------------------------------------------------------------------

/// 正向：表头两个数字按重解析结果回填，括号外整行逐字（含全角 `）` 与人工标题）。
#[test]
fn 表头两数按重解析回填() {
    let source = concat!(
        "# 01 · 核心（9 元素 / 10 关系）\n",
        "[element.\"App\"]\n",
        "kind = \"package\"\n",
    );
    let backfilled = backfill_header_counts(source, 8, 10);
    assert_eq!(
        backfilled.lines().next().expect("首行"),
        "# 01 · 核心（8 元素 / 10 关系）",
        "括号外整行逐字"
    );
    assert!(
        backfilled.ends_with("[element.\"App\"]\nkind = \"package\"\n"),
        "正文零改动：\n{backfilled}"
    );
}

/// 边界（回归臂）：表头被回填后，首部注释块之后的正文**逐字保留**——回填的拼接域是整份
/// 渲染文本，只收窄到首部块会把 `[element."…"]` / `[[relation]]` 全部截掉且自证仍是零诊断。
#[test]
fn 回填只替换两段数字且正文逐字保留() {
    let source = concat!(
        "# 06 · frontend（9 元素 / 10 关系）\n",
        "\n",
        "[element.\"Desktop.Components\"]\n",
        "kind = \"module\"\n",
        "\n",
        "[element.\"Desktop.Components.Ui\"]\n",
        "kind = \"module\"\n",
        "\n",
        "[[relation]]\n",
        "from = \"Desktop\"\n",
        "to = \"Desktop.Components.Ui\"\n",
    );
    let before = model_of(source);
    let backfilled = backfill_header_counts(source, before.elements.len(), before.relations.len());

    assert_eq!(
        backfilled,
        source.replace("（9 元素 / 10 关系）", "（2 元素 / 1 关系）"),
        "只替换两段数字"
    );
    assert_eq!(
        changed_lines(source, &backfilled),
        vec![0],
        "按行 diff 恰首行"
    );

    let body = |text: &str| text[text.find('[').expect("首部块之后有正文")..].to_owned();
    assert_eq!(body(&backfilled), body(source), "正文逐字保留（不截断）");

    let after = model_of(&backfilled);
    let fqns = |model: &Model| {
        model
            .elements
            .iter()
            .map(|element| element.fqn.clone())
            .collect::<Vec<String>>()
    };
    assert_eq!(fqns(&after), fqns(&before), "元素集合与改前相同");
    assert_eq!(
        after.relations.len(),
        before.relations.len(),
        "关系集合与改前相同"
    );
    assert_eq!(
        after.relations[0].to, "Desktop.Components.Ui",
        "关系条目逐字保留"
    );
}

/// 正向：计数准确 ⇒ 原样返回（与输入逐字节相等）。
#[test]
fn 计数准确即零差异() {
    let source = "# 02 · core 分层（3 元素 / 1 关系）\n[element.\"App\"]\n";
    assert_eq!(backfill_header_counts(source, 3, 1), source);
}

/// 异常（矩阵：3 形态）：无该括号不新增；块外同形文本与块内第二处同形文本零改动。
#[test]
fn 无括号不新增且块外零改动() {
    let plain = "# 01 · 核心\n[element.\"App\"]\nkind = \"package\"\n";
    assert_eq!(backfill_header_counts(plain, 7, 9), plain, "不新增括号");

    let outside = concat!(
        "# 01 · 核心\n",
        "[element.\"App\"]\n",
        "kind = \"package\"\n",
        "summary = \"（9 元素 / 10 关系）\"\n",
    );
    assert_eq!(
        backfill_header_counts(outside, 1, 0),
        outside,
        "首部注释块之外的同形文本零改动"
    );

    let second = concat!(
        "# 01 · 核心（9 元素 / 10 关系）\n",
        "# 附注（9 元素 / 10 关系）\n",
        "[element.\"App\"]\n",
        "kind = \"package\"\n",
    );
    let backfilled = backfill_header_counts(second, 1, 0);
    assert!(
        backfilled.contains("# 01 · 核心（1 元素 / 0 关系）"),
        "第一处回填：\n{backfilled}"
    );
    assert!(
        backfilled.contains("# 附注（9 元素 / 10 关系）"),
        "块内第二处零改动：\n{backfilled}"
    );
}

/// 边界（矩阵：位数 / CRLF / 首部形态 / 无尾换行）：各形态只替换两段数字。
#[test]
fn 表头数字位数与换行边界() {
    let grown = "# 01 · 核心（9 元素 / 9 关系）\n[element.\"App\"]\n";
    assert!(
        backfill_header_counts(grown, 10, 10).contains("（10 元素 / 10 关系）"),
        "位数增加"
    );

    let shrunk = "# 01 · 核心（100 元素 / 100 关系）\n[element.\"App\"]\n";
    assert!(
        backfill_header_counts(shrunk, 99, 99).contains("（99 元素 / 99 关系）"),
        "位数减少"
    );

    let crlf = "# 01 · 核心（3 元素 / 1 关系）\r\n[element.\"App\"]\r\nkind = \"package\"\r\n";
    let backfilled = backfill_header_counts(crlf, 4, 2);
    assert!(
        backfilled.contains("（4 元素 / 2 关系）\r\n"),
        "CRLF 首部只换两段数字：{backfilled:?}"
    );
    assert!(
        backfilled.contains("[element.\"App\"]\r\nkind = \"package\"\r\n"),
        "CRLF 正文零改动"
    );

    let mixed = "\n# 01 · 核心（3 元素 / 1 关系）\n\n[element.\"App\"]\n";
    let backfilled = backfill_header_counts(mixed, 5, 6);
    assert!(
        backfilled.contains("（5 元素 / 6 关系）"),
        "空行 + 注释混合首部"
    );
    assert_eq!(
        backfilled.matches("\n\n[element.").count(),
        1,
        "空行与正文零改动：{backfilled:?}"
    );

    let no_newline = "# 01 · 核心（3 元素 / 1 关系）";
    assert_eq!(
        backfill_header_counts(no_newline, 4, 0),
        "# 01 · 核心（4 元素 / 0 关系）",
        "无尾换行形态"
    );
}

// ---------------------------------------------------------------------------
// 自证链（私有 helper）
// ---------------------------------------------------------------------------

/// 正向（源码锚）：先渲染取计数 → 回填 → 对交付文本自证；三个私有件各恰一处且非 `pub`。
#[test]
fn 自证链步骤序固定() {
    let source = include_str!("mod.rs");
    let first = source
        .find("TomlParser::parse(&rendered)")
        .expect("首次重解析取计数");
    let second = source
        .find("TomlParser::parse(&content)")
        .expect("交付文本自证");
    let backfill = source
        .find("backfill_header_counts(")
        .expect("回填调用在案");
    assert!(
        first < backfill && backfill < second,
        "回填夹在两次重解析之间（先取计数 → 回填 → 对交付文本自证）"
    );
    assert_eq!(
        source.matches("TomlParser::parse").count(),
        5,
        "五处重解析：自证两处 + 写后计数一处 + 新文件表头两处"
    );
    for private in ["fn parse_source", "fn finish(", "fn backfill_header_counts"] {
        assert_eq!(source.matches(private).count(), 1, "{private} 恰一处");
        assert!(
            !source.contains(&format!("pub {private}")),
            "{private} 非 pub"
        );
    }
}

/// 异常（矩阵：2 臂）：自证失败与回读失败均零文本产出。
#[test]
fn 自证与回读失败均零文本() {
    let failure = apply_element_spec(
        "[element.\"App\"]\nkind = \"package\"\n",
        &ElementSpec {
            fqn: "App".to_owned(),
            path: Some(vec!["a".to_owned(), "b".to_owned()]),
            ..Default::default()
        },
    )
    .expect_err("越界值应自证失败");
    assert_eq!(failure.kind, EditErrorKind::RenderSelfCheck);
    assert!(
        failure.facts.contains("未产出文本"),
        "facts 明写零文本：{}",
        failure.facts
    );

    let document = "[element.\"App\"]\nkind = \"package\"\n"
        .parse::<DocumentMut>()
        .expect("fixture 应可解析");
    let failure = finish(document, |_| {
        Err(messages::error(
            EditErrorKind::ReadBackMismatch,
            "注入回读不一致",
        ))
    })
    .expect_err("回读失败应原样传出");
    assert_eq!(failure.kind, EditErrorKind::ReadBackMismatch);
    assert_eq!(failure.facts, "注入回读不一致", "无降级路径");
}

/// 边界：陈旧表头 + 删除 ⇒ 交付文本的表头两数 == 对交付文本重解析的计数。
#[test]
fn 回填后计数等于回读计数() {
    let stale = concat!(
        "# 01 · 核心（9 元素 / 0 关系）\n",
        "[element.\"App\"]\n",
        "kind = \"package\"\n",
        "\n",
        "[element.\"App.Auth\"]\n",
        "kind = \"domain\"\n",
        "\n",
        "[[relation]]\n",
        "from = \"App.Auth\"\n",
        "to = \"App\"\n",
    );
    let content = apply_element_delete(stale, &one("App.Auth")).expect("删除应成功");
    let model = model_of(&content);
    assert_eq!(
        header_counts(&content),
        Some((model.elements.len(), model.relations.len())),
        "回填后计数 == 回读计数：\n{content}"
    );
    assert_eq!(header_counts(&content), Some((1, 1)));
}

// ---------------------------------------------------------------------------
// 私有判据 helper
// ---------------------------------------------------------------------------

/// 异常（矩阵：7 拒 + 缺 kind + 合法臂）：新建判据的形态七拒与 `kind` 必填。
#[test]
fn 新建判据七拒与缺kind() {
    let rejects = [
        "",
        "App Auth",
        "App\"Auth",
        "App[",
        "App]",
        "App#x",
        "App\n.Auth",
    ];
    for fqn in rejects {
        let Err(failure) = check_create(fqn, Some(ElementKind::Module)) else {
            panic!("{fqn:?} 应被拒（写面要把它渲染成键）")
        };
        assert_eq!(failure.kind, EditErrorKind::FqnInvalid, "{fqn:?}");
        assert!(
            failure.facts.contains(&format!("{fqn:?}")),
            "facts 含形态：{}",
            failure.facts
        );
        assert!(
            failure.facts.contains("空白 / 引号 / 方括号 / # / 换行"),
            "facts 含字符清单：{}",
            failure.facts
        );
    }

    let failure = check_create("App.New", None).expect_err("缺 kind 应拒");
    assert_eq!(failure.kind, EditErrorKind::ElementKindMissing);
    assert!(
        failure.facts.contains("App.New") && failure.facts.contains("新建必须给出 kind"),
        "facts 点名 FQN 与缺 kind：{}",
        failure.facts
    );
    assert!(
        check_create("App.New", Some(ElementKind::Package)).is_ok(),
        "合法形态 + kind ⇒ Ok"
    );
}

/// 异常（矩阵：5 臂）：落点四情形 facts 逐条对齐。
#[test]
fn 落点四情形文案逐条对齐() {
    let temp = TempDir::new("check-target");
    fs::create_dir_all(archi_graph_model_dir(temp.root())).expect("建模型声明目录");
    let empty_model = Model::empty();
    let existing = Element {
        fqn: "App".to_owned(),
        kind: ElementKind::Package,
        gate: GateTier::Off,
        api: ApiStatus::Declared,
        entry: None,
        path: Vec::new(),
        doc: None,
        summary: None,
        metadata: BTreeMap::new(),
        file: ".dev-team/architecture/graph/model/01-core.toml".to_owned(),
        line: 1,
    };
    let mut with_parent = Model::empty();
    with_parent.elements.push(existing.clone());

    let failure = check_target(temp.root(), "Single", None, false, None, &empty_model)
        .expect_err("单段 FQN 省略 file 应拒");
    assert_eq!(failure.kind, EditErrorKind::TargetInvalid);
    assert!(failure.facts.contains("单段 FQN"), "{}", failure.facts);

    let failure = check_target(temp.root(), "App.New", None, false, None, &empty_model)
        .expect_err("父元素不在案应拒");
    assert_eq!(failure.kind, EditErrorKind::ParentMissing);
    assert!(failure.facts.contains("App"), "{}", failure.facts);

    for file in ["a/b.toml", "/abs.toml", "x.json"] {
        let failure = check_target(
            temp.root(),
            "App.New",
            Some(file),
            false,
            None,
            &with_parent,
        )
        .expect_err("非一级应拒");
        assert_eq!(failure.kind, EditErrorKind::TargetInvalid);
        assert!(
            failure.facts.contains("不在模型声明目录一级"),
            "{file} 的 facts：{}",
            failure.facts
        );
    }

    let bare = TempDir::new("check-target-bare");
    let failure = check_target(
        bare.root(),
        "App.New",
        Some("01-x.toml"),
        false,
        None,
        &with_parent,
    )
    .expect_err("目录缺席应拒");
    assert_eq!(failure.kind, EditErrorKind::TargetInvalid);
    assert!(
        failure.facts.contains("不存在") && failure.facts.contains("不代建目录"),
        "{}",
        failure.facts
    );

    let failure = check_target(
        temp.root(),
        "App",
        Some("02-other.toml"),
        false,
        Some(&existing),
        &with_parent,
    )
    .expect_err("改落点应拒");
    assert_eq!(failure.kind, EditErrorKind::TargetInvalid);
    assert!(
        failure.facts.contains("01-core.toml") && failure.facts.contains("迁文件属独立语义"),
        "{}",
        failure.facts
    );

    assert_eq!(
        check_target(
            temp.root(),
            "App",
            Some(&existing.file),
            false,
            Some(&existing),
            &with_parent,
        )
        .expect("既有元素同值落点应 Ok")
        .file,
        temp.root().join(&existing.file),
        "既有元素落其声明文件（repo 相对路径直挂 root）"
    );

    let failure = check_target(
        temp.root(),
        "App.New",
        Some("07-new.toml"),
        false,
        None,
        &with_parent,
    )
    .expect_err("目标文件缺席且无 create 应拒");
    assert_eq!(failure.kind, EditErrorKind::TargetInvalid);
    assert!(failure.facts.contains("create = true"), "{}", failure.facts);

    let fresh = check_target(
        temp.root(),
        "App.New",
        Some("07-new.toml"),
        true,
        None,
        &with_parent,
    )
    .expect("给 create ⇒ 新建落点 Ok");
    assert!(fresh.fresh, "目标文件缺席 ⇒ 新建文件（表头由计划层前置）");
    assert_eq!(
        fresh.file,
        archi_graph_model_dir(temp.root()).join("07-new.toml")
    );
}

/// 边界（矩阵：3 形态）：读源单点两态——允许缺席 ⇒ 空源；其余读失败 ⇒ `SourceReadFailed`。
#[test]
fn 读源单点两态() {
    let temp = TempDir::new("read-source");
    let missing = temp.root().join("no-such.toml");
    assert_eq!(
        read_source(&missing, true).expect("允许缺席 ⇒ 空源"),
        String::new()
    );

    let failure = read_source(&missing, false).expect_err("不允许缺席 ⇒ Err");
    assert_eq!(failure.kind, EditErrorKind::SourceReadFailed);
    assert!(
        failure.facts.contains("no-such.toml"),
        "facts 点名路径：{}",
        failure.facts
    );

    let bad = temp.root().join("bad.toml");
    fs::write(&bad, [0xff_u8, 0xfe, 0x80]).expect("写非 UTF-8 文件");
    let failure = read_source(&bad, true).expect_err("非 UTF-8 ⇒ Err");
    assert_eq!(failure.kind, EditErrorKind::SourceReadFailed);
    assert!(
        failure.facts.contains("bad.toml"),
        "facts 点名路径：{}",
        failure.facts
    );
}

/// 边界（矩阵：4 形态）：`expect` 校验四形态。
#[test]
fn expect校验四形态() {
    let temp = TempDir::new("check-expect");
    let path = temp.root().join("01-core.toml");
    fs::write(&path, CORE_SOURCE).expect("落源");
    let current = file_hash(&path);

    assert!(
        check_expect(&path, CORE_SOURCE, None).is_ok(),
        "None 不校验"
    );
    assert!(
        check_expect(&path, CORE_SOURCE, Some(&current)).is_ok(),
        "当前值匹配"
    );

    let failure = check_expect(&path, CORE_SOURCE, Some("sha256:错值")).expect_err("错值应 Err");
    assert_eq!(failure.kind, EditErrorKind::ExpectMismatch);
    assert!(
        failure.facts.contains("期望 sha256:错值")
            && failure.facts.contains("实际 sha256:")
            && failure.facts.contains("重读后再试"),
        "facts 含期望 / 实际 / 出路：{}",
        failure.facts
    );

    let absent = temp.root().join("no-such.toml");
    let failure = check_expect(&absent, "", Some("sha256:任意")).expect_err("文件缺席应 Err");
    assert_eq!(failure.kind, EditErrorKind::ExpectMismatch);
    assert!(
        failure.facts.contains("不存在") && failure.facts.contains("sha256:任意"),
        "facts 点名文件缺席：{}",
        failure.facts
    );
}

// ---------------------------------------------------------------------------
// 公开面 / 出口面 / 纪律锚
// ---------------------------------------------------------------------------

/// 异常（源码锚）：公开面恰 8 函数 + 2 类型，`messages` 为私有子模块（无第五路径）。
#[test]
fn 公开面恰八函数两类型且messages私有() {
    let source = include_str!("mod.rs");

    let public_functions: Vec<&str> = source
        .lines()
        .filter(|line| line.starts_with("pub fn "))
        .collect();
    assert_eq!(public_functions.len(), 8, "公开函数恰 8 个");
    for name in [
        "apply_element_spec",
        "apply_element_delete",
        "apply_relation_upsert",
        "apply_relation_delete",
        "plan_element_upsert",
        "plan_element_delete",
        "plan_relation_upsert",
        "plan_relation_delete",
    ] {
        assert!(
            source.contains(&format!("pub fn {name}(")),
            "{name} 在公开面"
        );
    }

    let public_types = source
        .lines()
        .filter(|line| line.starts_with("pub struct ") || line.starts_with("pub enum "))
        .count();
    assert_eq!(
        public_types, 8,
        "公开类型恰 8 个（ModelWrite / ElementSpec / WriteAction / RelationSpec / DeleteSet / WriteCounts / RelationTally / WriteOutcome）"
    );

    let public_uses: Vec<&str> = source
        .lines()
        .filter(|line| line.starts_with("pub use "))
        .collect();
    assert_eq!(
        public_uses,
        vec![
            "pub use messages::{EditError, EditErrorKind};",
            "pub use warnings::{delete_warnings, upsert_warnings, EditWarning};",
        ],
        "出口两块（判据族 + 反馈面）且形态固定"
    );
    assert!(
        source.lines().any(|line| line == "mod messages;"),
        "messages 为私有子模块"
    );
    assert!(
        !source.contains("pub mod messages"),
        "无 edit::messages::… 第五路径"
    );
}

/// 异常（源码锚）：`edit/` 零写盘、零子进程、零哈希拼装（写盘单点在壳层，哈希口径在
/// `drift`）。
#[test]
fn edit零写盘零哈希拼装() {
    for source in [include_str!("mod.rs"), include_str!("messages.rs")] {
        for forbidden in ["fs::write", "write_atomic", "spawn", "Sha256", "sha2"] {
            assert!(!source.contains(forbidden), "edit/ 内零 {forbidden}");
        }
    }
}

/// 正向（源码锚 + 编译期守卫）：crate 根 `edit` 出口含 12 个名字；本文件的 `use` 面
/// 经 crate 根导入，任一名字离场即编译失败。
#[test]
fn crate根出口含十二个edit名字() {
    let source = include_str!("../lib.rs");
    assert!(source.contains("pub use edit::{"), "edit 出口分组块在案");
    for name in [
        "apply_element_spec",
        "apply_element_delete",
        "apply_relation_upsert",
        "apply_relation_delete",
        "plan_element_upsert",
        "plan_element_delete",
        "plan_relation_upsert",
        "plan_relation_delete",
        "EditError",
        "EditErrorKind",
        "ElementSpec",
        "ModelWrite",
    ] {
        assert!(source.contains(name), "crate 根出口含 {name}");
    }
    assert_eq!(EditErrorKind::ALL.len(), 23, "经 crate 根可达的判据族完整");
}

/// 正向（源码锚）：crate 根 `drift` 出口三个既有名未丢 + 两个新名在场。
#[test]
fn crate根出口drift五名齐() {
    let source = include_str!("../lib.rs");
    assert!(source.contains("pub use drift::{"), "drift 出口分组块在案");
    for name in [
        "check",
        "CheckError",
        "CheckInput",
        "file_hash",
        "file_hash_of",
    ] {
        assert!(source.contains(name), "crate 根出口含 {name}");
    }
}

/// 异常（源码锚）：退役测试声明已离场，`edit` 模块登记仍在。
#[test]
fn edit_test声明已离场() {
    let source = include_str!("../lib.rs");
    assert!(!source.contains("mod edit_test"), "退役测试声明零命中");
    assert!(
        source.lines().any(|line| line == "pub mod edit;"),
        "edit 模块仍登记"
    );
}

/// 边界（源码锚）：crate 头注释改「写回原语族」，旧「`gate` 写回纯函数」表述零命中。
#[test]
fn crate头注释改原语族() {
    let source = include_str!("../lib.rs");
    assert!(
        source.contains("写回原语族（元素与关系的 upsert / delete）"),
        "头注释指向原语族"
    );
    assert!(!source.contains("`gate` 写回纯函数"), "旧表述零命中");
}

/// 正向：12 个 `edit` 名字经 **crate 根**可达——本文件顶部的 `use crate::{…}` 即编译期
/// 判据（任一名字离场即编译失败），此处再经每个原语各一次调用与四类型的使用钉住运行期
/// 可用性，并复核 19 值判据集合与两哈希函数形态。
#[test]
fn 十二名经crate根可达() {
    let spec = ElementSpec {
        fqn: "App".to_owned(),
        ..Default::default()
    };
    assert_eq!(
        apply_element_spec("", &spec)
            .expect_err("空源 + 缺 kind")
            .kind,
        EditErrorKind::ElementKindMissing,
        "apply_element_spec 经 crate 根可达"
    );
    assert_eq!(
        apply_element_delete("", &one("App"))
            .expect_err("无 element 表")
            .kind,
        EditErrorKind::ElementTableMissing,
        "apply_element_delete 经 crate 根可达"
    );
    for failure in [
        apply_relation_upsert("坏源 = [", "A", "B", None, None).expect_err("坏源"),
        apply_relation_delete("坏源 = [", "A", "B").expect_err("坏源"),
    ] {
        assert_eq!(
            failure.kind,
            EditErrorKind::SourceParseFailed,
            "关系原语两函数经 crate 根可达"
        );
    }

    let blank = Path::new("  ");
    assert!(
        plan_element_upsert(blank, &gate_spec("App", GateTier::Off)).is_err(),
        "plan_element_upsert 经 crate 根可达"
    );
    assert!(
        plan_element_delete(blank, "App", false, None).is_err(),
        "plan_element_delete 经 crate 根可达"
    );
    assert!(
        plan_relation_upsert(blank, "A", "B", None, None, None).is_err(),
        "plan_relation_upsert 经 crate 根可达"
    );
    assert!(
        plan_relation_delete(blank, "A", "B", None).is_err(),
        "plan_relation_delete 经 crate 根可达"
    );

    assert_eq!(EditErrorKind::ALL.len(), 23, "四类型之一：判据集合 23 值");
    let error: EditError = messages::error(EditErrorKind::RelationDuplicated, "出口面");
    assert!(
        error.message().starts_with("关系多处声明："),
        "EditError::message 经 crate 根可用：{}",
        error.message()
    );
    let write = ModelWrite {
        file: PathBuf::from("01-app.toml"),
        content: String::new(),
        hash: file_hash_of(Path::new("01-app.toml"), ""),
    };
    assert_eq!(
        write.hash,
        file_hash_of(Path::new("01-app.toml"), ""),
        "ModelWrite.hash 与 file_hash_of 同源"
    );

    assert!(
        file_hash(Path::new("不存在的单文件.toml")).starts_with("sha256:"),
        "drift 两个新名经 crate 根可达"
    );
}

// ===========================================================================
// 本变更：内联出边（`ElementSpec::relations`）
// ===========================================================================

use crate::{
    delete_warnings, upsert_warnings, EditWarning, RelationSpec, RelationTally, WriteAction,
    WriteCounts, WriteOutcome,
};

/// 内联出边的唯一构造形态（缺省动作 = upsert、三态字段不动）。
fn edge(to: &str) -> RelationSpec {
    RelationSpec {
        to: to.to_owned(),
        desc: None,
        apis: None,
        action: WriteAction::Upsert,
    }
}

/// 带动作的内联边。
fn edge_with(to: &str, action: WriteAction) -> RelationSpec {
    RelationSpec { action, ..edge(to) }
}

/// 正向：元素既有 + 该对已在同文件 ⇒ 更新 `desc`，关系条数不变、其余条目逐字保留。
#[test]
fn 内联边命中走更新() {
    let spec = ElementSpec {
        fqn: "A".to_owned(),
        relations: vec![RelationSpec {
            to: "B".to_owned(),
            desc: Some(Some("新说明".to_owned())),
            apis: None,
            action: WriteAction::Upsert,
        }],
        ..Default::default()
    };
    let written = apply_element_spec(RELATIONS_SOURCE, &spec).expect("内联更新应成功");
    let model = model_of(&written);
    assert_eq!(model.relations.len(), 3, "关系条数不变");
    assert_eq!(
        relation_of(&model, "A", "B").description.as_deref(),
        Some("新说明"),
        "命中条走更新"
    );
    assert_eq!(
        relation_of(&model, "B", "C").description,
        None,
        "其余条目逐字保留"
    );
    assert!(
        written.contains("desc = \"新说明\""),
        "文本内该条已改：\n{written}"
    );
}

/// 正向：该对缺席 ⇒ 追加一条 `[[relation]]`（键序 `from` / `to` / `desc`）、条数 +1、
/// 回读该对存在。
#[test]
fn 内联边缺席即建() {
    let source = concat!(
        "[element.\"A\"]\n",
        "kind = \"module\"\n",
        "\n",
        "[element.\"B\"]\n",
        "kind = \"module\"\n",
    );
    let spec = ElementSpec {
        fqn: "A".to_owned(),
        relations: vec![RelationSpec {
            desc: Some(Some("新边".to_owned())),
            ..edge("B")
        }],
        ..Default::default()
    };
    let written = apply_element_spec(source, &spec).expect("内联新建应成功");
    let model = model_of(&written);
    assert_eq!(model.relations.len(), 1, "缺席即建一条");
    assert_eq!(
        relation_of(&model, "A", "B").description.as_deref(),
        Some("新边")
    );
    let block = written.split("[[relation]]").nth(1).expect("关系块在场");
    let from_at = block.find("from = \"A\"").expect("from 键");
    let to_at = block.find("to = \"B\"").expect("to 键");
    let desc_at = block.find("desc = \"新边\"").expect("desc 键");
    assert!(
        from_at < to_at && to_at < desc_at,
        "键序 from / to / desc：\n{written}"
    );
}

/// 正向：`action = delete` 项命中源内该对 ⇒ 条目离场、条数 −1、回读缺席、其余逐字。
#[test]
fn 内联删边只动命中条() {
    let spec = ElementSpec {
        fqn: "A".to_owned(),
        relations: vec![edge_with("C", WriteAction::Delete)],
        ..Default::default()
    };
    let written = apply_element_spec(RELATIONS_SOURCE, &spec).expect("内联删边应成功");
    let model = model_of(&written);
    assert_eq!(model.relations.len(), 2, "条数 −1");
    assert!(
        !model
            .relations
            .iter()
            .any(|relation| relation.from == "A" && relation.to == "C"),
        "该对已离场"
    );
    assert_eq!(
        relation_of(&model, "A", "B").description.as_deref(),
        Some("旧说明"),
        "其余条目逐字"
    );
}

/// 正向：源含 3 条关系而 `relations` 只列 1 条 ⇒ 另 2 条逐字保留（零全量替换、零静默删）。
#[test]
fn 未列出的边保持不动() {
    let spec = ElementSpec {
        fqn: "A".to_owned(),
        relations: vec![edge("B")],
        ..Default::default()
    };
    let written = apply_element_spec(RELATIONS_SOURCE, &spec).expect("单条列出应成功");
    let model = model_of(&written);
    assert_eq!(model.relations.len(), 3, "零静默删");
    assert_eq!(
        relation_of(&model, "B", "C").description,
        None,
        "未列出的边零改动"
    );
    assert!(
        written.contains("desc = \"旧说明\""),
        "命中条同值亦零改动：\n{written}"
    );
}

/// 正向：新建元素带两条出边 ⇒ 同一次产出里元素表与两条关系并现、逐对回读、自证零诊断。
#[test]
fn 新建与出边同批写出() {
    let source = concat!(
        "[element.\"B\"]\n",
        "kind = \"module\"\n",
        "\n",
        "[element.\"C\"]\n",
        "kind = \"module\"\n",
    );
    let spec = ElementSpec {
        fqn: "A".to_owned(),
        kind: Some(ElementKind::Module),
        relations: vec![edge("B"), edge("C")],
        ..Default::default()
    };
    let written = apply_element_spec(source, &spec).expect("新建 + 出边应成功");
    let model = model_of(&written);
    assert_eq!(model.elements.len(), 3, "元素表已建");
    assert_eq!(model.relations.len(), 2, "两条出边并现");
    relation_of(&model, "A", "B");
    relation_of(&model, "A", "C");
}

/// 异常（矩阵 2 形态）：`action = delete` 项带 `desc` / `apis` ⇒ 各
/// `RelationDeleteHasFields`、零文本产出。
#[test]
fn delete项带字段即拒() {
    for relations in [
        vec![RelationSpec {
            desc: Some(Some("x".to_owned())),
            ..edge_with("C", WriteAction::Delete)
        }],
        vec![RelationSpec {
            apis: Some(vec!["open".to_owned()]),
            ..edge_with("C", WriteAction::Delete)
        }],
    ] {
        let spec = ElementSpec {
            fqn: "A".to_owned(),
            relations,
            ..Default::default()
        };
        let failure = apply_element_spec(RELATIONS_SOURCE, &spec).expect_err("delete 带字段应拒");
        assert_eq!(failure.kind, EditErrorKind::RelationDeleteHasFields);
        assert!(
            failure.facts.contains("action = delete") && failure.facts.contains("desc / apis"),
            "facts 点明互斥：{}",
            failure.facts
        );
    }
}

/// 异常（矩阵 2 形态）：同批重复 `to` ⇒ `RelationDuplicateInBatch`，facts 含 **1 基
/// 下标** + `action` + `to`。
#[test]
fn 同批重复to即拒且定位到项() {
    for relations in [
        vec![edge("B"), edge("B")],
        vec![
            edge_with("B", WriteAction::Upsert),
            edge_with("B", WriteAction::Delete),
        ],
    ] {
        let spec = ElementSpec {
            fqn: "A".to_owned(),
            relations,
            ..Default::default()
        };
        let failure = apply_element_spec(RELATIONS_SOURCE, &spec).expect_err("重复 to 应拒");
        assert_eq!(failure.kind, EditErrorKind::RelationDuplicateInBatch);
        assert!(
            failure.facts.contains("relations[2]")
                && failure.facts.contains("action =")
                && failure.facts.contains("to = \"B\""),
            "facts 含 1 基下标 / action / to：{}",
            failure.facts
        );
    }
}

// ===========================================================================
// 本变更：删除纯函数的唯一形态（`DeleteSet` = 元素集 + 关系对集）
// ===========================================================================

/// 正向：元素集 {父, 子1, 子2} + 关系对集 {(a, b)} ⇒ 三张元素表与一条关系同批离场、
/// 其余内容与注释逐字、表头计数随动下降。
///
/// 表头注释是**首张表的 prefix decor**（删首表会连注释一并离场，属渲染层既有语义）——
/// 故把表头挂在保留的表上，才能观察「回填只改括号内两数字」。
#[test]
fn 集合删除一把落净且计数随动() {
    let source = concat!(
        "# 01 · 集合（5 元素 / 2 关系）\n",
        "[element.\"B\"]\n",
        "kind = \"module\"\n",
        "\n",
        "[element.\"A\"]\n",
        "kind = \"module\"\n",
        "\n",
        "[element.\"A.X\"]\n",
        "kind = \"domain\"\n",
        "\n",
        "[element.\"A.X.Y\"]\n",
        "kind = \"module\"\n",
        "\n",
        "[element.\"C\"]\n",
        "kind = \"module\"\n",
        "\n",
        "[[relation]]\n",
        "from = \"A.X\"\n",
        "to = \"B\"\n",
        "\n",
        "[[relation]]\n",
        "from = \"B\"\n",
        "to = \"C\"\n",
    );
    let mut set = DeleteSet::default();
    for fqn in ["A", "A.X", "A.X.Y"] {
        set.elements.insert(fqn.to_owned());
    }
    set.relations.insert(("A.X".to_owned(), "B".to_owned()));

    let written = apply_element_delete(source, &set).expect("集合删除应成功");
    let model = model_of(&written);
    assert_eq!(
        model
            .elements
            .iter()
            .map(|element| element.fqn.as_str())
            .collect::<Vec<_>>(),
        vec!["B", "C"],
        "三张元素表离场"
    );
    assert_eq!(model.relations.len(), 1, "一条关系同批离场");
    assert_eq!(header_counts(&written), Some((2, 1)), "表头计数随动下降");
    assert!(written.contains("# 01 · 集合"), "注释逐字保留");
}

/// 异常：`relations` 含文件中不存在的对 ⇒ `RelationNotFound`，facts 列该文件现有对、
/// 零文本产出。
#[test]
fn 关系对缺席列现有对() {
    let mut set = DeleteSet::default();
    set.relations.insert(("A".to_owned(), "Z".to_owned()));
    let failure = apply_element_delete(RELATIONS_SOURCE, &set).expect_err("关系对缺席应拒");
    assert_eq!(failure.kind, EditErrorKind::RelationNotFound);
    assert!(
        failure.facts.contains("A → Z") && failure.facts.contains("A → B"),
        "facts 列现有对：{}",
        failure.facts
    );
}

/// 异常（矩阵 2 形态）：`elements` 含不在案的 FQN ⇒ `ElementNotFound`（facts 列现有
/// 元素）；缺 `element` 表 ⇒ `ElementTableMissing`。
#[test]
fn 元素缺席列现有元素() {
    let mut set = DeleteSet::default();
    set.elements.insert("App.Legacy".to_owned());
    let failure = apply_element_delete(CORE_SOURCE, &set).expect_err("元素缺席应拒");
    assert_eq!(failure.kind, EditErrorKind::ElementNotFound);
    assert!(
        failure.facts.contains("App.Legacy") && failure.facts.contains("现有元素"),
        "facts 列现有元素：{}",
        failure.facts
    );

    let mut set = DeleteSet::default();
    set.elements.insert("App".to_owned());
    let failure = apply_element_delete("[[relation]]\nfrom = \"A\"\nto = \"B\"\n", &set)
        .expect_err("缺 element 表应拒");
    assert_eq!(failure.kind, EditErrorKind::ElementTableMissing);
}

/// 边界（矩阵 2 形态）：`DeleteSet::of(fqn)` ≡ 单元素 + 零关系；公开字段判空两臂——
/// `default()` 两集合皆空、`of(fqn)` 非空（类型面零 `is_empty`）。
#[test]
fn 单元素删除是集合退化形() {
    let set = DeleteSet::of("App.Auth");
    assert_eq!(set.elements.len(), 1, "恰一个元素");
    assert!(set.relations.is_empty(), "零关系对");
    let empty = DeleteSet::default();
    assert!(
        empty.elements.is_empty() && empty.relations.is_empty(),
        "空集可辨"
    );

    let written = apply_element_delete(CORE_SOURCE, &set).expect("退化形删除应成功");
    assert_eq!(model_of(&written).elements.len(), 2, "恰删一个元素");
}

// ===========================================================================
// 本变更：落点 / 批量四条 / 计数与反馈面
// ===========================================================================

/// 正向 + 异常（矩阵 2 形态）：新文件表头 `# <file 去 .toml>（1 元素 / 0 关系）` 由写后
/// 重解析计数生成；目标文件缺席且无 `create` ⇒ `Err` 提示 `create = true`。
#[test]
fn 新文件表头按写后计数生成() {
    let temp = TempDir::new("plan-new-file-header");
    temp.write_model("01-core.toml", "[element.\"App\"]\nkind = \"package\"\n");
    let outcome = plan_element_upsert(
        temp.root(),
        &ElementSpec {
            fqn: "App.Fresh".to_owned(),
            kind: Some(ElementKind::Module),
            file: Some("07-fresh.toml".to_owned()),
            create: true,
            ..Default::default()
        },
    )
    .expect("新建文件应成功");
    assert_eq!(
        outcome.write.file,
        temp.root()
            .join(".dev-team/architecture/graph/model/07-fresh.toml"),
        "落点 = 模型声明目录一级"
    );
    assert_eq!(
        outcome.write.content.lines().next(),
        Some("# 07-fresh（1 元素 / 0 关系）"),
        "首行为表头（文件名去 .toml + 写后计数）"
    );

    let failure = plan_element_upsert(
        temp.root(),
        &ElementSpec {
            fqn: "App.Another".to_owned(),
            kind: Some(ElementKind::Module),
            file: Some("08-another.toml".to_owned()),
            ..Default::default()
        },
    )
    .expect_err("文件缺席且无 create 应拒");
    assert_eq!(failure.kind, EditErrorKind::TargetInvalid);
    assert!(
        failure.facts.contains("create = true"),
        "facts 提示 create = true：{}",
        failure.facts
    );
}

/// 异常（矩阵 4 形态）：批量四条各点名去向——同批重复 `to` / `delete` 带字段 / 内联边
/// 已声明在别的模型文件 / 端点不在案。
#[test]
fn 批量四条各点名去向() {
    let temp = TempDir::new("plan-batch-four");
    temp.write_model(
        "01-app.toml",
        "[element.\"A\"]\nkind = \"module\"\n\n[element.\"B\"]\nkind = \"module\"\n",
    );
    temp.write_model(
        "02-edges.toml",
        "# 02 · 边\n[element.\"C\"]\nkind = \"module\"\n\n[[relation]]\nfrom = \"A\"\nto = \"C\"\n",
    );

    let duplicate = plan_element_upsert(
        temp.root(),
        &ElementSpec {
            fqn: "A".to_owned(),
            summary: Some(Some("摘要".to_owned())),
            relations: vec![edge("B"), edge("B")],
            ..Default::default()
        },
    )
    .expect_err("同批重复应拒");
    assert_eq!(duplicate.kind, EditErrorKind::RelationDuplicateInBatch);

    let has_fields = plan_element_upsert(
        temp.root(),
        &ElementSpec {
            fqn: "A".to_owned(),
            relations: vec![RelationSpec {
                desc: Some(Some("x".to_owned())),
                ..edge_with("B", WriteAction::Delete)
            }],
            ..Default::default()
        },
    )
    .expect_err("delete 带字段应拒");
    assert_eq!(has_fields.kind, EditErrorKind::RelationDeleteHasFields);

    let elsewhere = plan_element_upsert(
        temp.root(),
        &ElementSpec {
            fqn: "A".to_owned(),
            relations: vec![edge("C")],
            ..Default::default()
        },
    )
    .expect_err("内联边在他处应拒");
    assert_eq!(elsewhere.kind, EditErrorKind::RelationOwnedElsewhere);
    assert!(
        elsewhere.facts.contains("02-edges.toml") && elsewhere.facts.contains("archi_relation"),
        "facts 点名文件并指向 archi_relation：{}",
        elsewhere.facts
    );

    let missing = plan_element_upsert(
        temp.root(),
        &ElementSpec {
            fqn: "A".to_owned(),
            relations: vec![edge("Z")],
            ..Default::default()
        },
    )
    .expect_err("端点不在案应拒");
    assert_eq!(missing.kind, EditErrorKind::RelationEndpointMissing);
    assert!(
        missing.facts.contains('Z'),
        "facts 点名缺失端点：{}",
        missing.facts
    );
}

/// 正向（矩阵 3 形态）：四计数守恒（混合批次 1 新建 + 1 改 + 1 删）；不带 `relations`
/// 与 `relations: []` 两形态皆四项归零（不是省略）。
#[test]
fn 四计数守恒且空批次归零() {
    let temp = TempDir::new("plan-tally");
    temp.write_model(
        "01-core.toml",
        concat!(
            "[element.\"A\"]\n",
            "kind = \"module\"\n",
            "\n",
            "[element.\"B\"]\n",
            "kind = \"module\"\n",
            "\n",
            "[element.\"C\"]\n",
            "kind = \"module\"\n",
            "\n",
            "[element.\"D\"]\n",
            "kind = \"module\"\n",
            "\n",
            "[[relation]]\n",
            "from = \"A\"\n",
            "to = \"B\"\n",
            "\n",
            "[[relation]]\n",
            "from = \"A\"\n",
            "to = \"C\"\n",
        ),
    );

    let outcome = plan_element_upsert(
        temp.root(),
        &ElementSpec {
            fqn: "A".to_owned(),
            relations: vec![
                edge("D"),
                RelationSpec {
                    desc: Some(Some("改".to_owned())),
                    ..edge("B")
                },
                edge_with("C", WriteAction::Delete),
            ],
            ..Default::default()
        },
    )
    .expect("混合批次应成功");
    assert_eq!(
        outcome.relations.created
            + outcome.relations.updated
            + outcome.relations.deleted
            + outcome.relations.unchanged,
        3,
        "四计数之和 == relations.len()（3 条）"
    );
    assert_eq!(outcome.relations.created, 1, "1 新建");
    assert_eq!(outcome.relations.updated, 1, "1 改值");
    assert_eq!(outcome.relations.deleted, 1, "1 删除");
    assert_eq!(outcome.relations.unchanged, 0);

    fs::write(&outcome.write.file, &outcome.write.content).expect("落盘混合批次");
    let same = plan_element_upsert(
        temp.root(),
        &ElementSpec {
            fqn: "A".to_owned(),
            relations: vec![RelationSpec {
                desc: Some(Some("改".to_owned())),
                ..edge("B")
            }],
            ..Default::default()
        },
    )
    .expect("同值批次应成功");
    assert_eq!(same.relations.unchanged, 1, "同值 ⇒ unchanged = 1");
    assert_eq!(
        same.relations.created
            + same.relations.updated
            + same.relations.deleted
            + same.relations.unchanged,
        1,
        "四计数守恒（len = 1）"
    );

    for spec in [
        ElementSpec {
            fqn: "A".to_owned(),
            summary: Some(Some("摘要".to_owned())),
            ..Default::default()
        },
        ElementSpec {
            fqn: "A".to_owned(),
            summary: Some(Some("摘要".to_owned())),
            relations: Vec::new(),
            ..Default::default()
        },
    ] {
        let outcome = plan_element_upsert(temp.root(), &spec).expect("空批次应成功");
        assert_eq!(
            outcome.relations,
            RelationTally::default(),
            "不带 / 空数组两形态四项皆 0（不是省略）"
        );
    }
}

/// 正向（矩阵 3 形态）：同值重复 upsert ⇒ `created = false`、`changed = []`、`content`
/// 与盘上逐字节相等；首次新建 ⇒ `created = true`；只改 `summary` ⇒
/// `changed == ["summary"]`。
#[test]
fn changed空即幂等() {
    let temp = TempDir::new("plan-changed");
    let path = temp.write_model("01-core.toml", "[element.\"A\"]\nkind = \"module\"\n");

    let first = plan_element_upsert(
        temp.root(),
        &ElementSpec {
            fqn: "A".to_owned(),
            summary: Some(Some("摘要".to_owned())),
            ..Default::default()
        },
    )
    .expect("首次改值应成功");
    assert!(!first.created, "在案元素不新建");
    assert_eq!(first.changed, vec!["summary".to_owned()], "只改 summary");
    fs::write(&path, &first.write.content).expect("落盘首次结果");

    let second = plan_element_upsert(
        temp.root(),
        &ElementSpec {
            fqn: "A".to_owned(),
            summary: Some(Some("摘要".to_owned())),
            ..Default::default()
        },
    )
    .expect("同值重放应成功");
    assert_eq!(second.changed, Vec::<String>::new(), "同值 ⇒ changed 空");
    assert_eq!(
        fs::read_to_string(&path).expect("读盘失败"),
        second.write.content,
        "幂等重放零字节变化"
    );

    let created = plan_element_upsert(
        temp.root(),
        &ElementSpec {
            fqn: "A.B".to_owned(),
            kind: Some(ElementKind::Module),
            summary: Some(Some("新".to_owned())),
            ..Default::default()
        },
    )
    .expect("新建应成功");
    assert!(created.created, "首次新建");
    assert!(
        created
            .changed
            .iter()
            .all(|field| ["kind", "summary"].contains(&field.as_str())),
        "changed ⊆ 实际给出的字段名：{:?}",
        created.changed
    );
}

/// 正向：`counts` == 写后 `TomlParser::parse(&content)` 的计数 == 表头括号内两数字；
/// `model` == 写后 `parse::parse_dir(root)` 的全模型计数。
#[test]
fn counts与表头与model三处同值() {
    let temp = TempDir::new("plan-counts");
    temp.write_model(
        "01-core.toml",
        "# 01 · 核心（1 元素 / 0 关系）\n[element.\"A\"]\nkind = \"module\"\n",
    );
    let outcome = plan_element_upsert(
        temp.root(),
        &ElementSpec {
            fqn: "A.B".to_owned(),
            kind: Some(ElementKind::Module),
            ..Default::default()
        },
    )
    .expect("新建应成功");

    let counted = model_of(&outcome.write.content);
    assert_eq!(
        outcome.counts,
        WriteCounts {
            elements: counted.elements.len(),
            relations: counted.relations.len(),
        },
        "counts == 写后本文件重解析计数"
    );
    assert_eq!(
        header_counts(&outcome.write.content),
        Some((outcome.counts.elements, outcome.counts.relations)),
        "表头两数字与 counts 同值"
    );
    fs::write(&outcome.write.file, &outcome.write.content).expect("模拟壳层落盘");
    let full = parse::parse_dir(temp.root());
    assert!(
        full.diagnostics.is_empty(),
        "写后模型零诊断：{:?}",
        full.diagnostics
    );
    assert_eq!(
        outcome.model,
        WriteCounts {
            elements: full.model.elements.len(),
            relations: full.model.relations.len(),
        },
        "model == parse_dir 的全模型计数"
    );
}

/// 正向：`outcome.warnings` 与 `upsert_warnings(root, 写后元素, &changed, created)` 逐条
/// 相等（`kind` / `severity` / `message` 三维）；plan 层零自拼 warning。
#[test]
fn warnings由模块单点产出() {
    let temp = TempDir::new("plan-warnings");
    temp.write_model("01-core.toml", "[element.\"App\"]\nkind = \"package\"\n");
    let outcome = plan_element_upsert(
        temp.root(),
        &ElementSpec {
            fqn: "App.Core".to_owned(),
            kind: Some(ElementKind::Module),
            ..Default::default()
        },
    )
    .expect("新建应成功");

    let written = model_of(&outcome.write.content);
    let element = written.element("App.Core").expect("写后元素在场");
    let expected = upsert_warnings(temp.root(), element, &outcome.changed, outcome.created);
    assert_eq!(outcome.warnings, expected, "warnings 由模块单点产出");
    assert!(
        !outcome.warnings.is_empty(),
        "空 path 的 module ⇒ 判据真实生效：{:?}",
        outcome.warnings
    );
    assert!(
        outcome
            .warnings
            .iter()
            .any(|warning| warning.kind == "noPath"),
        "noPath 计入反馈面：{:?}",
        outcome.warnings
    );
}

// ===========================================================================
// 本变更：级联删除（`cascade` 只作用同一模型文件内）
// ===========================================================================

/// 正向：目标元素有 2 层子树 + 出入边全在同一文件 ⇒ 闭包内元素与关系全删净、`content`
/// 无闭包内 FQN、表头计数随动；`changed = []` / `created = false` / `relations` 四项皆 0。
#[test]
fn 单文件闭包删净() {
    let temp = TempDir::new("cascade-same-file");
    temp.write_model(
        "01-core.toml",
        concat!(
            "# 01 · 闭包（4 元素 / 2 关系）\n",
            "[element.\"App\"]\n",
            "kind = \"package\"\n",
            "\n",
            "[element.\"App.Core\"]\n",
            "kind = \"domain\"\n",
            "\n",
            "[element.\"App.Core.Store\"]\n",
            "kind = \"module\"\n",
            "\n",
            "[element.\"App.Other\"]\n",
            "kind = \"domain\"\n",
            "\n",
            "[[relation]]\n",
            "from = \"App.Core.Store\"\n",
            "to = \"App.Other\"\n",
            "\n",
            "[[relation]]\n",
            "from = \"App.Other\"\n",
            "to = \"App.Core\"\n",
        ),
    );
    let outcome =
        plan_element_delete(temp.root(), "App.Core", true, None).expect("同文件闭包应成功");
    let remaining = model_of(&outcome.write.content);
    assert_eq!(
        remaining
            .elements
            .iter()
            .map(|element| element.fqn.as_str())
            .collect::<Vec<_>>(),
        vec!["App", "App.Other"],
        "闭包内元素全删净"
    );
    assert!(remaining.relations.is_empty(), "出入边全删净");
    assert_eq!(
        header_counts(&outcome.write.content),
        Some((2, 0)),
        "表头计数随动"
    );
    assert_eq!(outcome.changed, Vec::<String>::new(), "changed 空");
    assert!(!outcome.created, "created 假");
    assert_eq!(
        outcome.relations,
        RelationTally::default(),
        "关系四项皆 0（内联面无参与）"
    );
}

/// 异常：闭包含另一模型文件声明的子元素 ⇒ `CascadeCrossFile` 点名各文件与阻塞项、
/// 零文本产出。
#[test]
fn 跨文件闭包拒并点名() {
    let temp = TempDir::new("cascade-cross-file");
    temp.write_model(
        "01-core.toml",
        "[element.\"App\"]\nkind = \"package\"\n\n[element.\"App.Core\"]\nkind = \"domain\"\n",
    );
    temp.write_model(
        "02-more.toml",
        "# 02 · 更多\n[element.\"App.Core.Store\"]\nkind = \"module\"\n",
    );
    let failure =
        plan_element_delete(temp.root(), "App.Core", true, None).expect_err("跨文件闭包应拒");
    assert_eq!(failure.kind, EditErrorKind::CascadeCrossFile);
    assert!(
        failure.facts.contains("02-more.toml") && failure.facts.contains("App.Core.Store"),
        "facts 点名文件与阻塞项：{}",
        failure.facts
    );
}

/// 边界（矩阵 2 形态）：叶元素（无子无边）`cascade = true` 与 `false` 的 `content` 逐字节
/// 相同；元素缺席时 `cascade = true` 仍 `ElementNotFound`。
#[test]
fn 无子无边时cascade与缺省同文本() {
    let temp = TempDir::new("cascade-leaf");
    temp.write_model(
        "01-core.toml",
        "[element.\"App\"]\nkind = \"package\"\n\n[element.\"App.Leaf\"]\nkind = \"module\"\n",
    );
    let plain = plan_element_delete(temp.root(), "App.Leaf", false, None).expect("缺省删除");
    let cascaded = plan_element_delete(temp.root(), "App.Leaf", true, None).expect("级联删除");
    assert_eq!(
        plain.write.content, cascaded.write.content,
        "叶元素两形态同文本"
    );

    let failure =
        plan_element_delete(temp.root(), "App.Ghost", true, None).expect_err("缺席元素应拒");
    assert_eq!(failure.kind, EditErrorKind::ElementNotFound);
}

// ===========================================================================
// 本变更：关系写的薄形反馈面与类型面
// ===========================================================================

/// 正向（矩阵 4 形态）：两关系入口返回薄形 `WriteOutcome`——`warnings` 恒空、
/// `relations` 计一条（created / unchanged / updated / deleted 各一）、`changed`
/// ⊆ {`desc`, `apis`}（delete 为 `[]`）。
#[test]
fn 关系写薄形四计数取一() {
    let temp = TempDir::new("relation-thin");
    temp.write_model(
        "01-core.toml",
        "[element.\"A\"]\nkind = \"module\"\n\n[element.\"B\"]\nkind = \"module\"\n",
    );

    let created = plan_relation_upsert(temp.root(), "A", "B", None, None, None).expect("新建关系");
    assert!(created.warnings.is_empty(), "warnings 恒空");
    assert_eq!(
        created.relations,
        RelationTally {
            created: 1,
            ..Default::default()
        },
        "新增 ⇒ created = 1"
    );
    assert_eq!(created.changed, Vec::<String>::new(), "无字段 ⇒ changed 空");
    fs::write(&created.write.file, &created.write.content).expect("落盘新关系");

    let unchanged =
        plan_relation_upsert(temp.root(), "A", "B", None, None, None).expect("同值重放");
    assert_eq!(
        unchanged.relations,
        RelationTally {
            unchanged: 1,
            ..Default::default()
        },
        "同值 ⇒ unchanged = 1"
    );

    let updated =
        plan_relation_upsert(temp.root(), "A", "B", Some(Some("改")), None, None).expect("改值");
    assert_eq!(
        updated.relations,
        RelationTally {
            updated: 1,
            ..Default::default()
        },
        "改值 ⇒ updated = 1"
    );
    assert_eq!(
        updated.changed,
        vec!["desc".to_owned()],
        "changed ⊆ {{desc, apis}}"
    );
    fs::write(&updated.write.file, &updated.write.content).expect("落盘改值");

    let deleted = plan_relation_delete(temp.root(), "A", "B", None).expect("删除关系");
    assert!(deleted.warnings.is_empty(), "warnings 恒空");
    assert_eq!(
        deleted.relations,
        RelationTally {
            deleted: 1,
            ..Default::default()
        },
        "删除 ⇒ deleted = 1"
    );
    assert_eq!(
        deleted.changed,
        Vec::<String>::new(),
        "delete 的 changed 为 []"
    );
}

/// 边界（表驱动）：`WriteAction` 二值词表闭合——`ALL.len() == 2`、`as_str` 逐值、
/// `parse` 往返与词表外值、`Default = Upsert`。
#[test]
fn 动作二值词表闭合() {
    assert_eq!(WriteAction::ALL.len(), 2, "二值");
    assert_eq!(
        WriteAction::ALL
            .iter()
            .map(|action| action.as_str())
            .collect::<Vec<_>>(),
        vec!["upsert", "delete"],
        "线格式词逐值"
    );
    for action in WriteAction::ALL {
        assert_eq!(
            WriteAction::parse(action.as_str()),
            Some(action),
            "{} 往返",
            action.as_str()
        );
    }
    for invalid in ["Upsert", "DELETE", " upsert", "", "replace"] {
        assert_eq!(
            WriteAction::parse(invalid),
            None,
            "词表外值 {invalid:?} 不认（E1 据此拒）"
        );
    }
    assert_eq!(WriteAction::default(), WriteAction::Upsert, "缺省 upsert");
}

/// 边界（表驱动）：新增类型形状齐备——`RelationSpec` 三态可构造、`DeleteSet` 两集合按
/// `BTree` 序、`WriteCounts` / `RelationTally` 全 0 可构造。
#[test]
fn 新增类型形状齐备() {
    for desc in [None, Some(None), Some(Some("说明".to_owned()))] {
        let spec = RelationSpec {
            to: "B".to_owned(),
            desc: desc.clone(),
            apis: None,
            action: WriteAction::Upsert,
        };
        assert_eq!(spec.desc, desc, "三态可构造");
    }

    let mut set = DeleteSet::default();
    set.elements.insert("B".to_owned());
    set.elements.insert("A".to_owned());
    set.relations.insert(("B".to_owned(), "C".to_owned()));
    set.relations.insert(("A".to_owned(), "C".to_owned()));
    assert_eq!(
        set.elements.iter().cloned().collect::<Vec<_>>(),
        vec!["A".to_owned(), "B".to_owned()],
        "元素集按 BTree 序"
    );
    assert_eq!(
        set.relations.iter().cloned().collect::<Vec<_>>(),
        vec![
            ("A".to_owned(), "C".to_owned()),
            ("B".to_owned(), "C".to_owned())
        ],
        "关系对集按 BTree 序"
    );

    assert_eq!(
        WriteCounts::default(),
        WriteCounts {
            elements: 0,
            relations: 0
        }
    );
    assert_eq!(RelationTally::default().created, 0);
}

/// 边界：`WriteOutcome` 七字段可构造且内嵌 `ModelWrite` 三字段与既有三元组逐字同义。
#[test]
#[allow(non_snake_case)]
fn WriteOutcome内嵌ModelWrite不变() {
    let write = ModelWrite {
        file: PathBuf::from("01-app.toml"),
        content: "# 01 · app\n".to_owned(),
        hash: file_hash_of(Path::new("01-app.toml"), "# 01 · app\n"),
    };
    let outcome = WriteOutcome {
        write: write.clone(),
        warnings: Vec::new(),
        changed: Vec::new(),
        created: false,
        relations: RelationTally::default(),
        counts: WriteCounts::default(),
        model: WriteCounts::default(),
    };
    assert_eq!(outcome.write, write, "七字段之一内嵌 ModelWrite 原样");
    assert_eq!(outcome.write.file, PathBuf::from("01-app.toml"));
    assert_eq!(outcome.write.content, "# 01 · app\n");
    assert_eq!(
        outcome.write.hash,
        file_hash_of(Path::new("01-app.toml"), "# 01 · app\n"),
        "hash 口径不变"
    );
    let cloned = outcome.clone();
    assert_eq!(cloned, outcome, "PartialEq 与 Clone 可对拍");
    assert!(!format!("{outcome:?}").is_empty(), "Debug 可用");
}

/// 边界：`ElementSpec` 的 `Default` 扩展不破既有简写——两新字段缺省分别 `[]` / `false`，
/// `ElementSpec { fqn, ..Default::default() }` 仍成立；字段面封闭（全字面构造不含级联
/// 开关，D-17）。
#[test]
#[allow(non_snake_case)]
fn ElementSpec缺省扩展不破既有简写() {
    let spec = ElementSpec {
        fqn: "App".to_owned(),
        ..Default::default()
    };
    assert!(spec.relations.is_empty(), "relations 缺省空数组");
    assert!(!spec.create, "create 缺省 false");
    assert!(spec.expect.is_none(), "expect 缺省不校验");
    assert!(spec.kind.is_none() && spec.gate.is_none() && spec.api.is_none());

    // 字段面封闭：全字面构造（不带 `..Default`）只能写既有字段——把级联开关加回
    // spec 面（D-17）或扩出新字段即在此编译失败。
    let full = ElementSpec {
        fqn: "App.Full".to_owned(),
        kind: Some(ElementKind::Module),
        gate: Some(GateTier::Off),
        api: Some(ApiStatus::Declared),
        entry: Some(Some("./src/full/index.ts".to_owned())),
        path: Some(vec!["./src/full/".to_owned()]),
        doc: Some(None),
        summary: Some(Some("一行摘要".to_owned())),
        metadata: Some(BTreeMap::from([("k".to_owned(), "v".to_owned())])),
        relations: Vec::new(),
        file: Some("01-app.toml".to_owned()),
        create: false,
        expect: None,
    };
    assert_eq!(full.fqn, "App.Full");
    assert!(full.relations.is_empty(), "relations 空数组即出边不动");
}

// ===========================================================================
// 本变更：源码锚与 crate 根出口面
// ===========================================================================

/// 异常（源码锚）：删除纯函数恰一个形态、删元素表的 `remove` 只一处、`Sha256` /
/// `fs::write` 零命中（零哈希拼装、零写盘）。
#[test]
fn 删除实现恰一个且零写盘() {
    let source = include_str!("mod.rs");
    assert_eq!(
        source.matches("fn apply_element_delete").count(),
        1,
        "删除纯函数恰一处"
    );
    assert!(
        !source.contains("apply_element_delete_one"),
        "零第二份单元素删除路径"
    );
    assert_eq!(
        source.matches("table.remove(fqn)").count(),
        1,
        "删元素表的调用只一处（集合删除唯一形态）"
    );
    assert!(
        !source.contains("fn is_empty"),
        "DeleteSet 零测试专用方法（集合判空由公开字段直接判，D-18）"
    );
    for forbidden in ["Sha256", "sha2", "fs::write"] {
        assert!(!source.contains(forbidden), "edit/mod.rs 内零 {forbidden}");
    }
}

/// 异常（源码锚）：`edit` 零 drift 判定调用——`drift::check` / `ArchiDriftMode` /
/// `scan::` 零命中（`file_hash_of` 是唯一 drift 依赖面）。
#[test]
fn edit零drift判定调用() {
    let source = include_str!("mod.rs");
    for forbidden in ["drift::check", "ArchiDriftMode", "scan::"] {
        assert!(!source.contains(forbidden), "edit/mod.rs 内零 {forbidden}");
    }
    assert!(
        source.contains("file_hash_of"),
        "file_hash_of 是唯一 drift 依赖面"
    );
}

/// 正向（出口面）：crate 根 `edit` 分组含六个新写面类型与 warnings 三名；既有 12 名未丢。
#[test]
fn crate根出口含六个写面类型与warnings三名() {
    let source = include_str!("../lib.rs");
    for name in [
        "WriteAction",
        "RelationSpec",
        "DeleteSet",
        "WriteOutcome",
        "WriteCounts",
        "RelationTally",
        "EditWarning",
        "upsert_warnings",
        "delete_warnings",
    ] {
        assert!(source.contains(name), "crate 根出口含 `{name}`");
    }
    for name in [
        "apply_element_spec",
        "apply_element_delete",
        "apply_relation_upsert",
        "apply_relation_delete",
        "plan_element_upsert",
        "plan_element_delete",
        "plan_relation_upsert",
        "plan_relation_delete",
        "EditError",
        "EditErrorKind",
        "ElementSpec",
        "ModelWrite",
    ] {
        assert!(source.contains(name), "既有出口 `{name}` 未丢");
    }
    let _: fn(&Path, &crate::Element, &[String], bool) -> Vec<EditWarning> = upsert_warnings;
    let _: fn(&Path, &str) -> Vec<EditWarning> = delete_warnings;
}

/// 边界（源码锚）：crate 根 doc 的**五条机械纪律**逐句在场、恰五条（本变更不引入新的
/// 组织性约束），且旧立场句零命中（AC-12a 的窄面）——钉句口径沿 `crate头注释改原语族`。
#[test]
fn 头注释纪律零改动() {
    let source = include_str!("../lib.rs");
    assert!(
        source.contains("五条机械纪律："),
        "头注释的纪律段起手句在案"
    );
    for needle in [
        "**单点构建**：`ModelGraph` 只在 [`graph`] 建一次",
        "消费面 MUST NOT 各自再建图",
        "**投影零改写**：投影只裁剪（1-hop / 字段子集）",
        "推导、MUST NOT 补默认值",
        "**词汇全量对拍**：封闭词汇（元素种类 / gate 档 / 结构诊断 / 边标记 / 节点",
        "标记）只有一份定义（[`dto`]），三面覆盖同一词汇集",
        "**判定单源**：架构漂移对账 / 定级 / 豁免 / `status` 推导只有一处实现",
        "（[`drift::check`]）——命令面与门禁面都调它，MUST NOT 各写一份",
        "**契约面纯读**：模块 API 契约面（[`api`] 全族）零写盘零 spawn",
        "判定域不得反向持写盘能力",
    ] {
        assert!(source.contains(needle), "crate 根 doc 逐句在场：{needle}");
    }

    let disciplines: Vec<&str> = source
        .lines()
        .filter(|line| {
            line.strip_prefix("//! ")
                .and_then(|rest| rest.strip_prefix(|c: char| c.is_ascii_digit()))
                .is_some_and(|rest| rest.starts_with(". **"))
        })
        .collect();
    assert_eq!(
        disciplines.len(),
        5,
        "机械纪律恰五条（本变更不引入新的组织性约束）：{disciplines:?}"
    );

    // 旧立场禁止短语按片段拼接：产品源码面对三个整串零字面命中（AC-12a 含测试夹具
    // 面），断言语义不变——禁的是「字面持有」而非「判据」
    for stale in [
        ["无专用", "写工具"].concat(),
        ["不设专用", "写工具"].concat(),
        ["MUST NOT 存在", "专用写"].concat(),
    ] {
        assert!(!source.contains(&stale), "旧立场句零命中：{stale}");
    }
}

/// 边界（源码锚）：`warnings_test` 模块登记恰一处（`edit/mod.rs` 末尾），既有
/// `messages_test` 登记未动。
#[test]
fn warnings测试登记在册() {
    let source = include_str!("mod.rs");
    assert_eq!(
        source.matches("mod warnings_test;").count(),
        1,
        "warnings_test 登记恰一处"
    );
    assert!(
        source.contains("mod messages_test;"),
        "既有 messages_test 登记未动"
    );
}
