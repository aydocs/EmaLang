#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdint.h>

/**
 * EMA Native Runtime
 * Provides ARC helpers and standard library implementations for LLVM backend.
 */

typedef struct {
    int64_t ref_count;
    char data[];
} EmaString;

// --- Memory & ARC ---

void* ema_malloc(int64_t size) {
    EmaString* s = (EmaString*)malloc(sizeof(int64_t) + size);
    if (!s) return NULL;
    s->ref_count = 1;
    return s;
}

void ema_free(void* ptr) {
    if (!ptr) return;
    free(ptr);
}

void ema_retain(void* ptr) {
    if (!ptr) return;
    EmaString* s = (EmaString*)ptr;
    s->ref_count++;
}

void ema_release(void* ptr) {
    if (!ptr) return;
    EmaString* s = (EmaString*)ptr;
    s->ref_count--;
    if (s->ref_count <= 0) {
        free(s);
    }
}

// --- File I/O ---

void* ema_fs_read(void* path_ptr) {
    if (!path_ptr) return NULL;
    const char* path = ((EmaString*)path_ptr)->data;
    
    FILE* f = fopen(path, "rb");
    if (!f) return NULL;
    
    fseek(f, 0, SEEK_END);
    long size = ftell(f);
    fseek(f, 0, SEEK_SET);
    
    EmaString* res = (EmaString*)ema_malloc(size + 1);
    if (!res) {
        fclose(f);
        return NULL;
    }
    
    fread(res->data, 1, size, f);
    res->data[size] = '\0';
    fclose(f);
    
    return res;
}

void ema_fs_write(void* path_ptr, void* content_ptr) {
    if (!path_ptr || !content_ptr) return;
    const char* path = ((EmaString*)path_ptr)->data;
    const char* content = ((EmaString*)content_ptr)->data;
    
    FILE* f = fopen(path, "wb");
    if (!f) return;
    
    fwrite(content, 1, strlen(content), f);
    fclose(f);
}

// --- Debugging Helpers ---

void ema_debug_print_rc(void* ptr) {
    if (!ptr) return;
    EmaString* s = (EmaString*)ptr;
    printf("[EMA-DEBUG] RC: %lld\n", (long long)s->ref_count);
}
