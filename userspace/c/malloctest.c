#include <stdio.h>
#include <stdlib.h>
#include <string.h>
int main(void) {
    for (int round = 0; round < 8; ++round) {
        char *a = malloc(57), *b = calloc(19, 7);
        if (!a || !b) return 1;
        memset(a, 'N', 57);
        for (int i = 0; i < 133; ++i) if (b[i]) return 2;
        char *c = realloc(a, 153);
        if (!c) return 3;
        for (int i = 0; i < 57; ++i) if (c[i] != 'N') return 4;
        free(b); free(c);
    }
    puts("Malloc test passed.");
    return 0;
}
