#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
#include <nova/process.h>
#include <nova/fs.h>
int main(void) {
    enum { SIZE = 2 * 1024 * 1024 };
    unsigned char *large = malloc(SIZE);
    if (!large || (uintptr_t)large % 16) return 1;
    for (unsigned i = 0; i < SIZE; i += 4096) large[i] = (unsigned char)(i / 4096);
    for (unsigned i = 0; i < SIZE; i += 4096)
        if (large[i] != (unsigned char)(i / 4096)) return 2;
    void *blocks[64];
    for (unsigned i = 0; i < 64; ++i) {
        blocks[i] = malloc(31 + i * 19);
        if (!blocks[i]) return 3;
        ((unsigned char *)blocks[i])[0] = (unsigned char)i;
    }
    for (unsigned i = 0; i < 64; ++i) {
        if (((unsigned char *)blocks[i])[0] != i) return 4;
        free(blocks[i]);
    }
    free(large);
    const char *argv[] = {"badmem", 0};
    long child = nova_spawn("/bin/badmem", argv, 0);
    int status = 0;
    if (child < 0 || nova_wait(child, &status) < 0 || status != -14) return 5;
    const char *guard_args[] = {"stackguard", 0};
    child = nova_spawn("/bin/stackguard", guard_args, 0);
    if (child < 0 || nova_wait(child, &status) < 0 || status != -14) return 6;
    puts("Memory test passed. Invalid access isolated.");
    return 0;
}
