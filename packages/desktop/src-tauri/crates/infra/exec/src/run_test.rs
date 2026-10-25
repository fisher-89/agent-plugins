use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use foundation::cancel::CancelFlag;
use tempfile::TempDir;

use super::{
    run_shell, RunError, RunOptions, RunOutcome, RunReport, CANCEL_POLL_INTERVAL, FINISH_GRACE,
};
use crate::transient::RetryPolicy;
use crate::SystemCommand;

// ---------------------------------------------------------------------------
// 装置：临时 workspace + POSIX shim（解释器经 `resolve_shell` 探测）+ pid 探测
// ---------------------------------------------------------------------------

/// 测试 workspace：临时目录（shim 脚本 / payload / pid 文件落位）。
struct ShimWs {
    dir: TempDir,
}

impl ShimWs {
    fn new(tag: &str) -> Self {
        let dir = tempfile::Builder::new()
            .prefix(&format!("exec-run-{tag}-"))
            .tempdir()
            .expect("创建临时 workspace 失败");
        Self { dir }
    }

    fn path(&self) -> &Path {
        self.dir.path()
    }

    fn file(&self, name: &str) -> PathBuf {
        self.path().join(name)
    }

    /// 落盘文本文件（payload / 标记 / 任何被 shim 读取的原料）。
    fn put(&self, name: &str, body: &str) -> PathBuf {
        let path = self.file(name);
        std::fs::write(&path, body).expect("写临时文件失败");
        path
    }

    fn put_bytes(&self, name: &str, body: &[u8]) -> PathBuf {
        let path = self.file(name);
        std::fs::write(&path, body).expect("写临时二进制文件失败");
        path
    }

    /// 落盘 POSIX shim 脚本并返回可直拉的命令（解释器 + 脚本绝对路径）。
    fn shim(&self, name: &str, body: &str) -> SystemCommand {
        let path = self.put(name, body);
        interpreter_command(&path)
    }

    fn read_pid(&self, name: &str) -> Option<u32> {
        std::fs::read_to_string(self.file(name))
            .ok()
            .and_then(|text| text.trim().parse().ok())
    }
}

/// 解释器命令：Windows 走 `resolve_shell` 探测到的 Git bash（POSIX 语义统一
/// 两平台），unix 走 `sh`。脚本文件为唯一入参（无 shell 转义面）。
fn interpreter_command(script: &Path) -> SystemCommand {
    SystemCommand::bare(interpreter()).arg(script)
}

#[cfg(windows)]
fn interpreter() -> OsString {
    match crate::resolve_shell() {
        crate::ShellBase::GitBash(path) => path.into_os_string(),
        _ => panic!("run_test 需要 Git for Windows bash（resolve_shell 未探测到 GitBash）"),
    }
}

#[cfg(not(windows))]
fn interpreter() -> OsString {
    OsString::from("sh")
}

/// 缺省执行入参（`cwd = None` / `env = []` / `cancel = never()`）。
fn options(timeout: Duration) -> RunOptions {
    RunOptions {
        cwd: None,
        env: Vec::new(),
        timeout,
        cancel: CancelFlag::never(),
        retry: RetryPolicy::none(),
    }
}

/// pid 存活探测（Windows `tasklist`、unix `kill -0`）。
#[cfg(windows)]
fn pid_alive(pid: u32) -> bool {
    let output = std::process::Command::new("tasklist")
        .args(["/FI", &format!("PID eq {pid}"), "/NH", "/FO", "CSV"])
        .output();
    match output {
        Ok(output) => String::from_utf8_lossy(&output.stdout).contains(&pid.to_string()),
        Err(_) => false,
    }
}

#[cfg(not(windows))]
fn pid_alive(pid: u32) -> bool {
    std::process::Command::new("kill")
        .args(["-0", &pid.to_string()])
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

/// 等待 pid 消失（杀树是尽力语义 + 平台回收时延，轮询到上界即判定仍存活）。
fn wait_pid_gone(pid: u32, budget: Duration) -> bool {
    let deadline = Instant::now() + budget;
    while Instant::now() < deadline {
        if !pid_alive(pid) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    !pid_alive(pid)
}

/// 路径归一（bash `pwd -W` 的 `C:/x` 与 `pwd` 的 `/c/x` 与宿主 cwd 同形比较）。
fn normalize_path(raw: &str) -> String {
    let lowered = raw.trim().replace('\\', "/").to_lowercase();
    let trimmed = lowered
        .strip_prefix("/c/")
        .or_else(|| lowered.strip_prefix("c:/"))
        .map(str::to_owned)
        .unwrap_or(lowered);
    trimmed.trim_end_matches('/').to_owned()
}

// ---------------------------------------------------------------------------
// 正向：单流 / 退出码谱 / 双缓冲不合流
// ---------------------------------------------------------------------------

/// 正向：单流正常退出——`Completed` + `exit_code == 0` + stdout 含回显文本 +
/// stderr 空（缺省 `cwd = None` / `env = []` / `cancel = never()`）。
#[tokio::test]
async fn 单流正向_回显文本且零退出() {
    let report = run_shell(
        SystemCommand::script("echo hi"),
        options(Duration::from_secs(30)),
    )
    .await
    .expect("正常退出应 Ok");

    assert_eq!(report.outcome, RunOutcome::Completed);
    assert_eq!(report.exit_code, 0, "零退出码直传");
    let stdout = String::from_utf8_lossy(&report.stdout);
    assert!(stdout.contains("hi"), "stdout 含回显文本: {stdout:?}");
    assert!(report.stderr.is_empty(), "零 stderr: {:?}", report.stderr);
}

/// 正向（参数化 4 值）：退出码谱 `0 / 1 / 3 / 255` 逐值等值透传——`outcome`
/// 恒 `Completed`（非零退出不是执行体失败面），哨兵 `-1` 不被非零码误用。
#[tokio::test]
async fn 退出码谱_逐值等值透传且不用哨兵() {
    for code in [0i32, 1, 3, 255] {
        let report = run_shell(
            SystemCommand::script(&format!("exit {code}")),
            options(Duration::from_secs(30)),
        )
        .await
        .unwrap_or_else(|error| panic!("exit {code} 应跑完并返回报告: {error}"));

        assert_eq!(
            report.outcome,
            RunOutcome::Completed,
            "exit {code} 仍为 Completed"
        );
        assert_eq!(report.exit_code, code, "退出码逐值透传（exit {code}）");
        assert_ne!(report.exit_code, -1, "谱内退出码不得与哨兵 -1 混淆");
    }
}

/// 正向：双缓冲不合流——执行体不做 `2>&1`、不拼接：stdout / stderr 各自只含
/// 本路字节（D14）。
#[tokio::test]
async fn 双缓冲不合流_两路各自只含本路字节() {
    let ws = ShimWs::new("split");
    let command = ws.shim(
        "split.sh",
        "#!/bin/sh\nprintf 'OUT-TOKEN'\nprintf 'ERR-TOKEN' >&2\n",
    );

    let report = run_shell(command, options(Duration::from_secs(30)))
        .await
        .expect("正常退出应 Ok");
    assert_eq!(report.outcome, RunOutcome::Completed);
    assert_eq!(report.stdout, b"OUT-TOKEN", "stdout 只含本路字节");
    assert_eq!(report.stderr, b"ERR-TOKEN", "stderr 只含本路字节");
}

// ---------------------------------------------------------------------------
// 正向 / 边界：cwd 与 env 施加
// ---------------------------------------------------------------------------

/// 正向（表驱动 2 形态）：`cwd` 生效——`Some(tempdir)` 使相对路径探测命中；
/// `None` 继承测试进程 cwd（同一探测在该目录不存在）。
#[tokio::test]
async fn cwd生效_some施加与none继承两形态() {
    let ws = ShimWs::new("cwd");
    ws.put("probe.txt", "probe-body");
    let command_body = "#!/bin/sh\ncat probe.txt 2>/dev/null || printf 'MISS'\n";

    // 形态一：cwd = Some(tempdir) → 相对路径探测命中
    let mut with_cwd = options(Duration::from_secs(30));
    with_cwd.cwd = Some(ws.path().to_path_buf());
    let report = run_shell(ws.shim("probe.sh", command_body), with_cwd)
        .await
        .expect("cwd 施加后应 Ok");
    assert_eq!(report.outcome, RunOutcome::Completed);
    assert_eq!(
        String::from_utf8_lossy(&report.stdout),
        "probe-body",
        "Some(tempdir) 使相对路径探测命中（current_dir 施加生效）"
    );

    // 形态二：cwd = None → 继承测试进程 cwd，探测目录不同 → 未命中
    let report = run_shell(
        ws.shim("probe-inherit.sh", command_body),
        options(Duration::from_secs(30)),
    )
    .await
    .expect("继承 cwd 应 Ok");
    assert_eq!(report.outcome, RunOutcome::Completed);
    assert_ne!(
        String::from_utf8_lossy(&report.stdout),
        "probe-body",
        "cwd = None 时相对探测不落在临时目录（继承测试进程 cwd）"
    );
}

/// 边界（表驱动 2 形态）：`cwd = None` 的继承面以 `pwd` 回显归一后与测试进程
/// cwd 相等（`pwd -W` 的 `C:/x` 与宿主 `C:\x` 归一后同形）。
#[tokio::test]
async fn cwd为none时回显与测试进程cwd同形() {
    let ws = ShimWs::new("cwd-pwd");
    let command = ws.shim(
        "pwd.sh",
        "#!/bin/sh\nwin=$(pwd -W 2>/dev/null) && [ -n \"$win\" ] && printf '%s' \"$win\" || printf '%s' \"$(pwd)\"\n",
    );
    let report = run_shell(command, options(Duration::from_secs(30)))
        .await
        .expect("pwd 应 Ok");

    let printed = String::from_utf8_lossy(&report.stdout).to_string();
    let expected = std::env::current_dir()
        .expect("取测试进程 cwd")
        .to_string_lossy()
        .to_string();
    assert_eq!(
        normalize_path(&printed),
        normalize_path(&expected),
        "cwd = None 继承测试进程 cwd（printed={printed:?} expected={expected:?}）"
    );
}

/// 正向：`env` 生效——shim 回显 `RSE_MARK` 得 `on`；父进程环境不因
/// `RunOptions.env` 被污染（逐次施加，进程级零写入）。
#[tokio::test]
async fn env生效_标记回显且父进程不污染() {
    let ws = ShimWs::new("env");
    let command = ws.shim("env.sh", "#!/bin/sh\nprintf '%s' \"${RSE_MARK:-unset}\"\n");

    let mut with_env = options(Duration::from_secs(30));
    with_env.env = vec![(OsString::from("RSE_MARK"), OsString::from("on"))];
    let report = run_shell(command, with_env).await.expect("env 用例应 Ok");

    assert_eq!(report.outcome, RunOutcome::Completed);
    assert_eq!(
        String::from_utf8_lossy(&report.stdout),
        "on",
        "shim 回显 env 值"
    );
    assert!(
        std::env::var_os("RSE_MARK").is_none(),
        "RunOptions.env 逐次施加，不写入父进程环境"
    );
}

/// 边界（表驱动 4 形态）：`env` 规模与形态——空 / 单元素 / ≥50 项 / 重复键
/// （后值覆盖前值）四种形态均 `Completed` 且回显符合预期。
#[tokio::test]
async fn env规模与形态_空单元素大清单与重复键() {
    let ws = ShimWs::new("env-scale");

    // 形态一：空 env（全继承）
    let empty = ws.shim(
        "env-empty.sh",
        "#!/bin/sh\nprintf '%s' \"${RSE_MARK:-unset}\"\n",
    );
    let report = run_shell(empty, options(Duration::from_secs(30)))
        .await
        .expect("空 env 应 Ok");
    assert_eq!(report.outcome, RunOutcome::Completed);
    assert_eq!(
        String::from_utf8_lossy(&report.stdout),
        "unset",
        "空 env 不注入键"
    );

    // 形态二：单元素
    let single = ws.shim("env-single.sh", "#!/bin/sh\nprintf '%s' \"$RSE_MARK\"\n");
    let mut with_single = options(Duration::from_secs(30));
    with_single.env = vec![(OsString::from("RSE_MARK"), OsString::from("one"))];
    let report = run_shell(single, with_single)
        .await
        .expect("单元素 env 应 Ok");
    assert_eq!(String::from_utf8_lossy(&report.stdout), "one");

    // 形态三：≥50 项
    let large = ws.shim("env-large.sh", "#!/bin/sh\nprintf '%s' \"$RSE_K49\"\n");
    let mut with_large = options(Duration::from_secs(30));
    with_large.env = (0..50)
        .map(|index| {
            (
                OsString::from(format!("RSE_K{index}")),
                OsString::from(format!("v{index}")),
            )
        })
        .collect();
    let report = run_shell(large, with_large)
        .await
        .expect("大清单 env 应 Ok");
    assert_eq!(report.outcome, RunOutcome::Completed);
    assert_eq!(
        String::from_utf8_lossy(&report.stdout),
        "v49",
        "第 50 项可见"
    );

    // 形态四：重复键（后值覆盖前值）
    let dup = ws.shim("env-dup.sh", "#!/bin/sh\nprintf '%s' \"$RSE_DUP\"\n");
    let mut with_dup = options(Duration::from_secs(30));
    with_dup.env = vec![
        (OsString::from("RSE_DUP"), OsString::from("first")),
        (OsString::from("RSE_DUP"), OsString::from("second")),
    ];
    let report = run_shell(dup, with_dup).await.expect("重复键 env 应 Ok");
    assert_eq!(
        String::from_utf8_lossy(&report.stdout),
        "second",
        "重复键后值覆盖前值"
    );
}

// ---------------------------------------------------------------------------
// 异常：Spawn 面与文案
// ---------------------------------------------------------------------------

/// 异常：`cwd` 指向不存在的目录 → `Err(RunError::Spawn(_))`，`Display` 以
/// 「拉起失败: 」起头（`current_dir` 施加期失败归 `Spawn` 面）。
#[tokio::test]
async fn cwd不存在目录_spawn面报错且文案起头() {
    let ws = ShimWs::new("cwd-missing");
    let missing = ws.path().join("no-such-dir").join("nested");
    let mut with_cwd = options(Duration::from_secs(30));
    with_cwd.cwd = Some(missing);

    let error = run_shell(SystemCommand::script("echo hi"), with_cwd)
        .await
        .expect_err("不存在 cwd 应 Spawn 失败");
    match &error {
        RunError::Spawn(_) => {}
        other => panic!("不存在 cwd 应 Spawn 面，实际 {other:?}"),
    }
    assert!(
        error.to_string().starts_with("拉起失败: "),
        "Spawn 文案以类别词起头: {error}"
    );
}

/// 异常：程序不可达 → `Err(RunError::Spawn(_))`；`Display` 只承载类别词 +
/// `io::Error` 原文，不含任何调用点语境前缀（D1：语境由调用点拼）。
#[tokio::test]
async fn 程序不可达_spawn面文案无调用点语境() {
    let error = run_shell(
        SystemCommand::bare("no-such-program-xyz-9f3"),
        options(Duration::from_secs(30)),
    )
    .await
    .expect_err("不可达程序应 Spawn 失败");

    match &error {
        RunError::Spawn(_) => {}
        other => panic!("不可达程序应 Spawn 面，实际 {other:?}"),
    }
    let rendered = error.to_string();
    assert!(rendered.starts_with("拉起失败: "), "类别词起头: {rendered}");
    for caller_context in [
        "static-check",
        "测试命令",
        "cargo-mutants",
        "Stryker",
        "段执行",
    ] {
        assert!(
            !rendered.contains(caller_context),
            "调用点语境不入执行体文案（{caller_context}）: {rendered}"
        );
    }
    assert!(
        rendered.len() > "拉起失败: ".len(),
        "承载 io::Error 原文: {rendered}"
    );
}

/// 异常：`RunError` 两变体渲染——`Spawn` → 「拉起失败: {e}」、`Wait` →
/// 「等待失败: {e}」（真实 `child.wait()` io 失败不可稳定构造，此处直构变体）。
#[test]
fn run_error两变体渲染同型() {
    let spawn = RunError::Spawn("<io-spawn>".to_owned());
    assert_eq!(spawn.to_string(), "拉起失败: <io-spawn>");
    let wait = RunError::Wait("<io-wait>".to_owned());
    assert_eq!(wait.to_string(), "等待失败: <io-wait>");

    // 变体区分（`PartialEq` 派生）
    assert_ne!(spawn, wait, "两变体互不相等");
    assert_eq!(spawn.clone(), RunError::Spawn("<io-spawn>".to_owned()));
    // 空载荷边界：类别词仍完整（无空串 panic）
    assert_eq!(RunError::Wait(String::new()).to_string(), "等待失败: ");
}

// ---------------------------------------------------------------------------
// 边界：类型面 / 空脚本 / 超长串 / 特殊字符 / 非 UTF-8 / 输出规模
// ---------------------------------------------------------------------------

/// 边界：`RunOptions` 四字段面（`cwd` / `env` / `timeout` / `cancel` 无一缺省，
/// `timeout` 必填）+ `Debug` / `Clone` 派生。
#[test]
fn run_options四字段面与派生() {
    let options = RunOptions {
        cwd: Some(PathBuf::from("/tmp/exec-run-field")),
        env: vec![(OsString::from("K"), OsString::from("V"))],
        timeout: Duration::from_millis(1500),
        cancel: CancelFlag::never(),
        retry: RetryPolicy::none(),
    };
    let cloned = options.clone();
    assert_eq!(
        cloned.timeout,
        Duration::from_millis(1500),
        "timeout 逐值保留"
    );
    assert_eq!(cloned.cwd, options.cwd, "cwd 逐值保留");
    assert_eq!(cloned.env, options.env, "env 逐值保留");
    assert_eq!(cloned.cancel, options.cancel, "cancel 身份保留（同一原子）");

    // cwd 的 None 形态同样可表达且可克隆
    let none_cwd = RunOptions {
        cwd: None,
        env: Vec::new(),
        timeout: Duration::from_secs(1),
        cancel: CancelFlag::never(),
        retry: RetryPolicy::none(),
    };
    assert!(none_cwd.clone().cwd.is_none(), "cwd = None 形态保留");

    let debug = format!("{options:?}");
    for field in ["cwd", "env", "timeout", "cancel"] {
        assert!(debug.contains(field), "Debug 字段面含 {field}: {debug}");
    }
}

/// 边界：`RunOutcome` 三值遍历——分支可区分、`Copy` + `Eq` 语义成立（封闭集，
/// 无非法值臂）。
#[test]
fn run_outcome三值遍历与copy_eq语义() {
    let values = [
        RunOutcome::Completed,
        RunOutcome::TimedOut,
        RunOutcome::Cancelled,
    ];
    for value in values {
        let copied = value; // Copy：无需 clone 即可复制
        assert!(copied == value, "{value:?} 自反相等");

        let matched = match copied {
            RunOutcome::Completed => "completed",
            RunOutcome::TimedOut => "timed-out",
            RunOutcome::Cancelled => "cancelled",
        };
        let expected = match value {
            RunOutcome::Completed => "completed",
            RunOutcome::TimedOut => "timed-out",
            RunOutcome::Cancelled => "cancelled",
        };
        assert_eq!(matched, expected, "三值分支可区分");
    }
    assert_ne!(RunOutcome::Completed, RunOutcome::TimedOut);
    assert_ne!(RunOutcome::TimedOut, RunOutcome::Cancelled);
    assert_ne!(RunOutcome::Cancelled, RunOutcome::Completed);
}

/// 边界：空脚本——`Completed` + 壳退出码 `0` + 两路输出皆空（不 panic）。
#[tokio::test]
async fn 空脚本_零退出与双空输出() {
    let report = run_shell(SystemCommand::script(""), options(Duration::from_secs(30)))
        .await
        .expect("空脚本应 Ok");
    assert_eq!(report.outcome, RunOutcome::Completed);
    assert_eq!(report.exit_code, 0, "空脚本壳退出码 0");
    assert!(report.stdout.is_empty(), "stdout 空: {:?}", report.stdout);
    assert!(report.stderr.is_empty(), "stderr 空: {:?}", report.stderr);
}

/// 边界：超长命令串（>1000 字符）——`Completed` 且 stdout 逐字全量（不截断、
/// 不改写）。
#[tokio::test]
async fn 超长命令串_输出逐字全量不截断() {
    let token = "z".repeat(1200);
    let report = run_shell(
        SystemCommand::script(&format!("echo {token}")),
        options(Duration::from_secs(30)),
    )
    .await
    .expect("超长命令串应 Ok");

    assert_eq!(report.outcome, RunOutcome::Completed);
    let stdout = String::from_utf8_lossy(&report.stdout).to_string();
    assert!(
        stdout.contains(&token),
        "1200 字符 token 逐字全量（实际长度 {}）",
        stdout.trim_end().len()
    );
}

/// 边界（参数化 3 形态）：特殊字符输出——引号 / 换行 / emoji 逐字节保留
/// （执行体零转义、零归一化）。
#[tokio::test]
async fn 特殊字符输出_字节逐字相等() {
    let cases: [(&str, &[u8]); 3] = [
        ("引号", "he said \"hi\" and 'bye'".as_bytes()),
        ("换行", b"line-1\nline-2\n\nline-4\n"),
        ("emoji", "🎉 完成 ✅ rocket 🚀".as_bytes()),
    ];
    for (label, payload) in cases {
        let ws = ShimWs::new("special");
        let payload_path = ws.put_bytes("payload.bin", payload);
        let command = ws.shim(
            "special.sh",
            &format!("#!/bin/sh\ncat \"{}\"\n", payload_path.display()),
        );
        let report = run_shell(command, options(Duration::from_secs(30)))
            .await
            .unwrap_or_else(|error| panic!("{label} 用例应 Ok: {error}"));

        assert_eq!(report.outcome, RunOutcome::Completed, "{label}: Completed");
        assert_eq!(report.stdout, payload, "{label}: stdout 字节逐字相等");
        assert!(report.stderr.is_empty(), "{label}: stderr 空");
    }
}

/// 边界：非 UTF-8 原始字节——shim 输出 `0xFF 0xFE` 等裸字节，执行体不做
/// UTF-8 假设、不做 lossy 转换，原始字节逐字保留。
#[tokio::test]
async fn 非utf8原始字节_逐字保留() {
    let ws = ShimWs::new("non-utf8");
    let command = ws.shim("raw.sh", "#!/bin/sh\nprintf 'a\\377\\376b'\n");
    let report = run_shell(command, options(Duration::from_secs(30)))
        .await
        .expect("非 UTF-8 输出应 Ok");

    assert_eq!(report.outcome, RunOutcome::Completed);
    assert_eq!(
        report.stdout,
        vec![b'a', 0xFF, 0xFE, b'b'],
        "非 UTF-8 字节逐字保留（无 lossy 转换）"
    );
    assert!(
        std::str::from_utf8(&report.stdout).is_err(),
        "输出确为非 UTF-8 形态"
    );
}

/// 边界（表驱动 4 形态）：输出规模边界——双空 / 仅 stdout 单字节 / 仅 stderr
/// 单字节 / 双大块 → `RunReport` 逐字节等值、`Completed`。
#[tokio::test]
async fn 输出规模边界_四形态逐字节等值() {
    let big = 64 * 1024 + 1024;
    let cases: [(&str, String, Vec<u8>, Vec<u8>); 4] = [
        (
            "双空",
            "#!/bin/sh\nexit 0\n".to_owned(),
            Vec::new(),
            Vec::new(),
        ),
        (
            "仅 stdout 单字节",
            "#!/bin/sh\nprintf 'S'\n".to_owned(),
            b"S".to_vec(),
            Vec::new(),
        ),
        (
            "仅 stderr 单字节",
            "#!/bin/sh\nprintf 'E' >&2\n".to_owned(),
            Vec::new(),
            b"E".to_vec(),
        ),
        (
            "双大块",
            format!(
                "#!/bin/sh\nprintf '%s' \"{0}\"\nprintf '%s' \"{0}\" >&2\n",
                "q".repeat(big)
            ),
            vec![b'q'; big],
            vec![b'q'; big],
        ),
    ];

    for (label, body, expected_stdout, expected_stderr) in cases {
        let ws = ShimWs::new("sizes");
        let report = run_shell(ws.shim("sizes.sh", &body), options(Duration::from_secs(60)))
            .await
            .unwrap_or_else(|error| panic!("{label} 应 Ok: {error}"));

        assert_eq!(report.outcome, RunOutcome::Completed, "{label}: Completed");
        assert_eq!(
            report.stdout.len(),
            expected_stdout.len(),
            "{label}: stdout 长度"
        );
        assert_eq!(
            report.stderr.len(),
            expected_stderr.len(),
            "{label}: stderr 长度"
        );
        assert_eq!(report.stdout, expected_stdout, "{label}: stdout 逐字节等值");
        assert_eq!(report.stderr, expected_stderr, "{label}: stderr 逐字节等值");
    }
}

/// 正向（AC-5）：双管道并发排水不互锁——shim 交替向 stdout / stderr 各写
/// >64 KiB（超过管道缓冲；单线程顺序读会互锁）→ 两路字节数精确等于写量
/// （不丢字节）+ 墙钟上界断言（不互锁）。
#[tokio::test]
async fn 双管道并发不互锁_两路大块不丢字节() {
    const CHUNK: usize = 2048;
    const ROUNDS: usize = 40; // 40 × 2048 = 81,920 > 64 KiB（单路）
    let expected = CHUNK * ROUNDS;

    let ws = ShimWs::new("dual-pipe");
    let body = format!(
        "#!/bin/sh\npayload='{payload}'\ni=0\nwhile [ \"$i\" -lt {rounds} ]; do\n  printf '%s' \"$payload\"\n  printf '%s' \"$payload\" >&2\n  i=$((i+1))\ndone\n",
        payload = "p".repeat(CHUNK),
        rounds = ROUNDS,
    );
    let command = ws.shim("dual.sh", &body);

    let started = Instant::now();
    let report = run_shell(command, options(Duration::from_secs(120)))
        .await
        .expect("双管道并发应 Ok");
    let elapsed = started.elapsed();

    assert_eq!(report.outcome, RunOutcome::Completed);
    assert_eq!(
        report.stdout.len(),
        expected,
        "stdout 不丢字节（{expected} 字节）"
    );
    assert_eq!(
        report.stderr.len(),
        expected,
        "stderr 不丢字节（{expected} 字节）"
    );
    assert!(
        report.stdout.iter().all(|byte| *byte == b'p'),
        "stdout 内容形态"
    );
    assert!(
        report.stderr.iter().all(|byte| *byte == b'p'),
        "stderr 内容形态"
    );
    assert!(
        elapsed < Duration::from_secs(90),
        "并发排水不互锁（墙钟上界 {elapsed:?}）"
    );
}

// ---------------------------------------------------------------------------
// 正向：超时臂（AC-3）
// ---------------------------------------------------------------------------

/// 正向（AC-3）：超时 → `TimedOut` + `exit_code == -1`，墙钟上界（虚拟时钟推进
/// 触发，不真等预算；`kill_tree` + `FINISH_GRACE` 有界收尾）。
#[tokio::test(start_paused = true)]
async fn 超时臂_timedout与哨兵负一且不悬挂() {
    let ws = ShimWs::new("timeout");
    let command = ws.shim("spin.sh", "#!/bin/sh\nwhile :; do sleep 0.05; done\n");

    tokio::spawn(async {
        tokio::task::yield_now().await;
        tokio::time::advance(Duration::from_secs(31)).await;
    });

    let started = Instant::now();
    let report = run_shell(command, options(Duration::from_secs(30)))
        .await
        .expect("超时应返回报告而非 Err");
    let elapsed = started.elapsed();

    assert_eq!(report.outcome, RunOutcome::TimedOut, "到点 → TimedOut");
    assert_eq!(report.exit_code, -1, "超时记 -1 哨兵（MUST NOT 回落 0）");
    assert!(
        elapsed < Duration::from_secs(30),
        "有界收尾不悬挂（墙钟 {elapsed:?}）"
    );
}

/// 边界（AC-3 / D6）：被杀臂输出不完整但 `RunReport` 正常返回——自旋 shim 在
/// 超时前已刷出部分字节 → `TimedOut` 臂不 panic、不悬挂（完整性不断言）。
#[tokio::test(start_paused = true)]
async fn 超时臂_部分输出可返回不panic() {
    let ws = ShimWs::new("timeout-partial");
    let command = ws.shim(
        "spin-partial.sh",
        "#!/bin/sh\nprintf 'partial-before-timeout'\nwhile :; do sleep 0.05; done\n",
    );

    tokio::spawn(async {
        tokio::task::yield_now().await;
        tokio::time::advance(Duration::from_secs(31)).await;
    });

    let started = Instant::now();
    let report = run_shell(command, options(Duration::from_secs(30)))
        .await
        .expect("超时臂应返回报告");
    assert_eq!(report.outcome, RunOutcome::TimedOut);
    assert_eq!(report.exit_code, -1);
    assert!(started.elapsed() < Duration::from_secs(30), "被杀臂不悬挂");
    // 部分输出或空输出皆合法（不完整面在两被杀臂无消费者）；此处只断言可读回
    let _ = String::from_utf8_lossy(&report.stdout);
    let _ = String::from_utf8_lossy(&report.stderr);
}

// ---------------------------------------------------------------------------
// 正向：取消臂（AC-4 / D4）
// ---------------------------------------------------------------------------

/// 正向（AC-4 / D4）：已置位 cancel 的入口短路——不 spawn、墙钟近零、零输出。
#[tokio::test]
async fn 已置位cancel_入口短路零spawn() {
    let ws = ShimWs::new("cancel-pre");
    let marker = ws.file("executed.txt");
    let body = format!(
        "#!/bin/sh\nprintf 'ran' > \"{}\"\nexit 0\n",
        marker.display()
    );
    let command = ws.shim("marker.sh", &body);

    let cancel = CancelFlag::never();
    cancel.handle().set();
    let mut with_cancel = options(Duration::from_secs(30));
    with_cancel.cancel = cancel;

    let started = Instant::now();
    let report = run_shell(command, with_cancel)
        .await
        .expect("入口短路应 Ok");
    let elapsed = started.elapsed();

    assert_eq!(report.outcome, RunOutcome::Cancelled);
    assert_eq!(report.exit_code, -1, "取消记 -1 哨兵");
    assert!(report.stdout.is_empty(), "短路零输出");
    assert!(report.stderr.is_empty(), "短路零输出");
    assert!(!marker.exists(), "已置位 cancel 不 spawn（shim 零执行）");
    assert!(
        elapsed < Duration::from_secs(5),
        "入口短路墙钟近零（不依赖轮询时序，实际 {elapsed:?}）"
    );
}

/// 正向（AC-4）：运行中 cancel——shim 先落 pid 文件再自旋；主任务置位
/// `CancelHandle::set()` → `Cancelled` + `exit_code == -1` + 目标 pid 消失 +
/// 墙钟上界。
#[tokio::test]
async fn 运行中cancel_目标pid消失且秒级收敛() {
    let ws = ShimWs::new("cancel-running");
    let pid_file = ws.file("pid.txt");
    let command = ws.shim(
        "spin-pid.sh",
        &format!(
            "#!/bin/sh\nwin=$(cat \"/proc/$$/winpid\" 2>/dev/null || printf '%s' \"$$\")\nprintf '%s' \"$win\" > \"{}\"\nwhile :; do sleep 0.1; done\n",
            pid_file.display()
        ),
    );

    let cancel = CancelFlag::never();
    let handle = cancel.handle();
    let mut with_cancel = options(Duration::from_secs(120));
    with_cancel.cancel = cancel;

    let started = Instant::now();
    let runner = tokio::spawn(async move { run_shell(command, with_cancel).await });

    // 等 shim 落 pid（真实时钟：取消轮询语义本身依赖 200 ms 粒度；上界放宽到
    // 10 s 以覆盖 msys bash 启动延迟的抖动）
    let mut pid = None;
    for _ in 0..400 {
        if let Some(value) = ws.read_pid("pid.txt") {
            pid = Some(value);
            break;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    let pid = pid.expect("shim 应落 pid 文件（进程已建立）");

    handle.set();
    let report = runner
        .await
        .expect("run_shell 任务应正常结束")
        .expect("取消臂应返回报告");
    let elapsed = started.elapsed();

    assert_eq!(report.outcome, RunOutcome::Cancelled, "置位后 → Cancelled");
    assert_eq!(report.exit_code, -1, "取消记 -1 哨兵");
    assert!(
        wait_pid_gone(pid, Duration::from_secs(10)),
        "取消后目标 pid {pid} 应消失（kill_tree 生效）"
    );
    assert!(
        elapsed < Duration::from_secs(30),
        "秒级收敛（墙钟 {elapsed:?}）"
    );
}

/// 边界：cancel 不误杀——`never()` 全程不置位 + 正常退出脚本 → `Completed`
/// （轮询臂零副作用，不产生假 `Cancelled`）。
#[tokio::test]
async fn cancel不误杀_never全程不置位() {
    let ws = ShimWs::new("cancel-noop");
    let command = ws.shim("normal.sh", "#!/bin/sh\nprintf 'done'\nexit 0\n");
    let report = run_shell(command, options(Duration::from_secs(30)))
        .await
        .expect("常态应 Ok");
    assert_eq!(
        report.outcome,
        RunOutcome::Completed,
        "不得出现假 Cancelled"
    );
    assert_eq!(report.exit_code, 0);
    assert_eq!(report.stdout, b"done");
}

/// 边界（自然退出臂有界排水）：子进程退出后孤儿孙进程仍持有管道写端——
/// 收尸完成即按 [`FINISH_GRACE`] 收口，不得等孙进程退出才返回。
///
/// 装置：shim 起后台 `sleep` 继承 stdout / stderr 后自身退出；后台进程存活期间
/// 管道 EOF 不会到达。旧实现「先排干再收尸」会一直等到 `sleep` 结束（≈10 s）；
/// 修复后收尸完成即返回，墙钟有界。`sleep` 不做清理（10 s 后自行退出）。
///
/// 上界取值 = [`FINISH_GRACE`] + 8 s（而非 + 5 s）：本文件的 shim 走 msys bash，
/// 单次启动 1.3–1.8 s、并行满载（数十真实子进程用例）可抖到 4 s 量级，设备成本
/// 直接落在被测墙钟里。10 s 上界仍**严格区分**新旧语义——旧实现必须等孙进程
/// 退出（≈`sleep 10`）才 EOF，即「启动 + 10 s」；新实现是「启动 + ≤2 s」。
#[tokio::test]
async fn 自然退出_孤儿孙进程持写端时收尸后有界收口() {
    let ws = ShimWs::new("orphan-pipe");
    let command = ws.shim(
        "orphan.sh",
        "#!/bin/sh\n(sleep 10) &\nprintf 'done'\nexit 0\n",
    );
    let started = Instant::now();
    let report = run_shell(command, options(Duration::from_secs(30)))
        .await
        .expect("收尸应正常完成");
    let elapsed = started.elapsed();
    assert_eq!(report.outcome, RunOutcome::Completed);
    assert_eq!(report.exit_code, 0);
    assert_eq!(report.stdout, b"done", "退出前的输出应已排空");
    assert!(
        elapsed < FINISH_GRACE + Duration::from_secs(8),
        "收尸后排水应有界（墙钟 {elapsed:?}）"
    );
}

// ---------------------------------------------------------------------------
// 边界：被杀两臂的 pid 回收（AC-3 / AC-4）
// ---------------------------------------------------------------------------

#[tokio::test(start_paused = true)]
async fn 超时臂_pid消失且墙钟上界() {
    let _gate = ClockGate::new();
    let ws = ShimWs::new("timeout-pid");
    let pid_file = ws.file("pid.txt");
    let command = ws.shim(
        "spin-pid.sh",
        &format!(
            "#!/bin/sh\nwin=$(cat \"/proc/$$/winpid\" 2>/dev/null || printf '%s' \"$$\")\nprintf '%s' \"$win\" > \"{}\"\nwhile :; do sleep 0.1; done\n",
            pid_file.display()
        ),
    );

    let budget = Duration::from_secs(30);
    let started = Instant::now();
    let runner = tokio::spawn(async move { run_shell(command, options(budget)).await });

    // 真实时间等 shim 落 pid：窗口与虚拟预算解耦，启动慢不再误杀用例
    let mut pid = None;
    let deadline = Instant::now() + Duration::from_secs(30);
    while Instant::now() < deadline {
        if let Some(value) = ws.read_pid("pid.txt") {
            pid = Some(value);
            break;
        }
        real_sleep(Duration::from_millis(25)).await;
    }

    // 预算到期（虚拟时钟显式推进）→ 进入杀树 + FINISH_GRACE 收尾
    tokio::time::advance(budget + Duration::from_secs(1)).await;

    // 收尾期显式泵虚拟时钟：杀树失败时管道排水持续占用阻塞线程池，自动推进被
    // 抑制（见函数 doc），必须显式推进才能让 FINISH_GRACE 有界收口
    let pump = tokio::spawn(async {
        loop {
            real_sleep(Duration::from_millis(10)).await;
            tokio::time::advance(FINISH_GRACE).await;
        }
    });

    let report = runner
        .await
        .expect("run_shell 任务应正常结束")
        .expect("超时臂应返回报告");
    pump.abort();
    let elapsed = started.elapsed();
    let pid = pid.unwrap_or_else(|| {
        panic!(
            "桩 shim 应落 pid 文件（outcome={:?}, exit={}, stderr={}）",
            report.outcome,
            report.exit_code,
            String::from_utf8_lossy(&report.stderr)
        )
    });

    assert_eq!(report.outcome, RunOutcome::TimedOut);
    assert_eq!(report.exit_code, -1);
    if !wait_pid_gone(pid, Duration::from_secs(10)) {
        // 失败路径清理：目标进程仍持管道写端时，阻塞排水任务会让运行时收尾
        // 永久等待（用例挂死而非报红）——先杀树，再落到断言失败
        crate::kill_tree(Some(pid));
        panic!("超时被杀后目标 pid {pid} 应消失（kill_tree 生效）");
    }
    assert!(
        elapsed < Duration::from_secs(60),
        "被杀臂有界收尾（墙钟 {elapsed:?}）"
    );
}

// ---------------------------------------------------------------------------
// 边界：轮询粒度与收尾上界常量锚定
// ---------------------------------------------------------------------------

/// 边界（表驱动 2 常量）：`CANCEL_POLL_INTERVAL` = 200 ms（轮询粒度）与
/// `FINISH_GRACE` = 2 s（有界收尾上界）由常量钉死——取值改动即被击杀。
#[test]
fn 轮询与收尾上界常量锚定() {
    assert_eq!(
        CANCEL_POLL_INTERVAL,
        Duration::from_millis(200),
        "取消轮询粒度 200 ms"
    );
    assert_eq!(FINISH_GRACE, Duration::from_secs(2), "杀后收尾上界 2 s");
    assert!(
        CANCEL_POLL_INTERVAL > Duration::ZERO && CANCEL_POLL_INTERVAL < FINISH_GRACE,
        "轮询粒度为正且远小于收尾上界（不构成空转）"
    );
}

/// 报告类型面：`RunReport` 的 `PartialEq` / `Clone` / `Debug` 可比较且逐字段
/// 可断言（`TimedOut` / `Cancelled` 与「无可读退出码」同记 `-1`）。
#[test]
fn run_report字段面可比较且哨兵同值() {
    let report = RunReport {
        outcome: RunOutcome::TimedOut,
        exit_code: -1,
        stdout: b"partial".to_vec(),
        stderr: Vec::new(),
        attempts: 1,
        retried: Vec::new(),
    };
    let clone = report.clone();
    assert_eq!(clone, report, "RunReport 逐字段相等");
    assert_eq!(clone.exit_code, -1, "超时与无可读退出码同记 -1");

    let cancelled = RunReport {
        outcome: RunOutcome::Cancelled,
        exit_code: -1,
        stdout: Vec::new(),
        stderr: Vec::new(),
        attempts: 1,
        retried: Vec::new(),
    };
    assert_ne!(cancelled, report, "结局不同即不等");
    assert!(format!("{report:?}").contains("TimedOut"), "Debug 渲染结局");
}

// ---------------------------------------------------------------------------
// 正向：瞬态重试面（AC-1）
// ---------------------------------------------------------------------------

/// 尝试计数读取（shim 自身落盘；未写入 → 0）。
fn attempt_count(ws: &ShimWs, counter: &str) -> u32 {
    std::fs::read_to_string(ws.file(counter))
        .ok()
        .and_then(|text| text.trim().parse().ok())
        .unwrap_or(0)
}

/// 尝试次数分支 shim：每次拉起把计数文件 +1；前 `fail_times` 次输出锁文本并
/// `exit 1`，之后输出成功行并 `exit 0`（重试形态的唯一变量是计数文件）。
fn retry_shim(ws: &ShimWs, name: &str, counter: &str, fail_times: u32) -> SystemCommand {
    let counter_path = ws.file(counter);
    let body = format!(
        "#!/bin/sh\ncf=\"{cf}\"\nn=$(cat \"$cf\" 2>/dev/null || printf '0')\nn=$((n+1))\nprintf '%s' \"$n\" > \"$cf\"\nif [ \"$n\" -le {fail_times} ]; then\n  printf 'error: failed to rename archive file: /t/liba.rlib: os error 32\n'\n  exit 1\nfi\nprintf 'SUCCESS-OUT'\nprintf 'SUCCESS-ERR' >&2\nexit 0\n",
        cf = counter_path.display(),
        fail_times = fail_times,
    );
    ws.shim(name, &body)
}

/// 快速重试策略（仅用于「次数 / 上限」用例：退避压到 20 ms，避免真等
/// 1 / 2 / 4 / 8 s；退避取值本身由 [`退避序列_前两段各消费一次`] 的按段推进与
/// `transient_test` 的常量锚定覆盖）。
fn fast_policy(max_attempts: u32) -> RetryPolicy {
    RetryPolicy {
        max_attempts,
        backoff: [Duration::from_millis(20); 4],
        total_budget: Duration::from_secs(1500),
    }
}

/// 虚拟时钟闸门：以常驻 `spawn_blocking` 任务抑制 tokio 的「空闲即自动推进」。
///
/// `tokio::time::pause` 的自动推进是「运行时空转即把时钟推到下一个计时器」，所以
/// 暂停时钟下等待真实子进程（msys bash 启动 + 管道排水）会让虚拟时间成百倍空转；
/// 而阻塞线程池里有任务在跑时 tokio 会临时抑制自动推进（`Clock` 的
/// `inhibit_auto_advance` 计数），虚拟时间因此固定在暂停点，只随 `advance` 前进。
/// 这正是「退避 / 预算以虚拟时钟表达、不真等」的装置前提。
///
/// 释放：Drop 掉通道即让阻塞任务返回（`allow_auto_advance`），避免测试收尾时
/// 阻塞线程池仍挂在通道上。
struct ClockGate {
    release: Option<std::sync::mpsc::Sender<()>>,
    task: Option<tokio::task::JoinHandle<()>>,
}

impl ClockGate {
    fn new() -> Self {
        let (release, wait) = std::sync::mpsc::channel::<()>();
        let task = tokio::task::spawn_blocking(move || {
            let _ = wait.recv();
        });
        Self {
            release: Some(release),
            task: Some(task),
        }
    }
}

impl Drop for ClockGate {
    fn drop(&mut self) {
        self.release.take();
        self.task.take();
    }
}

/// 真实时间等待：虚拟时钟冻结时 `tokio::time::sleep` 不会流逝，只能借阻塞线程池
/// 等真实时间（顺带持续加固 [`ClockGate`] 的自动推进抑制）。
async fn real_sleep(duration: Duration) {
    let _ = tokio::task::spawn_blocking(move || std::thread::sleep(duration)).await;
}

/// 轮询等待 shim 的尝试计数到达 `want`（真实时间上界 `budget`；返回观察到的计数）。
///
/// 暂停时钟下不能用 `tokio::time::sleep` 轮询（那需要推进虚拟时间——会污染被测的
/// 退避 / 预算取值），故用 [`real_sleep`]。
async fn wait_attempts(ws: &ShimWs, counter: &str, want: u32, budget: Duration) -> u32 {
    let deadline = Instant::now() + budget;
    loop {
        let count = attempt_count(ws, counter);
        if count >= want || Instant::now() >= deadline {
            return count;
        }
        real_sleep(Duration::from_millis(25)).await;
    }
}

/// 等一次尝试落地并进入退避窗口：shim 写完计数即退出，退避紧随其后开始；真实等待
/// 覆盖「进程退出 → 管道 EOF → 收尸 → 组 `AttemptTrace`」的装置成本。
async fn wait_into_backoff() {
    real_sleep(Duration::from_millis(500)).await;
}

/// 正向（AC-1 组合锚）：flaky 锁冲突自愈——前 2 次锁文本 + `exit 1`、第 3 次
/// `exit 0` → `Completed` / `exit_code == 0` / `attempts == 3` /
/// `retried.len() == 2`（各携签名原文）/ 末次两路字节恰为第 3 次（不含历史尝试）。
#[tokio::test]
async fn 瞬态自愈_三次尝试后转绿() {
    let ws = ShimWs::new("retry-flaky");
    let command = retry_shim(&ws, "flaky.sh", "count.txt", 2);

    let mut with_retry = options(Duration::from_secs(60));
    with_retry.retry = RetryPolicy::transient();

    let report = run_shell(command, with_retry)
        .await
        .expect("flaky 自愈应 Ok");

    assert_eq!(
        report.outcome,
        RunOutcome::Completed,
        "耗尽前自愈 → Completed"
    );
    assert_eq!(report.exit_code, 0, "末次成功退出码直传");
    assert_eq!(report.attempts, 3, "前 2 次红 + 第 3 次绿 = 3 次尝试");
    assert_eq!(report.retried.len(), 2, "末次不入 retried");
    for (index, trace) in report.retried.iter().enumerate() {
        assert_eq!(trace.exit_code, 1, "第 {} 次尝试非零退出", index + 1);
        assert_eq!(
            trace.transient.as_deref(),
            Some("os error 32"),
            "第 {} 次尝试命中签名原文",
            index + 1
        );
        assert!(
            String::from_utf8_lossy(&trace.stdout).contains("failed to rename archive file"),
            "第 {} 次尝试的诊断留痕在案",
            index + 1
        );
    }
    assert_eq!(
        report.stdout, b"SUCCESS-OUT",
        "末次 stdout 不含历史尝试字节"
    );
    assert_eq!(
        report.stderr, b"SUCCESS-ERR",
        "末次 stderr 不含历史尝试字节"
    );
    assert_eq!(attempt_count(&ws, "count.txt"), 3, "shim 恰拉起 3 次");
}

/// 正向（AC-1）：退避序列——三次尝试均命中签名后成功。虚拟时钟（[`ClockGate`] 抑制
/// 自动推进）按段精确推进，断言每段退避的取值与索引：
/// - 第 1 段 `backoff[0]` ∈ (500 ms, 1 s]：推进 500 ms 并真实等待上界后仍无第 2 次
///   尝试，再推进 500 ms 即拉起第 2 次；
/// - 第 2 段 `backoff[1]` ∈ (1 s, 2 s]：同上（推进 1 s 无第 3 次，再推进 1 s 即拉起）；
/// - 4 s / 8 s 段未被消费：全程虚拟墙钟恰为 1 s + 2 s、`attempts == 3`、
///   `retried.len() == 2`（`backoff` 索引不越界、不多等）；shim 恰拉起 3 次。
#[tokio::test(start_paused = true)]
async fn 退避序列_前两段各消费一次() {
    let _gate = ClockGate::new();
    let ws = ShimWs::new("retry-backoff");
    let command = retry_shim(&ws, "backoff.sh", "count.txt", 2);

    let mut with_retry = options(Duration::from_secs(120));
    with_retry.retry = RetryPolicy::transient();

    let virtual_started = tokio::time::Instant::now();
    let runner = tokio::spawn(run_shell(command, with_retry));

    assert_eq!(
        wait_attempts(&ws, "count.txt", 1, Duration::from_secs(20)).await,
        1,
        "第 1 次尝试已拉起"
    );
    wait_into_backoff().await;

    // 第 1 段：推进 500 ms 不得越过，补满 1 s 即越过
    tokio::time::advance(Duration::from_millis(500)).await;
    assert_eq!(
        wait_attempts(&ws, "count.txt", 2, Duration::from_secs(3)).await,
        1,
        "推进 500 ms 未越过第 1 段（backoff[0] > 500 ms）"
    );
    tokio::time::advance(Duration::from_millis(500)).await;
    assert_eq!(
        wait_attempts(&ws, "count.txt", 2, Duration::from_secs(20)).await,
        2,
        "推进满 1 s 后拉起第 2 次尝试（backoff[0] == 1 s）"
    );
    wait_into_backoff().await;

    // 第 2 段：推进 1 s 不得越过，补满 2 s 即越过
    tokio::time::advance(Duration::from_secs(1)).await;
    assert_eq!(
        wait_attempts(&ws, "count.txt", 3, Duration::from_secs(3)).await,
        2,
        "推进 1 s 未越过第 2 段（backoff[1] > 1 s）"
    );
    tokio::time::advance(Duration::from_secs(1)).await;
    assert_eq!(
        wait_attempts(&ws, "count.txt", 3, Duration::from_secs(20)).await,
        3,
        "推进满 2 s 后拉起第 3 次尝试（backoff[1] == 2 s）"
    );

    let report = runner
        .await
        .expect("任务应正常结束")
        .expect("退避序列应 Ok");
    let elapsed = virtual_started.elapsed();

    assert_eq!(report.outcome, RunOutcome::Completed, "第 3 次转绿");
    assert_eq!(report.exit_code, 0);
    assert_eq!(report.attempts, 3, "两次重试后成功");
    assert_eq!(report.retried.len(), 2, "末次不入 retried");
    assert_eq!(attempt_count(&ws, "count.txt"), 3, "shim 恰拉起 3 次");
    assert!(
        elapsed >= Duration::from_secs(3),
        "前两段退避（1 s + 2 s）各消费一次（虚拟墙钟 {elapsed:?}）"
    );
    assert!(
        elapsed < Duration::from_millis(3200),
        "未消费 4 s / 8 s 段（虚拟墙钟 {elapsed:?}）"
    );
}

/// 正向（AC-1）：持续锁冲突耗尽——每次都锁文本 + `exit 1` → `attempts == 5`
///（不再增加）、`retried.len() == 4`、末次 `Completed` + 非零，全程墙钟远小于
/// 总预算。
#[tokio::test]
async fn 持续锁冲突耗尽_上限五次() {
    let ws = ShimWs::new("retry-exhaust");
    let command = retry_shim(&ws, "exhaust.sh", "count.txt", u32::MAX);

    let mut with_retry = options(Duration::from_secs(60));
    with_retry.retry = fast_policy(5);

    let started = Instant::now();
    let report = run_shell(command, with_retry).await.expect("耗尽面应 Ok");
    let elapsed = started.elapsed();

    assert_eq!(report.attempts, 5, "上限 5 次后不再增加");
    assert_eq!(report.retried.len(), 4, "末次不入 retried");
    assert_eq!(
        report.retried.len() as u32,
        report.attempts - 1,
        "不变量：末次之前的尝试数 == attempts - 1"
    );
    assert_eq!(report.outcome, RunOutcome::Completed, "耗尽仍为 Completed");
    assert_eq!(
        report.exit_code, 1,
        "末次非零退出原样返回（由调用方按 fault 收口）"
    );
    assert_eq!(attempt_count(&ws, "count.txt"), 5, "spawn 次数 == 上限");
    assert!(
        elapsed <= RetryPolicy::transient().total_budget,
        "全程墙钟受总预算约束（实际 {elapsed:?}）"
    );
}

/// 边界（AC-1 / D2）：预算早停——每次尝试耗时以虚拟时钟表达为 600 s（真实子进程
/// 不真等），总预算 1500 s：第 3 次结束时已用 1200 s（未耗尽）、第 4 次结束时已用
/// 1800 s（≥ 预算）→ 不再发起第 5 次：`attempts == 4`（未到上限 5）、
/// `retried.len() == 3`、末次原样返回、第 5 次不 spawn、全程虚拟墙钟未越过
/// 「总预算 + 单次尝试耗时」。
#[tokio::test(start_paused = true)]
async fn 预算耗尽早停不发起新尝试() {
    let _gate = ClockGate::new();
    let ws = ShimWs::new("retry-budget");
    let command = retry_shim(&ws, "budget.sh", "count.txt", u32::MAX);

    let mut with_retry = options(Duration::from_secs(600));
    with_retry.retry = RetryPolicy {
        max_attempts: 5,
        backoff: RetryPolicy::transient().backoff,
        total_budget: Duration::from_secs(1500),
    };

    let virtual_started = tokio::time::Instant::now();
    let runner = tokio::spawn(run_shell(command, with_retry));

    // 前 3 次尝试各耗时 600 s（虚拟）：同时越过此后的退避段（1 / 2 / 4 s）
    for expected in 1..=3u32 {
        assert_eq!(
            wait_attempts(&ws, "count.txt", expected, Duration::from_secs(20)).await,
            expected,
            "第 {expected} 次尝试已拉起（预算尚未耗尽）"
        );
        wait_into_backoff().await;
        tokio::time::advance(Duration::from_secs(600)).await;
    }
    assert_eq!(
        wait_attempts(&ws, "count.txt", 4, Duration::from_secs(20)).await,
        4,
        "第 4 次尝试已拉起（1200 s < 1500 s 预算）"
    );

    let report = runner
        .await
        .expect("任务应正常结束")
        .expect("预算早停应 Ok");
    let elapsed = virtual_started.elapsed();

    assert_eq!(report.attempts, 4, "预算耗尽后不再发起新尝试（未到上限 5）");
    assert_eq!(report.retried.len(), 3, "末次不入 retried");
    assert_eq!(
        report.outcome,
        RunOutcome::Completed,
        "早停末次原样返回（Completed + 非零）"
    );
    assert_ne!(report.exit_code, 0, "末次仍为红");
    assert_eq!(
        attempt_count(&ws, "count.txt"),
        4,
        "第 5 次未 spawn（shim 计数恒 4）"
    );
    assert!(
        elapsed >= Duration::from_secs(1500),
        "预算判定按累计虚拟墙钟（{elapsed:?}）"
    );
    assert!(
        elapsed <= Duration::from_secs(2100),
        "总墙钟 ≤ 总预算 + 单次尝试耗时（虚拟 {elapsed:?}）"
    );
}

/// 边界：极短预算——`total_budget = ZERO` → 首跑后即判耗尽（`attempts == 1`、
/// `retried` 空），且预算判定先于退避判定：虚拟时钟零推进（一条退避段都没等）。
#[tokio::test(start_paused = true)]
async fn 零预算首跑后即耗尽() {
    let _gate = ClockGate::new();
    let ws = ShimWs::new("retry-zero-budget");
    let command = retry_shim(&ws, "zero.sh", "count.txt", u32::MAX);

    let mut with_retry = options(Duration::from_secs(60));
    with_retry.retry = RetryPolicy {
        max_attempts: 5,
        backoff: RetryPolicy::transient().backoff,
        total_budget: Duration::ZERO,
    };

    let virtual_started = tokio::time::Instant::now();
    let report = run_shell(command, with_retry).await.expect("零预算应 Ok");
    let elapsed = virtual_started.elapsed();

    assert_eq!(report.attempts, 1, "零预算首跑后即耗尽");
    assert!(report.retried.is_empty(), "零重试 → retried 空");
    assert_eq!(
        elapsed,
        Duration::ZERO,
        "零退避等待（未消费任何退避段，虚拟墙钟零推进）"
    );
    assert_eq!(attempt_count(&ws, "count.txt"), 1, "只 spawn 一次");
}

/// 边界（表驱动 2 形态）：自定义上限——`max_attempts = 1` / `2` + 持续命中签名
/// → 分别 `attempts == 1`（`retried` 空）/ `attempts == 2`（`retried.len() == 1`），
/// spawn 次数与上限逐值相等。
#[tokio::test]
async fn 自定义上限与spawn次数逐值相等() {
    for max_attempts in [1u32, 2] {
        let ws = ShimWs::new(&format!("retry-limit-{max_attempts}"));
        let command = retry_shim(&ws, "limit.sh", "count.txt", u32::MAX);

        let mut with_retry = options(Duration::from_secs(60));
        with_retry.retry = fast_policy(max_attempts);

        let report = run_shell(command, with_retry)
            .await
            .unwrap_or_else(|error| panic!("上限 {max_attempts} 应 Ok: {error}"));

        assert_eq!(
            report.attempts, max_attempts,
            "attempts == max_attempts（{max_attempts}）"
        );
        assert_eq!(
            report.retried.len() as u32,
            max_attempts - 1,
            "retried.len() == max_attempts - 1（{max_attempts}）"
        );
        assert_eq!(
            attempt_count(&ws, "count.txt"),
            max_attempts,
            "spawn 次数逐值等于上限（{max_attempts}）"
        );
    }
}

// ---------------------------------------------------------------------------
// 正向：零重试面（AC-2）
// ---------------------------------------------------------------------------

/// 正向（AC-2 / R-1）：真红零重试——普通编译错误 + `exit 1` + `transient()` →
/// `attempts == 1`、`retried` 空、`Completed`；shim 计数 == 1（策略在位但不触发）。
#[tokio::test]
async fn 真红零重试() {
    let ws = ShimWs::new("retry-real-red");
    let marker = ws.file("count.txt");
    let command = ws.shim(
        "real-red.sh",
        &format!(
            "#!/bin/sh\nprintf '1' > \"{}\"\nprintf 'error[E0308]: mismatched types\n'\nprintf '  --> src/lib.rs:3:9\n' >&2\nexit 1\n",
            marker.display()
        ),
    );

    let mut with_retry = options(Duration::from_secs(60));
    with_retry.retry = RetryPolicy::transient();

    let report = run_shell(command, with_retry).await.expect("真红应 Ok");

    assert_eq!(report.attempts, 1, "非瞬态签名不重试");
    assert!(report.retried.is_empty(), "零重试 → retried 空");
    assert_eq!(report.outcome, RunOutcome::Completed, "真红仍是执行成功面");
    assert_eq!(report.exit_code, 1, "退出码直传");
    assert_eq!(attempt_count(&ws, "count.txt"), 1, "shim 只跑一次");
}

/// 正向（AC-2 / AC-8，表驱动 2 策略）：策略是唯一变量——同一 shim（命中签名 +
/// 非零）在 `none()` 下 `attempts == 1`、在 `transient()`（快速退避）下
/// `attempts == 5`，证 `none()` 未开重试面。
#[tokio::test]
async fn 策略是重试面的唯一变量() {
    for (label, policy, expected) in [
        ("none()", RetryPolicy::none(), 1u32),
        ("transient()", fast_policy(5), 5u32),
    ] {
        let ws = ShimWs::new(&format!("retry-policy-{expected}"));
        let command = retry_shim(&ws, "policy.sh", "count.txt", u32::MAX);

        let mut with_retry = options(Duration::from_secs(60));
        with_retry.retry = policy;

        let report = run_shell(command, with_retry)
            .await
            .unwrap_or_else(|error| panic!("{label} 应 Ok: {error}"));

        assert_eq!(
            report.attempts, expected,
            "{label} → attempts == {expected}"
        );
        assert_eq!(
            attempt_count(&ws, "count.txt"),
            expected,
            "{label} → spawn 次数 == {expected}"
        );
    }
}

/// 正向（AC-8）：单次等价回归——`none()` + `exit 0` + 双路输出 → 四字段与改造
/// 前逐字节一致，`attempts == 1`、`retried` 空、且虚拟时钟零推进（`max_attempts
/// == 1` → 循环不进入退避面，一条退避段都不等）。
#[tokio::test(start_paused = true)]
async fn 单次等价回归_逐字节同改造前() {
    let _gate = ClockGate::new();
    let ws = ShimWs::new("retry-single");
    let command = ws.shim(
        "single.sh",
        "#!/bin/sh\nprintf 'OUT'\nprintf 'ERR' >&2\nexit 0\n",
    );

    let virtual_started = tokio::time::Instant::now();
    let report = run_shell(command, options(Duration::from_secs(30)))
        .await
        .expect("单次执行应 Ok");
    let elapsed = virtual_started.elapsed();

    assert_eq!(report.outcome, RunOutcome::Completed);
    assert_eq!(report.exit_code, 0);
    assert_eq!(report.stdout, b"OUT", "stdout 逐字节同改造前");
    assert_eq!(report.stderr, b"ERR", "stderr 逐字节同改造前");
    assert_eq!(report.attempts, 1, "attempts == 1");
    assert!(report.retried.is_empty(), "retried 空");
    assert_eq!(elapsed, Duration::ZERO, "零退避等待（虚拟墙钟零推进）");
}

/// 正向（AC-8）：末次输出非合流——前两次各输出不同文本后成功 → `stdout` /
/// `stderr` 不含前两次字节；前两次字节只在 `retried` 按尝试序可读。
#[tokio::test]
async fn 末次输出不含历史尝试字节() {
    let ws = ShimWs::new("retry-last-only");
    let counter = ws.file("count.txt");
    let command = ws.shim(
        "last.sh",
        &format!(
            "#!/bin/sh\ncf=\"{cf}\"\nn=$(cat \"$cf\" 2>/dev/null || printf '0')\nn=$((n+1))\nprintf '%s' \"$n\" > \"$cf\"\nif [ \"$n\" -le 2 ]; then\n  printf 'OLD-OUT-%s os error 32' \"$n\"\n  printf 'OLD-ERR-%s' \"$n\" >&2\n  exit 1\nfi\nprintf 'NEW-OUT'\nprintf 'NEW-ERR' >&2\nexit 0\n",
            cf = counter.display()
        ),
    );

    let mut with_retry = options(Duration::from_secs(60));
    with_retry.retry = fast_policy(5);

    let report = run_shell(command, with_retry).await.expect("末次面应 Ok");

    assert_eq!(report.attempts, 3);
    assert_eq!(report.stdout, b"NEW-OUT", "末次 stdout 不含前两次字节");
    assert_eq!(report.stderr, b"NEW-ERR", "末次 stderr 不含前两次字节");
    assert_eq!(
        report.retried[0].stdout, b"OLD-OUT-1 os error 32",
        "第 1 次字节在 retried[0] 可读"
    );
    assert_eq!(report.retried[0].stderr, b"OLD-ERR-1");
    assert_eq!(
        report.retried[1].stdout, b"OLD-OUT-2 os error 32",
        "第 2 次字节在 retried[1] 按尝试序可读"
    );
    assert_eq!(report.retried[1].stderr, b"OLD-ERR-2");
}

/// 正向（AC-2）：超时零重试——自旋 shim（输出含签名）+ `transient()` + 虚拟时钟
/// 到点 → `TimedOut` / `exit_code == -1` / `attempts == 1` / `retried` 空。
#[tokio::test(start_paused = true)]
async fn 超时臂零重试() {
    let ws = ShimWs::new("retry-timeout");
    let command = ws.shim(
        "spin-lock.sh",
        "#!/bin/sh\nprintf 'os error 32\n'\nwhile :; do sleep 0.05; done\n",
    );

    let mut with_retry = options(Duration::from_secs(30));
    with_retry.retry = RetryPolicy::transient();

    tokio::spawn(async {
        tokio::task::yield_now().await;
        tokio::time::advance(Duration::from_secs(31)).await;
    });

    let report = run_shell(command, with_retry).await.expect("超时应 Ok");

    assert_eq!(report.outcome, RunOutcome::TimedOut, "到点 → TimedOut");
    assert_eq!(report.exit_code, -1, "超时记哨兵");
    assert_eq!(report.attempts, 1, "超时零重试");
    assert!(report.retried.is_empty(), "超时臂 retried 空");
}

/// 正向（AC-2 / D4①）：已置位 cancel 零重试——`transient()` + 先置位再调 →
/// `Cancelled` / `attempts == 1` / `retried` 空 / shim 零执行 / 墙钟近零。
#[tokio::test]
async fn 已置位cancel零重试且零spawn() {
    let ws = ShimWs::new("retry-cancel-pre");
    let marker = ws.file("executed.txt");
    let command = ws.shim(
        "pre.sh",
        &format!(
            "#!/bin/sh\nprintf 'ran' > \"{}\"\nprintf 'os error 32\n'\nexit 1\n",
            marker.display()
        ),
    );

    let cancel = CancelFlag::never();
    cancel.handle().set();
    let mut with_retry = options(Duration::from_secs(30));
    with_retry.cancel = cancel;
    with_retry.retry = RetryPolicy::transient();

    let started = Instant::now();
    let report = run_shell(command, with_retry).await.expect("入口短路应 Ok");
    let elapsed = started.elapsed();

    assert_eq!(report.outcome, RunOutcome::Cancelled);
    assert_eq!(report.exit_code, -1);
    assert_eq!(report.attempts, 1, "短路记 1 次（不因重试面增长）");
    assert!(report.retried.is_empty(), "短路 retried 空");
    assert!(!marker.exists(), "已置位 cancel 零 spawn");
    assert!(
        elapsed < Duration::from_secs(5),
        "短路墙钟近零（{elapsed:?}）"
    );
}

/// 正向（AC-2 / D4③）：运行中 cancel 零重试——shim 落 pid 后自旋并输出签名文本，
/// 运行中置位 → `Cancelled` / `attempts == 1` / `retried` 空 / pid 消失。
#[tokio::test]
async fn 运行中cancel零重试() {
    let ws = ShimWs::new("retry-cancel-running");
    let pid_file = ws.file("pid.txt");
    let command = ws.shim(
        "spin-pid.sh",
        &format!(
            "#!/bin/sh\nwin=$(cat \"/proc/$$/winpid\" 2>/dev/null || printf '%s' \"$$\")\nprintf '%s' \"$win\" > \"{}\"\nprintf 'os error 32\n'\nwhile :; do sleep 0.1; done\n",
            pid_file.display()
        ),
    );

    let cancel = CancelFlag::never();
    let handle = cancel.handle();
    let mut with_retry = options(Duration::from_secs(120));
    with_retry.cancel = cancel;
    with_retry.retry = RetryPolicy::transient();

    let runner = tokio::spawn(async move { run_shell(command, with_retry).await });

    let mut pid = None;
    for _ in 0..400 {
        if let Some(value) = ws.read_pid("pid.txt") {
            pid = Some(value);
            break;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    let pid = pid.expect("shim 应落 pid 文件");
    handle.set();

    let report = runner
        .await
        .expect("任务应正常结束")
        .expect("取消臂应返回报告");

    assert_eq!(report.outcome, RunOutcome::Cancelled);
    assert_eq!(report.exit_code, -1);
    assert_eq!(report.attempts, 1, "运行中取消零重试");
    assert!(report.retried.is_empty(), "取消臂 retried 空");
    assert!(
        wait_pid_gone(pid, Duration::from_secs(10)),
        "取消后目标 pid {pid} 应消失"
    );
}

/// 正向（AC-1 / D3 / D4③）：退避期 cancel 即刻生效——第 1 次命中签名后进入退避
/// （首段拉长到 30 s，窗口足够宽，不依赖 1 s 段的时序余量），窗口内置位 → 不再发起
/// 第 2 次（shim 计数 == 1）、`Cancelled` / `attempts == 1`（已完成次数）/
/// `retried` 空（该末次不入）/ `stdout` 取第 1 次字节 / 墙钟远小于剩余退避（有界
/// 上界 10 s ≪ 30 s，不先等完退避段）。
#[tokio::test]
async fn 退避期cancel即刻生效() {
    let ws = ShimWs::new("retry-cancel-backoff");
    let command = retry_shim(&ws, "backoff-cancel.sh", "count.txt", u32::MAX);

    let cancel = CancelFlag::never();
    let handle = cancel.handle();
    let mut with_retry = options(Duration::from_secs(120));
    with_retry.cancel = cancel;
    with_retry.retry = RetryPolicy {
        backoff: [Duration::from_secs(30); 4],
        ..RetryPolicy::transient()
    };

    let runner = tokio::spawn(async move { run_shell(command, with_retry).await });

    // 等第 1 次尝试落计数（此后进入 30 s 退避窗口）
    let mut counted = 0u32;
    for _ in 0..400 {
        counted = attempt_count(&ws, "count.txt");
        if counted >= 1 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    assert_eq!(counted, 1, "第 1 次尝试已落计数");
    // 退避窗口起点（≈ 第 1 次尝试完成时刻）
    let backoff_started = Instant::now();
    tokio::time::sleep(Duration::from_millis(150)).await;
    handle.set();

    let report = runner
        .await
        .expect("任务应正常结束")
        .expect("退避期取消应返回报告");
    let since_attempt = backoff_started.elapsed();

    assert_eq!(
        report.outcome,
        RunOutcome::Cancelled,
        "退避期取消 → Cancelled"
    );
    assert_eq!(report.exit_code, -1);
    assert_eq!(report.attempts, 1, "记已完成次数");
    assert!(report.retried.is_empty(), "该末次不入 retried");
    assert!(
        String::from_utf8_lossy(&report.stdout).contains("failed to rename archive file"),
        "stdout 取第 1 次字节: {:?}",
        String::from_utf8_lossy(&report.stdout)
    );
    assert_eq!(attempt_count(&ws, "count.txt"), 1, "不再发起第 2 次尝试");
    assert!(
        since_attempt < Duration::from_secs(10),
        "不先等完 30 s 退避段（实际 {since_attempt:?}）"
    );
}

/// 边界：退避期 cancel（两次尝试后）——第 1、2 次命中，第 2 次之后的退避窗口
/// （`backoff[1]` 拉长到 30 s，窗口宽到不依赖时序余量）内取消 → `attempts == 2`、
/// `retried.len() == 1`（第 1 次已入栈，第 2 次为「刚完成的末次」不入栈）、
/// `stdout` 取第 2 次字节、不再发起第 3 次。
#[tokio::test]
async fn 退避期cancel在两次尝试后() {
    let ws = ShimWs::new("retry-cancel-backoff2");
    let command = retry_shim(&ws, "backoff-cancel2.sh", "count.txt", u32::MAX);

    let cancel = CancelFlag::never();
    let handle = cancel.handle();
    let mut with_retry = options(Duration::from_secs(120));
    with_retry.cancel = cancel;
    // 首段归零（第 2 次尝试紧跟第 1 次）、次段拉长到 30 s：取消窗口宽到不依赖
    // 时序余量，同时钉住「第 2 次重试消费 `backoff[1]`」
    with_retry.retry = RetryPolicy {
        backoff: [
            Duration::ZERO,
            Duration::from_secs(30),
            Duration::ZERO,
            Duration::ZERO,
        ],
        ..RetryPolicy::transient()
    };

    let runner = tokio::spawn(async move { run_shell(command, with_retry).await });

    let mut counted = 0u32;
    for _ in 0..800 {
        counted = attempt_count(&ws, "count.txt");
        if counted >= 2 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    assert_eq!(counted, 2, "第 2 次尝试已落计数");
    tokio::time::sleep(Duration::from_millis(150)).await;
    handle.set();

    let report = runner
        .await
        .expect("任务应正常结束")
        .expect("退避期取消应返回报告");

    assert_eq!(report.outcome, RunOutcome::Cancelled);
    assert_eq!(report.exit_code, -1);
    assert_eq!(report.attempts, 2, "记已完成次数（2）");
    assert_eq!(report.retried.len(), 1, "第 1 次入栈、第 2 次为末次不入栈");
    assert_eq!(report.retried[0].exit_code, 1, "第 1 次尝试留痕在栈内可读");
    assert!(
        String::from_utf8_lossy(&report.stdout).contains("failed to rename archive file"),
        "stdout 取第 2 次字节"
    );
    assert_eq!(attempt_count(&ws, "count.txt"), 2, "不再发起第 3 次尝试");
}

/// 异常：spawn 失败不重试——程序不可达 + `transient()` → 首次尝试即
/// `Err(RunError::Spawn(_))`（无 `RunReport` 可携 `attempts`）。
#[tokio::test]
async fn spawn失败不重试() {
    let mut with_retry = options(Duration::from_secs(30));
    with_retry.retry = RetryPolicy::transient();

    let error = run_shell(SystemCommand::bare("no-such-program-xyz-retry"), with_retry)
        .await
        .expect_err("不可达程序应 Spawn 失败");
    match error {
        RunError::Spawn(_) => {}
        other => panic!("应为 Spawn 面，实际 {other:?}"),
    }
}

/// 边界：双管道 + 大输出在重试面——每次尝试 stdout / stderr 各 >64 KiB（含
/// 签名）后成功 → `retried` 每次字节数精确等于写量、末次同样全量（排水语义与
/// 单次一致，不互锁）。
#[tokio::test]
async fn 重试面大输出双管道不丢字节() {
    let ws = ShimWs::new("retry-big");
    let mut failing_with_signature = vec![b'q'; 64 * 1024 + 7];
    failing_with_signature.extend_from_slice(b" os error 32");
    let failing_path = ws.put_bytes("failing.bin", &failing_with_signature);
    let ok_bytes = vec![b'z'; 64 * 1024 + 3];
    let ok_path = ws.put_bytes("ok.bin", &ok_bytes);
    let counter = ws.file("count.txt");

    let command = ws.shim(
        "big.sh",
        &format!(
            "#!/bin/sh\ncf=\"{cf}\"\nn=$(cat \"$cf\" 2>/dev/null || printf '0')\nn=$((n+1))\nprintf '%s' \"$n\" > \"$cf\"\nif [ \"$n\" -le 1 ]; then\n  cat \"{fail}\"\n  cat \"{fail}\" >&2\n  exit 1\nfi\ncat \"{ok}\"\ncat \"{ok}\" >&2\nexit 0\n",
            cf = counter.display(),
            fail = failing_path.display(),
            ok = ok_path.display(),
        ),
    );

    let mut with_retry = options(Duration::from_secs(120));
    with_retry.retry = fast_policy(5);

    let report = run_shell(command, with_retry)
        .await
        .expect("大输出重试应 Ok");

    assert_eq!(report.attempts, 2, "一次重试后成功");
    assert_eq!(
        report.retried[0].stdout.len(),
        failing_with_signature.len(),
        "此前尝试 stdout 字节数精确等于写量"
    );
    assert_eq!(
        report.retried[0].stderr.len(),
        failing_with_signature.len(),
        "此前尝试 stderr 字节数精确等于写量"
    );
    assert_eq!(report.stdout, ok_bytes, "末次 stdout 全量无丢字节");
    assert_eq!(report.stderr, ok_bytes, "末次 stderr 全量无丢字节");
}

/// 边界：`RunOptions.retry` 字段面——必填字段可读、缺省装置给 `none()`、
/// `Debug` 含该字段（生产四调用点均显式给值的编译期面由静态检查覆盖）。
#[test]
fn run_options_retry必填字段面() {
    let base = options(Duration::from_secs(1));
    assert_eq!(base.retry, RetryPolicy::none(), "缺省装置给 none()");

    let with_transient = RunOptions {
        retry: RetryPolicy::transient(),
        ..base.clone()
    };
    assert_eq!(
        with_transient.retry.max_attempts, 5,
        "retry 字段可直构为瞬态策略"
    );
    assert!(
        with_transient.cwd.is_none() && with_transient.env.is_empty(),
        "其余字段随动（其余字段面零变）"
    );

    let debug = format!("{with_transient:?}");
    assert!(debug.contains("retry"), "Debug 含 retry 字段: {debug}");
}

/// 边界（表驱动 2 常量）：重试面相关常量与单次面常量同源锚定——
/// `RetryPolicy::transient().total_budget` 为 1500 s（执行体预算判定取值面），
/// 取消轮询 / 收尾上界不受重试面影响。
#[test]
fn 重试预算与收尾常量锚定() {
    assert_eq!(
        RetryPolicy::transient().total_budget,
        Duration::from_secs(1500),
        "重试总预算 1500 s"
    );
    assert_eq!(
        RetryPolicy::none().total_budget,
        Duration::from_secs(1500),
        "单次策略取同类常量（不消费）"
    );
    assert_eq!(CANCEL_POLL_INTERVAL, Duration::from_millis(200));
    assert_eq!(FINISH_GRACE, Duration::from_secs(2));
}
