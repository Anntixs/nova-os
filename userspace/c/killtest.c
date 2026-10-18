#include <stdio.h>
#include <nova/process.h>
int main(void) {
    const char *args[] = {"sleeper", "5000", 0};
    long child = nova_spawn("/bin/sleeper", args, 0);
    if (child < 0) return 1;
    if (nova_kill(child, 143) < 0) return 2;
    int status = 0;
    if (nova_wait(child, &status) != child || status != 143) return 3;
    puts("Kill test passed.");
    return 0;
}
