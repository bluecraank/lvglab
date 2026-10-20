/* The display of the LVGL 9 preview: draws into a 32 bit buffer. */
#include "lvgl.h"
#include <stdlib.h>

static lv_display_t *display;
static uint8_t *pixels;
static int width, height;
static int initialized;

static void flush(lv_display_t *disp, const lv_area_t *area, uint8_t *px_map)
{
    (void)area;
    (void)px_map;
    lv_display_flush_ready(disp);
}

int lvglab9_init(int w, int h)
{
    if (display && w == width && h == height) return 1;
    if (!initialized) {
        lv_init();
        initialized = 1;
    }
    if (display) {
        lv_display_delete(display);
        display = NULL;
        free(pixels);
    }
    width = w;
    height = h;
    uint32_t stride = lv_draw_buf_width_to_stride((uint32_t)w, LV_COLOR_FORMAT_XRGB8888);
    size_t size = (size_t)stride * (size_t)h;
    pixels = (uint8_t *)calloc(1, size + 64);
    if (!pixels) return 0;
    display = lv_display_create(w, h);
    lv_display_set_color_format(display, LV_COLOR_FORMAT_XRGB8888);
    lv_display_set_buffers(display, pixels, NULL, (uint32_t)size, LV_DISPLAY_RENDER_MODE_FULL);
    lv_display_set_flush_cb(display, flush);
    return 1;
}

/* Bytes per row of the pixel buffer. */
int lvglab9_stride(void)
{
    return (int)lv_draw_buf_width_to_stride((uint32_t)width, LV_COLOR_FORMAT_XRGB8888);
}

const uint8_t *lvglab9_pixels(void)
{
    return pixels;
}

void lvglab9_refresh(void)
{
    lv_obj_invalidate(lv_screen_active());
    lv_refr_now(display);
}

void *lvglab9_screen(void)
{
    return lv_screen_active();
}

/* Fonts and images of the export, built with the structs of this LVGL. */

typedef struct {
    uint32_t index;
    uint32_t adv;
    uint8_t w;
    uint8_t h;
    int8_t x;
    int8_t y;
} lvglab_glyph;

typedef struct {
    uint32_t start;
    uint16_t length;
    uint16_t first;
} lvglab_cmap;

void *lvglab9_font_new(const uint8_t *bitmap, const lvglab_glyph *glyphs, uint32_t glyph_count,
                       const lvglab_cmap *cmaps, uint32_t cmap_count, int line_height, int base_line,
                       int underline_position, int underline_thickness, int bpp)
{
    lv_font_fmt_txt_glyph_dsc_t *g = calloc(glyph_count ? glyph_count : 1, sizeof(*g));
    lv_font_fmt_txt_cmap_t *c = calloc(cmap_count ? cmap_count : 1, sizeof(*c));
    lv_font_fmt_txt_dsc_t *d = calloc(1, sizeof(*d));
    lv_font_t *f = calloc(1, sizeof(*f));
    if (!g || !c || !d || !f) return NULL;
    for (uint32_t i = 0; i < glyph_count; i++) {
        g[i].bitmap_index = glyphs[i].index;
        g[i].adv_w = glyphs[i].adv;
        g[i].box_w = glyphs[i].w;
        g[i].box_h = glyphs[i].h;
        g[i].ofs_x = glyphs[i].x;
        g[i].ofs_y = glyphs[i].y;
    }
    for (uint32_t i = 0; i < cmap_count; i++) {
        c[i].range_start = cmaps[i].start;
        c[i].range_length = cmaps[i].length;
        c[i].glyph_id_start = cmaps[i].first;
        c[i].type = LV_FONT_FMT_TXT_CMAP_FORMAT0_TINY;
    }
    d->glyph_bitmap = bitmap;
    d->glyph_dsc = g;
    d->cmaps = c;
    d->cmap_num = (uint16_t)cmap_count;
    d->bpp = (uint8_t)bpp;
    f->get_glyph_dsc = lv_font_get_glyph_dsc_fmt_txt;
    f->get_glyph_bitmap = lv_font_get_bitmap_fmt_txt;
    f->line_height = line_height;
    f->base_line = base_line;
    f->subpx = LV_FONT_SUBPX_NONE;
    f->underline_position = (int8_t)underline_position;
    f->underline_thickness = (int8_t)underline_thickness;
    f->dsc = d;
    return f;
}

void lvglab9_font_free(void *font)
{
    lv_font_t *f = font;
    if (!f) return;
    const lv_font_fmt_txt_dsc_t *d = f->dsc;
    if (d) {
        free((void *)d->glyph_dsc);
        free((void *)d->cmaps);
        free((void *)d);
    }
    free(f);
}

void *lvglab9_image_new(uint32_t cf, uint32_t w, uint32_t h, uint32_t stride, const uint8_t *data, uint32_t size)
{
    lv_image_dsc_t *d = calloc(1, sizeof(*d));
    if (!d) return NULL;
    d->header.magic = LV_IMAGE_HEADER_MAGIC;
    d->header.cf = cf;
    d->header.w = w;
    d->header.h = h;
    d->header.stride = stride;
    d->data_size = size;
    d->data = data;
    return d;
}

void lvglab9_image_free(void *image)
{
    free(image);
}
