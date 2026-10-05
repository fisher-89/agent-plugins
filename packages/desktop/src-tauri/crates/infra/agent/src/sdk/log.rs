use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// 日志文件名（初始化目录下单文件）。
const LOG_FILE_NAME: &str = "engine.log";

/// 进程级日志路径单点（None = 未初始化，append 零效应）。
static LOG_PATH: OnceLock<Option<PathBuf>> = OnceLock::new();

/// 初始化日志目录（进程一次，重复调用幂等保留首值；目录不预建，首次写入
/// 时随文件创建失败而静默——数据根由壳层保证存在）。
pub fn init_engine_log(dir: &Path) {
    let _ = LOG_PATH.set(Some(dir.join(LOG_FILE_NAME)));
}

/// 追加一行（未初始化 / IO 失败静默；unix 毫秒前缀）。
pub fn append_engine_log(line: &str) {
    let Some(Some(path)) = LOG_PATH.get() else {
        return;
    };
    let millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0);
    let _ = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .and_then(|mut file| writeln!(file, "[{millis}] {line}"));
}

/// panic 钩子：panic 信息追加进引擎日志后链式调用既有默认 hook（tokio 泵
/// 任务 panic 默认只打印到 stderr，GUI 进程无控制台即无痕——轮行将停在
/// running，日志补观测半边）。
pub fn install_engine_panic_hook() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        append_engine_log(&format!("panic: {info}"));
        previous(info);
    }));
}
