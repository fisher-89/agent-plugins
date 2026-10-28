//! 运行期预算装置（AC-18 / design §6「运行期预算」）：**1000 源文件 + 300 元素**的夹具上
//! 逐次跑全量判定 `check`，取 p95 判定「单次检查 ≤ 3 s（硬线）/ 1.5 s（软线）」。
//!
//! 口径（与 `tests/README.md`「三、实测表 · 运行期预算」同名行逐字对应）：
//! - **release 构建**：`cargo test -p archi --release --test perf -- --nocapture`
//!   （`--nocapture` 才看得到逐样本耗时与 p50 / p95；test-execution 相位据此回填
//!   `tests/README.md` 的「实测 p95」格与 `design.md` §6 同格）；
//! - 计时只用 `std::time::Instant`，零统计框架（一次性门禁用不上 criterion），也零
//!   第三方断言库（与既有 Rust 用例同风格）；
//! - 样本口径：夹具装载后的那一次判定兼作**预热**（排除首轮冷页缓存 / 目录项缓存），
//!   随后**计时 10 次**（design「10 次取 p95」）；p95 用最近秩法（`n = 10` ⇒ 落在最慢
//!   一次，从紧）；
//! - debug 构建（`cargo test -p archi --test perf`）只取 1 个样本做装置连通性与夹具
//!   完整性验证，**硬线判据只在 release 口径下生效**——design 明定 p95 由 test-execution
//!   相位在 release 口径执行后回填，MUST NOT 以 debug 跑值充数。
//!
//! 越硬线的处置（design「二、回退序」，代价递增、逐步重测）：① `oxc_parser` 只开 parse
//! （已在钉版时执行：`default-features = false`）→ ② 自算路径替代 `oxc_resolver` →
//! ③ 正则 import（须回到设计讨论）。回退后重测，并把两轮数据一并留在
//! `tests/README.md`「三、实测表」——本装置只出数字与判据，零写盘、不改文档。
//!
//! 夹具形状（与 proposal「运行期预算」行一致，且刻意避开排除矩阵的七条默认约定，否则
//! 命中的文件不计入 `scannedFiles`）：300 个平级元素 `E000`…`E299`（FQN 无点 ⇒ 零孤儿
//! 层级诊断），每个元素独占目录 `src/eNNN/`（互不为前缀 ⇒ 归属唯一），前 100 个元素各
//! 4 个文件、其余各 3 个 ⇒ 恰 1000 个源文件；每文件恰好一处跨元素 import（value 与
//! type-only 交替，两形态同权），`from` 侧 gate 按 strict / warn 交替定级。

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use archi::{check, ArchiCheckStatus, ArchiDriftMode, ArchiDriftReport, CheckInput};
use foundation::cancel::CancelFlag;
use foundation::layout;

/// 模型元素数（proposal「300 元素」）。
const ELEMENT_COUNT: usize = 300;

/// 源文件数（proposal「1000 源文件」）。
const FILE_COUNT: usize = 1000;

/// 带 4 个文件的元素个数（其余 3 个）：`100 × 4 + 200 × 3 = 1000`。
const WIDE_ELEMENTS: usize = 100;

/// 软线（`tests/README.md`「一、阈值」：单次检查 1.5 s）。
const SOFT_LINE: Duration = Duration::from_millis(1500);

/// 硬线（`tests/README.md`「一、阈值」：单次检查 3 s，p95，release）。
const HARD_LINE: Duration = Duration::from_secs(3);

/// debug 口径的连通性上限（**不是**性能判据：只兜住「装置本身跑爆」这类异常）。
const DEBUG_SANITY_LINE: Duration = Duration::from_secs(60);

// ---------------------------------------------------------------------------
// 真实临时目录（本 crate 无 dev-dependency，故用 std 自建）
// ---------------------------------------------------------------------------

static COUNTER: AtomicU64 = AtomicU64::new(0);

/// 夹具根目录（RAII：`Drop` 整棵删；与 `dto_test` / `parse/mod_test` 同式）。
struct Fixture {
    root: PathBuf,
    /// `strict` 档引入方期望报出的违例条数（元素对去重后）
    strict_pairs: usize,
    /// `warn` 档引入方期望报出的提示条数（元素对去重后）
    warn_pairs: usize,
}

impl Fixture {
    /// 落模型（1 份 300 元素）+ 1000 个源文件，并同时算出期望的元素对计数。
    fn build() -> Fixture {
        let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
        let root =
            std::env::temp_dir().join(format!("archi-perf-test-{}-{}", std::process::id(), unique));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("建夹具根失败");

        // ① 模型：300 个平级元素（`gate` 按 strict / warn 交替，两档各半边天）
        let models = layout::archi_models_dir(&root);
        fs::create_dir_all(&models).expect("建模型目录失败");
        let mut model = String::from(
            "# 运行期预算夹具（AC-18）：300 元素 / 1000 源文件，由 tests/perf.rs 生成\n",
        );
        for element in 0..ELEMENT_COUNT {
            model.push_str(&format!(
                "\n[element.\"E{element:03}\"]\nkind = \"module\"\ngate = \"{}\"\npath = [\"./src/e{element:03}/\"]\n",
                gate_of(element)
            ));
        }
        fs::write(models.join("01-perf.toml"), model).expect("写模型失败");

        // ② 源文件：逐元素目录，文件数 `files_of`，每文件一处跨元素 import
        let mut strict_pairs: BTreeSet<(usize, usize)> = BTreeSet::new();
        let mut warn_pairs: BTreeSet<(usize, usize)> = BTreeSet::new();
        let mut files = 0usize;
        for element in 0..ELEMENT_COUNT {
            let directory = root.join(format!("src/e{element:03}"));
            fs::create_dir_all(&directory).expect("建元素目录失败");
            for index in 0..files_of(element) {
                let target = target_of(element, index);
                fs::write(
                    directory.join(format!("f{index}.ts")),
                    source_of(element, index, target),
                )
                .expect("写源文件失败");
                if element.is_multiple_of(2) {
                    strict_pairs.insert((element, target));
                } else {
                    warn_pairs.insert((element, target));
                }
                files += 1;
            }
        }

        assert_eq!(files, FILE_COUNT, "夹具源文件数必须是 1000");
        Fixture {
            root,
            strict_pairs: strict_pairs.len(),
            warn_pairs: warn_pairs.len(),
        }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

/// 元素 `element` 名下的源文件数（前 `WIDE_ELEMENTS` 个 4 个、其余 3 个）。
fn files_of(element: usize) -> usize {
    if element < WIDE_ELEMENTS {
        4
    } else {
        3
    }
}

/// 元素 `element` 的第 `index` 个文件所 import 的目标元素（恒 ≠ 自身：同元素内边被
/// 剔除，构造不出依赖事实）。
fn target_of(element: usize, index: usize) -> usize {
    let candidate = (element * 7 + index * 13 + 5) % ELEMENT_COUNT;
    if candidate == element {
        (element + 1) % ELEMENT_COUNT
    } else {
        candidate
    }
}

/// 引入方 `gate` 档（下标偶数 → `strict`，奇数 → `warn`）。
fn gate_of(element: usize) -> &'static str {
    if element.is_multiple_of(2) {
        "strict"
    } else {
        "warn"
    }
}

/// 单文件源码：偶数下标写 value import、奇数下标写 `import type`（两形态同权）。
fn source_of(element: usize, index: usize, target: usize) -> String {
    if index.is_multiple_of(2) {
        format!(
            "import {{ value as imported }} from '../e{target:03}/f0';\n\nexport const value = {element} * 100 + {index};\n"
        )
    } else {
        format!(
            "// type-only 与 value 两形态同权（AC-4 交汇）\nimport type {{ Value as ImportedType }} from '../e{target:03}/f0';\n\nexport type Alias = ImportedType;\n"
        )
    }
}

// ---------------------------------------------------------------------------
// 判定驱动与计时口径
// ---------------------------------------------------------------------------

/// 全量判定一次（`changed` 空、无 change 上下文、零取消）。
fn run_full(root: &Path) -> ArchiDriftReport {
    check(CheckInput {
        root,
        mode: ArchiDriftMode::Full,
        changed: &[],
        change: None,
        cancel: CancelFlag::never(),
    })
    .expect("夹具合法（模型 / 源文件齐备、零配置、零结构错），全量判定不得失败")
}

/// 计时样本数：release 取 design 口径的 10 次；debug 只取 1 次（见模块 doc）。
const fn sample_budget() -> usize {
    if cfg!(debug_assertions) {
        1
    } else {
        10
    }
}

/// 最近秩法百分位：`sorted[ceil(percent × n / 100) − 1]`（`percent ∈ (0, 100]`）。
///
/// `n = 10` 时 p95 落在最慢一次（从紧读法：单次离群值不被稀释）；`n = 1` 时 p50 / p95
/// 都等于该次。
fn percentile(samples: &[Duration], percent: u32) -> Duration {
    assert!(!samples.is_empty(), "计时样本不得为空");
    assert!(
        (1..=100).contains(&percent),
        "百分位须落在 1..=100，实际 {percent}"
    );
    let mut sorted: Vec<Duration> = samples.to_vec();
    sorted.sort_unstable();
    let rank = (percent as usize * sorted.len()).div_ceil(100).max(1);
    sorted[rank - 1]
}

/// 毫秒读数（打印用；`Duration` 无 `as_millis_f64`）。
fn millis(sample: Duration) -> f64 {
    sample.as_secs_f64() * 1000.0
}

// ---------------------------------------------------------------------------
// 装置自检（算术面：本文件的判据由 `percentile` 出，先锁它）
// ---------------------------------------------------------------------------

/// 最近秩法：`n = 10` 的 p95 取最慢一次、p50 取第 5 个；`n = 1` 两值同解；`n = 20` 的
/// p95 落在第 19 个。
#[test]
fn 最近秩p95取最慢样本而p50取中位() {
    let samples: Vec<Duration> = (1..=10).map(Duration::from_millis).collect();
    assert_eq!(
        percentile(&samples, 95),
        Duration::from_millis(10),
        "n = 10 的 p95 = 最慢一次（最近秩 ceil(9.5) = 10）"
    );
    assert_eq!(
        percentile(&samples, 50),
        Duration::from_millis(5),
        "n = 10 的 p50 = 第 5 个样本"
    );

    let single = [Duration::from_millis(7)];
    assert_eq!(
        percentile(&single, 50),
        Duration::from_millis(7),
        "单样本 p50 = 该样本"
    );
    assert_eq!(
        percentile(&single, 95),
        Duration::from_millis(7),
        "单样本 p95 = 该样本（debug 口径不因样本少而爆）"
    );

    let twenty: Vec<Duration> = (1..=20).map(Duration::from_millis).collect();
    assert_eq!(
        percentile(&twenty, 95),
        Duration::from_millis(19),
        "n = 20 的 p95 = 第 19 个（ceil(19) = 19）"
    );
    assert_eq!(
        percentile(&twenty, 100),
        Duration::from_millis(20),
        "p100 = 最慢一次"
    );
}

// ---------------------------------------------------------------------------
// 主装置：p95 ≤ 硬线（AC-18 运行期预算）
// ---------------------------------------------------------------------------

/// 正向 + 边界（AC-18）：1000 源文件 / 300 元素夹具上的全量判定 p95 不越硬线（release
/// 口径）。同时锁夹具完整性——判据若跑在「文件没落盘 / 归属失配」的空转面上，计时再低
/// 也是假证据。
#[test]
fn 千文件三百元素夹具上的全量判定p95不越硬线() {
    let fixture = Fixture::build();

    // ① 预热兼夹具自证：这一次判定不计时，但必须真的走了 1000 文件 / 300 元素的面
    let baseline = run_full(&fixture.root);
    assert_eq!(
        baseline.model.elements, ELEMENT_COUNT as u32,
        "模型元素数须为 300（夹具落盘即被测面）"
    );
    assert_eq!(
        baseline.scan.scanned_files, FILE_COUNT as u32,
        "可判源文件数须为 1000（无文件被排除矩阵吃掉）"
    );
    assert!(
        baseline.scan.excluded.is_empty(),
        "夹具刻意避开七条默认排除约定，不得有被排除文件：{:?}",
        baseline.scan.excluded.first()
    );
    assert!(
        baseline.scan.unmatched_files.is_empty(),
        "每个元素独占目录 ⇒ 零无主文件：{:?}",
        baseline.scan.unmatched_files.first()
    );
    assert!(
        baseline.model.structural_errors.is_empty(),
        "平级 FQN + 零关系 ⇒ 零结构诊断：{:?}",
        baseline.model.structural_errors.first()
    );
    assert_eq!(
        baseline.scan.matched.len(),
        ELEMENT_COUNT,
        "300 个元素各自认领到源文件（归属索引真的被逐文件走了一遍）"
    );
    assert!(
        baseline.scan.unresolved.is_empty(),
        "夹具零语法错 / 零动态 import ⇒ 不可判定留痕为空：{:?}",
        baseline.scan.unresolved.first()
    );
    assert_eq!(
        baseline.errors.len(),
        fixture.strict_pairs,
        "strict 档引入方的未声明依赖按元素对聚合后条数一致"
    );
    assert_eq!(
        baseline.warnings.len(),
        fixture.warn_pairs,
        "warn 档引入方的未声明依赖按元素对聚合后条数一致"
    );
    assert_eq!(
        baseline.status,
        ArchiCheckStatus::Errors,
        "有 strict 档违例 ⇒ status = errors（夹具确实压到判定链路末端）"
    );

    // ② 计时（release 10 次、debug 1 次；逐次判定必须做同样的工作量）
    let budget = sample_budget();
    let mut samples: Vec<Duration> = Vec::with_capacity(budget);
    let mut violations = 0usize;
    for _ in 0..budget {
        let started = Instant::now();
        let report = run_full(&fixture.root);
        let elapsed = started.elapsed();
        violations += report.errors.len();
        samples.push(elapsed);
    }
    assert_eq!(samples.len(), budget, "样本数与口径一致");
    assert_eq!(
        violations,
        fixture.strict_pairs * budget,
        "每次计时判定都报出同一批违例（工作量未因缓存 / 短路而塌陷）"
    );

    // ③ 判读：p50 / p95 与阈值对照（打印面即 test-execution 回填 `tests/README.md` 的
    //    数据源：`cargo test -p archi --release --test perf -- --nocapture`）
    let p50 = percentile(&samples, 50);
    let p95 = percentile(&samples, 95);
    let rendered: Vec<String> = samples
        .iter()
        .map(|s| format!("{:.1}", millis(*s)))
        .collect();
    println!("—— 运行期预算（AC-18）：1000 源文件 / 300 元素 / 全量判定 ——");
    println!(
        "  构建口径：{}",
        if cfg!(debug_assertions) {
            "debug（连通性验证，非回填口径）"
        } else {
            "release（回填口径）"
        }
    );
    println!(
        "  样本（ms，n = {}）：[{}]",
        samples.len(),
        rendered.join(", ")
    );
    println!(
        "  p50 = {:.1} ms / p95 = {:.1} ms（最近秩法）",
        millis(p50),
        millis(p95)
    );
    println!(
        "  阈值：软线 {:.1} s / 硬线 {:.1} s",
        SOFT_LINE.as_secs_f64(),
        HARD_LINE.as_secs_f64()
    );
    println!(
        "  tests/README.md「运行期预算」实测 p95 格：{:.2} s",
        p95.as_secs_f64()
    );

    if cfg!(debug_assertions) {
        println!(
            "  判读：debug 口径只验装置连通性（1 个样本）——硬线判据只在 release 口径生效，\
             回填请跑 `cargo test -p archi --release --test perf -- --nocapture`"
        );
        assert!(
            p95 < DEBUG_SANITY_LINE,
            "debug 口径连通性上限 {:.0} s 被突破（装置异常，非性能判据）：p95 = {:.1} ms",
            DEBUG_SANITY_LINE.as_secs_f64(),
            millis(p95)
        );
        return;
    }

    if p95 > SOFT_LINE && p95 <= HARD_LINE {
        println!(
            "  判读：越软线（{:.1} s）未越硬线 ⇒ 通过但留痕：进 spec「已知成本」清单并在 \
             proposal 记录",
            SOFT_LINE.as_secs_f64()
        );
    }
    assert!(
        p95 <= HARD_LINE,
        "单次检查 p95 越硬线 {:.1} s：实际 {:.1} ms（软线 {:.1} s）。按 design「二、回退序」\
         执行留痕：① `oxc_parser` 只开 parse（`default-features = false`，钉版时已执行）→ \
         ② 自算路径替代 `oxc_resolver` → ③ 正则 import（须回设计讨论）；回退后重测，并把\
         两轮数据一并留在 tests/README.md「三、实测表」",
        HARD_LINE.as_secs_f64(),
        millis(p95),
        SOFT_LINE.as_secs_f64()
    );
}
