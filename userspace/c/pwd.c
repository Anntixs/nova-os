#include <stdio.h>
#include <nova/fs.h>
int main(void) {
    char cwd[256];
    if (nova_getcwd(cwd, sizeof cwd) < 0) { puts("pwd: getcwd failed"); return 1; }
    puts(cwd);
    return 0;
}
