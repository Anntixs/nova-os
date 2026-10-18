#ifndef NOVA_GRAPHICS_H
#define NOVA_GRAPHICS_H
#include <stdint.h>
struct nova_fb_info { uint32_t width,height,stride,format,bits_per_pixel,pitch_bytes; };
struct nova_fb_fill { uint32_t x,y,width,height,rgb; };
struct nova_fb_blit { uint32_t x,y,width,height,stride,reserved; const uint32_t *pixels; };
long nova_fb_info(struct nova_fb_info *info);
long nova_fb_acquire(void);
long nova_fb_release(void);
long nova_fb_fill(const struct nova_fb_fill *rect);
long nova_fb_blit(const struct nova_fb_blit *rect);
#endif
