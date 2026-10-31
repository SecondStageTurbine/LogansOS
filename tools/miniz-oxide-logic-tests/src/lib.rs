// SPDX-License-Identifier: MPL-2.0
//! Host known-answer tests for the vendored `miniz_oxide` (Lane 2a).
//!
//! The compressed vectors are fetched raw from upstream projects at pinned
//! commits, never re-typed or summarised; `tests/data/PROVENANCE.md` records
//! each URL, commit, SHA-256, decoded bytes and the upstream project's own
//! expected value for them.
//!
//! `miniz_oxide` implements DEFLATE (RFC 1951) and the zlib wrapper (RFC 1950),
//! not the gzip container (RFC 1952). The gzip KATs therefore parse the gzip
//! header here, raw-inflate each member, and verify the member's CRC32 and
//! ISIZE trailer, which is the same split a gzip front-end in the OS will need.
//! The round-trip tests exercise the vendored compressor and inflater together;
//! they are not a substitute for the known streams, because a writer and a
//! reader built from the same code agree even when both are wrong.

#[cfg(test)]
mod tests {
    use miniz_oxide::deflate::{compress_to_vec, compress_to_vec_zlib};
    use miniz_oxide::inflate::stream::{InflateState, inflate};
    use miniz_oxide::inflate::{decompress_to_vec, decompress_to_vec_zlib};
    use miniz_oxide::{DataFormat, MZFlush, MZStatus};

    // Fetched raw; see tests/data/PROVENANCE.md.
    const GZIP_ZLIB_RS_EXAMPLE: &[u8] = include_bytes!("../tests/data/gzip/zlib-rs-example.gz");
    const GZIP_FLATE2_MULTI: &[u8] = include_bytes!("../tests/data/gzip/flate2-multi.gz");
    const ZLIB_RATARMOUNT_SIMPLE: &[u8] =
        include_bytes!("../tests/data/zlib/ratarmount-simple.zlib");
    const ZLIB_DEXADOR_TEST: &[u8] = include_bytes!("../tests/data/zlib/dexador-test.zlib");

    /// CRC-32/ISO-HDLC (the gzip trailer checksum), table-free so the KAT does
    /// not add a dependency just to read the container.
    fn crc32(data: &[u8]) -> u32 {
        let mut crc = !0u32;
        for &byte in data {
            crc ^= u32::from(byte);
            for _ in 0..8 {
                crc = if crc & 1 != 0 {
                    (crc >> 1) ^ 0xedb8_8320
                } else {
                    crc >> 1
                };
            }
        }
        !crc
    }

    fn skip_nul_terminated(data: &[u8], mut pos: usize) -> usize {
        while pos < data.len() {
            let byte = data[pos];
            pos += 1;
            if byte == 0 {
                return pos;
            }
        }
        panic!("truncated NUL-terminated gzip header field");
    }

    /// Inflate every member of an RFC 1952 gzip stream, verifying each member's
    /// CRC32 and ISIZE. Returns the members separately so a multi-member stream
    /// cannot pass by decoding only its first member.
    fn inflate_gzip_members(data: &[u8]) -> Vec<Vec<u8>> {
        let mut members = Vec::new();
        let mut pos = 0;

        while pos < data.len() {
            assert!(
                data.len() - pos >= 10,
                "truncated gzip header at byte {pos}"
            );
            assert_eq!(
                &data[pos..pos + 2],
                &[0x1f, 0x8b],
                "gzip magic at byte {pos}"
            );
            assert_eq!(data[pos + 2], 8, "gzip compression method at byte {pos}");

            let flags = data[pos + 3];
            let mut payload = pos + 10;

            if flags & 0x04 != 0 {
                // FEXTRA: XLEN little-endian, then XLEN bytes.
                assert!(data.len() - payload >= 2, "truncated gzip FEXTRA");
                let xlen = usize::from(u16::from_le_bytes([data[payload], data[payload + 1]]));
                payload += 2 + xlen;
                assert!(payload <= data.len(), "gzip FEXTRA runs past the stream");
            }
            if flags & 0x08 != 0 {
                // FNAME
                payload = skip_nul_terminated(data, payload);
            }
            if flags & 0x10 != 0 {
                // FCOMMENT
                payload = skip_nul_terminated(data, payload);
            }
            if flags & 0x02 != 0 {
                // FHCRC
                assert!(data.len() - payload >= 2, "truncated gzip FHCRC");
                payload += 2;
            }

            let mut state = InflateState::new_boxed(DataFormat::Raw);
            let mut member = Vec::new();
            let mut chunk = [0u8; 8192];
            let mut consumed = 0usize;

            loop {
                let result = inflate(
                    &mut state,
                    &data[payload + consumed..],
                    &mut chunk,
                    MZFlush::Finish,
                );
                member.extend_from_slice(&chunk[..result.bytes_written]);
                consumed += result.bytes_consumed;

                match result.status {
                    Ok(MZStatus::StreamEnd) => break,
                    Ok(MZStatus::Ok) => assert!(
                        result.bytes_consumed > 0 || result.bytes_written > 0,
                        "inflate made no progress"
                    ),
                    Ok(MZStatus::NeedDict) => panic!("gzip member needs a preset dictionary"),
                    Err(error) => panic!("inflate failed: {error:?}"),
                }
            }

            payload += consumed;
            assert!(data.len() - payload >= 8, "truncated gzip trailer");

            let want_crc = u32::from_le_bytes(data[payload..payload + 4].try_into().unwrap());
            let want_isize = u32::from_le_bytes(data[payload + 4..payload + 8].try_into().unwrap());
            assert_eq!(crc32(&member), want_crc, "gzip member CRC32");
            assert_eq!(
                u32::try_from(member.len()).unwrap(),
                want_isize,
                "gzip member ISIZE"
            );

            members.push(member);
            pos = payload + 8;
        }

        members
    }

    fn inflate_gzip(data: &[u8]) -> Vec<u8> {
        inflate_gzip_members(data).concat()
    }

    fn round_trip_inputs() -> Vec<Vec<u8>> {
        // A deterministic xorshift stream: incompressible bytes without a
        // random-number dependency.
        let mut state = 0x9e37_79b9_7f4a_7c15u64;
        let mut random = Vec::with_capacity(32 * 1024);
        for _ in 0..32 * 1024 {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            random.push(state as u8);
        }

        vec![
            Vec::new(),
            b"hello, deflate".to_vec(),
            vec![0u8; 1024],
            (0u8..=255).collect(),
            "the quick brown fox jumps over the lazy dog\n"
                .repeat(512)
                .into_bytes(),
            random,
        ]
    }

    #[test]
    fn crc32_matches_the_standard_check_value() {
        assert_eq!(crc32(b"123456789"), 0xcbf4_3926);
    }

    #[test]
    fn gzip_zlib_rs_example_inflates_to_its_plaintext() {
        let members = inflate_gzip_members(GZIP_ZLIB_RS_EXAMPLE);
        assert_eq!(members, [b"zlib-rs gzip example".to_vec()]);
    }

    #[test]
    fn gzip_flate2_stream_decodes_every_member() {
        let members = inflate_gzip_members(GZIP_FLATE2_MULTI);
        assert_eq!(members, [b"first\n".to_vec(), b"second\n".to_vec()]);
        assert_eq!(inflate_gzip(GZIP_FLATE2_MULTI), b"first\nsecond\n");
    }

    #[test]
    fn zlib_ratarmount_stream_inflates_to_the_upstream_checksum() {
        assert_eq!(&ZLIB_RATARMOUNT_SIMPLE[..2], &[0x78, 0x5e]);
        assert_eq!(
            decompress_to_vec_zlib(ZLIB_RATARMOUNT_SIMPLE).unwrap(),
            b"foo fighter\n"
        );
    }

    #[test]
    fn zlib_dexador_stream_inflates_to_the_upstream_body() {
        assert_eq!(&ZLIB_DEXADOR_TEST[..2], &[0x78, 0x9c]);
        assert_eq!(
            decompress_to_vec_zlib(ZLIB_DEXADOR_TEST).unwrap(),
            b"Deflate test string."
        );
    }

    #[test]
    fn zlib_stream_with_a_corrupt_adler_is_rejected() {
        let mut corrupt = ZLIB_RATARMOUNT_SIMPLE.to_vec();
        *corrupt.last_mut().unwrap() ^= 0x01;
        assert!(decompress_to_vec_zlib(&corrupt).is_err());
    }

    #[test]
    fn raw_deflate_round_trips_across_the_level_band() {
        for input in round_trip_inputs() {
            for level in [0, 1, 3, 6, 9, 10] {
                let compressed = compress_to_vec(&input, level);
                let decoded = decompress_to_vec(&compressed).unwrap_or_else(|error| {
                    panic!(
                        "raw deflate level {level}, {} input bytes: {error:?}",
                        input.len()
                    )
                });
                assert_eq!(decoded, input, "raw deflate level {level}");
            }
        }
    }

    #[test]
    fn zlib_round_trips_across_the_level_band() {
        for input in round_trip_inputs() {
            for level in [0, 1, 3, 6, 9, 10] {
                let compressed = compress_to_vec_zlib(&input, level);
                let decoded = decompress_to_vec_zlib(&compressed).unwrap_or_else(|error| {
                    panic!("zlib level {level}, {} input bytes: {error:?}", input.len())
                });
                assert_eq!(decoded, input, "zlib level {level}");
            }
        }
    }
}
