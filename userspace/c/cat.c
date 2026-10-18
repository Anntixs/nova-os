#include <stdio.h>
#include <nova/fs.h>
int main(int argc, char **argv) {
    if (argc < 2) { puts("usage: cat <file>"); return 2; }
    for (int i = 1; i < argc; ++i) {
        long fd = nova_open(argv[i], NOVA_O_RDONLY);
        if (fd < 0) { printf("cat: cannot open %s\n", argv[i]); return 1; }
        char buffer[512]; long count;
        while ((count = nova_read(fd, buffer, sizeof buffer)) > 0)
            if (nova_write(1, buffer, (size_t)count) != count) { nova_close(fd); return 1; }
        nova_close(fd);
        if (count < 0) return 1;
    }
    return 0;
}
