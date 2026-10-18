#include <stdio.h>
#include <nova/process.h>
int main(void) {
    const char *argv[] = {"exit42", 0};
    long child = nova_spawn("/bin/exit42", argv, 0);
    if (child < 0) return 1;
    printf("Orphan test spawned PID %d\n", (int)child);
    return 0; /* init must adopt and reap the child. */
}
