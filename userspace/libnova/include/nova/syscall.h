#ifndef NOVA_SYSCALL_H
#define NOVA_SYSCALL_H
#include <stddef.h>
#include <stdint.h>
#include <nova/abi_numbers.h>
long nova_syscall3(long number,long a,long b,long c);
#endif
