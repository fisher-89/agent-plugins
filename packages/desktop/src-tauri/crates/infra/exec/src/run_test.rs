use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use foundation::cancel::CancelFlag;
use tempfile::TempDir;

use super::{
    run_shell, RunError, RunOptions, RunOutcome, RunReport, CANCEL_POLL_INTERVAL, FINISH_GRACE,
};
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

        assert_eq!(report.outcome, RunOutcome::Completed, "exit {code} 仍为 Completed");
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
    assert_eq!(String::from_utf8_lossy(&report.stdout), "on", "shim 回显 env 值");
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
    let empty = ws.shim("env-empty.sh", "#!/bin/sh\nprintf '%s' \"${RSE_MARK:-unset}\"\n");
    let report = run_shell(empty, options(Duration::from_secs(30)))
        .await
        .expect("空 env 应 Ok");
    assert_eq!(report.outcome, RunOutcome::Completed);
    assert_eq!(String::from_utf8_lossy(&report.stdout), "unset", "空 env 不注入键");

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
    assert_eq!(String::from_utf8_lossy(&report.stdout), "v49", "第 50 项可见");

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

    let error = run_shell(
        SystemCommand::script("echo hi"),
        with_cwd,
    )
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
    for caller_context in ["static-check", "测试命令", "cargo-mutants", "Stryker", "段执行"] {
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
    };
    let cloned = options.clone();
    assert_eq!(cloned.timeout, Duration::from_millis(1500), "timeout 逐值保留");
    assert_eq!(cloned.cwd, options.cwd, "cwd 逐值保留");
    assert_eq!(cloned.env, options.env, "env 逐值保留");
    assert_eq!(cloned.cancel, options.cancel, "cancel 身份保留（同一原子）");

    // cwd 的 None 形态同样可表达且可克隆
    let none_cwd = RunOptions {
        cwd: None,
        env: Vec::new(),
        timeout: Duration::from_secs(1),
        cancel: CancelFlag::never(),
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
    let report = run_shell(
        SystemCommand::script(""),
        options(Duration::from_secs(30)),
    )
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
            format!("#!/bin/sh\nprintf '%s' \"{0}\"\nprintf '%s' \"{0}\" >&2\n", "q".repeat(big)),
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
        assert_eq!(report.stdout.len(), expected_stdout.len(), "{label}: stdout 长度");
        assert_eq!(report.stderr.len(), expected_stderr.len(), "{label}: stderr 长度");
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
    assert_eq!(report.stdout.len(), expected, "stdout 不丢字节（{expected} 字节）");
    assert_eq!(report.stderr.len(), expected, "stderr 不丢字节（{expected} 字节）");
    assert!(report.stdout.iter().all(|byte| *byte == b'p'), "stdout 内容形态");
    assert!(report.stderr.iter().all(|byte| *byte == b'p'), "stderr 内容形态");
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
    assert!(
        started.elapsed() < Duration::from_secs(30),
        "被杀臂不悬挂"
    );
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

    // 等 shim 落 pid（真实时钟：取消轮询语义本身依赖 200 ms 粒度）
    let mut pid = None;
    for _ in 0..200 {
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
    assert_eq!(report.outcome, RunOutcome::Completed, "不得出现假 Cancelled");
    assert_eq!(report.exit_code, 0);
    assert_eq!(report.stdout, b"done");
}

/// 边界（自然退出臂有界排水）：子进程退出后孤儿孙进程仍持有管道写端——
/// 收尸完成即按 [`FINISH_GRACE`] 收口，不得等孙进程退出才返回。
///
/// 装置：shim 起后台 `sleep` 继承 stdout / stderr 后自身退出；后台进程存活期间
/// 管道 EOF 不会到达。旧实现「先排干再收尸」会一直等到 `sleep` 结束（≈10 s）；
/// 修复后收尸完成即返回，墙钟有界。`sleep` 不做清理（10 s 后自行退出）。
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
        elapsed < FINISH_GRACE + Duration::from_secs(5),
        "收尸后排水应有界（墙钟 {elapsed:?}）"
    );
}

// ---------------------------------------------------------------------------
// 边界：被杀两臂的 pid 回收（AC-3 / AC-4）
// ---------------------------------------------------------------------------

/// 边界（AC-3）：超时臂的 pid 回收——桩 shim 先落自身 pid 再自旋，预算到期
/// 后 `kill_tree` 使目标 pid 消失（墙钟上界断言不悬挂）。
///
/// 装置说明：Windows 侧 shim 经 msys bash 承载，`bash &` 起的子孙在 Win32 进程
/// 树上不体现为该 bash 的子孙（msys fork 的中间进程），故 pid 断言收敛到
/// **直接子进程**——这正是 `run_shell` 契约里 `kill_tree(child.id())` 的可稳定
/// 观测面；`taskkill /T /F` 的整树语义由代码审查承接（不构造会遗留孤儿的
/// 假树装置，避免装置污染影响其它用例）。
#[tokio::test]
async fn 超时臂_pid消失且墙钟上界() {
    let ws = ShimWs::new("timeout-pid");
    let pid_file = ws.file("pid.txt");
    let command = ws.shim(
        "spin-pid.sh",
        &format!(
            "#!/bin/sh\nwin=$(cat \"/proc/$$/winpid\" 2>/dev/null || printf '%s' \"$$\")\nprintf '%s' \"$win\" > \"{}\"\nwhile :; do sleep 0.1; done\n",
            pid_file.display()
        ),
    );

    let started = Instant::now();
    let runner = tokio::spawn(async move { run_shell(command, options(Duration::from_secs(3))).await });

    let mut pid = None;
    for _ in 0..200 {
        if let Some(value) = ws.read_pid("pid.txt") {
            pid = Some(value);
            break;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    let pid = pid.expect("桩 shim 应落 pid 文件");

    let report = runner
        .await
        .expect("run_shell 任务应正常结束")
        .expect("超时臂应返回报告");
    let elapsed = started.elapsed();
    assert_eq!(report.outcome, RunOutcome::TimedOut);
    assert_eq!(report.exit_code, -1);
    assert!(
        wait_pid_gone(pid, Duration::from_secs(10)),
        "超时被杀后目标 pid {pid} 应消失（kill_tree 生效）"
    );
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
    assert_eq!(
        FINISH_GRACE,
        Duration::from_secs(2),
        "杀后收尾上界 2 s"
    );
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
    };
    let clone = report.clone();
    assert_eq!(clone, report, "RunReport 逐字段相等");
    assert_eq!(clone.exit_code, -1, "超时与无可读退出码同记 -1");

    let cancelled = RunReport {
        outcome: RunOutcome::Cancelled,
        exit_code: -1,
        stdout: Vec::new(),
        stderr: Vec::new(),
    };
    assert_ne!(cancelled, report, "结局不同即不等");
    assert!(format!("{report:?}").contains("TimedOut"), "Debug 渲染结局");
}
