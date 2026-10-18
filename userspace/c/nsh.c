#include <stdio.h>
#include <string.h>
#include <stdlib.h>
#include <nova/fs.h>
#include <nova/process.h>

static char line[512], cwd[256], path[320];
static char *argv[33];
static long background[16];
static int background_count;

static void collect_background(void) {
    for (int i = 0; i < background_count;) {
        int status = 0;
        long result = nova_wait_try(background[i], &status);
        if (result >= 0 || result == -10) {
            if (result >= 0) printf("[PID %d exited %d]\n", (int)result, status);
            background[i] = background[--background_count];
        } else ++i;
    }
}

static long start_external(void) {
    if (strchr(argv[0], '/')) return nova_spawn(argv[0], (const char *const *)argv, 0);
    const char *search = getenv("PATH");
    if (!search || !*search) search = "/bin";
    while (*search) {
        size_t prefix = 0;
        while (search[prefix] && search[prefix] != ':') ++prefix;
        size_t name = strlen(argv[0]);
        if (prefix + name + 2 < sizeof path) {
            memcpy(path, search, prefix); path[prefix] = '/';
            memcpy(path + prefix + 1, argv[0], name + 1);
            long pid = nova_spawn(path, (const char *const *)argv, 0);
            if (pid >= 0 || pid != -2) return pid;
        }
        search += prefix;
        if (*search == ':') ++search;
    }
    return -2;
}

static int parse(void) {
    int count = 0;
    char *read = line, *write = line;
    while (*read) {
        while (*read == ' ' || *read == '\t' || *read == '\n') ++read;
        if (!*read) break;
        if (count == 32) return -1;
        argv[count++] = write;
        char quote = 0;
        while (*read) {
            char ch = *read++;
            if (ch == '\\' && *read) { *write++ = *read++; continue; }
            if (quote) { if (ch == quote) quote = 0; else *write++ = ch; continue; }
            if (ch == '\'' || ch == '"') { quote = ch; continue; }
            if (ch == ' ' || ch == '\t' || ch == '\n') break;
            *write++ = ch;
        }
        if (quote) return -1;
        *write++ = 0;
    }
    argv[count] = 0;
    return count;
}

int main(void) {
    puts("NovaShell (Ring 3). Type help for commands.");
    for (;;) {
        collect_background();
        if (nova_getcwd(cwd, sizeof cwd) < 0) strcpy(cwd, "?");
        printf("nova:%s$ ", cwd);
        long count = nova_read(0, line, sizeof line - 1);
        if (count <= 0) { if (count < 0) puts("nsh: stdin error"); return 1; }
        line[count] = 0;
        int argc = parse();
        if (argc < 0) { puts("nsh: invalid command line"); continue; }
        if (!argc) continue;
        int detached = argc > 1 && !strcmp(argv[argc - 1], "&");
        if (detached) argv[--argc] = 0;
        if (!strcmp(argv[0], "help")) { puts("Builtins: cd pwd clear env kill help exit\nPrograms: ls cat echo ps uptime sysinfo dmesg hello-c"); continue; }
        if (!strcmp(argv[0], "exit")) return 0;
        if (!strcmp(argv[0], "clear")) { nova_console_clear(); continue; }
        if (!strcmp(argv[0], "env")) {const char *value;
            if ((value = getenv("PATH"))) printf("PATH=%s\n", value);
            if ((value = getenv("HOME"))) printf("HOME=%s\n", value);
            if ((value = getenv("SHELL"))) printf("SHELL=%s\n", value);
            continue; }
        if (!strcmp(argv[0], "kill")) {
            if (argc != 2) {puts("usage: kill <child-pid>"); continue;}
            long target = atoi(argv[1]);
            if (nova_kill(target, 143) < 0) puts("kill: process unavailable or not a child");
            else {int status; nova_wait(target, &status); printf("Killed PID %d\n", (int)target);}
            continue;
        }
        if (!strcmp(argv[0], "cd")) {
            if (nova_chdir(argc > 1 ? argv[1] : "/") < 0) puts("nsh: cd failed");
            continue;
        }
        if (!strcmp(argv[0], "pwd")) { puts(cwd); continue; }
        long child = start_external();
        if (child < 0) { printf("nsh: cannot run %s (error %d)\n", argv[0], (int)child); continue; }
        if (detached) {
            if (background_count < 16) background[background_count++] = child;
            printf("[PID %d running]\n", (int)child);
            continue;
        }
        int status;
        if (nova_wait(child, &status) < 0) puts("nsh: wait failed");
        else if (status) printf("nsh: process exited with code %d\n", status);
    }
}
