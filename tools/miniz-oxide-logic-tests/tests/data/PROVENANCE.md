<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# KAT fixture provenance (Lane 2a, `miniz_oxide`)

Every stream here was fetched **raw** with `curl` from the upstream repository at the commit named, then checked
against that project's own expected plaintext or checksum. Exact bytes come from the files, not from a summary.
The decoded SHA-256 values are below so a re-fetch can be compared without trusting this document.

`miniz_oxide` implements DEFLATE (RFC 1951) and the zlib wrapper (RFC 1950), not the gzip container (RFC 1952).
The gzip fixtures are therefore also a test of the KAT helper: it parses the gzip header, raw-inflates each member,
and verifies the member's CRC32 and ISIZE trailer.

## gzip

| Fixture | Upstream file | Commit | Compressed SHA-256 | Decoded | Decoded SHA-256 | Independent oracle |
|---|---|---|---|---|---|---|
| `gzip/zlib-rs-example.gz` | `libz-rs-sys/src/test-data/example.gz` in [zlib-rs](https://github.com/trifectatechfoundation/zlib-rs) | `5dbef501d08c31190bedf07dae5d3d3a29149831` | `9fabf7976e0f5e5dc9f7124aeeb0efcf2083989d418aa0094590d5df3f7cc088` | `zlib-rs gzip example` (20 B) | `94a0efa899d05d95b019aa45995085240cf0727f975d06198521feca376b894f` | the same commit's `example.txt` is those 20 bytes |
| `gzip/flate2-multi.gz` | `tests/multi.gz` in [flate2-rs](https://github.com/rust-lang/flate2-rs) | `54f49cd9f4c36774f68cc6ab95edf62a3b6c506e` | `efa3341da052f95056314cc6920e02a3da15bdef30234b2552fb407812db5cc6` | `first\n` + `second\n` (two members) | `dbea9325179efe46ea2add94f7b6b745ca983fabb208dc6d34aa064623d7ee23` | the same commit's `tests/multi.txt` is `first\nsecond\n` |

`flate2-multi.gz` is a multi-member stream: member 1 decodes to `first\n`, member 2 to `second\n`. The KAT asserts
that split, so a decoder that stops after the first member fails.

## zlib

| Fixture | Upstream file | Commit | Compressed SHA-256 | Decoded | Decoded SHA-256 | Independent oracle |
|---|---|---|---|---|---|---|
| `zlib/ratarmount-simple.zlib` | `tests/simple.zlib` in [ratarmount](https://github.com/mxmlnkn/ratarmount) | `7179828ac5456ff46a38bd7175b4804e252948ad` | `e29fd1c9edb65cb229f75369aa1e9835deccf10c6ff6e2d9cdcc3820c559b737` | `foo fighter\n` (12 B) | `c6f4a66f1b6e1fe5f4a562e3f30ab0486b362ff979991a0fbad8a7701efa18e9` | that commit's `tests/create-fixed-archives-list.sh` records MD5 `19696f24a91fc4e8950026f9c801a0d0` for both `simple` and `simple.zlib`'s decompressed content |
| `zlib/dexador-test.zlib` | `t/data/test.zlib` in [dexador](https://github.com/fukamachi/dexador) | `6289b8a3a6da19d7a45481dd1f51364ce5190864` | `a66128fc73e67428357f5a266f06f99e7a808fc4b7648092993bc985af3091b5` | `Deflate test string.` (20 B) | `859b896265dee4eaf2b313be0401d01f94b1e08935f388d198de9c6ad1e4bb7b` | that commit's `t/dexador.lisp` asserts the response body is exactly `"Deflate test string."` |

The two zlib streams have different FLG bytes (`0x785e` and `0x789c`), so both are valid RFC 1950 headers at the same
32 KiB window size; the KAT checks the header bytes and the decoded body.

`zlib/dexador-test.zlib` is 29 bytes: the zlib stream (adler32 `4e97077b` at bytes 24..27) followed by one trailing
byte `0x85` that upstream ships as part of the HTTP test body. The reference decoder ignores it and returns the
20-byte body, so the KAT also exercises "trailing bytes after the zlib stream are not an error". The corrupt-adler
test therefore uses `zlib/ratarmount-simple.zlib`, whose four adler bytes are the last bytes of the file.

## Fetch commands

```sh
curl -sSL -o gzip/zlib-rs-example.gz \
  https://raw.githubusercontent.com/trifectatechfoundation/zlib-rs/5dbef501d08c31190bedf07dae5d3d3a29149831/libz-rs-sys/src/test-data/example.gz
curl -sSL -o gzip/flate2-multi.gz \
  https://raw.githubusercontent.com/rust-lang/flate2-rs/54f49cd9f4c36774f68cc6ab95edf62a3b6c506e/tests/multi.gz
curl -sSL -o zlib/ratarmount-simple.zlib \
  https://raw.githubusercontent.com/mxmlnkn/ratarmount/7179828ac5456ff46a38bd7175b4804e252948ad/tests/simple.zlib
curl -sSL -o zlib/dexador-test.zlib \
  https://raw.githubusercontent.com/fukamachi/dexador/6289b8a3a6da19d7a45481dd1f51364ce5190864/t/data/test.zlib
```

## Re-check

```sh
python3 - <<'PY'
import gzip, hashlib, zlib
for f in ("gzip/zlib-rs-example.gz", "gzip/flate2-multi.gz"):
    raw = open(f, "rb").read()
    out = gzip.decompress(raw)
    print(f, hashlib.sha256(raw).hexdigest(), repr(out), hashlib.sha256(out).hexdigest())
for f in ("zlib/ratarmount-simple.zlib", "zlib/dexador-test.zlib"):
    raw = open(f, "rb").read()
    out = zlib.decompress(raw)
    print(f, hashlib.sha256(raw).hexdigest(), repr(out), hashlib.sha256(out).hexdigest())
PY
```

The values printed must match the table above. Python's `gzip`/`zlib` are the reference implementation here; the
in-repo KAT then checks the same bytes through the vendored crate.
