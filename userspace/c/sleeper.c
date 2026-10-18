#include <stdio.h>
#include <stdlib.h>
#include <nova/process.h>
int main(int argc,char **argv) {
    unsigned long ms = argc > 1 ? (unsigned long)atoi(argv[1]) : 1000;
    if (nova_sleep(ms) < 0) return 1;
    puts("Sleeper woke.");
    return 0;
}
