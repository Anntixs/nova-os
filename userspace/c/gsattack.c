#include <nova/process.h>
#include <stdio.h>

int main(void) {
    /* A Ring 3 program can load a null GS selector. Kernel entry must restore
       the trusted CPU-local GS base before looking up its syscall stack. */
    __asm__ volatile("xor %%eax, %%eax; mov %%ax, %%gs" ::: "rax", "memory");
    long start = nova_clock_get();
    if (start < 0) return 1;
    while (nova_clock_get() - start < 25) {
        /* Allow a timer IRQ to arrive while userspace GS is null. */
    }
    if (nova_getpid() <= 0) return 2;
    puts("GS syscall and timer entry test passed.");
    return 0;
}
