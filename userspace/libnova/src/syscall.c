#include <nova/syscall.h>
#include <nova/process.h>
#include <nova/fs.h>
#include <nova/graphics.h>
#include <nova/input.h>
#include <stdlib.h>
#include <string.h>
char **nova_environ;
char *getenv(const char *name){
 if(!nova_environ)return 0;
 size_t length=strlen(name);
 for(char **item=nova_environ;*item;item++)
  if(!strncmp(*item,name,length)&&(*item)[length]=='=')return *item+length+1;
 return 0;
}
long nova_syscall3(long number,long a,long b,long c) {
 long result;
 __asm__ volatile("syscall":"=a"(result):"a"(number),"D"(a),"S"(b),"d"(c):"rcx","r11","memory");
 return result;
}
long nova_spawn(const char *path,const char *const *argv,const char *const *envp){return nova_syscall3(NOVA_SYS_SPAWN,(long)path,(long)argv,(long)envp);}
long nova_wait(long pid,int *status){return nova_syscall3(NOVA_SYS_WAIT,pid,(long)status,0);}
long nova_getpid(void){return nova_syscall3(NOVA_SYS_GETPID,0,0,0);}
long nova_yield(void){return nova_syscall3(NOVA_SYS_YIELD,0,0,0);}
long nova_sleep(unsigned long ms){return nova_syscall3(NOVA_SYS_SLEEP,ms,0,0);}
long nova_clock_get(void){return nova_syscall3(NOVA_SYS_CLOCK_GET,0,0,0);}
long nova_thread_create(void (*entry)(void),void *stack_top){return nova_syscall3(NOVA_SYS_THREAD_CREATE,(long)entry,(long)stack_top,0);}
void nova_thread_exit(void){nova_syscall3(NOVA_SYS_THREAD_EXIT,0,0,0);for(;;){}}
long nova_thread_join(long tid){return nova_syscall3(NOVA_SYS_THREAD_JOIN,tid,0,0);}
long nova_kill(long pid,int code){return nova_syscall3(NOVA_SYS_KILL,pid,code,0);}
long nova_wait_try(long pid,int *status){return nova_syscall3(NOVA_SYS_WAIT_TRY,pid,(long)status,0);}
long nova_console_clear(void){return nova_syscall3(NOVA_SYS_TTY_CLEAR,0,0,0);}
long nova_open(const char *path,unsigned long flags){return nova_syscall3(NOVA_SYS_OPEN,(long)path,flags,0);}
long nova_close(long fd){return nova_syscall3(NOVA_SYS_CLOSE,fd,0,0);}
long nova_read(long fd,void *buf,size_t count){return nova_syscall3(NOVA_SYS_READ,fd,(long)buf,count);}
long nova_write(long fd,const void *buf,size_t count){return nova_syscall3(NOVA_SYS_WRITE,fd,(long)buf,count);}
long nova_seek(long fd,long offset,unsigned long whence){return nova_syscall3(NOVA_SYS_SEEK,fd,offset,whence);}
long nova_stat(long fd,struct nova_stat *info){return nova_syscall3(NOVA_SYS_STAT,fd,(long)info,0);}
long nova_readdir(long fd,struct nova_dirent *entry){return nova_syscall3(NOVA_SYS_READDIR,fd,(long)entry,sizeof(*entry));}
long nova_getcwd(char *buffer,size_t size){return nova_syscall3(NOVA_SYS_GETCWD,(long)buffer,size,0);}
long nova_chdir(const char *path){return nova_syscall3(NOVA_SYS_CHDIR,(long)path,0,0);}
void *nova_map(size_t length,unsigned long protection){long value=nova_syscall3(NOVA_SYS_MAP,length,protection,0);return value<0?(void*)0:(void*)value;}
long nova_unmap(void *address,size_t length){return nova_syscall3(NOVA_SYS_UNMAP,(long)address,length,0);}
long nova_pipe(long fds[2]){return nova_syscall3(NOVA_SYS_PIPE,(long)fds,0,0);}
long nova_channel_create(unsigned long flags){return nova_syscall3(NOVA_SYS_CHANNEL_CREATE,flags,0,0);}
long nova_channel_send(long fd,const void *data,size_t length){return nova_syscall3(NOVA_SYS_CHANNEL_SEND,fd,(long)data,length);}
long nova_channel_receive(long fd,void *data,size_t capacity){return nova_syscall3(NOVA_SYS_CHANNEL_RECEIVE,fd,(long)data,capacity);}
long nova_fb_info(struct nova_fb_info *info){return nova_syscall3(NOVA_SYS_FB_INFO,(long)info,0,0);}
long nova_fb_acquire(void){return nova_syscall3(NOVA_SYS_FB_ACQUIRE,0,0,0);}
long nova_fb_release(void){return nova_syscall3(NOVA_SYS_FB_RELEASE,0,0,0);}
long nova_fb_fill(const struct nova_fb_fill *rect){return nova_syscall3(NOVA_SYS_FB_FILL,(long)rect,0,0);}
long nova_fb_blit(const struct nova_fb_blit *rect){return nova_syscall3(NOVA_SYS_FB_BLIT,(long)rect,0,0);}
long nova_input_acquire(void){return nova_syscall3(NOVA_SYS_INPUT_ACQUIRE,0,0,0);}
long nova_input_release(void){return nova_syscall3(NOVA_SYS_INPUT_RELEASE,0,0,0);}
void exit(int code){nova_syscall3(NOVA_SYS_EXIT,code,0,0);for(;;){}}
void abort(void){exit(134);}
