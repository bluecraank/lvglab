#ifndef LVGLAB_SHIM_H
#define LVGLAB_SHIM_H
#include <stdint.h>

/* Sets up LVGL with a display of w x h pixels that draws into a 32 bit buffer. */
int lvglab_init(int w, int h);
/* Pixels (ARGB8888 as bytes B, G, R, A) of the display. */
const uint8_t *lvglab_pixels(void);
/* Redraws everything now. */
void lvglab_refresh(void);
/* The active screen. */
void *lvglab_screen(void);

/* Functions and constants of LVGL by name, for the interpreter. */
typedef struct { const char *name; void *address; } lvglab_function;
typedef struct { const char *name; long long value; } lvglab_constant;
extern const lvglab_function lvglab_functions[];
extern const lvglab_function lvglab_symbols[];
extern const lvglab_constant lvglab_constants[];
#endif
