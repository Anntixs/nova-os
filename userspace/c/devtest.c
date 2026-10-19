#include <stdio.h>
#include <stdint.h>
#include <nova/fs.h>
#include <nova/input.h>
int main(void) {
    long fd = nova_open("/dev/null", NOVA_O_RDONLY);
    if (fd < 0) return 1;
    char data[32];
    if (nova_read(fd, data, sizeof data) != 0) return 2;
    nova_close(fd);
    fd = nova_open("/dev/zero", NOVA_O_RDONLY);
    if (fd < 0 || nova_read(fd, data, sizeof data) != sizeof data) return 3;
    for (unsigned i = 0; i < sizeof data; ++i) if (data[i]) return 4;
    nova_close(fd);
    fd = nova_open("/dev/fb0", NOVA_O_RDONLY);
    if (fd < 0) return 5;
    uint32_t geometry[5];
    if (nova_read(fd, geometry, sizeof geometry) != sizeof geometry) return 6;
    nova_close(fd);
    if (!geometry[0] || !geometry[1] || geometry[2] < geometry[0] || geometry[4] != 32) return 7;
    fd = nova_open("/dev/kbd0", NOVA_O_RDONLY);
    if (fd < 0) return 8;
    unsigned char event[16];
    if (nova_read(fd, event, sizeof event) != -1) return 9;
    if (nova_input_acquire() < 0) return 9;
    puts("[devtest] keyboard lease active");
    for (;;) {
        if (nova_read(fd, event, sizeof event) != sizeof event || event[2] == 0) return 10;
        /* A delayed release from the command line may arrive first. */
        if (event[0] == 1 && event[4] == 'x') break;
    }
    if (nova_input_release() < 0) return 11;
    nova_close(fd);
    printf("Device test passed. Framebuffer %u x %u\n", geometry[0], geometry[1]);
    return 0;
}
