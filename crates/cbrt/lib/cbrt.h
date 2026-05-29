/*
 * cbrt.h — C encoder for the CBRT (CycBox Real-Time) protocol.
 *
 * A zero-dependency ANSI/C99 encoder for sending CBRT frames (see
 * ../protocol.md) from an MCU. One header + one source file. No malloc,
 * no memcpy / memset, no float math required by the core. Only <stdint.h>
 * and <stddef.h> are pulled in.
 *
 * ── Quick start ───────────────────────────────────────────────────────────
 *
 *   1. Copy cbrt.h and cbrt.c into your project.
 *   2. Add cbrt.c to your build (one TU); #include "cbrt.h" anywhere you
 *      call the API.
 *   3. Use it:
 *
 *        static uint8_t      tx_buf[128];
 *        static cbrt_frame_t frame;   // initialized once, reused per frame
 *
 *        void app_init(void) {
 *            cbrt_frame_init(&frame, tx_buf, sizeof(tx_buf),
 *                            CBRT_DT_I16, 4,
 *                            CBRT_F_TS | CBRT_F_PERIOD | CBRT_F_SEQ | CBRT_F_CRC);
 *        }
 *
 *        void app_tick(const int16_t *samples_4ch_x_10) {
 *            cbrt_frame_set_ts(&frame, micros());
 *            cbrt_frame_set_period(&frame, 1000);
 *            for (int i = 0; i < 40; i++)
 *                cbrt_append_i16(&frame, samples_4ch_x_10[i]);
 *
 *            size_t n = cbrt_frame_finalize(&frame);  // returns frame length;
 *                                                     // also bumps seq and
 *                                                     // resets payload state
 *            if (n) uart_send(tx_buf, n);
 *        }
 *
 * The frame struct holds the persistent session config (datatype, channel
 * count, flag set, buffer) and is reused for every frame in that session.
 * `cbrt_frame_finalize` returns the encoded length of the just-built frame
 * and internally resets payload state and bumps the sequence counter so the
 * next set_ts / set_period / append_* calls compose the following frame.
 *
 * ── API summary ───────────────────────────────────────────────────────────
 *
 *   cbrt_frame_init(f, buf, cap, datatype, channels, flags)
 *       Initialize the builder once per session; writes the persistent
 *       header bytes and prepares the first frame. The seq counter is
 *       owned by f.
 *   cbrt_frame_set_ts(f, ts_us)
 *       Patch the µs timestamp (requires CBRT_F_TS).
 *   cbrt_frame_set_period(f, period_us)
 *       Patch the period (requires CBRT_F_PERIOD).
 *   cbrt_append_<TYPE>(f, v)
 *       Append one sample. Type must match the frame's datatype.
 *   cbrt_append_raw(f, src, n)
 *       Bulk-append already-LE bytes (e.g. ADC DMA buffer).
 *   cbrt_append_bool_sample(f, ch_vals)
 *       Append one bool-packed sample (array of `channels` bytes).
 *   cbrt_frame_finalize(f)
 *       Patch payload length, pad bools, write CRC. Returns total bytes,
 *       or 0 on error. On success also bumps the seq counter and resets
 *       payload state so the next set_* / append_* calls compose the
 *       following frame.
 *
 * ── Session discipline ────────────────────────────────────────────────────
 *
 * Per §5.2 the decoder treats the first frame's datatype, channel count,
 * and flag set as the session profile. `cbrt_frame_init` captures these
 * once and reuses them for every following frame, so the encoder enforces
 * session consistency by construction. To start a new session, call
 * `cbrt_frame_init` again with the new parameters.
 *
 * The sequence counter lives inside the frame struct. It is initialized
 * to 0 on `cbrt_frame_init` and incremented (mod 256) by
 * `cbrt_frame_finalize` after a successful encode. If CBRT_F_SEQ is not
 * set, the seq byte is omitted from the wire format and the counter is
 * unused.
 *
 * ── Errors ────────────────────────────────────────────────────────────────
 *
 * All builder functions are sticky: once an error is recorded on the
 * frame, subsequent appends become no-ops and `cbrt_frame_finalize`
 * returns 0. Inspect f->err to diagnose:
 *
 *   CBRT_ERR_OVERFLOW    Buffer too small for the next append (or CRC).
 *   CBRT_ERR_WRONG_TYPE  Append helper doesn't match the frame datatype.
 *   CBRT_ERR_BAD_STATE   set_ts / set_period called without the flag.
 *   CBRT_ERR_BAD_ALIGN   append_raw byte count isn't a multiple of
 *                        (channels × bytes_per_sample).
 *   CBRT_ERR_BAD_ARG     NULL pointer, bad channel count, bad datatype.
 *
 * ── Buffer sizing ─────────────────────────────────────────────────────────
 *
 * The wire frame is the fixed header (8..14 bytes depending on flags)
 * plus payload plus optional 2-byte CRC. Size `buf` for the worst-case
 * payload your session will produce. Transport guidance from §3.7:
 *
 *   * BLE notifications: keep payload ≤ 4 KiB (typical 247-byte MTU
 *     prefers far less per packet — split across frames).
 *   * Serial / TCP: 16 KiB is comfortable.
 *
 * ── Build options ─────────────────────────────────────────────────────────
 *
 * Define when compiling cbrt.c, e.g. -DCBRT_CRC_TABLE:
 *
 *   CBRT_CRC_TABLE     Use a 512-byte CRC-16/MODBUS table (~4× faster,
 *                      costs flash) instead of the bitwise loop.
 *   CBRT_NO_FLOAT_API  Drop cbrt_append_f32 / cbrt_append_f64. Useful on
 *                      MCUs without FPU when you don't need IEEE float
 *                      datatypes.
 *
 * ── Notes ─────────────────────────────────────────────────────────────────
 *
 *   * No libc calls. No memcpy / memset — all byte writes are explicit
 *     loops or single stores. Floats are reinterpreted via a C99 union
 *     (well-defined type punning), not memcpy.
 *   * Endianness. All multi-byte writes are little-endian, done byte by
 *     byte — the code is correct on both LE and BE hosts.
 *   * Timestamp wrap. The u32 µs field wraps every ~71 minutes (§3.5).
 *     The decoder handles the wrap; the encoder just truncates
 *     esp_timer_get_time() / whatever µs source you have.
 *
 * License: same as the parent cbrt-codec crate.
 */
#ifndef CBRT_H
#define CBRT_H

#include <stdint.h>
#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

/* ---- Datatype codes (low nibble of flags byte). -------------------------- */
enum {
    CBRT_DT_U8   = 0x0,
    CBRT_DT_I8   = 0x1,
    CBRT_DT_U16  = 0x2,
    CBRT_DT_I16  = 0x3,
    CBRT_DT_U32  = 0x4,
    CBRT_DT_I32  = 0x5,
    CBRT_DT_U64  = 0x6,
    CBRT_DT_I64  = 0x7,
    CBRT_DT_F32  = 0x8,
    CBRT_DT_F64  = 0x9,
    CBRT_DT_Q15  = 0xA,
    CBRT_DT_Q31  = 0xB,
    CBRT_DT_BF16 = 0xC,
    CBRT_DT_F16  = 0xD,
    CBRT_DT_BOOL = 0xE
};

/* ---- Optional-field flag bits (high nibble of flags byte). --------------- */
enum {
    CBRT_F_TS     = 0x80,
    CBRT_F_PERIOD = 0x40,
    CBRT_F_CRC    = 0x20,
    CBRT_F_SEQ    = 0x10
};

/* ---- Status / error codes. ----------------------------------------------- */
enum {
    CBRT_OK              =  0,
    CBRT_ERR_BAD_ARG     = -1, /* NULL pointer, bad datatype/channel range, etc. */
    CBRT_ERR_OVERFLOW    = -2, /* user buffer too small for the next byte */
    CBRT_ERR_WRONG_TYPE  = -3, /* append helper doesn't match frame datatype */
    CBRT_ERR_BAD_STATE   = -4, /* set_ts/set_period called when flag not enabled */
    CBRT_ERR_BAD_ALIGN   = -5  /* append_raw size not a multiple of sample stride */
};

/* ---- Per-session builder, owns the user's buffer for the whole session. --
 *
 * Per-frame header values (seq, ts, period, payload_len, crc) are accumulated
 * in struct fields and committed to `buf` only inside cbrt_frame_finalize.
 * That means the bytes returned by finalize stay intact until the caller's
 * next set_xx / append_xx call — safe to transmit synchronously without
 * racing the encoder. */
typedef struct {
    /* Persistent across all frames of one session. */
    uint8_t  *buf;
    size_t    cap;
    uint8_t   flags;           /* high-nibble flag bits, exactly as user passed   */
    uint8_t   datatype;        /* low-nibble datatype code                        */
    uint8_t   channels;        /* 1..64                                           */
    uint8_t   header_size;     /* fixed prefix length: sync..payload_len (8..14)  */
    uint8_t   bytes_per_sample;/* 0 for CBRT_DT_BOOL                              */
    int8_t    ts_off;          /* offset of ts field in buf, or -1                */
    int8_t    period_off;      /* offset of period field in buf, or -1            */
    int8_t    len_off;         /* offset of payload_len field in buf (always set) */

    /* Per-frame state. Reset by cbrt_frame_finalize after a successful encode.
     * seq, ts_us, period_us are written into buf by finalize (not earlier). */
    uint8_t   seq;             /* sequence counter to emit (mod 256, §5.5)        */
    uint16_t  period_us;       /* value queued by cbrt_frame_set_period           */
    uint32_t  ts_us;           /* value queued by cbrt_frame_set_ts               */
    size_t    payload_len;     /* bytes appended into the payload region so far  */
    uint32_t  bool_acc;        /* bool-packed bit accumulator                    */
    uint8_t   bool_nbits;
    int       err;             /* sticky within one frame; cleared by finalize    */
} cbrt_frame_t;

/* ---- API ----------------------------------------------------------------- */

/* Initialize a frame builder for one logical session. Writes the constant
 * prefix (sync word, flags+datatype, channels) into `buf` once, and zeros
 * the internal seq counter and per-frame queued values. The per-frame
 * header bytes (seq, ts, period, payload_len, crc) are committed to `buf`
 * only when cbrt_frame_finalize runs.
 *
 * The same `buf` is reused for every frame. After the user has consumed
 * the bytes returned by finalize (e.g. via uart_send) they can immediately
 * begin appending the next frame — there is no separate "begin" call. */
int cbrt_frame_init(cbrt_frame_t *f,
                    uint8_t *buf,
                    size_t   cap,
                    uint8_t  datatype,
                    uint8_t  channels,
                    uint8_t  flags);

/* Queue the optional header values for the current frame. The corresponding
 * CBRT_F_* must have been set at init time; otherwise returns
 * CBRT_ERR_BAD_STATE. The value is stored in the frame struct and committed
 * to `buf` by cbrt_frame_finalize — so calling these does not modify the
 * bytes of the previously-finalized frame still sitting in `buf`. May be
 * called at any point before the next finalize. */
int cbrt_frame_set_ts(cbrt_frame_t *f, uint32_t ts_us);
int cbrt_frame_set_period(cbrt_frame_t *f, uint16_t period_us);

/* Typed sample appenders. The function must match the frame's datatype, else
 * CBRT_ERR_WRONG_TYPE. q15/q31 take pre-quantized raw integers; bf16/f16 take
 * raw 16-bit bit patterns (the user is responsible for conversion). */
int cbrt_append_u8 (cbrt_frame_t *f, uint8_t  v);
int cbrt_append_i8 (cbrt_frame_t *f, int8_t   v);
int cbrt_append_u16(cbrt_frame_t *f, uint16_t v);
int cbrt_append_i16(cbrt_frame_t *f, int16_t  v);
int cbrt_append_u32(cbrt_frame_t *f, uint32_t v);
int cbrt_append_i32(cbrt_frame_t *f, int32_t  v);
int cbrt_append_u64(cbrt_frame_t *f, uint64_t v);
int cbrt_append_i64(cbrt_frame_t *f, int64_t  v);
#ifndef CBRT_NO_FLOAT_API
int cbrt_append_f32(cbrt_frame_t *f, float    v);
int cbrt_append_f64(cbrt_frame_t *f, double   v);
#endif
int cbrt_append_q15 (cbrt_frame_t *f, int16_t  raw);
int cbrt_append_q31 (cbrt_frame_t *f, int32_t  raw);
int cbrt_append_bf16(cbrt_frame_t *f, uint16_t raw_bits);
int cbrt_append_f16 (cbrt_frame_t *f, uint16_t raw_bits);

/* Append one bool-packed sample. `channel_values` is an array of `channels`
 * bytes (0 = false, non-zero = true). Channel 0 is emitted first per §3.8. */
int cbrt_append_bool_sample(cbrt_frame_t *f, const uint8_t *channel_values);

/* Append raw bytes already laid out in LE wire format (e.g. straight from
 * an ADC DMA buffer). Not valid for CBRT_DT_BOOL frames. The byte count
 * must be a multiple of (channels * bytes_per_sample). */
int cbrt_append_raw(cbrt_frame_t *f, const void *src, size_t n);

/* Finalize the current frame: pads bool-packed bits, writes the per-frame
 * header bytes (seq, ts, period, payload_len) into `buf`, and appends the
 * CRC trailer if enabled. Returns the total frame size in bytes (0 on
 * error — inspect f->err).
 *
 * On success, also rolls the builder forward to the next frame:
 *   - increments the internal seq counter (mod 256)
 *   - clears the queued ts / period values and the payload accumulator
 * `buf` is NOT modified after this returns, so the caller can transmit the
 * returned [buf, buf+n) range synchronously without racing the encoder.
 * The next set_xx / append_xx call composes the following frame and is the
 * first write back into `buf`. */
size_t cbrt_frame_finalize(cbrt_frame_t *f);

#ifdef __cplusplus
} /* extern "C" */
#endif

#endif /* CBRT_H */
