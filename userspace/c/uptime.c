#include <stdio.h>
#include <nova/process.h>
int main(void) {
    long milliseconds = nova_clock_get();
    if (milliseconds < 0) return 1;
    printf("Uptime: %d seconds\n", (int)(milliseconds / 1000));
    return 0;
}
