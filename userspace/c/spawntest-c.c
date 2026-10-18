#include <stdio.h>
#include <nova/process.h>
int main(void) {
    const char *args[] = {"exit42", 0};
    long pid = nova_spawn("/bin/exit42", args, 0);
    if (pid < 0) return 1;
    int status = -1;
    if (nova_wait(pid, &status) < 0 || status != 42) return 2;
    printf("C spawn test passed: PID %d exited %d\n", (int)pid, status);
    return 0;
}
