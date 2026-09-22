//! matcher/parser 静态注册表：编译期注册、无 dylib。
//!
//! 每类产物一个自包含插件模块（matcher 判定命中、parser 产出信封），
//! 新增产物 = 新增一个模块 + 在 `PLUGINS` 追加一行；
//! 扫描循环 / DTO / 前端路由对产物类型零硬编码分支。

use std::fs;
use std::path::{Path, PathBuf};

use super::envelope::{ArtifactCandidate, ArtifactDescriptor, ArtifactEnvelope};
use super::eval_checklist::EvalChecklistPlugin;
use super::markdown_doc::MarkdownDocPlugin;
use super::tasks_progress::TasksProgressPlugin;
use crate::model::{Inventory, Workflow};

/// matcher / parser 的入参。
#[derive(Debug, Clone, Copy)]
pub struct ArtifactInput<'a> {
    pub change_dir: &'a Path,
    pub inventory: Inventory,
    pub workflow: Option<&'a Workflow>,
    pub candidate: &'a ArtifactCandidate,
}

/// 一类产物的自包含插件：matcher 判定命中，parser 产出信封。
pub trait ArtifactPlugin: Send + Sync {
    fn kind(&self) -> &'static str;
    /// 同 kind 内的展示排序键（升序）；默认 0 保持候选枚举顺序。
    fn order(&self, _input: &ArtifactInput) -> u32 {
        0
    }
    fn matches(&self, input: &ArtifactInput) -> bool;
    fn parse(&self, input: &ArtifactInput) -> Option<ArtifactEnvelope>;
}

// 第一波三插件，全部自包含模块、编译期注册。
// 输出顺序：特化 kind（tasks-progress / eval-checklist）先于 markdown-doc。
static TASKS_PROGRESS: TasksProgressPlugin = TasksProgressPlugin;
static EVAL_CHECKLIST: EvalChecklistPlugin = EvalChecklistPlugin;
static MARKDOWN_DOC: MarkdownDocPlugin = MarkdownDocPlugin;

static PLUGINS: &[&dyn ArtifactPlugin] = &[&TASKS_PROGRESS, &EVAL_CHECKLIST, &MARKDOWN_DOC];

/// 枚举候选：文件树遍历（跳过点前缀项）+ eval 条目。
/// 文件按相对 POSIX 路径排序保证确定性。
fn enumerate_candidates(change_dir: &Path, workflow: Option<&Workflow>) -> Vec<ArtifactCandidate> {
    let mut files: Vec<PathBuf> = Vec::new();
    collect_files(change_dir, PathBuf::new(), &mut files);
    files.sort_by_key(|a| encode_file_source(a));

    let mut candidates: Vec<ArtifactCandidate> = files
        .into_iter()
        .map(|relative_path| ArtifactCandidate::File { relative_path })
        .collect();
    if let Some(workflow) = workflow {
        candidates
            .extend((0..workflow.eval.len()).map(|index| ArtifactCandidate::EvalEntry { index }));
    }
    candidates
}

fn collect_files(root: &Path, prefix: PathBuf, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(root.join(&prefix)) else {
        return;
    };
    for entry in entries.flatten() {
        let file_name = entry.file_name();
        let file_name = file_name.to_string_lossy();
        // 跳过点前缀项（隐藏文件 / 目录）
        if file_name.starts_with('.') {
            continue;
        }
        let relative = prefix.join(file_name.as_ref());
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_dir() {
            collect_files(root, relative, out);
        } else if file_type.is_file() {
            out.push(relative);
        }
    }
}

/// 候选 → source 串：文件为相对 POSIX 路径，eval 条目为序号串。
pub(crate) fn encode_source(candidate: &ArtifactCandidate) -> String {
    match candidate {
        ArtifactCandidate::File { relative_path } => encode_file_source(relative_path),
        ArtifactCandidate::EvalEntry { index } => index.to_string(),
    }
}

fn encode_file_source(relative_path: &Path) -> String {
    relative_path.to_string_lossy().replace('\\', "/")
}

/// source 串 → 候选：纯数字解析为 eval 条目序号，否则视为相对 POSIX 路径。
fn decode_source(source: &str) -> ArtifactCandidate {
    if let Ok(index) = source.parse::<usize>() {
        return ArtifactCandidate::EvalEntry { index };
    }
    ArtifactCandidate::File {
        relative_path: PathBuf::from(source),
    }
}

/// IPC `kind` 参数格式检查：非空即可（kind 仅与静态注册表精确比对，无路径语义）。
fn is_valid_kind(kind: &str) -> bool {
    !kind.is_empty()
}

/// IPC `source` 参数的包含性校验（与 change 名防护同一风格，字符串层即可
/// 保证 join 后不逃出 change 目录）：合法形态为 eval 条目序号串，或各分量
/// 非空且非 `.`/`..`、无反斜杠、无盘符冒号、无绝对路径前缀的相对 POSIX 路径。
fn is_valid_source(source: &str) -> bool {
    if source.parse::<usize>().is_ok() {
        return true; // eval 条目序号串
    }
    if source.is_empty()
        || source.starts_with('/')
        || source.starts_with('\\')
        || source.contains('\\')
        || source.contains(':')
    {
        return false;
    }
    source
        .split('/')
        .all(|segment| !segment.is_empty() && segment != "." && segment != "..")
}

/// 枚举候选喂给注册表，收集全部命中为 Descriptor 清单。
/// 输出按（插件顺序，插件内排序键）稳定排序：kind 分组且组内顺序确定。
pub fn discover_artifacts(
    change_dir: &Path,
    inventory: Inventory,
    workflow: Option<&Workflow>,
) -> Vec<ArtifactDescriptor> {
    let candidates = enumerate_candidates(change_dir, workflow);
    let mut hits: Vec<(usize, u32, ArtifactDescriptor)> = Vec::new();
    for (plugin_index, plugin) in PLUGINS.iter().enumerate() {
        for candidate in &candidates {
            let input = ArtifactInput {
                change_dir,
                inventory,
                workflow,
                candidate,
            };
            if !plugin.matches(&input) {
                continue;
            }
            let Some(envelope) = plugin.parse(&input) else {
                continue;
            };
            hits.push((
                plugin_index,
                plugin.order(&input),
                ArtifactDescriptor {
                    kind: envelope.kind,
                    source: encode_source(candidate),
                    title: envelope.title,
                },
            ));
        }
    }
    // sort_by 稳定：排序键相同者保持候选枚举顺序
    hits.sort_by_key(|(plugin_index, order, _)| (*plugin_index, *order));
    hits.into_iter()
        .map(|(_, _, descriptor)| descriptor)
        .collect()
}

/// 按 kind 定位插件、按 source 重建候选后解析出单个信封。
///
/// `kind` / `source` 均先过显式格式与包含性校验（IPC 面入参，不信任）：
/// source 的相对路径分量被限制在 change 目录内，文件候选再以 canonical
/// 路径兜底确认包含关系（防 symlink 类逃逸）；任一校验不过返回 `None`。
pub fn read_artifact(
    change_dir: &Path,
    inventory: Inventory,
    workflow: Option<&Workflow>,
    kind: &str,
    source: &str,
) -> Option<ArtifactEnvelope> {
    if !is_valid_kind(kind) || !is_valid_source(source) {
        return None;
    }
    let candidate = decode_source(source);
    if let ArtifactCandidate::File { relative_path } = &candidate {
        let canonical_change = fs::canonicalize(change_dir).ok()?;
        let canonical_file = fs::canonicalize(change_dir.join(relative_path)).ok()?;
        if !canonical_file.starts_with(&canonical_change) {
            return None;
        }
    }
    let input = ArtifactInput {
        change_dir,
        inventory,
        workflow,
        candidate: &candidate,
    };
    let plugin = PLUGINS.iter().find(|plugin| plugin.kind() == kind)?;
    plugin.parse(&input)
}
