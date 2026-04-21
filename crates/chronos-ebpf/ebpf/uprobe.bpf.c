/* SPDX-License-Identifier: (MIT OR Apache-2.0)
 * Minimal eBPF uprobe program that captures function entry/exit events.
 *
 * This program attaches to function entry/exit and writes fixed-size
 * EbpfEvent structs into a BPF ring buffer for consumption by userspace.
 */

#include <uapi/linux/bpf.h>
#include <uapi/linux/bpf_perf_event.h>
#include <bpf/bpf_helpers.h>
#include <bpf/bpf_tracing.h>

/* Max function name length — must match MAX_FUNC_NAME_LEN in Rust */
#define MAX_FUNC_NAME_LEN 64

/* Ring buffer map — written by this program, read by userspace */
struct {
    __uint(type, BPF_MAP_TYPE_RINGBUF);
    __uint(max_entries, 4096 * 128); /* 512KB ring buffer */
} events SEC(".maps");

/* Event kind — must match EbpfEventKind in Rust */
enum event_kind {
    EVENT_ENTRY = 0,
    EVENT_EXIT  = 1,
};

/* Fixed-size event struct — must match EbpfEvent in Rust (repr(C)) */
struct event {
    __u64 timestamp_ns;   /* bpf_ktime_get_ns() */
    __u64 thread_id;      /* bpf_get_current_pid_tgid() */
    __u64 address;         /* uprobe attach address */
    __u64 value;          /* for VariableWrite/MemoryWrite */
    __u8  kind;           /* event_kind */
    __u8  _pad[7];        /* padding to 8-byte alignment */
    __u8  function_name[MAX_FUNC_NAME_LEN];
};

/* Attach to function entry — pt_regs passed by BPF uprobe */
SEC("uprobe")
int trace_entry(struct pt_regs *ctx)
{
    __u64 timestamp_ns = bpf_ktime_get_ns();
    __u64 pid_tgid = bpf_get_current_pid_tgid();
    __u64 ip = bpf_get_func_ip(ctx);

    struct event *ev = bpf_ringbuf_reserve(&events, sizeof(struct event), 0);
    if (!ev)
        return 0;

    ev->timestamp_ns = timestamp_ns;
    ev->thread_id = pid_tgid >> 32; /* TGID in high 32 bits */
    ev->address = ip;
    ev->value = 0;
    ev->kind = EVENT_ENTRY;
    __builtin_memset(ev->function_name, 0, MAX_FUNC_NAME_LEN);

    /* Function name will be set via bpf_d_path or passed separately;
     * for now we use a placeholder since getting the symbol name
     * from within a uprobe is complex.
     */
    bpf_ringbuf_submit(ev, 0);
    return 0;
}

/* Attach to function exit */
SEC("uretprobe")
int trace_exit(struct pt_regs *ctx)
{
    __u64 timestamp_ns = bpf_ktime_get_ns();
    __u64 pid_tgid = bpf_get_current_pid_tgid();
    __u64 ip = bpf_get_func_ip(ctx);

    struct event *ev = bpf_ringbuf_reserve(&events, sizeof(struct event), 0);
    if (!ev)
        return 0;

    ev->timestamp_ns = timestamp_ns;
    ev->thread_id = pid_tgid >> 32;
    ev->address = ip;
    ev->value = 0;
    ev->kind = EVENT_EXIT;
    __builtin_memset(ev->function_name, 0, MAX_FUNC_NAME_LEN);

    bpf_ringbuf_submit(ev, 0);
    return 0;
}

char LICENSE[] SEC("license") = "Dual MIT/Apache-2.0";
