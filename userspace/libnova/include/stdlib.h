#ifndef NOVA_STDLIB_H
#define NOVA_STDLIB_H
#include <stddef.h>
void *malloc(size_t size);
void free(void *ptr);
void *calloc(size_t count,size_t size);
void *realloc(void *ptr,size_t size);
int atoi(const char *text);
long strtol(const char *text,char **end,int base);
char *getenv(const char *name);
void abort(void) __attribute__((noreturn));
void exit(int code) __attribute__((noreturn));
#endif
