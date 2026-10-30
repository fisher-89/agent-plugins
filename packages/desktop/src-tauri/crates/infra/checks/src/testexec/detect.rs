use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, Instant};

use checks::parser::CoverageFormat;
use config::TestFramework;
use glob::glob;

use checks::aggregate::derive_plan_id;
use checks::globmatch::{match_glob, try_expand_braces};

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
    /// 展开为 ` --randomize` 或空串），其余框架模板文本与 CLI 逐字对齐（rust
    /// 档为有意偏离，见 REGISTRY 头注释）
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

/// 五框架注册表（常量表移植；jest / vitest / vite-plus / node-test 与 CLI
/// `test-framework.ts` 逐字对齐，唯 rust 档单条命令**有意偏离** CLI 双段链：
/// ① `cargo llvm-cov` 自身即跑测试，双段链付两遍全量构建且 target 目录不
/// 复用；② cmd 臂 `&` 退出码取末段，覆盖率工件缺失仍判绿（假绿）；
/// ③ llvm-cov 不带 `--workspace` 只测根包 25 文件，与全 workspace 用例
/// 口径不同源）。
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
        shell_template: "cargo llvm-cov nextest --workspace --json --output-path \"{coverage_file}\" > \"{results_file}\"",
        cmd_template: "cargo llvm-cov nextest --workspace --json --output-path \"{coverage_file}\" > \"{results_file}\"",
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
    /// 降级安全判据（detect 期一次算定）：`default_glob` 以**同一剪枝管线、不施加
    /// suite `excludes`** 枚举出的集合 == [`Self::files`] 集合 → 省略 `{files}` 后
    /// 框架自发现跑的还是同一批。保守方向：集合不等即拒降级（`excludes` 实际剔掉
    /// 了某个文件 → 对照面仍含该文件 → 不等 → 拒降级）
    pub omit_files_safe: bool,
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

        // 降级对照面（`{files}` 省略后框架走自身发现，故只有「自发现集合 == files
        // 集合」才保证跑的还是同一批）；**MUST NOT 施加 suite `excludes`**——框架
        // 自发现不认这套，`excludes` 一旦真剔掉了文件两侧即不等、降级被拒（保守）
        let default_files = collect_files(
            root,
            &abs_root,
            &abs_cwd,
            suite,
            &[entry.default_glob.to_owned()],
            false, // apply_excludes：对照面不施加（框架自发现不认这套 excludes）
        )?;
        // 两侧均由 `BTreeSet` 导出（排序去重），故逐项相等即集合相等
        let omit_files_safe = default_files.len() == files.len()
            && default_files
                .iter()
                .zip(files.iter())
                .all(|(detected, listed)| detected == listed);

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
            omit_files_safe,
        });
    }
    Ok(plans)
}

/// glob 文件探测（includes 发现面）：模式取 suite `includes`（缺省吃框架
/// `default_glob`）、施加 suite `excludes`。收集主体单点驻 [`collect_files`]
/// （includes 发现面与降级对照面共用，MUST NOT 两写 glob 方言 / 剪枝）。
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
    Ok(collect_files(
        project_root,
        abs_root,
        abs_cwd,
        suite,
        &include_patterns,
        true, // apply_excludes：发现面施加 suite excludes
    )?
    .into_iter()
    .collect())
}

/// glob 收集主体（单一方言 / 单点剪枝）：花括号交替先展开 → `glob` → dot 与
/// 依赖产物目录剪枝（**恒施加**）→ suite `excludes` 过滤（末位开关为 `false` 时
/// 跳过——降级对照面用：框架自发现不认这套 excludes）→ cwd 相对 POSIX 入
/// `BTreeSet`（排序去重，集合比较即 `Vec` 相等）。
fn collect_files(
    project_root: &Path,
    abs_root: &Path,
    abs_cwd: &Path,
    suite: &config::TestSuite,
    patterns: &[String],
    apply_excludes: bool,
) -> Result<BTreeSet<String>, String> {
    let mut detected: BTreeSet<String> = BTreeSet::new();
    for include in patterns {
        // 花括号交替先展开（`glob` crate 无此方言——CLI picomatch 语义对齐；
        // 缺省 default_glob `**/*.{test,spec}.{js,ts,jsx,tsx}` 即此形态）
        for variant in try_expand_braces(include)
            .map_err(|error| format!("suite root \"{}\" includes glob 非法: {error}", suite.root))?
        {
            let pattern = format!("{}/{}", to_posix(abs_root).trim_end_matches('/'), variant);
            let paths = glob(&pattern).map_err(|error| {
                format!("suite root \"{}\" includes glob 非法: {error}", suite.root)
            })?;
            for path in paths.flatten() {
                if !path.is_file() {
                    continue;
                }
                let Ok(project_rel) = path.strip_prefix(project_root) else {
                    continue;
                };
                let project_rel = to_posix(project_rel);
                if is_pruned_path(Path::new(&project_rel)) {
                    continue;
                }
                if apply_excludes && is_excluded_by_suite(&project_rel, suite) {
                    continue;
                }
                // cwd 相对形式入清单（`{files}` 展开原料）
                let Ok(cwd_rel) = path.strip_prefix(abs_cwd) else {
                    continue;
                };
                detected.insert(to_posix(cwd_rel));
            }
        }
    }
    Ok(detected)
}

/// 探测剪枝（项目相对路径面）：组件名判定委托 foundation 名单单点（dot 剪枝
/// 与依赖 / 工具产物目录名封闭集两消费点同源——detect 剪枝与 vcs
/// `restore_baseline` 排除面）；路径组件迭代语义驻本消费点。
fn is_pruned_path(path: &Path) -> bool {
    path.components().any(|component| {
        foundation::prune::is_pruned_dir_name(&component.as_os_str().to_string_lossy())
    })
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
        match_glob(&project_scoped, &scoped) || match_glob(&rel_to_root, exclude)
    })
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
    let mut command = exec::SystemCommand::script(version_command).into_std();
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
