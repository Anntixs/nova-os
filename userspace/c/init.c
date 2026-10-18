#include <stdio.h>
#include <nova/process.h>
 #include <nova/fs.h>
static void reap_orphans(void) {
    for (;;) {
        int status;
        long pid = nova_wait(0, &status);
        (void)pid;
    }
}
int main(void) {
    puts("Nova init (development build)");
    char *reaper_stack = nova_map(16384, 3);
    if (!reaper_stack || nova_thread_create(reap_orphans, reaper_stack + 16384) < 0) return 3;
    for (;;) {
        const char *argv[] = {"nsh", 0};
        long child = nova_spawn("/bin/nsh", argv, 0);
        if (child < 0) { puts("init: cannot start /bin/nsh"); return 1; }
        puts("Starting NovaShell...");
        int status = -1;
        if (nova_wait(child, &status) < 0) { puts("init: wait failed"); return 2; }
        printf("init: shell exited with code %d\n", status);
        nova_sleep(1000);
    }
}
