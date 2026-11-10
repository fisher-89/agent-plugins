#[cfg(windows)]
mod imp {
    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
    use windows_sys::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, TerminateJobObject,
    };

    /// 作业对象句柄 RAII（`handle` 恒非空——构造面只经 [`JobTree::attach`]）。
    pub(crate) struct JobTree {
        handle: HANDLE,
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
            let job = Self { handle };
            let assigned = unsafe { AssignProcessToJobObject(job.handle, raw as HANDLE) };
            (assigned != 0).then_some(job)
        }

        /// 整组终止（含已继承成员资格、但父子关系已被改写 / 快照漏掉的子孙）。
        pub(crate) fn terminate(&self) {
            unsafe {
                TerminateJobObject(self.handle, 1);
            }
        }
    }

    impl Drop for JobTree {
        fn drop(&mut self) {
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

        pub(crate) fn terminate(&self) {}
    }
}

pub(crate) use imp::JobTree;
