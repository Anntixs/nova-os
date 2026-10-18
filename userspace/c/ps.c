#include <stdio.h>
#include <nova/fs.h>
#include <string.h>
int main(void) {
    long directory = nova_open("/proc", NOVA_O_RDONLY);
    if (directory < 0) return 1;
    struct nova_dirent entry;
    while (nova_readdir(directory, &entry) > 0) {
        if (entry.kind != NOVA_DIRECTORY) continue;
        entry.name[entry.name_length] = 0;
        char path[64];
        strcpy(path,"/proc/");strcpy(path+6,entry.name);strcpy(path+6+entry.name_length,"/status");
        long status = nova_open(path,NOVA_O_RDONLY);
        if (status < 0) continue;
        char data[256];long count = nova_read(status,data,sizeof data);
        nova_close(status);
        if (count > 0) nova_write(1,data,(size_t)count);
    }
    nova_close(directory);
    return 0;
}
