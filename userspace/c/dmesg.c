#include <stdio.h>
#include <nova/fs.h>
int main(void) {
    long fd = nova_open("/proc/kmsg", NOVA_O_RDONLY);
    if (fd < 0) return 1;
    char data[512]; long count;
    while ((count = nova_read(fd, data, sizeof data)) > 0)
        if (nova_write(1, data, (size_t)count) != count) {nova_close(fd);return 2;}
    nova_close(fd);
    return count < 0 ? 3 : 0;
}
