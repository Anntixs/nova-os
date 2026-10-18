#ifndef NOVA_PROCESS_H
#define NOVA_PROCESS_H
long nova_spawn(const char *path,const char *const *argv,const char *const *envp);
long nova_wait(long pid,int *status);
long nova_getpid(void);
long nova_yield(void);
long nova_sleep(unsigned long milliseconds);
long nova_clock_get(void);
long nova_thread_create(void (*entry)(void), void *stack_top);
void nova_thread_exit(void) __attribute__((noreturn));
long nova_thread_join(long tid);
long nova_kill(long pid,int code);
long nova_wait_try(long pid,int *status);
long nova_console_clear(void);
#endif
