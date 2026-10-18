#include <stdio.h>
#include <nova/fs.h>
int main(int argc, char **argv) {
    const char *path = argc > 1 ? argv[1] : ".";
    long fd = nova_open(path, NOVA_O_RDONLY);
    if (fd < 0) { printf("ls: cannot open %s\n", path); return 1; }
    struct nova_stat st;
    if (nova_stat(fd, &st) < 0) { nova_close(fd); return 1; }
    if (st.kind != NOVA_DIRECTORY) { puts(path); nova_close(fd); return 0; }
    struct nova_dirent entry; long result;
    while ((result = nova_readdir(fd, &entry)) > 0) {
        entry.name[entry.name_length < sizeof entry.name ? entry.name_length : sizeof entry.name - 1] = 0;
        printf("%s%s\n", entry.name, entry.kind == NOVA_DIRECTORY ? "/" : "");
    }
    nova_close(fd);
    return result < 0 ? 1 : 0;
}
