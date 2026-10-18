#include <stdio.h>
#include <string.h>
#include <stdint.h>
#include <stdarg.h>
#include <nova/fs.h>

static FILE out = {1}, err = {2};
FILE *stdout = &out, *stderr = &err;

size_t fwrite(const void *p, size_t size, size_t count, FILE *stream) {
    if (size && count > ((size_t)-1) / size) return 0;
    size_t n = size * count;
    long r = nova_write(stream->fd, p, n);
    return r < 0 ? 0 : (size ? (size_t)r / size : count);
}
int putchar(int c) {
    char b = (char)c;
    return nova_write(1, &b, 1) == 1 ? (unsigned char)b : -1;
}

typedef struct {
    char bytes[512];
    size_t used;
    int total;
    int failed;
} PrintBuffer;

static void flush(PrintBuffer *b) {
    size_t done = 0;
    while (done < b->used && !b->failed) {
        long n = nova_write(1, b->bytes + done, b->used - done);
        if (n <= 0) { b->failed = 1; break; }
        done += (size_t)n;
    }
    b->used = 0;
}
static void emit(PrintBuffer *b, const char *text, size_t length) {
    while (length && !b->failed) {
        if (b->used == sizeof b->bytes) flush(b);
        size_t room = sizeof b->bytes - b->used;
        size_t n = length < room ? length : room;
        memcpy(b->bytes + b->used, text, n);
        b->used += n;
        b->total += (int)n;
        text += n;
        length -= n;
    }
}
static void number(PrintBuffer *b, uint64_t value, unsigned base, int negative, int upper) {
    char text[32];
    size_t i = sizeof text;
    do {
        unsigned digit = (unsigned)(value % base);
        text[--i] = (char)(digit < 10 ? '0' + digit : (upper ? 'A' : 'a') + digit - 10);
        value /= base;
    } while (value);
    if (negative) text[--i] = '-';
    emit(b, text + i, sizeof text - i);
}
int printf(const char *fmt, ...) {
    PrintBuffer b = {{0}, 0, 0, 0};
    va_list args;
    va_start(args, fmt);
    for (const char *p = fmt; *p && !b.failed; ++p) {
        if (*p != '%') { emit(&b, p, 1); continue; }
        ++p;
        if (!*p) break;
        switch (*p) {
        case '%': emit(&b, "%", 1); break;
        case 'c': { char c = (char)va_arg(args, int); emit(&b, &c, 1); break; }
        case 's': { const char *s = va_arg(args, const char *); if (!s) s = "(null)"; emit(&b, s, strlen(s)); break; }
        case 'd': { int v = va_arg(args, int); number(&b, v < 0 ? (uint64_t)(-(int64_t)v) : (uint64_t)v, 10, v < 0, 0); break; }
        case 'u': number(&b, va_arg(args, unsigned), 10, 0, 0); break;
        case 'x': number(&b, va_arg(args, unsigned), 16, 0, 0); break;
        case 'p': emit(&b, "0x", 2); number(&b, (uintptr_t)va_arg(args, void *), 16, 0, 0); break;
        default: b.failed = 1; break;
        }
    }
    va_end(args);
    if (!b.failed) flush(&b);
    return b.failed ? -1 : b.total;
}
int puts(const char *s) { return printf("%s\n", s); }
