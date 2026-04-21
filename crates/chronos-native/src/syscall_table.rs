//! Linux x86_64 syscall number → name mapping.
//!
//! Source: https://filippo.io/linux-syscall-table/
//! Generated from the kernel's `arch/x86/entry/syscalls/syscall_64.tbl`.

use std::collections::HashMap;
use std::sync::LazyLock;

static SYSCALL_TABLE: LazyLock<HashMap<u64, &'static str>> = LazyLock::new(|| {
    let mut m = HashMap::new();
    m.insert(0, "read");
    m.insert(1, "write");
    m.insert(2, "open");
    m.insert(3, "close");
    m.insert(4, "stat");
    m.insert(5, "fstat");
    m.insert(6, "lstat");
    m.insert(7, "poll");
    m.insert(8, "lseek");
    m.insert(9, "mmap");
    m.insert(10, "mprotect");
    m.insert(11, "munmap");
    m.insert(12, "brk");
    m.insert(13, "rt_sigaction");
    m.insert(14, "rt_sigprocmask");
    m.insert(15, "rt_sigreturn");
    m.insert(16, "ioctl");
    m.insert(17, "pread64");
    m.insert(18, "pwrite64");
    m.insert(19, "readv");
    m.insert(20, "writev");
    m.insert(21, "access");
    m.insert(22, "pipe");
    m.insert(23, "select");
    m.insert(24, "sched_yield");
    m.insert(25, "mremap");
    m.insert(26, "msync");
    m.insert(27, "mincore");
    m.insert(28, "madvise");
    m.insert(29, "shmget");
    m.insert(30, "shmat");
    m.insert(31, "shmctl");
    m.insert(32, "dup");
    m.insert(33, "dup2");
    m.insert(34, "pause");
    m.insert(35, "nanosleep");
    m.insert(36, "getitimer");
    m.insert(37, "alarm");
    m.insert(38, "setitimer");
    m.insert(39, "getpid");
    m.insert(40, "sendfile");
    m.insert(41, "socket");
    m.insert(42, "connect");
    m.insert(43, "accept");
    m.insert(44, "sendto");
    m.insert(45, "recvfrom");
    m.insert(46, "sendmsg");
    m.insert(47, "recvmsg");
    m.insert(48, "shutdown");
    m.insert(49, "bind");
    m.insert(50, "listen");
    m.insert(51, "getsockname");
    m.insert(52, "getpeername");
    m.insert(53, "socketpair");
    m.insert(54, "setsockopt");
    m.insert(55, "getsockopt");
    m.insert(56, "clone");
    m.insert(57, "fork");
    m.insert(58, "vfork");
    m.insert(59, "execve");
    m.insert(60, "exit");
    m.insert(61, "wait4");
    m.insert(62, "kill");
    m.insert(63, "uname");
    m.insert(64, "semget");
    m.insert(65, "semop");
    m.insert(66, "semctl");
    m.insert(67, "shmdt");
    m.insert(68, "msgget");
    m.insert(69, "msgsnd");
    m.insert(70, "msgrcv");
    m.insert(71, "msgctl");
    m.insert(72, "fcntl");
    m.insert(73, "flock");
    m.insert(74, "fsync");
    m.insert(75, "fdatasync");
    m.insert(76, "truncate");
    m.insert(77, "ftruncate");
    m.insert(78, "getdents");
    m.insert(79, "getcwd");
    m.insert(80, "chdir");
    m.insert(81, "fchdir");
    m.insert(82, "rename");
    m.insert(83, "mkdir");
    m.insert(84, "rmdir");
    m.insert(85, "creat");
    m.insert(86, "link");
    m.insert(87, "unlink");
    m.insert(88, "symlink");
    m.insert(89, "readlink");
    m.insert(90, "chmod");
    m.insert(91, "fchmod");
    m.insert(92, "chown");
    m.insert(93, "fchown");
    m.insert(94, "lchown");
    m.insert(95, "umask");
    m.insert(96, "gettimeofday");
    m.insert(97, "getrlimit");
    m.insert(98, "getrusage");
    m.insert(99, "sysinfo");
    m.insert(100, "times");
    m.insert(101, "ptrace");
    m.insert(102, "getuid");
    m.insert(103, "syslog");
    m.insert(104, "getgid");
    m.insert(105, "setuid");
    m.insert(106, "setgid");
    m.insert(107, "geteuid");
    m.insert(108, "getegid");
    m.insert(109, "setpgid");
    m.insert(110, "getppid");
    m.insert(111, "getpgrp");
    m.insert(112, "setsid");
    m.insert(113, "setreuid");
    m.insert(114, "setregid");
    m.insert(115, "getgroups");
    m.insert(116, "setgroups");
    m.insert(117, "setresuid");
    m.insert(118, "getresuid");
    m.insert(119, "setresgid");
    m.insert(120, "getresgid");
    m.insert(121, "getpgid");
    m.insert(122, "setfsuid");
    m.insert(123, "setfsgid");
    m.insert(124, "getsid");
    m.insert(125, "capget");
    m.insert(126, "capset");
    m.insert(127, "rt_sigpending");
    m.insert(128, "rt_sigtimedwait");
    m.insert(129, "rt_sigqueueinfo");
    m.insert(130, "rt_sigsuspend");
    m.insert(131, "sigaltstack");
    m.insert(132, "utime");
    m.insert(133, "mknod");
    m.insert(134, "uselib");
    m.insert(135, "personality");
    m.insert(136, "ustat");
    m.insert(137, "statfs");
    m.insert(138, "fstatfs");
    m.insert(139, "sysfs");
    m.insert(140, "getpriority");
    m.insert(141, "setpriority");
    m.insert(142, "sched_setparam");
    m.insert(143, "sched_getparam");
    m.insert(144, "sched_setscheduler");
    m.insert(145, "sched_getscheduler");
    m.insert(146, "sched_get_priority_max");
    m.insert(147, "sched_get_priority_min");
    m.insert(148, "sched_rr_get_interval");
    m.insert(149, "mlock");
    m.insert(150, "munlock");
    m.insert(151, "mlockall");
    m.insert(152, "munlockall");
    m.insert(153, "vhangup");
    m.insert(154, "modify_ldt");
    m.insert(155, "pivot_root");
    m.insert(156, "_sysctl");
    m.insert(157, "prctl");
    m.insert(158, "arch_prctl");
    m.insert(159, "adjtimex");
    m.insert(160, "setrlimit");
    m.insert(161, "chroot");
    m.insert(162, "sync");
    m.insert(163, "acct");
    m.insert(164, "settimeofday");
    m.insert(165, "mount");
    m.insert(166, "umount2");
    m.insert(167, "swapon");
    m.insert(168, "swapoff");
    m.insert(169, "reboot");
    m.insert(170, "sethostname");
    m.insert(171, "setdomainname");
    m.insert(172, "iopl");
    m.insert(173, "ioperm");
    m.insert(174, "create_module");
    m.insert(175, "init_module");
    m.insert(176, "delete_module");
    m.insert(177, "get_kernel_syms");
    m.insert(178, "query_module");
    m.insert(179, "quotactl");
    m.insert(180, "nfsservctl");
    m.insert(181, "getpmsg");
    m.insert(182, "putpmsg");
    m.insert(183, "afs_syscall");
    m.insert(184, "tuxcall");
    m.insert(185, "security");
    m.insert(186, "gettid");
    m.insert(187, "readahead");
    m.insert(188, "setxattr");
    m.insert(189, "lsetxattr");
    m.insert(190, "fsetxattr");
    m.insert(191, "getxattr");
    m.insert(192, "lgetxattr");
    m.insert(193, "fgetxattr");
    m.insert(194, "listxattr");
    m.insert(195, "llistxattr");
    m.insert(196, "flistxattr");
    m.insert(197, "removexattr");
    m.insert(198, "lremovexattr");
    m.insert(199, "fremovexattr");
    m.insert(200, "tkill");
    m.insert(201, "time");
    m.insert(202, "futex");
    m.insert(203, "sched_setaffinity");
    m.insert(204, "sched_getaffinity");
    m.insert(205, "set_thread_area");
    m.insert(206, "io_setup");
    m.insert(207, "io_destroy");
    m.insert(208, "io_getevents");
    m.insert(209, "io_submit");
    m.insert(210, "io_cancel");
    m.insert(211, "get_thread_area");
    m.insert(212, "lookup_dcookie");
    m.insert(213, "epoll_create");
    m.insert(214, "epoll_ctl_old");
    m.insert(215, "epoll_wait_old");
    m.insert(216, "remap_file_pages");
    m.insert(217, "getdents64");
    m.insert(218, "set_tid_address");
    m.insert(219, "restart_syscall");
    m.insert(220, "semtimedop");
    m.insert(221, "fadvise64");
    m.insert(222, "timer_create");
    m.insert(223, "timer_settime");
    m.insert(224, "timer_gettime");
    m.insert(225, "timer_getoverrun");
    m.insert(226, "timer_delete");
    m.insert(227, "clock_settime");
    m.insert(228, "clock_gettime");
    m.insert(229, "clock_getres");
    m.insert(230, "clock_nanosleep");
    m.insert(231, "exit_group");
    m.insert(232, "epoll_wait");
    m.insert(233, "epoll_ctl");
    m.insert(234, "tgkill");
    m.insert(235, "utimes");
    m.insert(236, "vserver");
    m.insert(237, "mbind");
    m.insert(238, "set_mempolicy");
    m.insert(239, "get_mempolicy");
    m.insert(240, "mq_open");
    m.insert(241, "mq_unlink");
    m.insert(242, "mq_timedsend");
    m.insert(243, "mq_timedreceive");
    m.insert(244, "mq_notify");
    m.insert(245, "mq_getsetattr");
    m.insert(246, "kexec_load");
    m.insert(247, "waitid");
    m.insert(248, "add_key");
    m.insert(249, "request_key");
    m.insert(250, "keyctl");
    m.insert(251, "ioprio_set");
    m.insert(252, "ioprio_get");
    m.insert(253, "inotify_init");
    m.insert(254, "inotify_add_watch");
    m.insert(255, "inotify_rm_watch");
    m.insert(256, "migrate_pages");
    m.insert(257, "openat");
    m.insert(258, "mkdirat");
    m.insert(259, "mknodat");
    m.insert(260, "fchownat");
    m.insert(261, "futimesat");
    m.insert(262, "newfstatat");
    m.insert(263, "unlinkat");
    m.insert(264, "renameat");
    m.insert(265, "linkat");
    m.insert(266, "symlinkat");
    m.insert(267, "readlinkat");
    m.insert(268, "fchmodat");
    m.insert(269, "faccessat");
    m.insert(270, "pselect6");
    m.insert(271, "ppoll");
    m.insert(272, "unshare");
    m.insert(273, "set_robust_list");
    m.insert(274, "get_robust_list");
    m.insert(275, "splice");
    m.insert(276, "tee");
    m.insert(277, "sync_file_range");
    m.insert(278, "vmsplice");
    m.insert(279, "move_pages");
    m.insert(280, "utimensat");
    m.insert(281, "epoll_pwait");
    m.insert(282, "signalfd");
    m.insert(283, "timerfd_create");
    m.insert(284, "eventfd");
    m.insert(285, "fallocate");
    m.insert(286, "timerfd_settime");
    m.insert(287, "timerfd_gettime");
    m.insert(288, "accept4");
    m.insert(289, "signalfd4");
    m.insert(290, "eventfd2");
    m.insert(291, "epoll_create1");
    m.insert(292, "dup3");
    m.insert(293, "pipe2");
    m.insert(294, "inotify_init1");
    m.insert(295, "preadv");
    m.insert(296, "pwritev");
    m.insert(297, "rt_tgsigqueueinfo");
    m.insert(298, "perf_event_open");
    m.insert(299, "recvmmsg");
    m.insert(300, "fanotify_init");
    m.insert(301, "fanotify_mark");
    m.insert(302, "prlimit64");
    m.insert(303, "name_to_handle_at");
    m.insert(304, "open_by_handle_at");
    m.insert(305, "clock_adjtime");
    m.insert(306, "syncfs");
    m.insert(307, "sendmmsg");
    m.insert(308, "setns");
    m.insert(309, "getcpu");
    m.insert(310, "process_vm_readv");
    m.insert(311, "process_vm_writev");
    m.insert(312, "kcmp");
    m.insert(313, "finit_module");
    m.insert(314, "sched_setattr");
    m.insert(315, "sched_getattr");
    m.insert(316, "renameat2");
    m.insert(317, "seccomp");
    m.insert(318, "getrandom");
    m.insert(319, "memfd_create");
    m.insert(320, "kexec_file_load");
    m.insert(321, "bpf");
    m.insert(322, "execveat");
    m.insert(323, "userfaultfd");
    m.insert(324, "membarrier");
    m.insert(325, "mlock2");
    m.insert(326, "copy_file_range");
    m.insert(327, "preadv2");
    m.insert(328, "pwritev2");
    m.insert(329, "pkey_mprotect");
    m.insert(330, "pkey_alloc");
    m.insert(331, "pkey_free");
    m.insert(332, "statx");
    m.insert(333, "io_pgetevents");
    m.insert(334, "rseq");
    // Common modern syscalls
    m.insert(435, "clone3");
    m.insert(439, "faccessat2");
    m.insert(448, "process_mrelease");
    m.insert(449, "futex_waitv");
    m.insert(450, "set_mempolicy_home_node");
    m.insert(451, "cachestat");
    m.insert(452, "fchmodat2");
    m
});

/// Resolve a syscall number to its name.
///
/// Returns the name (e.g. "read", "write", "exit") or a fallback
/// "syscall_{nr}" if the number is not in the table.
pub fn resolve_syscall(nr: u64) -> String {
    SYSCALL_TABLE
        .get(&nr)
        .map(|s| s.to_string())
        .unwrap_or_else(|| format!("syscall_{}", nr))
}

/// Get the full syscall table (for inspection / testing).
pub fn syscall_table() -> &'static HashMap<u64, &'static str> {
    &SYSCALL_TABLE
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_common_syscalls() {
        assert_eq!(resolve_syscall(0), "read");
        assert_eq!(resolve_syscall(1), "write");
        assert_eq!(resolve_syscall(2), "open");
        assert_eq!(resolve_syscall(3), "close");
        assert_eq!(resolve_syscall(9), "mmap");
        assert_eq!(resolve_syscall(39), "getpid");
        assert_eq!(resolve_syscall(56), "clone");
        assert_eq!(resolve_syscall(59), "execve");
        assert_eq!(resolve_syscall(60), "exit");
        assert_eq!(resolve_syscall(231), "exit_group");
        assert_eq!(resolve_syscall(257), "openat");
        assert_eq!(resolve_syscall(318), "getrandom");
        assert_eq!(resolve_syscall(435), "clone3");
    }

    #[test]
    fn test_unknown_syscall() {
        assert_eq!(resolve_syscall(99999), "syscall_99999");
    }

    #[test]
    fn test_table_size() {
        let table = syscall_table();
        assert!(table.len() >= 300, "Expected at least 300 syscalls, got {}", table.len());
    }
}
