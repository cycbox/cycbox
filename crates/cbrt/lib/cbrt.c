/*
 * cbrt.c — implementation of the CBRT (CycBox Real-Time) protocol encoder.
 *
 * See cbrt.h for the public API. Compile this file once and link it into
 * your project. Build options (pass via -D when compiling this file):
 *
 *     CBRT_CRC_TABLE       // use 512-byte LUT instead of bitwise loop
 *     CBRT_NO_FLOAT_API    // omit cbrt_append_f32 / cbrt_append_f64
 *
 * License: same as the parent cbrt-codec crate.
 */
#include "cbrt.h"

/* Bytes per fixed-width datatype. Returns 0 for bool-packed (0xE) and 0xF. */
static uint8_t bps_(uint8_t dt) {
    switch (dt) {
        case CBRT_DT_U8: case CBRT_DT_I8:                                return 1;
        case CBRT_DT_U16: case CBRT_DT_I16:
        case CBRT_DT_Q15: case CBRT_DT_BF16: case CBRT_DT_F16:           return 2;
        case CBRT_DT_U32: case CBRT_DT_I32:
        case CBRT_DT_F32: case CBRT_DT_Q31:                              return 4;
        case CBRT_DT_U64: case CBRT_DT_I64: case CBRT_DT_F64:            return 8;
        default:                                                         return 0;
    }
}

/* CRC-16/MODBUS: poly 0x8005, init 0xFFFF, input/output reflected, no final XOR.
 * Reflected form uses 0xA001 in the inner shift. Verified vector:
 *     crc16("123456789") == 0x4B37
 */
#ifdef CBRT_CRC_TABLE
static const uint16_t crc_tab_[256] = {
    0x0000,0xC0C1,0xC181,0x0140,0xC301,0x03C0,0x0280,0xC241,
    0xC601,0x06C0,0x0780,0xC741,0x0500,0xC5C1,0xC481,0x0440,
    0xCC01,0x0CC0,0x0D80,0xCD41,0x0F00,0xCFC1,0xCE81,0x0E40,
    0x0A00,0xCAC1,0xCB81,0x0B40,0xC901,0x09C0,0x0880,0xC841,
    0xD801,0x18C0,0x1980,0xD941,0x1B00,0xDBC1,0xDA81,0x1A40,
    0x1E00,0xDEC1,0xDF81,0x1F40,0xDD01,0x1DC0,0x1C80,0xDC41,
    0x1400,0xD4C1,0xD581,0x1540,0xD701,0x17C0,0x1680,0xD641,
    0xD201,0x12C0,0x1380,0xD341,0x1100,0xD1C1,0xD081,0x1040,
    0xF001,0x30C0,0x3180,0xF141,0x3300,0xF3C1,0xF281,0x3240,
    0x3600,0xF6C1,0xF781,0x3740,0xF501,0x35C0,0x3480,0xF441,
    0x3C00,0xFCC1,0xFD81,0x3D40,0xFF01,0x3FC0,0x3E80,0xFE41,
    0xFA01,0x3AC0,0x3B80,0xFB41,0x3900,0xF9C1,0xF881,0x3840,
    0x2800,0xE8C1,0xE981,0x2940,0xEB01,0x2BC0,0x2A80,0xEA41,
    0xEE01,0x2EC0,0x2F80,0xEF41,0x2D00,0xEDC1,0xEC81,0x2C40,
    0xE401,0x24C0,0x2580,0xE541,0x2700,0xE7C1,0xE681,0x2640,
    0x2200,0xE2C1,0xE381,0x2340,0xE101,0x21C0,0x2080,0xE041,
    0xA001,0x60C0,0x6180,0xA141,0x6300,0xA3C1,0xA281,0x6240,
    0x6600,0xA6C1,0xA781,0x6740,0xA501,0x65C0,0x6480,0xA441,
    0x6C00,0xACC1,0xAD81,0x6D40,0xAF01,0x6FC0,0x6E80,0xAE41,
    0xAA01,0x6AC0,0x6B80,0xAB41,0x6900,0xA9C1,0xA881,0x6840,
    0x7800,0xB8C1,0xB981,0x7940,0xBB01,0x7BC0,0x7A80,0xBA41,
    0xBE01,0x7EC0,0x7F80,0xBF41,0x7D00,0xBDC1,0xBC81,0x7C40,
    0xB401,0x74C0,0x7580,0xB541,0x7700,0xB7C1,0xB681,0x7640,
    0x7200,0xB2C1,0xB381,0x7340,0xB101,0x71C0,0x7080,0xB041,
    0x5000,0x90C1,0x9181,0x5140,0x9301,0x53C0,0x5280,0x9241,
    0x9601,0x56C0,0x5780,0x9741,0x5500,0x95C1,0x9481,0x5440,
    0x9C01,0x5CC0,0x5D80,0x9D41,0x5F00,0x9FC1,0x9E81,0x5E40,
    0x5A00,0x9AC1,0x9B81,0x5B40,0x9901,0x59C0,0x5880,0x9841,
    0x8801,0x48C0,0x4980,0x8941,0x4B00,0x8BC1,0x8A81,0x4A40,
    0x4E00,0x8EC1,0x8F81,0x4F40,0x8D01,0x4DC0,0x4C80,0x8C41,
    0x4400,0x84C1,0x8581,0x4540,0x8701,0x47C0,0x4680,0x8641,
    0x8201,0x42C0,0x4380,0x8341,0x4100,0x81C1,0x8081,0x4040
};
static uint16_t crc16_(const uint8_t *buf, size_t len) {
    uint16_t crc = 0xFFFF;
    while (len--) {
        crc = (uint16_t)((crc >> 8) ^ crc_tab_[(crc ^ *buf++) & 0xFF]);
    }
    return crc;
}
#else
static uint16_t crc16_(const uint8_t *buf, size_t len) {
    uint16_t crc = 0xFFFF;
    size_t i, b;
    for (i = 0; i < len; i++) {
        crc ^= (uint16_t)buf[i];
        for (b = 0; b < 8; b++) {
            if (crc & 1u) crc = (uint16_t)((crc >> 1) ^ 0xA001u);
            else          crc = (uint16_t)(crc >> 1);
        }
    }
    return crc;
}
#endif

static void put_u16_(uint8_t *p, uint16_t v) { *(uint16_t *)p = v; }
static void put_u32_(uint8_t *p, uint32_t v) { *(uint32_t *)p = v; }
static void put_u64_(uint8_t *p, uint64_t v) { *(uint64_t *)p = v; }

/* Reset per-frame struct state for the next frame. Does NOT modify `buf` —
 * the previously-finalized frame stays intact in the buffer for the caller
 * to transmit. The new frame's header bytes will be committed to `buf` only
 * when cbrt_frame_finalize runs next. */
static void frame_reset_(cbrt_frame_t *f) {
    f->payload_len = 0;
    f->bool_acc    = 0;
    f->bool_nbits  = 0;
    f->err         = CBRT_OK;
    f->ts_us       = 0;
    f->period_us   = 0;
}

/* Reserve `n` payload bytes; returns the destination pointer or NULL on overflow. */
static uint8_t *reserve_(cbrt_frame_t *f, size_t n) {
    size_t off, end;
    if (f->err) return NULL;
    off = (size_t)f->header_size + f->payload_len;
    end = off + n;
    /* Reserve room for the optional CRC trailer too. */
    if ((f->flags & CBRT_F_CRC) && end + 2 > f->cap) { f->err = CBRT_ERR_OVERFLOW; return NULL; }
    if (end > f->cap)                                 { f->err = CBRT_ERR_OVERFLOW; return NULL; }
    f->payload_len += n;
    return f->buf + off;
}

/* Type-check that the current append matches the frame's datatype. */
static int check_dt_(cbrt_frame_t *f, uint8_t expect) {
    if (!f) return CBRT_ERR_BAD_ARG;
    if (f->err) return f->err;
    if (f->datatype != expect) { f->err = CBRT_ERR_WRONG_TYPE; return CBRT_ERR_WRONG_TYPE; }
    return CBRT_OK;
}

int cbrt_frame_init(cbrt_frame_t *f,
                    uint8_t *buf,
                    size_t   cap,
                    uint8_t  datatype,
                    uint8_t  channels,
                    uint8_t  flags)
{
    uint8_t cur;
    if (!f || !buf) return CBRT_ERR_BAD_ARG;
    if (datatype > 0xE) return CBRT_ERR_BAD_ARG;
    if (channels == 0 || channels > 64) return CBRT_ERR_BAD_ARG;
    if (flags & 0x0F) return CBRT_ERR_BAD_ARG; /* low nibble belongs to datatype */

    f->buf              = buf;
    f->cap              = cap;
    f->flags            = flags;
    f->datatype         = datatype;
    f->channels         = channels;
    f->bytes_per_sample = bps_(datatype); /* 0 for bool-packed */
    f->seq              = 0;

    /* Compute fixed header offsets and total header size. */
    cur = 6; /* after sync(4) + flags(1) + channels(1) */
    if (flags & CBRT_F_SEQ)    cur += 1;
    f->ts_off     = (flags & CBRT_F_TS)     ? (int8_t)cur : (int8_t)-1; if (flags & CBRT_F_TS)     cur += 4;
    f->period_off = (flags & CBRT_F_PERIOD) ? (int8_t)cur : (int8_t)-1; if (flags & CBRT_F_PERIOD) cur += 2;
    f->len_off    = (int8_t)cur;
    f->header_size = (uint8_t)(cur + 2);

    frame_reset_(f);
    if (cap < (size_t)f->header_size + ((flags & CBRT_F_CRC) ? 2u : 0u)) {
        f->err = CBRT_ERR_OVERFLOW;
        return CBRT_ERR_OVERFLOW;
    }

    /* Persistent header bytes (written once per session). The per-frame
     * slots (seq / ts / period / payload_len / crc) are committed to buf
     * only by cbrt_frame_finalize. */
    buf[0] = 0x43; /* 'C' */
    buf[1] = 0x42; /* 'B' */
    buf[2] = 0x52; /* 'R' */
    buf[3] = 0x54; /* 'T' */
    buf[4] = (uint8_t)((flags & 0xF0) | (datatype & 0x0F));
    buf[5] = channels;
    return CBRT_OK;
}

int cbrt_frame_set_ts(cbrt_frame_t *f, uint32_t ts_us) {
    if (!f) return CBRT_ERR_BAD_ARG;
    if (f->err) return f->err;
    if (f->ts_off < 0) { f->err = CBRT_ERR_BAD_STATE; return CBRT_ERR_BAD_STATE; }
    f->ts_us = ts_us; /* committed to buf by cbrt_frame_finalize */
    return CBRT_OK;
}

int cbrt_frame_set_period(cbrt_frame_t *f, uint16_t period_us) {
    if (!f) return CBRT_ERR_BAD_ARG;
    if (f->err) return f->err;
    if (f->period_off < 0) { f->err = CBRT_ERR_BAD_STATE; return CBRT_ERR_BAD_STATE; }
    f->period_us = period_us; /* committed to buf by cbrt_frame_finalize */
    return CBRT_OK;
}

/* ---- Typed appenders ----------------------------------------------------- */

int cbrt_append_u8(cbrt_frame_t *f, uint8_t v) {
    uint8_t *p;
    int r = check_dt_(f, CBRT_DT_U8); if (r) return r;
    p = reserve_(f, 1); if (!p) return f->err;
    p[0] = v;
    return CBRT_OK;
}
int cbrt_append_i8(cbrt_frame_t *f, int8_t v) {
    uint8_t *p;
    int r = check_dt_(f, CBRT_DT_I8); if (r) return r;
    p = reserve_(f, 1); if (!p) return f->err;
    p[0] = (uint8_t)v;
    return CBRT_OK;
}
int cbrt_append_u16(cbrt_frame_t *f, uint16_t v) {
    uint8_t *p;
    int r = check_dt_(f, CBRT_DT_U16); if (r) return r;
    p = reserve_(f, 2); if (!p) return f->err;
    put_u16_(p, v);
    return CBRT_OK;
}
int cbrt_append_i16(cbrt_frame_t *f, int16_t v) {
    uint8_t *p;
    int r = check_dt_(f, CBRT_DT_I16); if (r) return r;
    p = reserve_(f, 2); if (!p) return f->err;
    put_u16_(p, (uint16_t)v);
    return CBRT_OK;
}
int cbrt_append_u32(cbrt_frame_t *f, uint32_t v) {
    uint8_t *p;
    int r = check_dt_(f, CBRT_DT_U32); if (r) return r;
    p = reserve_(f, 4); if (!p) return f->err;
    put_u32_(p, v);
    return CBRT_OK;
}
int cbrt_append_i32(cbrt_frame_t *f, int32_t v) {
    uint8_t *p;
    int r = check_dt_(f, CBRT_DT_I32); if (r) return r;
    p = reserve_(f, 4); if (!p) return f->err;
    put_u32_(p, (uint32_t)v);
    return CBRT_OK;
}
int cbrt_append_u64(cbrt_frame_t *f, uint64_t v) {
    uint8_t *p;
    int r = check_dt_(f, CBRT_DT_U64); if (r) return r;
    p = reserve_(f, 8); if (!p) return f->err;
    put_u64_(p, v);
    return CBRT_OK;
}
int cbrt_append_i64(cbrt_frame_t *f, int64_t v) {
    uint8_t *p;
    int r = check_dt_(f, CBRT_DT_I64); if (r) return r;
    p = reserve_(f, 8); if (!p) return f->err;
    put_u64_(p, (uint64_t)v);
    return CBRT_OK;
}

#ifndef CBRT_NO_FLOAT_API
/* Float -> bit pattern via union (well-defined type punning in C99). */
int cbrt_append_f32(cbrt_frame_t *f, float v) {
    union { float    fv; uint32_t uv; } pun;
    uint8_t *p;
    int r = check_dt_(f, CBRT_DT_F32); if (r) return r;
    p = reserve_(f, 4); if (!p) return f->err;
    pun.fv = v;
    put_u32_(p, pun.uv);
    return CBRT_OK;
}
int cbrt_append_f64(cbrt_frame_t *f, double v) {
    union { double   fv; uint64_t uv; } pun;
    uint8_t *p;
    int r = check_dt_(f, CBRT_DT_F64); if (r) return r;
    p = reserve_(f, 8); if (!p) return f->err;
    pun.fv = v;
    put_u64_(p, pun.uv);
    return CBRT_OK;
}
#endif

int cbrt_append_q15(cbrt_frame_t *f, int16_t raw) {
    uint8_t *p;
    int r = check_dt_(f, CBRT_DT_Q15); if (r) return r;
    p = reserve_(f, 2); if (!p) return f->err;
    put_u16_(p, (uint16_t)raw);
    return CBRT_OK;
}
int cbrt_append_q31(cbrt_frame_t *f, int32_t raw) {
    uint8_t *p;
    int r = check_dt_(f, CBRT_DT_Q31); if (r) return r;
    p = reserve_(f, 4); if (!p) return f->err;
    put_u32_(p, (uint32_t)raw);
    return CBRT_OK;
}
int cbrt_append_bf16(cbrt_frame_t *f, uint16_t raw_bits) {
    uint8_t *p;
    int r = check_dt_(f, CBRT_DT_BF16); if (r) return r;
    p = reserve_(f, 2); if (!p) return f->err;
    put_u16_(p, raw_bits);
    return CBRT_OK;
}
int cbrt_append_f16(cbrt_frame_t *f, uint16_t raw_bits) {
    uint8_t *p;
    int r = check_dt_(f, CBRT_DT_F16); if (r) return r;
    p = reserve_(f, 2); if (!p) return f->err;
    put_u16_(p, raw_bits);
    return CBRT_OK;
}

/* Bool-packed: accumulate MSB-first into bool_acc, flush full bytes into the
 * payload as soon as 8 bits are available. */
int cbrt_append_bool_sample(cbrt_frame_t *f, const uint8_t *channel_values) {
    int r;
    uint8_t ch;
    if (!f || !channel_values) return CBRT_ERR_BAD_ARG;
    r = check_dt_(f, CBRT_DT_BOOL); if (r) return r;

    for (ch = 0; ch < f->channels; ch++) {
        f->bool_acc = (f->bool_acc << 1) | (channel_values[ch] ? 1u : 0u);
        f->bool_nbits++;
        if (f->bool_nbits == 8) {
            uint8_t *p = reserve_(f, 1);
            if (!p) return f->err;
            *p = (uint8_t)(f->bool_acc & 0xFF);
            f->bool_acc   = 0;
            f->bool_nbits = 0;
        }
    }
    return CBRT_OK;
}

int cbrt_append_raw(cbrt_frame_t *f, const void *src, size_t n) {
    const uint8_t *s;
    uint8_t *p;
    size_t unit, i;
    if (!f || (!src && n)) return CBRT_ERR_BAD_ARG;
    if (f->err) return f->err;
    if (f->datatype == CBRT_DT_BOOL) { f->err = CBRT_ERR_WRONG_TYPE; return CBRT_ERR_WRONG_TYPE; }

    unit = (size_t)f->channels * (size_t)f->bytes_per_sample;
    if (unit == 0 || (n % unit) != 0) { f->err = CBRT_ERR_BAD_ALIGN; return CBRT_ERR_BAD_ALIGN; }

    p = reserve_(f, n); if (!p) return f->err;
    s = (const uint8_t *)src;
    for (i = 0; i < n; i++) p[i] = s[i];
    return CBRT_OK;
}

size_t cbrt_frame_finalize(cbrt_frame_t *f) {
    size_t total;

    if (!f) return 0;
    if (f->err) return 0;

    /* Flush bool-packed tail: pad with whole zero samples until byte-aligned (§3.8). */
    if (f->datatype == CBRT_DT_BOOL && f->bool_nbits != 0) {
        while (f->bool_nbits != 0) {
            /* One zero-valued sample = `channels` zero bits. */
            uint8_t ch;
            for (ch = 0; ch < f->channels; ch++) {
                f->bool_acc = (f->bool_acc << 1);
                f->bool_nbits++;
                if (f->bool_nbits == 8) {
                    uint8_t *p = reserve_(f, 1);
                    if (!p) return 0;
                    *p = (uint8_t)(f->bool_acc & 0xFF);
                    f->bool_acc   = 0;
                    f->bool_nbits = 0;
                }
            }
        }
    }

    /* Payload length must fit in u16 (§3.7). */
    if (f->payload_len > 0xFFFFu) { f->err = CBRT_ERR_OVERFLOW; return 0; }

    /* Commit the per-frame header bytes to buf. The persistent prefix
     * (sync word + flags+datatype + channels) was written once by
     * cbrt_frame_init and is left alone here. */
    if (f->flags & CBRT_F_SEQ) f->buf[6] = f->seq;
    if (f->ts_off     >= 0) put_u32_(f->buf + f->ts_off,     f->ts_us);
    if (f->period_off >= 0) put_u16_(f->buf + f->period_off, f->period_us);
    put_u16_(f->buf + f->len_off, (uint16_t)f->payload_len);

    total = (size_t)f->header_size + f->payload_len;

    if (f->flags & CBRT_F_CRC) {
        uint16_t crc;
        if (total + 2 > f->cap) { f->err = CBRT_ERR_OVERFLOW; return 0; }
        /* CRC covers offset 4 through end of payload (§3.9). */
        crc = crc16_(f->buf + 4, total - 4);
        put_u16_(f->buf + total, crc);
        total += 2;
    }

    /* Roll forward to the next frame: bump the internal seq, clear the
     * per-frame struct fields. `buf` is NOT touched here — the bytes the
     * caller just got back from us stay intact until they call set_xx /
     * append_xx for the next frame. */
    f->seq = (uint8_t)(f->seq + 1);
    frame_reset_(f);

    return total;
}
