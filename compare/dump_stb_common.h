/* What the two stb comparisons share (dump_stb_image_write.c and dump_stb_image.c): the made-up
   numbers, the running hash, made-up data and made-up pictures. examples/dump.rs has the same
   in Rust (tests/made_up/mod.rs). */
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

/* The two programs make the same data from the same seed: a 64-bit linear congruential
   generator, the high half of its state. */
static uint64_t g_state;
static uint32_t Next(void)
{
    g_state = g_state * 6364136223846793005ull + 1442695040888963407ull;
    return (uint32_t)(g_state >> 32);
}

/* FNV-1a, 64 bits. */
static uint64_t g_hash = 14695981039346656037ull;
static void Add(const void *data, size_t size)
{
    const unsigned char *bytes = (const unsigned char *)data;
    size_t i;
    for (i = 0; i < size; ++i)
    {
        g_hash ^= bytes[i];
        g_hash *= 1099511628211ull;
    }
}
static void AddNumber(uint32_t value)
{
    unsigned char bytes[4];
    bytes[0] = (unsigned char)value;
    bytes[1] = (unsigned char)(value >> 8);
    bytes[2] = (unsigned char)(value >> 16);
    bytes[3] = (unsigned char)(value >> 24);
    Add(bytes, 4);
}

static const char *const kWords[16] = { "the", "texture", "block", "of", "and", "compress", "a", "level", "mip", "to", "pixels", "in", "format", "is", "cube", "data" };

/* Made-up data: its length, then its bytes, of one of eight kinds (as dump_miniz.c has it). */
static size_t MakeData(uint8_t *buf)
{
    size_t len = 0, at = 0, i;
    uint32_t kind;
    switch (Next() % 8)
    {
        case 0: len = Next() % 16; break;
        case 1: len = Next() % 300; break;
        case 2:
        case 3: len = Next() % 5000; break;
        case 4:
        case 5: len = Next() % 40000; break;
        case 6: len = 60000 + Next() % 20000; break;
        default: len = Next() % 200000; break;
    }
    kind = Next() % 8;
    switch (kind)
    {
        case 0: /* noise */
            for (i = 0; i < len; ++i)
                buf[i] = (uint8_t)(Next() >> 13);
            break;
        case 1: /* four letters */
            for (i = 0; i < len; ++i)
                buf[i] = (uint8_t)('a' + Next() % 4);
            break;
        case 2: /* words */
            while (at < len)
            {
                const char *word = kWords[Next() % 16];
                for (i = 0; word[i] && at < len; ++i)
                    buf[at++] = (uint8_t)word[i];
                if (at < len)
                    buf[at++] = ' ';
            }
            break;
        case 3: /* runs */
            while (at < len)
            {
                uint8_t byte = (uint8_t)Next();
                uint32_t run = 1 + Next() % 300;
                for (i = 0; i < run && at < len; ++i)
                    buf[at++] = byte;
            }
            break;
        case 4: /* copies of what came before */
            while (at < len)
            {
                if (at < 64 || Next() % 4 == 0)
                    buf[at++] = (uint8_t)(Next() >> 9);
                else
                {
                    size_t reach = at < 40000 ? at : 40000;
                    size_t dist = 1 + Next() % reach;
                    uint32_t n = 3 + Next() % 260;
                    for (i = 0; i < n && at < len; ++i, ++at)
                        buf[at] = buf[at - dist];
                }
            }
            break;
        case 5: /* nothing */
            memset(buf, 0, len);
            break;
        case 6: /* a slope */
            for (i = 0; i < len; ++i)
                buf[i] = (uint8_t)(i / 7 + i % 13);
            break;
        default: /* stretches of noise, of one value and of counting */
            while (at < len)
            {
                uint32_t stretch = 1 + Next() % 2000, what = Next() % 3, value = Next();
                for (i = 0; i < stretch && at < len; ++i, ++at)
                    buf[at] = (uint8_t)(what == 0 ? Next() >> 11 : what == 1 ? value : value + i);
            }
            break;
    }
    return len;
}

/* A made-up image of 8 bits a channel: its channels, its size, how far its rows are apart,
   and its bytes, of one of seven kinds. What lies between the rows is 0xCD. */
typedef struct
{
    int w, h, comp;
    size_t stride;
    uint8_t *bytes;
} Picture;

static uint32_t Wave(uint32_t t)
{
    const int v = (int)(t & 511) - 256;
    const int up = v < 0 ? -v : v;
    return (uint32_t)(up > 255 ? 255 : up);
}

static Picture MakePicture(void)
{
    Picture p;
    uint32_t kind, coefficient[4][3], palette[16][4], cell, colours, noise;
    int x, y, c;
    p.comp = 1 + (int)(Next() % 4);
    switch (Next() % 6)
    {
        case 0:
            p.w = 1 + (int)(Next() % 4);
            p.h = 1 + (int)(Next() % 4);
            break;
        case 1:
        case 2:
            p.w = 1 + (int)(Next() % 40);
            p.h = 1 + (int)(Next() % 40);
            break;
        case 3:
        case 4:
            p.w = 1 + (int)(Next() % 100);
            p.h = 1 + (int)(Next() % 100);
            break;
        default:
            p.w = 1 + (int)(Next() % 300);
            p.h = 1 + (int)(Next() % 200);
            break;
    }
    p.stride = (size_t)(p.w * p.comp) + (Next() % 3 == 0 ? Next() % 7 : 0);
    p.bytes = (uint8_t *)malloc(p.stride * (size_t)p.h);
    memset(p.bytes, 0xCD, p.stride * (size_t)p.h);
    kind = Next() % 7;
    for (c = 0; c < 4; ++c)
    {
        coefficient[c][0] = Next() % 64;
        coefficient[c][1] = Next() % 64;
        coefficient[c][2] = Next() % 4096;
    }
    for (y = 0; y < 16; ++y)
        for (c = 0; c < 4; ++c)
            palette[y][c] = Next() & 255;
    cell = 1 + Next() % 12;
    colours = 2 + Next() % 4;
    noise = 1 + Next() % 32;
    for (y = 0; y < p.h; ++y)
    {
        for (x = 0; x < p.w; ++x)
        {
            uint8_t *at = p.bytes + (size_t)y * p.stride + (size_t)(x * p.comp);
            const uint32_t ux = (uint32_t)x, uy = (uint32_t)y;
            const uint32_t pick = kind == 5 ? Next() % colours : ((ux / cell) * 7 + (uy / cell) * 13 + (ux / cell) * (uy / cell)) % 16;
            for (c = 0; c < p.comp; ++c)
            {
                const uint32_t slope = (ux * coefficient[c][0] + uy * coefficient[c][1] + coefficient[c][2]) >> 4;
                uint32_t value;
                switch (kind)
                {
                    case 0: value = Next() >> 13; break;               /* noise */
                    case 1: value = palette[0][c]; break;              /* one colour */
                    case 2: value = slope; break;                      /* a slope */
                    case 3: value = palette[pick][c]; break;           /* squares */
                    case 4: value = slope + Next() % noise; break;     /* a slope with noise on it */
                    case 5: value = palette[pick][c]; break;           /* a few colours, anywhere */
                    default: value = Wave(ux * coefficient[c][0] + uy * coefficient[c][1] + coefficient[c][2]); break; /* waves */
                }
                at[c] = (uint8_t)value;
            }
        }
    }
    return p;
}

typedef struct
{
    uint8_t *bytes;
    size_t size, room;
} Out;

static void Put(void *context, void *data, int size)
{
    Out *out = (Out *)context;
    if (out->size + (size_t)size > out->room)
    {
        out->room = (out->size + (size_t)size) * 2 + 1024;
        out->bytes = (uint8_t *)realloc(out->bytes, out->room);
    }
    memcpy(out->bytes + out->size, data, (size_t)size);
    out->size += (size_t)size;
}
