#include <stdint.h>
int main(void) {
    volatile unsigned char byte = *(volatile unsigned char *)(uintptr_t)0x000040000fffdfffULL;
    return (int)byte;
}
