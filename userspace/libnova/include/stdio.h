#ifndef NOVA_STDIO_H
#define NOVA_STDIO_H
#include <stddef.h>
typedef struct {int fd;} FILE;
extern FILE *stdout;
extern FILE *stderr;
int putchar(int c);
int puts(const char *text);
int printf(const char *fmt,...);
size_t fwrite(const void *ptr,size_t size,size_t count,FILE *stream);
#endif
