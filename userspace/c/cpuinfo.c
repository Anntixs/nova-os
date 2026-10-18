#include <stdio.h>
#include <nova/fs.h>
int main(void) {
    long fd=nova_open("/proc/cpuinfo",NOVA_O_RDONLY);
    if(fd<0){puts("cpuinfo: cannot read /proc/cpuinfo");return 1;}
    char buffer[256];long count;
    while((count=nova_read(fd,buffer,sizeof buffer))>0){
        if(nova_write(1,buffer,(size_t)count)!=count){nova_close(fd);return 1;}
    }
    nova_close(fd);
    return count<0;
}
