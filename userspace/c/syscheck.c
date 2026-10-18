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
    puts("Syscall validation passed.");
    return 0;
}
