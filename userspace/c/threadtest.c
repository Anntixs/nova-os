#include <stdio.h>
#include <nova/process.h>
#include <nova/fs.h>
static volatile unsigned first, second;
static volatile unsigned short_lived_done;
static void pause_cpu(void) { volatile unsigned n = 0; for (unsigned i = 0; i < 5000000; ++i) n += i; (void)n; }
static void worker_first(void) {
    for (unsigned i = 0; i < 3; ++i) { ++first; pause_cpu(); }
    nova_thread_exit();
}
static void worker_second(void) {
    for (unsigned i = 0; i < 3; ++i) { ++second; pause_cpu(); }
    nova_thread_exit();
}
static void short_lived_worker(void) {
    ++short_lived_done;
    nova_thread_exit();
}
int main(void) {
    char *a = nova_map(16384, 3), *b = nova_map(16384, 3);
    if (!a || !b) return 1;
    long t1 = nova_thread_create(worker_first, a + 16384);
    long t2 = nova_thread_create(worker_second, b + 16384);
    if (t1 < 0 || t2 < 0) return 2;
    if (nova_thread_join(t1) < 0 || nova_thread_join(t2) < 0) return 3;
    if (first != 3 || second != 3) return 4;
    for (unsigned i = 0; i < 64; ++i) {
        long tid = nova_thread_create(short_lived_worker, a + 16384);
        if (tid < 0 || nova_thread_join(tid) < 0 || short_lived_done != i + 1) return 6;
    }
    if (nova_unmap(a, 16384) < 0 || nova_unmap(b, 16384) < 0) return 5;
    puts("Userspace thread test passed.");
    return 0;
}
