#include <stdio.h>
#include <stdint.h>
#include <nova/graphics.h>
#include <nova/process.h>
int main(void) {
    struct nova_fb_info info;
    if (nova_fb_info(&info) < 0 || info.width < 100 || info.height < 100) return 1;
    if (nova_fb_acquire() < 0) return 2;
    struct nova_fb_fill rect = {info.width - 80, 20, 64, 64, 0x245ca8};
    if (nova_fb_fill(&rect) < 0) return 3;
    uint32_t pixels[64];
    for (unsigned i = 0; i < 64; ++i) pixels[i] = (i + i / 8) & 1 ? 0xffffff : 0x62a0ea;
    struct nova_fb_blit patch = {rect.x + 28, rect.y + 28, 8, 8, 8, 0, pixels};
    if (nova_fb_blit(&patch) < 0) return 4;
    nova_sleep(150);
    rect.rgb = 0x10131b;
    if (nova_fb_fill(&rect) < 0 || nova_fb_release() < 0) return 5;
    puts("Framebuffer test passed.");
    return 0;
}
