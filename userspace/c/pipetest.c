#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <nova/fs.h>
#include <nova/process.h>

int main(int argc, char **argv) {
    if (argc == 3) {
        long read_fd = atoi(argv[1]), write_fd = atoi(argv[2]);
        nova_close(read_fd);
        const char message[] = "pipe message";
        long count = nova_write(write_fd, message, sizeof message - 1);
        nova_close(write_fd);
        return count == sizeof message - 1 ? 0 : 1;
    }
    long fds[2];
    if (nova_pipe(fds) < 0) return 2;
    /* Initial Nova pipe descriptors are allocated after standard FDs. */
    char r[16], w[16];
    if (fds[0] > 9 || fds[1] > 9) return 3;
    r[0] = (char)('0' + fds[0]); r[1] = 0;
    w[0] = (char)('0' + fds[1]); w[1] = 0;
    const char *child_args[] = {"pipetest", r, w, 0};
    long child = nova_spawn("/bin/pipetest", child_args, 0);
    if (child < 0) return 4;
    nova_close(fds[1]);
    char buffer[32];
    long count = nova_read(fds[0], buffer, sizeof buffer);
    if (count != 12 || memcmp(buffer, "pipe message", 12)) return 5;
    if (nova_read(fds[0], buffer, sizeof buffer) != 0) return 6;
    nova_close(fds[0]);
    int status = -1;
    if (nova_wait(child, &status) < 0 || status) return 7;
    puts("Pipe test passed.");
    return 0;
}
