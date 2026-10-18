#include <stdio.h>
#include <nova/process.h>

static void spin(void) {
    volatile unsigned long value = 0;
    for (unsigned long i = 0; i < 12000000UL; ++i) value += i;
    (void)value;
}

int main(int argc, char **argv) {
    if (argc > 1) {
        for (int i = 1; i <= 3; ++i) {
            printf("%s%d\n", argv[1], i);
            spin();
        }
        return 0;
    }
    const char *a[] = {"schedtest", "A", 0};
    const char *b[] = {"schedtest", "B", 0};
    long first = nova_spawn("/bin/schedtest", a, 0);
    long second = nova_spawn("/bin/schedtest", b, 0);
    if (first < 0 || second < 0) return 1;
    int first_status = -1, second_status = -1;
    if (nova_wait(first, &first_status) < 0 || nova_wait(second, &second_status) < 0)
        return 2;
    if (first_status || second_status) return 3;
    puts("Scheduler test passed.");
    return 0;
}
