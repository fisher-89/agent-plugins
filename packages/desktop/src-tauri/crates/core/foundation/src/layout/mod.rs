use std::path::{Path, PathBuf};

/// 域根目录名。
const DOMAIN_DIR_NAME: &str = "openspec";

/// 进行中 change 子目录名（相对域根锚定）
const CHANGES_DIR_NAME: &str = "changes";

/// 已归档 change 子目录名（相对 changes 子目录锚定，物理嵌套于 changes 之内）
const ARCHIVE_DIR_NAME: &str = "archive";

/// 探索笔记子目录名（相对域根锚定）
const EXPLORES_DIR_NAME: &str = "explores";

/// 工作区配置文件名（相对域根锚定）
const CONFIG_FILE_NAME: &str = "config.json";

/// change 报告子目录名（相对 change 目录锚定）
const REPORTS_DIR_NAME: &str = "reports";

/// 测试报告子目录名（相对 reports 子目录锚定）
const TEST_REPORTS_DIR_NAME: &str = "test";

/// 基线报告子目录名（相对 reports 子目录锚定）
const BASELINE_REPORTS_DIR_NAME: &str = "baseline";

/// 架构域顶层目录名（相对 workspace 根锚定；`.dev-team` 隐藏目录——架构资产与
/// `openspec/` 变更语料分树；与壳层数据根 `<home>/.dev-team` 同名不同物）
const ARCHI_ROOT_DIR_NAME: &str = ".dev-team";

/// 架构域子目录名（相对架构域顶层锚定；架构域段名的唯一持有者）
const ARCHITECTURE_DIR_NAME: &str = "architecture";

/// 架构模型子目录名（相对架构域根锚定）
const MODELS_DIR_NAME: &str = "models";

/// 架构决策记录（ADR）子目录名（相对架构域根锚定）
const DECISIONS_DIR_NAME: &str = "decisions";

/// 报告槽位（封闭二值，纯进程内枚举——零 serde / specta，不进 IPC）：`Test`
/// 为缺省槽位（`reports/test`——run 内门禁与 test-execution 相位语义零改动），
/// `Baseline` 为 initialize 基线检查槽位（`reports/baseline`——两报告树分树
/// 隔离，零混居）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReportSlot {
    /// 缺省槽位：`reports/test`（既有语义）
    Test,
    /// 基线检查槽位：`reports/baseline`（initialize 相位两检查步）
    Baseline,
}

/// workspace 磁盘布局：进行中 / 已归档 / 探索笔记三棵目录树。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Layout {
    /// 进行中 change 目录树
    pub changes_root: PathBuf,
    /// 已归档 change 目录树
    pub archive_root: PathBuf,
    /// 探索笔记目录树
    pub explores_root: PathBuf,
}

/// 域根目录名（裸名消费方唯一来源）
pub fn domain_dir_name() -> &'static str {
    DOMAIN_DIR_NAME
}

/// 把用户选定的 workspace 根目录解析为布局结构。
pub fn resolve(root: &Path) -> Layout {
    let domain_root = root.join(DOMAIN_DIR_NAME);
    Layout {
        changes_root: domain_root.join(CHANGES_DIR_NAME),
        archive_root: domain_root.join(CHANGES_DIR_NAME).join(ARCHIVE_DIR_NAME),
        explores_root: domain_root.join(EXPLORES_DIR_NAME),
    }
}

/// 把 workspace 根目录解析为配置文件路径
pub fn config_path(root: &Path) -> PathBuf {
    root.join(DOMAIN_DIR_NAME).join(CONFIG_FILE_NAME)
}

/// 槽位 → reports 子目录名映射（两树后缀单点——消费方零路径字面量扩散）。
fn report_dir_name(slot: ReportSlot) -> &'static str {
    match slot {
        ReportSlot::Test => TEST_REPORTS_DIR_NAME,
        ReportSlot::Baseline => BASELINE_REPORTS_DIR_NAME,
    }
}

/// 槽位驱动的 change 报告目录单点（`reports/<test|baseline>` 后缀同源派生；
/// 两命名变体委托本函数）。
pub fn change_reports(root: &Path, change: &str, slot: ReportSlot) -> PathBuf {
    change_reports_in(&resolve(root).changes_root.join(change), slot)
}

/// [`change_reports`] 的 change 目录变体（exec-outputs 锚定消费）。
pub fn change_reports_in(change_dir: &Path, slot: ReportSlot) -> PathBuf {
    change_dir
        .join(REPORTS_DIR_NAME)
        .join(report_dir_name(slot))
}

/// 把 workspace 根目录与 change 名解析为 change 测试报告目录（缺省槽位命名
/// 变体——既有调用点零感知）。
pub fn change_test_reports(root: &Path, change: &str) -> PathBuf {
    change_reports(root, change, ReportSlot::Test)
}

/// 把 change 目录解析为该 change 的测试报告目录（缺省槽位命名变体；`reports/test`
/// 后缀单点——change 目录由 `locate_change` 类三态定位面供给，worktree / active /
/// archive 各态目录同后缀派生）。
pub fn change_test_reports_in(change_dir: &Path) -> PathBuf {
    change_reports_in(change_dir, ReportSlot::Test)
}

/// 把 workspace 根目录与 change 名解析为 change 基线报告目录（baseline 槽位
/// 命名变体）。
pub fn change_baseline_reports(root: &Path, change: &str) -> PathBuf {
    change_reports(root, change, ReportSlot::Baseline)
}

/// [`change_baseline_reports`] 的 change 目录形态（baseline 槽位命名变体）。
pub fn change_baseline_reports_in(change_dir: &Path) -> PathBuf {
    change_reports_in(change_dir, ReportSlot::Baseline)
}

/// 架构域根目录派生单点（`<root>/.dev-team/architecture`，架构域顶层段与架构
/// 段名的唯一持有者）。纯推导零 IO——不存在的 root 亦正常返回（存在性检查交上层）。
pub fn archi_dir(root: &Path) -> PathBuf {
    root.join(ARCHI_ROOT_DIR_NAME).join(ARCHITECTURE_DIR_NAME)
}

/// 架构模型目录派生（`<架构域根>/models`；全从 [`archi_dir`] 派生，架构域段名
/// 不二次出现）。
pub fn archi_models_dir(root: &Path) -> PathBuf {
    archi_dir(root).join(MODELS_DIR_NAME)
}

/// 架构决策记录目录派生（`<架构域根>/decisions`；全从 [`archi_dir`] 派生）。
pub fn archi_decisions_dir(root: &Path) -> PathBuf {
    archi_dir(root).join(DECISIONS_DIR_NAME)
}

/// repo 相对路径是否落在架构域之内（`/` 与 `\` 分隔均归一后判定）：判据 = 归一
/// 剥前导 `./` 与尾分隔符后，前两段依序为架构域顶层段与架构域段。`<顶层>/architecture`
/// 与 `<顶层>/architecture/models/x.toml` 均命中，`openspec/architecture/...` 与
/// `<顶层>/archived/...` 不命中。归档冲突分流的唯一判据——core 侧 MUST NOT 比较
/// 架构域段名字面量。
pub fn is_archi_rel_path(rel: &str) -> bool {
    let normalized = rel.replace('\\', "/");
    let trimmed = normalized.trim_start_matches("./").trim_end_matches('/');
    let segments: Vec<&str> = trimmed.split('/').collect();
    segments.len() >= 2
        && segments[0] == ARCHI_ROOT_DIR_NAME
        && segments[1] == ARCHITECTURE_DIR_NAME
}

/// 架构域根的 repo 相对路径（`<顶层>/architecture`）——架构域相对路径族的单点，
/// 唯一消费者为架构会话知识文本（模型可见文本直接用，故 `/` 分隔、零前导 `./`）。
/// 纯推导零 IO，不接 root 入参。
pub fn archi_rel_dir() -> String {
    format!("{ARCHI_ROOT_DIR_NAME}/{ARCHITECTURE_DIR_NAME}")
}

/// 架构模型目录的 repo 相对路径（`<archi_rel_dir>/models`；全从 [`archi_rel_dir`]
/// 派生，架构域段名不二次出现）。
pub fn archi_models_rel_dir() -> String {
    format!("{}/{}", archi_rel_dir(), MODELS_DIR_NAME)
}

/// 架构决策记录目录的 repo 相对路径（`<archi_rel_dir>/decisions`；全从
/// [`archi_rel_dir`] 派生）。
pub fn archi_decisions_rel_dir() -> String {
    format!("{}/{}", archi_rel_dir(), DECISIONS_DIR_NAME)
}

#[cfg(test)]
mod mod_test;
