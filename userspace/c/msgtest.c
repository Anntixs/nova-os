#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <nova/fs.h>
#include <nova/process.h>
#include <nova/syscall.h>

int main(int argc, char **argv) {
    if (argc == 2) {
        long fd = atoi(argv[1]);
        char data[16];
        long count = nova_channel_receive(fd, data, sizeof data);
        if (count != 4 || memcmp(data, "ping", 4)) return 1;
        if (nova_channel_send(fd, "pong", 4) != 4) return 2;
        return 0;
    }
    long fd = nova_channel_create(0);
    if (fd < 0 || fd > 9) return 3;
    char number[2] = {(char)('0' + fd), 0};
    const char *args[] = {"msgtest", number, 0};
    long child = nova_spawn("/bin/msgtest", args, 0);
    if (child < 0) return 4;
    nova_yield(); /* Let the child block in receive before sending. */
    if (nova_channel_send(fd, "ping", 4) != 4) return 5;
    int status = -1;
    if (nova_wait(child, &status) < 0 || status) return 6;
    char result[8];
    if (nova_channel_receive(fd, result, 2) != NOVA_EMSGSIZE) return 7;
    if (nova_channel_receive(fd, result, sizeof result) != 4 || memcmp(result, "pong", 4)) return 8;
    nova_close(fd);
    long nonblocking = nova_channel_create(1);
    if (nonblocking < 0) return 9;
    if (nova_channel_receive(nonblocking, result, sizeof result) != NOVA_EAGAIN) return 10;
    nova_close(nonblocking);
    puts("Message channel test passed.");
    return 0;
}
