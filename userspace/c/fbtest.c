#include <stdio.h>
#include <stdint.h>
#include <nova/graphics.h>
#include <nova/process.h>
int main(void) {
    struct nova_fb_info info;
    if (nova_fb_info(&info) < 0 || info.width < 100 || info.height < 100) return 1;
    if (nova_fb_acquire() < 0) return 2;
    struct nova_fb_fill rect = {0, 0, info.width, info.height, 0x245ca8};
    if (nova_fb_fill(&rect) < 0) return 3;
    uint32_t pixels[64];
    for (unsigned i = 0; i < 64; ++i) pixels[i] = (i + i / 8) & 1 ? 0xffffff : 0x62a0ea;
    struct nova_fb_blit patch = {info.width - 52, 48, 8, 8, 8, 0, pixels};
    if (nova_fb_blit(&patch) < 0) return 4;
    puts("[fbtest] framebuffer lease active");
    nova_sleep(1000);
    if (nova_fb_release() < 0) return 5;
    puts("Framebuffer test passed.");
    return 0;
}
