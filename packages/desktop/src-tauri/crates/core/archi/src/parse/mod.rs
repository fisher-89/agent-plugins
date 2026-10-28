use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use foundation::layout::archi_models_dir;

use crate::dto::DiagnosticKind;
use crate::model::{Diagnostic, Model, ParseOutcome};

pub mod toml;

pub use toml::TomlParser;

#[cfg(test)]
mod mod_test;

#[cfg(test)]
mod toml_test;

/// 单文件解析契约（关联函数，无接收者）：产出诊断不带文件身份（`file` 留空），
/// 由聚合层回填该文件的 repo 相对路径。
pub trait SourceParser {
    /// 解析一段源文本为该格式的模型 + 诊断。
    fn parse(source: &str) -> ParseOutcome;
}

/// 模型源文件扩展名（唯一收面判据）。
const MODEL_FILE_SUFFIX: &str = ".toml";

/// 聚合入口：模型目录下全部 `.toml` 按**文件名**字典序解析 → 合并（元素 / 关系
/// 按文件序 + 各自声明序）→ 跨文件重复 FQN 检出 → 文件相对路径回填。
///
/// 模型目录缺席 / 空目录 → 空模型零诊断（冷启动合法起点，不报错）；单文件读取
/// 失败 → `syntaxError` 诊断（读取面 MUST NOT 静默丢文件）。
pub fn parse_dir(root: &Path) -> ParseOutcome {
    let dir = archi_models_dir(root);
    let mut files: Vec<(String, PathBuf)> = Vec::new();
    let Ok(reader) = fs::read_dir(&dir) else {
        return ParseOutcome {
            model: Model::empty(),
            diagnostics: Vec::new(),
        };
    };
    for entry in reader.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if !name.ends_with(MODEL_FILE_SUFFIX) {
            continue;
        }
        files.push((name.to_owned(), path));
    }
    files.sort_by(|left, right| left.0.cmp(&right.0));

    let mut model = Model::empty();
    let mut diagnostics: Vec<Diagnostic> = Vec::new();
    let mut seen: BTreeMap<String, ()> = BTreeMap::new();
    for (name, path) in files {
        let file = relative_path(root, &path).unwrap_or(name);
        let source = match fs::read_to_string(&path) {
            Ok(source) => source,
            Err(error) => {
                diagnostics.push(Diagnostic {
                    kind: DiagnosticKind::SyntaxError,
                    message: format!("模型文件读取失败：{error}"),
                    file,
                    line: 0,
                    column: None,
                });
                continue;
            }
        };
        let outcome = TomlParser::parse(&source);
        for mut diagnostic in outcome.diagnostics {
            diagnostic.file = file.clone();
            diagnostics.push(diagnostic);
        }
        for mut element in outcome.model.elements {
            element.file = file.clone();
            if seen.contains_key(&element.fqn) {
                diagnostics.push(Diagnostic {
                    kind: DiagnosticKind::DuplicateElement,
                    message: format!("跨文件重复声明元素：{}", element.fqn),
                    file: element.file,
                    line: element.line,
                    column: None,
                });
                continue;
            }
            seen.insert(element.fqn.clone(), ());
            model.elements.push(element);
        }
        for mut relation in outcome.model.relations {
            relation.file = file.clone();
            model.relations.push(relation);
        }
    }
    ParseOutcome { model, diagnostics }
}

/// 绝对路径 → repo 相对路径（`/` 分隔）单点：解析聚合层与文档关联层共用同一
/// 取法（不在两处各写一份）。不在 root 之下即 `None`。
pub(crate) fn relative_path(root: &Path, path: &Path) -> Option<String> {
    let relative = path.strip_prefix(root).ok()?;
    Some(relative.to_string_lossy().replace('\\', "/"))
}
