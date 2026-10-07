use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, Instant};

use checks::parser::CoverageFormat;
use config::TestFramework;
use glob::glob;

use checks::aggregate::derive_plan_id;

use super::runner::VERSION_DETECT_TIMEOUT;

/// jest `--randomize` 的最低版本（CLI `isVersionAtLeast(version, '29.5.0')`
/// 同值）。
const JEST_RANDOMIZE_MINIMUM: &str = "29.5.0";

/// 单框架注册表条目：命令模板（shell / cmd 双形态，占位符原样保留由
/// runner 展开）、覆盖格式与工件名、缺省 glob、配置注入旗标、版本命令。
struct FrameworkEntry {
    /// 框架标识（CLI 注册表键同串）
    framework: &'static str,
    /// suite cwd 下探测版本的命令
    version_command: &'static str,
    /// POSIX shell 测试执行模板；`{randomize}` 哨兵仅 jest 使用（按版本
    /// 展开为 ` --randomize` 或空串），其余文本与 CLI 逐字对齐
    shell_template: &'static str,
    /// Windows cmd 测试执行模板
    cmd_template: &'static str,
    /// 覆盖格式（V1 支持面三值）
    coverage_format: CoverageFormat,
    /// 覆盖工件名（相对 plan 报告目录）
    coverage_output: &'static str,
    /// 结果工件名（相对 plan 报告目录）
    results_output: &'static str,
    /// suite 未设 includes 时的缺省 glob（相对 suite root）
    default_glob: &'static str,
    /// 框架配置注入旗标（`None` = 不支持配置注入）
    config_flag: Option<&'static str>,
}

/// 五框架注册表（常量表移植；模板串与 CLI `test-framework.ts` 逐字对齐——
/// 唯 rust cmd 臂单侧修复偏离：CLI 保留旧 `if errorlevel` 形态）。
const REGISTRY: [FrameworkEntry; 5] = [
    FrameworkEntry {
        framework: "jest",
        version_command: "npx jest --version",
        shell_template: "npx jest{randomize} --reporters=default --json --outputFile=\"{results_file}\" --silent --coverage --coverageDirectory=\"{report_dir}\" --coverageReporters=json-summary {config_args} {files}",
        cmd_template: "npx jest{randomize} --reporters=default --json --outputFile=\"{results_file}\" --silent --coverage --coverageDirectory=\"{report_dir}\" --coverageReporters=json-summary {config_args} {files}",
        coverage_format: CoverageFormat::Istanbul,
        coverage_output: "coverage-summary.json",
        results_output: "results.json",
        default_glob: "**/*.{test,spec}.{js,ts,jsx,tsx}",
        config_flag: Some("--config"),
    },
    FrameworkEntry {
        framework: "vitest",
        version_command: "npx vitest --version",
        shell_template: "npx vitest run --sequence.shuffle --reporter=json --outputFile.json=\"{results_file}\" --coverage --coverage.reportsDirectory=\"{report_dir}\" --coverage.reporter=json-summary {config_args} {files}",
        cmd_template: "npx vitest run --sequence.shuffle --reporter=json --outputFile.json=\"{results_file}\" --coverage --coverage.reportsDirectory=\"{report_dir}\" --coverage.reporter=json-summary {config_args} {files}",
        coverage_format: CoverageFormat::Istanbul,
        coverage_output: "coverage-summary.json",
        results_output: "results.json",
        default_glob: "**/*.{test,spec}.{js,ts,jsx,tsx}",
        config_flag: Some("--config"),
    },
    FrameworkEntry {
        framework: "vite-plus",
        version_command: "vp --version",
        shell_template: "vp test --sequence.shuffle --reporter=json --outputFile.json=\"{results_file}\" --coverage --coverage.reportsDirectory=\"{report_dir}\" --coverage.reporter=json-summary {config_args} {files}",
        cmd_template: "vp test --sequence.shuffle --reporter=json --outputFile.json=\"{results_file}\" --coverage --coverage.reportsDirectory=\"{report_dir}\" --coverage.reporter=json-summary {config_args} {files}",
        coverage_format: CoverageFormat::Istanbul,
        coverage_output: "coverage-summary.json",
        results_output: "results.json",
        default_glob: "**/*.{test,spec}.{js,ts,jsx,tsx}",
        config_flag: Some("--config"),
    },
    FrameworkEntry {
        framework: "rust",
        version_command: "cargo --version",
        shell_template: "cargo test --workspace; _X=$?; cargo llvm-cov --json --output-path \"{coverage_file}\"; exit $_X",
        cmd_template: "cargo llvm-cov --json --output-path \"{coverage_file}\" & cargo test --workspace",
        coverage_format: CoverageFormat::LlvmCov,
        coverage_output: "coverage-summary.json",
        results_output: "results.txt",
        default_glob: "**/tests/**/*.rs",
        config_flag: None,
    },
    FrameworkEntry {
        framework: "node-test",
        version_command: "node --version",
        shell_template: "node --test --experimental-test-coverage --test-reporter=spec --test-reporter-destination=\"{results_file}\" {files}",
        cmd_template: "node --test --experimental-test-coverage --test-reporter=spec --test-reporter-destination=\"{results_file}\" {files}",
        coverage_format: CoverageFormat::NodeTest,
        coverage_output: "results.txt",
        results_output: "results.txt",
        default_glob: "**/*.test.{mjs,js,cjs}",
        config_flag: None,
    },
];

/// 显式不支持的框架（遇配置显式 `Err`，CLI 注册表外的三值封闭集）。
fn unsupported_framework_reason(framework: &TestFramework) -> Option<&'static str> {
    match framework {
        TestFramework::Bun => Some("bun"),
        TestFramework::Go => Some("go"),
        TestFramework::Pytest => Some("pytest"),
        _ => None,
    }
}

/// 框架枚举 → 注册表条目。
fn registry_entry(framework: &TestFramework) -> Result<&'static FrameworkEntry, String> {
    if let Some(name) = unsupported_framework_reason(framework) {
        return Err(format!(
            "框架 \"{name}\" 不被支持（V1 支持 jest / vitest / vite-plus / rust / node-test），不静默跳过"
        ));
    }
    framework_name(framework)
        .and_then(|key| REGISTRY.iter().find(|entry| entry.framework == key))
        .ok_or_else(|| "框架未注册".to_owned())
}

/// 框架枚举 → 线格式名（CLI 注册表键同串；不支持框架 → `None`）。
pub(crate) fn framework_name(framework: &TestFramework) -> Option<&'static str> {
    match framework {
        TestFramework::Jest => Some("jest"),
        TestFramework::Vitest => Some("vitest"),
        TestFramework::VitePlus => Some("vite-plus"),
        TestFramework::Rust => Some("rust"),
        TestFramework::NodeTest => Some("node-test"),
        _ => None,
    }
}

/// detect 产物：单 suite 的执行计划（进程内，不持久化）。
///
/// `root` 为项目相对 POSIX（planId 与子报告 root 基准）；`cwd` 为 spawn 用
/// 绝对路径；`files` 为 cwd 相对 POSIX 清单（`{files}` 占位符展开原料）。
#[derive(Debug)]
pub(crate) struct TestPlan {
    /// plan 目录 id（报告树子目录名）
    pub id: String,
    /// 测试框架标识
    pub framework: String,
    /// suite 锚点（项目相对 POSIX）
    pub root: String,
    /// 执行目录（绝对路径，spawn cwd）
    pub cwd: PathBuf,
    /// 命中的测试文件（cwd 相对 POSIX）
    pub files: Vec<String>,
    /// 配置注入参数（`{config_args}` 展开原料；未注入为 `None`）
    pub config_args: Option<String>,
    /// shell 测试执行模板（版本哨兵已展开）
    pub shell_template: String,
    /// cmd 测试执行模板（版本哨兵已展开）
    pub cmd_template: String,
    /// 覆盖格式
    pub coverage_format: CoverageFormat,
    /// 覆盖工件名
    pub coverage_output: &'static str,
    /// 结果工件名
    pub results_output: &'static str,
}

impl TestPlan {
    /// 平台模板选臂（Windows 走 cmd 模板配 `cmd /C`，其余 shell 模板配
    /// `sh -c`——运行期判别使双模板字段全平台可读，CLI `isWinCmd` 选臂
    /// 语义的平台钉定形态）。
    pub(crate) fn template(&self) -> &str {
        if cfg!(windows) {
            self.cmd_template.as_str()
        } else {
            self.shell_template.as_str()
        }
    }
}

/// 逐 suite 探测：glob 文件清单 + 排除过滤 + plan id + 版本探测。
///
/// 任一 suite 声明不支持框架（bun / go / pytest）或声明不支持注入的 config
/// → 显式 `Err`；includes 全不命中的 suite 不产 plan（空清单面，整体不
/// panic）。
pub(crate) fn detect_plans(
    root: &Path,
    suites: &[config::TestSuite],
) -> Result<Vec<TestPlan>, String> {
    let mut plans = Vec::new();
    for suite in suites {
        let entry = registry_entry(&suite.framework)?;

        // 配置注入声明校验（CLI validateSuiteConfig 同语义：不支持注入的
        // 框架声明 config 即显式 Err）
        let config_args = match (&suite.config, entry.config_flag) {
            (None, _) => None,
            (Some(_), None) => {
                return Err(format!(
                    "suite root \"{}\" 声明 config \"{}\"，但框架 \"{}\" 不支持配置注入",
                    suite.root,
                    suite.config.as_deref().unwrap_or_default(),
                    entry.framework
                ));
            }
            (Some(config), Some(flag)) => {
                let abs_config = root.join(&suite.root).join(config);
                Some(format!("{} \"{}\"", flag, to_posix(&abs_config)))
            }
        };

        // glob 文件探测（includes 缺省吃框架 default_glob；排除过滤 +
        // dot 目录剪枝；清单以 cwd 相对 POSIX 记）
        let abs_root = root.join(&suite.root);
        let abs_cwd = abs_root.join(&suite.cwd);
        let files = detect_files(root, &abs_root, &abs_cwd, suite, entry)?;

        // includes 全不命中 → 该 suite 不产 plan（空清单面）
        if files.is_empty() {
            continue;
        }

        // 版本探测（失败 / 空版本按「特性不支持」走基础模板，不失败不报错）
        let version = detect_framework_version(&abs_cwd, entry.version_command);
        let randomize = match entry.framework {
            "jest" if is_version_at_least(&version, JEST_RANDOMIZE_MINIMUM) => " --randomize",
            _ => "",
        };

        plans.push(TestPlan {
            id: derive_plan_id(&suite.root, entry.framework),
            framework: entry.framework.to_owned(),
            root: suite.root.clone(),
            cwd: abs_cwd,
            files,
            config_args,
            shell_template: entry.shell_template.replace("{randomize}", randomize),
            cmd_template: entry.cmd_template.replace("{randomize}", randomize),
            coverage_format: entry.coverage_format,
            coverage_output: entry.coverage_output,
            results_output: entry.results_output,
        });
    }
    Ok(plans)
}

/// glob 文件探测：includes（缺省 default_glob）挂 abs_root 遍历 → 项目相对
/// POSIX 排除过滤 + dot 目录剪枝 → cwd 相对 POSIX 清单（去重、字典序稳定）。
fn detect_files(
    project_root: &Path,
    abs_root: &Path,
    abs_cwd: &Path,
    suite: &config::TestSuite,
    entry: &FrameworkEntry,
) -> Result<Vec<String>, String> {
    let include_patterns: Vec<String> = match &suite.includes {
        Some(list) if !list.is_empty() => list.clone(),
        _ => vec![entry.default_glob.to_owned()],
    };
    let mut detected: BTreeSet<String> = BTreeSet::new();
    for include in &include_patterns {
        let pattern = format!("{}/{}", to_posix(abs_root).trim_end_matches('/'), include);
        let paths = glob(&pattern).map_err(|error| {
            format!("suite root \"{}\" includes glob 非法: {error}", suite.root)
        })?;
        for path in paths.flatten() {
            if !path.is_file() || has_dot_segment(&path) {
                continue;
            }
            // 项目相对形式过排除过滤（glob 相对 suite root 解释）
            let Ok(project_rel) = path.strip_prefix(project_root) else {
                continue;
            };
            let project_rel = to_posix(project_rel);
            if is_excluded_by_suite(&project_rel, suite) {
                continue;
            }
            // cwd 相对形式入清单（`{files}` 展开原料）
            let Ok(cwd_rel) = path.strip_prefix(abs_cwd) else {
                continue;
            };
            detected.insert(to_posix(cwd_rel));
        }
    }
    Ok(detected.into_iter().collect())
}

/// dot 目录剪枝：路径任一分量以 `.` 起手即剪（`.git`、工具缓存等）。
fn has_dot_segment(path: &Path) -> bool {
    path.components()
        .any(|component| component.as_os_str().to_string_lossy().starts_with('.'))
}

/// suite 级排除判定（CLI `isExcludedBySuite` 同语义：文件须先落在 suite
/// root 树下；排除 glob 相对 root 解释，项目相对与 root 相对双臂匹配）。
fn is_excluded_by_suite(project_rel: &str, suite: &config::TestSuite) -> bool {
    let Some(excludes) = suite.excludes.as_ref().filter(|list| !list.is_empty()) else {
        return false;
    };
    let root = to_forward_slash(&suite.root);
    let root = root.trim_end_matches('/').to_owned();
    let rel_to_root = match project_rel.strip_prefix(&format!("{root}/")) {
        Some(rest) => rest.to_owned(),
        None if project_rel == root => ".".to_owned(),
        None => return false,
    };
    excludes.iter().any(|exclude| {
        let scoped = posix_join(&root, exclude);
        let project_scoped = if rel_to_root == "." {
            root.clone()
        } else {
            posix_join(&root, &rel_to_root)
        };
        matches_glob(&project_scoped, &scoped) || matches_glob(&rel_to_root, exclude)
    })
}

/// glob 匹配（CLI `matchGlob` 同语义：无通配符按目录前缀，含通配符经
/// `glob::Pattern` 全匹配，`*` 不跨路径分隔符）。
pub(crate) fn matches_glob(path: &str, pattern: &str) -> bool {
    let pattern = to_forward_slash(pattern);
    if !pattern.contains(['*', '?', '{', '[']) {
        let path = to_forward_slash(path);
        return path == pattern || path.starts_with(&format!("{pattern}/"));
    }
    glob::Pattern::new(&pattern)
        .map(|compiled| {
            compiled.matches_with(
                &to_forward_slash(path),
                glob::MatchOptions {
                    case_sensitive: true,
                    require_literal_separator: true,
                    require_literal_leading_dot: false,
                },
            )
        })
        .unwrap_or(false)
}

/// POSIX 拼接归一（CLI `path.posix.join` 同语义的最小实现）。
fn posix_join(base: &str, tail: &str) -> String {
    let joined = if base == "." || base.is_empty() {
        tail.to_owned()
    } else {
        format!("{base}/{tail}")
    };
    let normalized: Vec<&str> = joined
        .split('/')
        .filter(|segment| !segment.is_empty() && segment != &".")
        .collect();
    normalized.join("/")
}

/// 绝对路径 POSIX 化（CLIs 对 rootDir / cwd 歧义的规避口径）。
pub(crate) fn to_posix(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

/// 路径 POSIX 化。
fn to_forward_slash(path: &str) -> String {
    path.replace('\\', "/")
}

/// 框架版本探测（CLI `detectFrameworkVersion` 同语义）：version_command 在
/// suite cwd 下 shell 语义执行（30s 截止），stdout + stderr 提取首个
/// `major.minor.patch`；命令失败 / 无 semver / 超时 → 空串（「特性不支持」
/// 走基础模板，不失败不报错）。
fn detect_framework_version(cwd: &Path, version_command: &str) -> String {
    let mut command = shell_command_std(version_command);
    command
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let Ok(mut child) = command.spawn() else {
        return String::new();
    };
    let deadline = Instant::now() + VERSION_DETECT_TIMEOUT;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) if Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                break None;
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(50)),
            Err(_) => return String::new(),
        }
    };
    let Some(status) = status else {
        return String::new();
    };
    if !status.success() {
        return String::new();
    }
    let stdout = child
        .stdout
        .take()
        .and_then(|mut pipe| {
            use std::io::Read;
            let mut text = String::new();
            pipe.read_to_string(&mut text).ok().map(|_| text)
        })
        .unwrap_or_default();
    let stderr = child
        .stderr
        .take()
        .and_then(|mut pipe| {
            use std::io::Read;
            let mut text = String::new();
            pipe.read_to_string(&mut text).ok().map(|_| text)
        })
        .unwrap_or_default();
    extract_semver(&format!("{stdout}\n{stderr}")).unwrap_or_default()
}

/// shell 语义包装（同步探测面）：Windows 经 cmd /C，其余经 sh -c。
fn shell_command_std(command: &str) -> std::process::Command {
    #[cfg(target_os = "windows")]
    {
        let mut shell = std::process::Command::new("cmd");
        shell.arg("/C").arg(command);
        shell
    }
    #[cfg(not(target_os = "windows"))]
    {
        let mut shell = std::process::Command::new("sh");
        shell.arg("-c").arg(command);
        shell
    }
}

/// 提取文本中首个 `major.minor.patch`（CLI `extractSemver` 同语义）。
fn extract_semver(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    for index in 0..bytes.len() {
        if !bytes[index].is_ascii_digit() {
            continue;
        }
        let rest = &text[index..];
        let end = rest
            .find(|c: char| !c.is_ascii_digit() && c != '.')
            .unwrap_or(rest.len());
        let candidate = &rest[..end];
        let parts: Vec<&str> = candidate.split('.').collect();
        if parts.len() == 3
            && parts
                .iter()
                .all(|part| !part.is_empty() && part.chars().all(|c| c.is_ascii_digit()))
        {
            return Some(candidate.to_owned());
        }
    }
    None
}

/// semver 比较（CLI `isVersionAtLeast` 同语义：任一侧不可解析 → false，
/// 调用方按「特性不支持」走基础模板）。
fn is_version_at_least(version: &str, minimum: &str) -> bool {
    let (Some(actual), Some(baseline)) = (extract_semver(version), extract_semver(minimum)) else {
        return false;
    };
    let parse = |value: &str| -> Vec<u64> {
        value
            .split('.')
            .filter_map(|part| part.parse::<u64>().ok())
            .collect()
    };
    let (a, b) = (parse(&actual), parse(&baseline));
    a.len() == 3 && b.len() == 3 && a >= b
}

#[cfg(test)]
#[path = "detect_test.rs"]
mod detect_test;
