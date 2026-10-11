//! matcher/parser 静态注册表：编译期注册、无 dylib。
//!
//! 每类产物一个自包含插件模块（matcher 判定命中、parser 产出信封），
//! 新增产物 = 新增一个模块 + 在 `PLUGINS` 追加一行；
//! 扫描循环 / DTO / 前端路由对产物类型零硬编码分支。
//!
//! **候选来源 = phase 定义清单（白名单驱动）**：`discover_artifacts` 按各相位
//! `artifact_manifest` 声明的相对路径模式枚举 change 目录命中候选（精确路径 /
//! `<前缀>/**` 前缀 glob），matcher 仅在清单命中的候选项内判定 kind；全树扫描候
//! 选整体退役（清单未命中的文件零候选零信封）。eval-checklist 为 PhaseRecord 投
//! 影分支（`(phase, attempt)` 关联，不走相对路径清单）——两来源在出口汇合同一
//! `Vec<ArtifactDescriptor>`（`phase` 字段承归属）。

use std::fs;
use std::path::{Path, PathBuf};

use super::envelope::{ArtifactCandidate, ArtifactDescriptor, ArtifactEnvelope};
use super::eval_checklist::EvalChecklistPlugin;
use super::markdown_doc::MarkdownDocPlugin;
use super::tasks_progress::TasksProgressPlugin;
use crate::state::{ArtifactSpec, PhaseStateRecord};
use crate::write::PhaseDefinition;

/// matcher / parser 的入参。
#[derive(Debug, Clone, Copy)]
pub struct ArtifactInput<'a> {
    pub change_dir: &'a Path,
    /// 相位评估史（db 读源；eval-checklist 候选锚定面）
    pub phases: &'a [PhaseStateRecord],
    pub candidate: &'a ArtifactCandidate,
    /// 清单命中上下文（文件候选在位：声明 kind / 标题；IPC 直查面为 `None`——
    /// matcher 据此判 kind，不靠扩展名猜测）
    pub spec: Option<&'static ArtifactSpec>,
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

// 三插件，全部自包含模块、编译期注册。
// 输出顺序：特化 kind（tasks-progress / eval-checklist）先于 markdown-doc。
static TASKS_PROGRESS: TasksProgressPlugin = TasksProgressPlugin;
static EVAL_CHECKLIST: EvalChecklistPlugin = EvalChecklistPlugin;
static MARKDOWN_DOC: MarkdownDocPlugin = MarkdownDocPlugin;

static PLUGINS: &[&dyn ArtifactPlugin] = &[&TASKS_PROGRESS, &EVAL_CHECKLIST, &MARKDOWN_DOC];

/// 清单命中候选 + 归属上下文（文件候选来自清单；eval 候选来自 PhaseRecord 投
/// 影）。
struct Candidate {
    candidate: ArtifactCandidate,
    /// 清单命中项（文件候选在位）
    spec: Option<&'static ArtifactSpec>,
    /// 归属相位（文件候选 = 清单声明相位；eval 候选 = 记录相位）
    phase: String,
}

/// 清单驱动候选枚举：按 manifest 声明顺序读各相位产物清单 → 相对路径枚举匹配
/// （精确路径 / `<前缀>/**` 前缀 glob）→ 命中项去重（同路径双重命中只留一条）
/// → 相对 POSIX 路径升序（确定性）+ eval 候选（评估史下标序）追加。
fn manifest_candidates(
    change_dir: &Path,
    phases: &[PhaseStateRecord],
    manifest: &[PhaseDefinition],
) -> Vec<Candidate> {
    let mut hits: Vec<(String, PathBuf, &'static ArtifactSpec, String)> = Vec::new();
    let mut seen: Vec<String> = Vec::new();
    for definition in manifest {
        for spec in definition.artifact_manifest {
            for relative_path in spec_paths(change_dir, spec) {
                let source = encode_file_source(&relative_path);
                if seen.contains(&source) {
                    continue;
                }
                seen.push(source.clone());
                hits.push((source, relative_path, spec, definition.id.to_owned()));
            }
        }
    }
    // 相对路径升序（插件内 order / rank 之外的稳定底序）
    hits.sort_by(|a, b| a.0.cmp(&b.0));

    let mut candidates: Vec<Candidate> = hits
        .into_iter()
        .map(|(_, relative_path, spec, phase)| Candidate {
            candidate: ArtifactCandidate::File { relative_path },
            spec: Some(spec),
            phase,
        })
        .collect();
    candidates.extend(phases.iter().enumerate().map(|(index, entry)| Candidate {
        candidate: ArtifactCandidate::EvalEntry { index },
        spec: None,
        phase: entry.phase.clone(),
    }));
    candidates
}

/// 单条清单项的命中文件：精确路径直接探测；`<前缀>/**` 递归枚举该前缀目录树
/// （点前缀项跳过——与产物可见性口径一致）。
fn spec_paths(change_dir: &Path, spec: &'static ArtifactSpec) -> Vec<PathBuf> {
    match spec.path.strip_suffix("/**") {
        Some(prefix) if !prefix.is_empty() => {
            let mut files: Vec<PathBuf> = Vec::new();
            walk_prefix(change_dir, Path::new(prefix), &mut files);
            files
        }
        _ => {
            let relative_path = PathBuf::from(spec.path);
            if change_dir.join(&relative_path).is_file() {
                vec![relative_path]
            } else {
                Vec::new()
            }
        }
    }
}

/// 清单声明前缀内的递归枚举（**只在清单命中的前缀内走**——非全树扫描）。
fn walk_prefix(root: &Path, prefix: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(root.join(prefix)) else {
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
            walk_prefix(root, &relative, out);
        } else if file_type.is_file() {
            out.push(relative);
        }
    }
}

/// 候选 → source 串：文件为相对 POSIX 路径，评估条目为序号串。
pub(crate) fn encode_source(candidate: &ArtifactCandidate) -> String {
    match candidate {
        ArtifactCandidate::File { relative_path } => encode_file_source(relative_path),
        ArtifactCandidate::EvalEntry { index } => index.to_string(),
    }
}

fn encode_file_source(relative_path: &Path) -> String {
    relative_path.to_string_lossy().replace('\\', "/")
}

/// source 串 → 候选：纯数字解析为评估条目序号，否则视为相对 POSIX 路径。
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
/// 保证 join 后不逃出 change 目录）：合法形态为评估条目序号串，或各分量
/// 非空且非 `.`/`..`、无反斜杠、无盘符冒号、无绝对路径前缀的相对 POSIX 路径。
fn is_valid_source(source: &str) -> bool {
    if source.parse::<usize>().is_ok() {
        return true; // 评估条目序号串
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

/// 清单驱动发现：候选喂给注册表，收集全部命中为 Descriptor 清单（归属相位随
/// 候选上下文）。输出按（插件顺序，插件内排序键）稳定排序：kind 分组且组内顺序
/// 确定。
pub fn discover_artifacts(
    change_dir: &Path,
    phases: &[PhaseStateRecord],
    manifest: &[PhaseDefinition],
) -> Vec<ArtifactDescriptor> {
    let candidates = manifest_candidates(change_dir, phases, manifest);
    let mut hits: Vec<(usize, u32, ArtifactDescriptor)> = Vec::new();
    for (plugin_index, plugin) in PLUGINS.iter().enumerate() {
        for candidate in &candidates {
            let input = ArtifactInput {
                change_dir,
                phases,
                candidate: &candidate.candidate,
                spec: candidate.spec,
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
                    source: encode_source(&candidate.candidate),
                    title: envelope.title,
                    phase: candidate.phase.clone(),
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
/// 候选无清单上下文（`spec = None`）：插件 parse 只读文件本体，标题名的清单兜底
/// 退回文件名主干。
pub fn read_artifact(
    change_dir: &Path,
    phases: &[PhaseStateRecord],
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
        phases,
        candidate: &candidate,
        spec: None,
    };
    let plugin = PLUGINS.iter().find(|plugin| plugin.kind() == kind)?;
    plugin.parse(&input)
}
