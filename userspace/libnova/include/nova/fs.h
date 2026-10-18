#ifndef NOVA_FS_H
#define NOVA_FS_H
#include <stddef.h>
#include <stdint.h>
#define NOVA_O_RDONLY 0
#define NOVA_O_WRONLY 1
#define NOVA_O_RDWR 2
#define NOVA_O_CREAT 0x100
#define NOVA_O_TRUNC 0x200
#define NOVA_FILE 1
#define NOVA_DIRECTORY 2
struct nova_stat {uint32_t kind;uint32_t reserved;uint64_t size;};
struct nova_dirent {uint32_t kind;uint32_t name_length;char name[264];};
long nova_open(const char *path,unsigned long flags);
long nova_close(long fd);
long nova_read(long fd,void *buf,size_t count);
long nova_write(long fd,const void *buf,size_t count);
long nova_seek(long fd,long offset,unsigned long whence);
long nova_stat(long fd,struct nova_stat *info);
long nova_readdir(long fd,struct nova_dirent *entry);
long nova_getcwd(char *buffer,size_t size);
long nova_chdir(const char *path);
void *nova_map(size_t length,unsigned long protection);
long nova_unmap(void *address,size_t length);
long nova_pipe(long fds[2]);
long nova_channel_create(unsigned long flags);
long nova_channel_send(long fd,const void *data,size_t length);
long nova_channel_receive(long fd,void *data,size_t capacity);
#endif
