#include "lvgl.h"
#include "shim.h"
#include <stdlib.h>

static lv_disp_draw_buf_t draw_buf;
static lv_disp_drv_t disp_drv;
static lv_color_t *pixels;
static int width, height;
static int ready;

static void flush(lv_disp_drv_t *drv, const lv_area_t *area, lv_color_t *color_p)
{
    (void)area;
    (void)color_p;
    lv_disp_flush_ready(drv);
}

static lv_disp_t *disp;
static int lv_ready;

int lvglab_init(int w, int h)
{
    if (ready && w == width && h == height) return 1;
    if (ready) {
        /* A new display size: drop the old display with its screens. */
        lv_disp_remove(disp);
        free(pixels);
        pixels = NULL;
    }
    width = w;
    height = h;
    pixels = (lv_color_t *)calloc((size_t)w * (size_t)h, sizeof(lv_color_t));
    if (!pixels) return 0;
    if (!lv_ready) {
        lv_init();
        lv_ready = 1;
    }
    lv_disp_draw_buf_init(&draw_buf, pixels, NULL, (uint32_t)(w * h));
    lv_disp_drv_init(&disp_drv);
    disp_drv.draw_buf = &draw_buf;
    disp_drv.flush_cb = flush;
    disp_drv.hor_res = (lv_coord_t)w;
    disp_drv.ver_res = (lv_coord_t)h;
    disp_drv.full_refresh = 1;
    disp = lv_disp_drv_register(&disp_drv);
    ready = 1;
    return 1;
}

const uint8_t *lvglab_pixels(void)
{
    return (const uint8_t *)pixels;
}

void lvglab_refresh(void)
{
    lv_refr_now(NULL);
}

void *lvglab_screen(void)
{
    return lv_scr_act();
}
