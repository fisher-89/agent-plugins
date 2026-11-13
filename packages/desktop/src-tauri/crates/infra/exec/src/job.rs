#[cfg(windows)]
mod imp {
    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
    use windows_sys::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, TerminateJobObject,
    };

    /// 作业对象句柄 RAII（`handle` 恒非空——构造面只经 [`JobTree::attach`]）。
    pub(crate) struct JobTree {
        handle: HANDLE,
        /// 未解除武装 ⇒ `Drop` 整组终止。被杀两臂与 future 被 drop（panic 展开 /
        /// 调用方 abort）都靠它兜底；自然退出臂先 [`JobTree::disarm`] 保住
        /// 「孤儿孙进程存续、排水有界截断」语义。
        armed: bool,
    }

    // 句柄本身无别名共享（每棵进程树独占一个作业），可跨线程移动。
    unsafe impl Send for JobTree {}

    impl JobTree {
        /// 把刚 spawn 的直接子进程挂进新建作业。任一系统调用失败 → `None`
        /// （调用方回落既有 `taskkill /T /F` 语义，记录面零阻断）。
        pub(crate) fn attach(child: &tokio::process::Child) -> Option<Self> {
            let raw = child.raw_handle()?;
            let handle = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
            if handle.is_null() {
                return None;
            }
            let assigned = unsafe { AssignProcessToJobObject(handle, raw as HANDLE) };
            if assigned == 0 {
                unsafe { CloseHandle(handle) };
                return None;
            }
            Some(Self {
                handle,
                armed: true,
            })
        }

        /// 整组终止（含已继承成员资格、但父子关系已被改写 / 快照漏掉的子孙）。
        /// 幂等：首次调用后解除武装，重复调用与随后的 `Drop` 均为空操作。
        pub(crate) fn terminate(&mut self) {
            if !self.armed {
                return;
            }
            self.armed = false;
            unsafe {
                TerminateJobObject(self.handle, 1);
            }
        }

        /// 解除武装：自然退出臂在收尾前调用，使 `Drop` 不再终止作业——孤儿子孙
        /// 存续，排水以 `FINISH_GRACE` 有界截断（既有语义零变）。
        pub(crate) fn disarm(&mut self) {
            self.armed = false;
        }
    }

    impl Drop for JobTree {
        /// 未解除武装即说明执行体没能走到任何一条收口路径（future 被 drop：调用方
        /// abort、panic 展开、外层 select 取消）——此时 `kill_on_drop` 只杀直接
        /// 子进程，孙进程会带着继承的管道写端存活并钉死 tokio 阻塞线程池（runtime
        /// 析构无界等待）。故此处整组终止。
        fn drop(&mut self) {
            self.terminate();
            unsafe {
                CloseHandle(self.handle);
            }
        }
    }
}

#[cfg(not(windows))]
mod imp {
    /// 非 Windows 目标：无作业对象概念，恒不挂载、终止为空操作。
    pub(crate) struct JobTree;

    impl JobTree {
        pub(crate) fn attach(_child: &tokio::process::Child) -> Option<Self> {
            None
        }

        pub(crate) fn terminate(&mut self) {}

        pub(crate) fn disarm(&mut self) {}
    }
}

pub(crate) use imp::JobTree;
