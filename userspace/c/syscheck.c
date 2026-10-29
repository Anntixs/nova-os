#include <stdio.h>
#include <nova/fs.h>
#include <nova/process.h>
#include <nova/syscall.h>
int main(void) {
    if (nova_read(42, (void *)0x1000, 1) != NOVA_EFAULT) return 1;
    if (nova_read(42, (void *)0, 0) != NOVA_EBADF) return 2;
    if (nova_open((const char *)0x1000, 0) != NOVA_EFAULT) return 3;
    if (nova_spawn("/bin/hello", (const char *const *)0x1000, 0) != NOVA_EFAULT) return 4;
    if (nova_wait(99999, 0) >= 0) return 5;
    if (nova_syscall3(999, 0, 0, 0) != NOVA_ENOSYS) return 6;
    if (nova_write(2, "Nova stderr works.\n", 19) != 19) return 7;
    long zero = nova_open("/dev/zero", NOVA_O_RDONLY);
    long sink = nova_open("/dev/null", NOVA_O_WRONLY);
    unsigned char *pages = nova_map(8192, 3);
    if (zero < 0 || sink < 0 || !pages) return 8;
    for (unsigned i = 0; i < 32; ++i) pages[4096 - 16 + i] = (unsigned char)(i + 1);
    if (nova_write(sink, pages + 4096 - 16, 32) != 32) return 9;
    if (nova_read(zero, pages + 4096 - 16, 32) != 32) return 10;
    for (unsigned i = 0; i < 32; ++i) if (pages[4096 - 16 + i] != 0) return 11;
    if (nova_unmap(pages, 8192) < 0) return 12;
    pages = nova_map(4096, 3);
    if (!pages) return 13;
    if (nova_write(sink, pages + 4096 - 8, 16) != NOVA_EFAULT) return 14;
    if (nova_read(zero, pages + 4096 - 8, 16) != NOVA_EFAULT) return 15;
    if (nova_unmap(pages, 4096) < 0 || nova_close(zero) < 0 || nova_close(sink) < 0) return 16;
    puts("Cross-page usercopy validation passed.");
    puts("Syscall validation passed.");
    return 0;
}
