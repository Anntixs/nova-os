#include <stdio.h>
#include <nova/fs.h>
static int show(const char *path) {
    long fd = nova_open(path, NOVA_O_RDONLY);
    if (fd < 0) return 1;
    char data[512]; long n;
    while ((n = nova_read(fd, data, sizeof data)) > 0)
        if (nova_write(1, data, (size_t)n) != n) { nova_close(fd); return 1; }
    nova_close(fd);
    return n < 0;
}
int main(void) {
    puts("Nova system information");
    if (show("/proc/system") || show("/proc/cpuinfo") || show("/proc/meminfo") || show("/proc/uptime")) return 1;
    return 0;
}
