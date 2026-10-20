/* Generated: wrappers of LVGL 9 functions with lv_color_t in their signature (see colors9.py). */
#include "lvgl.h"
#include <stddef.h>

static void lvglab9_c_lv_canvas_fill_bg(lv_obj_t * obj, uint32_t color, lv_opa_t opa)
{
    lv_canvas_fill_bg(obj, lv_color_hex(color), opa);
}

static void lvglab9_c_lv_canvas_set_px(lv_obj_t * obj, int32_t x, int32_t y, uint32_t color, lv_opa_t opa)
{
    lv_canvas_set_px(obj, x, y, lv_color_hex(color), opa);
}

static lv_chart_cursor_t  * lvglab9_c_lv_chart_add_cursor(lv_obj_t * obj, uint32_t color, lv_dir_t dir)
{
    return lv_chart_add_cursor(obj, lv_color_hex(color), dir);
}

static lv_chart_series_t * lvglab9_c_lv_chart_add_series(lv_obj_t * obj, uint32_t color, lv_chart_axis_t axis)
{
    return lv_chart_add_series(obj, lv_color_hex(color), axis);
}

static uint32_t lvglab9_c_lv_chart_get_series_color(lv_obj_t * chart, const lv_chart_series_t * series)
{
    return lv_color_to_u32(lv_chart_get_series_color(chart, series)) & 0xFFFFFF;
}

static void lvglab9_c_lv_chart_set_series_color(lv_obj_t * chart, lv_chart_series_t * series, uint32_t color)
{
    lv_chart_set_series_color(chart, series, lv_color_hex(color));
}

static uint32_t lvglab9_c_lv_color16_to_color(lv_color16_t c)
{
    return lv_color_to_u32(lv_color16_to_color(c)) & 0xFFFFFF;
}

static uint32_t lvglab9_c_lv_color_black(void)
{
    return lv_color_to_u32(lv_color_black()) & 0xFFFFFF;
}

static uint8_t lvglab9_c_lv_color_brightness(uint32_t c)
{
    return lv_color_brightness(lv_color_hex(c));
}

static uint32_t lvglab9_c_lv_color_darken(uint32_t c, lv_opa_t lvl)
{
    return lv_color_to_u32(lv_color_darken(lv_color_hex(c), lvl)) & 0xFFFFFF;
}

static bool lvglab9_c_lv_color_eq(uint32_t c1, uint32_t c2)
{
    return lv_color_eq(lv_color_hex(c1), lv_color_hex(c2));
}

static uint32_t lvglab9_c_lv_color_hex(uint32_t c)
{
    return lv_color_to_u32(lv_color_hex(c)) & 0xFFFFFF;
}

static uint32_t lvglab9_c_lv_color_hex3(uint32_t c)
{
    return lv_color_to_u32(lv_color_hex3(c)) & 0xFFFFFF;
}

static uint32_t lvglab9_c_lv_color_hsv_to_rgb(uint16_t h, uint8_t s, uint8_t v)
{
    return lv_color_to_u32(lv_color_hsv_to_rgb(h, s, v)) & 0xFFFFFF;
}

static bool lvglab9_c_lv_color_is_in_range(uint32_t color, uint32_t l_color, uint32_t h_color)
{
    return lv_color_is_in_range(lv_color_hex(color), lv_color_hex(l_color), lv_color_hex(h_color));
}

static uint32_t lvglab9_c_lv_color_lighten(uint32_t c, lv_opa_t lvl)
{
    return lv_color_to_u32(lv_color_lighten(lv_color_hex(c), lvl)) & 0xFFFFFF;
}

static uint8_t lvglab9_c_lv_color_luminance(uint32_t c)
{
    return lv_color_luminance(lv_color_hex(c));
}

static uint32_t lvglab9_c_lv_color_make(uint8_t r, uint8_t g, uint8_t b)
{
    return lv_color_to_u32(lv_color_make(r, g, b)) & 0xFFFFFF;
}

static uint32_t lvglab9_c_lv_color_mix(uint32_t c1, uint32_t c2, uint8_t mix)
{
    return lv_color_to_u32(lv_color_mix(lv_color_hex(c1), lv_color_hex(c2), mix)) & 0xFFFFFF;
}

static lv_color32_t lvglab9_c_lv_color_to_32(uint32_t color, lv_opa_t opa)
{
    return lv_color_to_32(lv_color_hex(color), opa);
}

static lv_color_hsv_t lvglab9_c_lv_color_to_hsv(uint32_t color)
{
    return lv_color_to_hsv(lv_color_hex(color));
}

static uint32_t lvglab9_c_lv_color_to_int(uint32_t c)
{
    return lv_color_to_int(lv_color_hex(c));
}

static uint16_t lvglab9_c_lv_color_to_u16(uint32_t color)
{
    return lv_color_to_u16(lv_color_hex(color));
}

static uint32_t lvglab9_c_lv_color_to_u32(uint32_t color)
{
    return lv_color_to_u32(lv_color_hex(color));
}

static uint32_t lvglab9_c_lv_color_white(void)
{
    return lv_color_to_u32(lv_color_white()) & 0xFFFFFF;
}

static uint32_t lvglab9_c_lv_led_get_color(const lv_obj_t * obj)
{
    return lv_color_to_u32(lv_led_get_color(obj)) & 0xFFFFFF;
}

static void lvglab9_c_lv_led_set_color(lv_obj_t * obj, uint32_t color)
{
    lv_led_set_color(obj, lv_color_hex(color));
}

static uint32_t lvglab9_c_lv_obj_get_style_arc_color(const lv_obj_t * obj, lv_part_t part)
{
    return lv_color_to_u32(lv_obj_get_style_arc_color(obj, part)) & 0xFFFFFF;
}

static uint32_t lvglab9_c_lv_obj_get_style_arc_color_filtered(const lv_obj_t * obj, lv_part_t part)
{
    return lv_color_to_u32(lv_obj_get_style_arc_color_filtered(obj, part)) & 0xFFFFFF;
}

static uint32_t lvglab9_c_lv_obj_get_style_bg_color(const lv_obj_t * obj, lv_part_t part)
{
    return lv_color_to_u32(lv_obj_get_style_bg_color(obj, part)) & 0xFFFFFF;
}

static uint32_t lvglab9_c_lv_obj_get_style_bg_color_filtered(const lv_obj_t * obj, lv_part_t part)
{
    return lv_color_to_u32(lv_obj_get_style_bg_color_filtered(obj, part)) & 0xFFFFFF;
}

static uint32_t lvglab9_c_lv_obj_get_style_bg_grad_color(const lv_obj_t * obj, lv_part_t part)
{
    return lv_color_to_u32(lv_obj_get_style_bg_grad_color(obj, part)) & 0xFFFFFF;
}

static uint32_t lvglab9_c_lv_obj_get_style_bg_grad_color_filtered(const lv_obj_t * obj, lv_part_t part)
{
    return lv_color_to_u32(lv_obj_get_style_bg_grad_color_filtered(obj, part)) & 0xFFFFFF;
}

static uint32_t lvglab9_c_lv_obj_get_style_bg_image_recolor(const lv_obj_t * obj, lv_part_t part)
{
    return lv_color_to_u32(lv_obj_get_style_bg_image_recolor(obj, part)) & 0xFFFFFF;
}

static uint32_t lvglab9_c_lv_obj_get_style_bg_image_recolor_filtered(const lv_obj_t * obj, lv_part_t part)
{
    return lv_color_to_u32(lv_obj_get_style_bg_image_recolor_filtered(obj, part)) & 0xFFFFFF;
}

static uint32_t lvglab9_c_lv_obj_get_style_border_color(const lv_obj_t * obj, lv_part_t part)
{
    return lv_color_to_u32(lv_obj_get_style_border_color(obj, part)) & 0xFFFFFF;
}

static uint32_t lvglab9_c_lv_obj_get_style_border_color_filtered(const lv_obj_t * obj, lv_part_t part)
{
    return lv_color_to_u32(lv_obj_get_style_border_color_filtered(obj, part)) & 0xFFFFFF;
}

static uint32_t lvglab9_c_lv_obj_get_style_drop_shadow_color(const lv_obj_t * obj, lv_part_t part)
{
    return lv_color_to_u32(lv_obj_get_style_drop_shadow_color(obj, part)) & 0xFFFFFF;
}

static uint32_t lvglab9_c_lv_obj_get_style_drop_shadow_color_filtered(const lv_obj_t * obj, lv_part_t part)
{
    return lv_color_to_u32(lv_obj_get_style_drop_shadow_color_filtered(obj, part)) & 0xFFFFFF;
}

static uint32_t lvglab9_c_lv_obj_get_style_image_recolor(const lv_obj_t * obj, lv_part_t part)
{
    return lv_color_to_u32(lv_obj_get_style_image_recolor(obj, part)) & 0xFFFFFF;
}

static uint32_t lvglab9_c_lv_obj_get_style_image_recolor_filtered(const lv_obj_t * obj, lv_part_t part)
{
    return lv_color_to_u32(lv_obj_get_style_image_recolor_filtered(obj, part)) & 0xFFFFFF;
}

static uint32_t lvglab9_c_lv_obj_get_style_line_color(const lv_obj_t * obj, lv_part_t part)
{
    return lv_color_to_u32(lv_obj_get_style_line_color(obj, part)) & 0xFFFFFF;
}

static uint32_t lvglab9_c_lv_obj_get_style_line_color_filtered(const lv_obj_t * obj, lv_part_t part)
{
    return lv_color_to_u32(lv_obj_get_style_line_color_filtered(obj, part)) & 0xFFFFFF;
}

static uint32_t lvglab9_c_lv_obj_get_style_outline_color(const lv_obj_t * obj, lv_part_t part)
{
    return lv_color_to_u32(lv_obj_get_style_outline_color(obj, part)) & 0xFFFFFF;
}

static uint32_t lvglab9_c_lv_obj_get_style_outline_color_filtered(const lv_obj_t * obj, lv_part_t part)
{
    return lv_color_to_u32(lv_obj_get_style_outline_color_filtered(obj, part)) & 0xFFFFFF;
}

static uint32_t lvglab9_c_lv_obj_get_style_recolor(const lv_obj_t * obj, lv_part_t part)
{
    return lv_color_to_u32(lv_obj_get_style_recolor(obj, part)) & 0xFFFFFF;
}

static uint32_t lvglab9_c_lv_obj_get_style_shadow_color(const lv_obj_t * obj, lv_part_t part)
{
    return lv_color_to_u32(lv_obj_get_style_shadow_color(obj, part)) & 0xFFFFFF;
}

static uint32_t lvglab9_c_lv_obj_get_style_shadow_color_filtered(const lv_obj_t * obj, lv_part_t part)
{
    return lv_color_to_u32(lv_obj_get_style_shadow_color_filtered(obj, part)) & 0xFFFFFF;
}

static uint32_t lvglab9_c_lv_obj_get_style_text_color(const lv_obj_t * obj, lv_part_t part)
{
    return lv_color_to_u32(lv_obj_get_style_text_color(obj, part)) & 0xFFFFFF;
}

static uint32_t lvglab9_c_lv_obj_get_style_text_color_filtered(const lv_obj_t * obj, lv_part_t part)
{
    return lv_color_to_u32(lv_obj_get_style_text_color_filtered(obj, part)) & 0xFFFFFF;
}

static uint32_t lvglab9_c_lv_obj_get_style_text_outline_stroke_color(const lv_obj_t * obj, lv_part_t part)
{
    return lv_color_to_u32(lv_obj_get_style_text_outline_stroke_color(obj, part)) & 0xFFFFFF;
}

static uint32_t lvglab9_c_lv_obj_get_style_text_outline_stroke_color_filtered(const lv_obj_t * obj, lv_part_t part)
{
    return lv_color_to_u32(lv_obj_get_style_text_outline_stroke_color_filtered(obj, part)) & 0xFFFFFF;
}

static void lvglab9_c_lv_obj_set_style_arc_color(lv_obj_t * obj, uint32_t value, lv_style_selector_t selector)
{
    lv_obj_set_style_arc_color(obj, lv_color_hex(value), selector);
}

static void lvglab9_c_lv_obj_set_style_bg_color(lv_obj_t * obj, uint32_t value, lv_style_selector_t selector)
{
    lv_obj_set_style_bg_color(obj, lv_color_hex(value), selector);
}

static void lvglab9_c_lv_obj_set_style_bg_grad_color(lv_obj_t * obj, uint32_t value, lv_style_selector_t selector)
{
    lv_obj_set_style_bg_grad_color(obj, lv_color_hex(value), selector);
}

static void lvglab9_c_lv_obj_set_style_bg_image_recolor(lv_obj_t * obj, uint32_t value, lv_style_selector_t selector)
{
    lv_obj_set_style_bg_image_recolor(obj, lv_color_hex(value), selector);
}

static void lvglab9_c_lv_obj_set_style_border_color(lv_obj_t * obj, uint32_t value, lv_style_selector_t selector)
{
    lv_obj_set_style_border_color(obj, lv_color_hex(value), selector);
}

static void lvglab9_c_lv_obj_set_style_drop_shadow_color(lv_obj_t * obj, uint32_t value, lv_style_selector_t selector)
{
    lv_obj_set_style_drop_shadow_color(obj, lv_color_hex(value), selector);
}

static void lvglab9_c_lv_obj_set_style_image_recolor(lv_obj_t * obj, uint32_t value, lv_style_selector_t selector)
{
    lv_obj_set_style_image_recolor(obj, lv_color_hex(value), selector);
}

static void lvglab9_c_lv_obj_set_style_line_color(lv_obj_t * obj, uint32_t value, lv_style_selector_t selector)
{
    lv_obj_set_style_line_color(obj, lv_color_hex(value), selector);
}

static void lvglab9_c_lv_obj_set_style_outline_color(lv_obj_t * obj, uint32_t value, lv_style_selector_t selector)
{
    lv_obj_set_style_outline_color(obj, lv_color_hex(value), selector);
}

static void lvglab9_c_lv_obj_set_style_recolor(lv_obj_t * obj, uint32_t value, lv_style_selector_t selector)
{
    lv_obj_set_style_recolor(obj, lv_color_hex(value), selector);
}

static void lvglab9_c_lv_obj_set_style_shadow_color(lv_obj_t * obj, uint32_t value, lv_style_selector_t selector)
{
    lv_obj_set_style_shadow_color(obj, lv_color_hex(value), selector);
}

static void lvglab9_c_lv_obj_set_style_text_color(lv_obj_t * obj, uint32_t value, lv_style_selector_t selector)
{
    lv_obj_set_style_text_color(obj, lv_color_hex(value), selector);
}

static void lvglab9_c_lv_obj_set_style_text_outline_stroke_color(lv_obj_t * obj, uint32_t value, lv_style_selector_t selector)
{
    lv_obj_set_style_text_outline_stroke_color(obj, lv_color_hex(value), selector);
}

static uint32_t lvglab9_c_lv_palette_darken(lv_palette_t p, uint8_t lvl)
{
    return lv_color_to_u32(lv_palette_darken(p, lvl)) & 0xFFFFFF;
}

static uint32_t lvglab9_c_lv_palette_lighten(lv_palette_t p, uint8_t lvl)
{
    return lv_color_to_u32(lv_palette_lighten(p, lvl)) & 0xFFFFFF;
}

static uint32_t lvglab9_c_lv_palette_main(lv_palette_t p)
{
    return lv_color_to_u32(lv_palette_main(p)) & 0xFFFFFF;
}

static void lvglab9_c_lv_style_set_arc_color(lv_style_t * style, uint32_t value)
{
    lv_style_set_arc_color(style, lv_color_hex(value));
}

static void lvglab9_c_lv_style_set_bg_color(lv_style_t * style, uint32_t value)
{
    lv_style_set_bg_color(style, lv_color_hex(value));
}

static void lvglab9_c_lv_style_set_bg_grad_color(lv_style_t * style, uint32_t value)
{
    lv_style_set_bg_grad_color(style, lv_color_hex(value));
}

static void lvglab9_c_lv_style_set_bg_image_recolor(lv_style_t * style, uint32_t value)
{
    lv_style_set_bg_image_recolor(style, lv_color_hex(value));
}

static void lvglab9_c_lv_style_set_border_color(lv_style_t * style, uint32_t value)
{
    lv_style_set_border_color(style, lv_color_hex(value));
}

static void lvglab9_c_lv_style_set_drop_shadow_color(lv_style_t * style, uint32_t value)
{
    lv_style_set_drop_shadow_color(style, lv_color_hex(value));
}

static void lvglab9_c_lv_style_set_image_recolor(lv_style_t * style, uint32_t value)
{
    lv_style_set_image_recolor(style, lv_color_hex(value));
}

static void lvglab9_c_lv_style_set_line_color(lv_style_t * style, uint32_t value)
{
    lv_style_set_line_color(style, lv_color_hex(value));
}

static void lvglab9_c_lv_style_set_outline_color(lv_style_t * style, uint32_t value)
{
    lv_style_set_outline_color(style, lv_color_hex(value));
}

static void lvglab9_c_lv_style_set_recolor(lv_style_t * style, uint32_t value)
{
    lv_style_set_recolor(style, lv_color_hex(value));
}

static void lvglab9_c_lv_style_set_shadow_color(lv_style_t * style, uint32_t value)
{
    lv_style_set_shadow_color(style, lv_color_hex(value));
}

static void lvglab9_c_lv_style_set_text_color(lv_style_t * style, uint32_t value)
{
    lv_style_set_text_color(style, lv_color_hex(value));
}

static void lvglab9_c_lv_style_set_text_outline_stroke_color(lv_style_t * style, uint32_t value)
{
    lv_style_set_text_outline_stroke_color(style, lv_color_hex(value));
}

static uint32_t lvglab9_c_lv_subject_get_color(lv_subject_t * subject)
{
    return lv_color_to_u32(lv_subject_get_color(subject)) & 0xFFFFFF;
}

static uint32_t lvglab9_c_lv_subject_get_previous_color(lv_subject_t * subject)
{
    return lv_color_to_u32(lv_subject_get_previous_color(subject)) & 0xFFFFFF;
}

static void lvglab9_c_lv_subject_init_color(lv_subject_t * subject, uint32_t color)
{
    lv_subject_init_color(subject, lv_color_hex(color));
}

static void lvglab9_c_lv_subject_set_color(lv_subject_t * subject, uint32_t color)
{
    lv_subject_set_color(subject, lv_color_hex(color));
}

static lv_theme_t * lvglab9_c_lv_theme_default_init(lv_display_t * disp, uint32_t color_primary, uint32_t color_secondary, bool dark, const lv_font_t * font)
{
    return lv_theme_default_init(disp, lv_color_hex(color_primary), lv_color_hex(color_secondary), dark, font);
}

static uint32_t lvglab9_c_lv_theme_get_color_primary(lv_obj_t * obj)
{
    return lv_color_to_u32(lv_theme_get_color_primary(obj)) & 0xFFFFFF;
}

static uint32_t lvglab9_c_lv_theme_get_color_secondary(lv_obj_t * obj)
{
    return lv_color_to_u32(lv_theme_get_color_secondary(obj)) & 0xFFFFFF;
}

typedef struct { const char *name; void *address; } lvglab9_wrapper;
const lvglab9_wrapper lvglab9_color_wrappers[] = {
    {"lv_canvas_fill_bg", (void *)lvglab9_c_lv_canvas_fill_bg},
    {"lv_canvas_set_px", (void *)lvglab9_c_lv_canvas_set_px},
    {"lv_chart_add_cursor", (void *)lvglab9_c_lv_chart_add_cursor},
    {"lv_chart_add_series", (void *)lvglab9_c_lv_chart_add_series},
    {"lv_chart_get_series_color", (void *)lvglab9_c_lv_chart_get_series_color},
    {"lv_chart_set_series_color", (void *)lvglab9_c_lv_chart_set_series_color},
    {"lv_color16_to_color", (void *)lvglab9_c_lv_color16_to_color},
    {"lv_color_black", (void *)lvglab9_c_lv_color_black},
    {"lv_color_brightness", (void *)lvglab9_c_lv_color_brightness},
    {"lv_color_darken", (void *)lvglab9_c_lv_color_darken},
    {"lv_color_eq", (void *)lvglab9_c_lv_color_eq},
    {"lv_color_hex", (void *)lvglab9_c_lv_color_hex},
    {"lv_color_hex3", (void *)lvglab9_c_lv_color_hex3},
    {"lv_color_hsv_to_rgb", (void *)lvglab9_c_lv_color_hsv_to_rgb},
    {"lv_color_is_in_range", (void *)lvglab9_c_lv_color_is_in_range},
    {"lv_color_lighten", (void *)lvglab9_c_lv_color_lighten},
    {"lv_color_luminance", (void *)lvglab9_c_lv_color_luminance},
    {"lv_color_make", (void *)lvglab9_c_lv_color_make},
    {"lv_color_mix", (void *)lvglab9_c_lv_color_mix},
    {"lv_color_to_32", (void *)lvglab9_c_lv_color_to_32},
    {"lv_color_to_hsv", (void *)lvglab9_c_lv_color_to_hsv},
    {"lv_color_to_int", (void *)lvglab9_c_lv_color_to_int},
    {"lv_color_to_u16", (void *)lvglab9_c_lv_color_to_u16},
    {"lv_color_to_u32", (void *)lvglab9_c_lv_color_to_u32},
    {"lv_color_white", (void *)lvglab9_c_lv_color_white},
    {"lv_led_get_color", (void *)lvglab9_c_lv_led_get_color},
    {"lv_led_set_color", (void *)lvglab9_c_lv_led_set_color},
    {"lv_obj_get_style_arc_color", (void *)lvglab9_c_lv_obj_get_style_arc_color},
    {"lv_obj_get_style_arc_color_filtered", (void *)lvglab9_c_lv_obj_get_style_arc_color_filtered},
    {"lv_obj_get_style_bg_color", (void *)lvglab9_c_lv_obj_get_style_bg_color},
    {"lv_obj_get_style_bg_color_filtered", (void *)lvglab9_c_lv_obj_get_style_bg_color_filtered},
    {"lv_obj_get_style_bg_grad_color", (void *)lvglab9_c_lv_obj_get_style_bg_grad_color},
    {"lv_obj_get_style_bg_grad_color_filtered", (void *)lvglab9_c_lv_obj_get_style_bg_grad_color_filtered},
    {"lv_obj_get_style_bg_image_recolor", (void *)lvglab9_c_lv_obj_get_style_bg_image_recolor},
    {"lv_obj_get_style_bg_image_recolor_filtered", (void *)lvglab9_c_lv_obj_get_style_bg_image_recolor_filtered},
    {"lv_obj_get_style_border_color", (void *)lvglab9_c_lv_obj_get_style_border_color},
    {"lv_obj_get_style_border_color_filtered", (void *)lvglab9_c_lv_obj_get_style_border_color_filtered},
    {"lv_obj_get_style_drop_shadow_color", (void *)lvglab9_c_lv_obj_get_style_drop_shadow_color},
    {"lv_obj_get_style_drop_shadow_color_filtered", (void *)lvglab9_c_lv_obj_get_style_drop_shadow_color_filtered},
    {"lv_obj_get_style_image_recolor", (void *)lvglab9_c_lv_obj_get_style_image_recolor},
    {"lv_obj_get_style_image_recolor_filtered", (void *)lvglab9_c_lv_obj_get_style_image_recolor_filtered},
    {"lv_obj_get_style_line_color", (void *)lvglab9_c_lv_obj_get_style_line_color},
    {"lv_obj_get_style_line_color_filtered", (void *)lvglab9_c_lv_obj_get_style_line_color_filtered},
    {"lv_obj_get_style_outline_color", (void *)lvglab9_c_lv_obj_get_style_outline_color},
    {"lv_obj_get_style_outline_color_filtered", (void *)lvglab9_c_lv_obj_get_style_outline_color_filtered},
    {"lv_obj_get_style_recolor", (void *)lvglab9_c_lv_obj_get_style_recolor},
    {"lv_obj_get_style_shadow_color", (void *)lvglab9_c_lv_obj_get_style_shadow_color},
    {"lv_obj_get_style_shadow_color_filtered", (void *)lvglab9_c_lv_obj_get_style_shadow_color_filtered},
    {"lv_obj_get_style_text_color", (void *)lvglab9_c_lv_obj_get_style_text_color},
    {"lv_obj_get_style_text_color_filtered", (void *)lvglab9_c_lv_obj_get_style_text_color_filtered},
    {"lv_obj_get_style_text_outline_stroke_color", (void *)lvglab9_c_lv_obj_get_style_text_outline_stroke_color},
    {"lv_obj_get_style_text_outline_stroke_color_filtered", (void *)lvglab9_c_lv_obj_get_style_text_outline_stroke_color_filtered},
    {"lv_obj_set_style_arc_color", (void *)lvglab9_c_lv_obj_set_style_arc_color},
    {"lv_obj_set_style_bg_color", (void *)lvglab9_c_lv_obj_set_style_bg_color},
    {"lv_obj_set_style_bg_grad_color", (void *)lvglab9_c_lv_obj_set_style_bg_grad_color},
    {"lv_obj_set_style_bg_image_recolor", (void *)lvglab9_c_lv_obj_set_style_bg_image_recolor},
    {"lv_obj_set_style_border_color", (void *)lvglab9_c_lv_obj_set_style_border_color},
    {"lv_obj_set_style_drop_shadow_color", (void *)lvglab9_c_lv_obj_set_style_drop_shadow_color},
    {"lv_obj_set_style_image_recolor", (void *)lvglab9_c_lv_obj_set_style_image_recolor},
    {"lv_obj_set_style_line_color", (void *)lvglab9_c_lv_obj_set_style_line_color},
    {"lv_obj_set_style_outline_color", (void *)lvglab9_c_lv_obj_set_style_outline_color},
    {"lv_obj_set_style_recolor", (void *)lvglab9_c_lv_obj_set_style_recolor},
    {"lv_obj_set_style_shadow_color", (void *)lvglab9_c_lv_obj_set_style_shadow_color},
    {"lv_obj_set_style_text_color", (void *)lvglab9_c_lv_obj_set_style_text_color},
    {"lv_obj_set_style_text_outline_stroke_color", (void *)lvglab9_c_lv_obj_set_style_text_outline_stroke_color},
    {"lv_palette_darken", (void *)lvglab9_c_lv_palette_darken},
    {"lv_palette_lighten", (void *)lvglab9_c_lv_palette_lighten},
    {"lv_palette_main", (void *)lvglab9_c_lv_palette_main},
    {"lv_style_set_arc_color", (void *)lvglab9_c_lv_style_set_arc_color},
    {"lv_style_set_bg_color", (void *)lvglab9_c_lv_style_set_bg_color},
    {"lv_style_set_bg_grad_color", (void *)lvglab9_c_lv_style_set_bg_grad_color},
    {"lv_style_set_bg_image_recolor", (void *)lvglab9_c_lv_style_set_bg_image_recolor},
    {"lv_style_set_border_color", (void *)lvglab9_c_lv_style_set_border_color},
    {"lv_style_set_drop_shadow_color", (void *)lvglab9_c_lv_style_set_drop_shadow_color},
    {"lv_style_set_image_recolor", (void *)lvglab9_c_lv_style_set_image_recolor},
    {"lv_style_set_line_color", (void *)lvglab9_c_lv_style_set_line_color},
    {"lv_style_set_outline_color", (void *)lvglab9_c_lv_style_set_outline_color},
    {"lv_style_set_recolor", (void *)lvglab9_c_lv_style_set_recolor},
    {"lv_style_set_shadow_color", (void *)lvglab9_c_lv_style_set_shadow_color},
    {"lv_style_set_text_color", (void *)lvglab9_c_lv_style_set_text_color},
    {"lv_style_set_text_outline_stroke_color", (void *)lvglab9_c_lv_style_set_text_outline_stroke_color},
    {"lv_subject_get_color", (void *)lvglab9_c_lv_subject_get_color},
    {"lv_subject_get_previous_color", (void *)lvglab9_c_lv_subject_get_previous_color},
    {"lv_subject_init_color", (void *)lvglab9_c_lv_subject_init_color},
    {"lv_subject_set_color", (void *)lvglab9_c_lv_subject_set_color},
    {"lv_theme_default_init", (void *)lvglab9_c_lv_theme_default_init},
    {"lv_theme_get_color_primary", (void *)lvglab9_c_lv_theme_get_color_primary},
    {"lv_theme_get_color_secondary", (void *)lvglab9_c_lv_theme_get_color_secondary},
    {NULL, NULL}
};
