/* The C half of the comparison with stb_image_write.h (https://github.com/nothings/stb): writes
   made-up images as .png, .bmp and .jpg files in memory and prints a running hash of the
   files. examples/dump.rs prints the same from the Rust port, and compare/compare.sh compares
   the two.

     dump stb_image_write <seed> <count>   made-up images of one to four channels, of seven
                                           kinds and many sizes, some with rows further apart
                                           than they are wide, written as .png (with made-up
                                           compression levels and filters), as .bmp and as
                                           .jpg (with made-up qualities), some upside down;
                                           and for every fourth, made-up data through the
                                           file's own zlib compressor */
#define STB_IMAGE_WRITE_IMPLEMENTATION
#include "stb_image_write.h"

#include "dump_stb_common.h"

int main(int argc, char **argv)
{
    uint32_t count, i;
    uint8_t *data = (uint8_t *)malloc(200000 + 16);
    uint64_t png_total = 0, bmp_total = 0, jpg_total = 0, zlib_total = 0, pixel_total = 0;
    if (argc < 4 || strcmp(argv[1], "stb_image_write") != 0)
    {
        fprintf(stderr, "usage: dump stb_image_write <seed> <count>\n");
        return 2;
    }
    g_state = strtoull(argv[2], NULL, 10);
    count = (uint32_t)strtoul(argv[3], NULL, 10);
    for (i = 0; i < count; ++i)
    {
        Picture p = MakePicture();
        Out out = { NULL, 0, 0 };
        int size = 0, quality;
        unsigned char *png;
        pixel_total += (uint64_t)p.w * (uint64_t)p.h;

        /* The three settings. */
        stbi_write_png_compression_level = Next() % 4 == 0 ? (int)(Next() % 40) - 3 : 8;
        stbi_write_force_png_filter = Next() % 3 == 0 ? (int)(Next() % 8) - 1 : -1;
        stbi_flip_vertically_on_write(Next() % 4 == 0);

        png = stbi_write_png_to_mem(p.bytes, (ptrdiff_t)p.stride, p.w, p.h, p.comp, &size);
        AddNumber((uint32_t)size);
        Add(png, (size_t)size);
        png_total += (uint64_t)size;
        free(png);

        stbi_write_bmp_strided_to_func(Put, &out, p.w, p.h, p.comp, p.bytes, (ptrdiff_t)p.stride);
        AddNumber((uint32_t)out.size);
        Add(out.bytes, out.size);
        bmp_total += out.size;

        /* Nothing for the one it stands for, in range, near where the colour stops being
           halved, and out of range. */
        switch (Next() % 4)
        {
            case 0: quality = 0; break;
            case 1: quality = 1 + (int)(Next() % 100); break;
            case 2: quality = 85 + (int)(Next() % 16); break;
            default: quality = (int)(Next() % 130) - 10; break;
        }
        out.size = 0;
        stbi_write_jpg_strided_to_func(Put, &out, p.w, p.h, p.comp, p.bytes, (ptrdiff_t)p.stride, quality);
        AddNumber((uint32_t)out.size);
        Add(out.bytes, out.size);
        jpg_total += out.size;
        free(out.bytes);
        free(p.bytes);

        if (i % 4 == 0)
        {
            const size_t len = MakeData(data);
            const int level = Next() % 4 == 0 ? (int)(Next() % 50) : 8;
            unsigned char *packed = stbi_zlib_compress(data, (int)len, &size, level);
            AddNumber((uint32_t)size);
            Add(packed, (size_t)size);
            zlib_total += (uint64_t)size;
            free(packed);
        }

        if ((i + 1) % 200 == 0 || i + 1 == count)
            printf("stb_image_write %u %016llx\n", i + 1, (unsigned long long)g_hash);
    }
    printf("stb_image_write pixels: %llu; bytes of .png: %llu, .bmp: %llu, .jpg: %llu, zlib: %llu\n", (unsigned long long)pixel_total, (unsigned long long)png_total, (unsigned long long)bmp_total,
           (unsigned long long)jpg_total, (unsigned long long)zlib_total);
    free(data);
    return 0;
}
