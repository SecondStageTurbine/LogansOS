#!/usr/bin/env python3
# SPDX-License-Identifier: MPL-2.0
"""Read-only NeoFS v1 inspector (OT-73).

Parses the on-disk layout defined by docs/specs/neofs-format.md and reports the
directory tree, per-file sizes, and the BLAKE3 hash each inode RECORDS. With
--verify it also RECOMPUTES all three hash layers and compares:

  * superblock  checksum      = BLAKE3(bytes 0..480)
  * superblock  root_inode_hash = BLAKE3(root inode bytes 0..256)
  * inode       checksum      = BLAKE3(bytes 0..224)
  * inode       content_hash  = BLAKE3(logical file bytes)

That comparison is the actual integrity check. A stored hash on its own proves
nothing -- which is the whole point: the volume carries its own proof, so no
external reference hash is needed to verify a model offline.

OPENS THE TARGET READ-ONLY AND NEVER WRITES. Safe against a live block device.

Note on `neofs-mkfs verify`: that tool performs the same checks but takes its
size from stat(), and Linux reports st_size = 0 for block devices -- so running
it on /dev/sda fails with "image does not contain all three required superblock
copies", which reads like corruption and is not. This tool addresses the device
by offset and has no such failure mode.

Self-contained: a pure-Python BLAKE3 is bundled so a live USB with no network
still produces a verdict. It is slow (~0.8 MiB/s), so large-file content
verification is skipped unless --force-slow is given. Install a native backend
(`apt install python3-blake3`, or `pip install --break-system-packages blake3`)
to verify a multi-hundred-MiB model in seconds instead of tens of minutes.
"""

import argparse
import struct
import sys

# --- bundled pure-Python BLAKE3 fallback ---------------------------------
IV = [0x6A09E667, 0xBB67AE85, 0x3C6EF372, 0xA54FF53A,
      0x510E527F, 0x9B05688C, 0x1F83D9AB, 0x5BE0CD19]
MSG_PERMUTATION = [2, 6, 3, 10, 7, 0, 4, 13, 1, 11, 12, 5, 9, 14, 15, 8]

CHUNK_START = 1 << 0
CHUNK_END = 1 << 1
PARENT = 1 << 2
ROOT = 1 << 3

M32 = 0xFFFFFFFF
BLOCK_LEN = 64
CHUNK_LEN = 1024


def _rotr(x, n):
    return ((x >> n) | (x << (32 - n))) & M32


def _g(s, a, b, c, d, mx, my):
    s[a] = (s[a] + s[b] + mx) & M32
    s[d] = _rotr(s[d] ^ s[a], 16)
    s[c] = (s[c] + s[d]) & M32
    s[b] = _rotr(s[b] ^ s[c], 12)
    s[a] = (s[a] + s[b] + my) & M32
    s[d] = _rotr(s[d] ^ s[a], 8)
    s[c] = (s[c] + s[d]) & M32
    s[b] = _rotr(s[b] ^ s[c], 7)


def _round(s, m):
    _g(s, 0, 4, 8, 12, m[0], m[1])
    _g(s, 1, 5, 9, 13, m[2], m[3])
    _g(s, 2, 6, 10, 14, m[4], m[5])
    _g(s, 3, 7, 11, 15, m[6], m[7])
    _g(s, 0, 5, 10, 15, m[8], m[9])
    _g(s, 1, 6, 11, 12, m[10], m[11])
    _g(s, 2, 7, 8, 13, m[12], m[13])
    _g(s, 3, 4, 9, 14, m[14], m[15])


def _compress(cv, block_words, counter, block_len, flags):
    s = [cv[0], cv[1], cv[2], cv[3], cv[4], cv[5], cv[6], cv[7],
         IV[0], IV[1], IV[2], IV[3],
         counter & M32, (counter >> 32) & M32, block_len, flags]
    m = list(block_words)
    for r in range(7):
        _round(s, m)
        if r < 6:
            m = [m[MSG_PERMUTATION[i]] for i in range(16)]
    for i in range(8):
        s[i] ^= s[i + 8]
        s[i + 8] ^= cv[i]
    return s


def _words(block):
    if len(block) < BLOCK_LEN:
        block = block + b"\0" * (BLOCK_LEN - len(block))
    return [int.from_bytes(block[i * 4:i * 4 + 4], "little") for i in range(16)]


def _chunk_cv(data, counter, flags_base, want_root):
    """Chaining value (or root output) of one chunk's bytes."""
    cv = list(IV)
    blocks = [data[i:i + BLOCK_LEN] for i in range(0, len(data), BLOCK_LEN)] or [b""]
    for i, blk in enumerate(blocks):
        flags = flags_base
        if i == 0:
            flags |= CHUNK_START
        if i == len(blocks) - 1:
            flags |= CHUNK_END
            if want_root:
                flags |= ROOT
        out = _compress(cv, _words(blk), counter, len(blk), flags)
        cv = out[:8]
    return cv


def _parent_cv(left, right, want_root):
    words = left + right
    flags = PARENT | (ROOT if want_root else 0)
    return _compress(list(IV), words, 0, BLOCK_LEN, flags)[:8]


def blake3_digest(data):
    """32-byte BLAKE3 digest of `data` (unkeyed hash mode)."""
    n_chunks = max(1, (len(data) + CHUNK_LEN - 1) // CHUNK_LEN)

    # Every chunk EXCEPT the last is folded into the subtree stack, merging
    # whenever the completed-chunk count has trailing zero bits. The final chunk
    # is deliberately NOT pushed -- finalization folds the stack into it, which
    # is what places the ROOT flag on the outermost compression.
    stack = []
    for i in range(n_chunks - 1):
        cv = _chunk_cv(data[i * CHUNK_LEN:(i + 1) * CHUNK_LEN], i, 0, want_root=False)
        total = i + 1
        while total & 1 == 0:
            cv = _parent_cv(stack.pop(), cv, want_root=False)
            total >>= 1
        stack.append(cv)

    last = n_chunks - 1
    tail = data[last * CHUNK_LEN:(last + 1) * CHUNK_LEN]
    if not stack:
        cv = _chunk_cv(tail, last, 0, want_root=True)
    else:
        cv = _chunk_cv(tail, last, 0, want_root=False)
        while stack:
            cv = _parent_cv(stack.pop(), cv, want_root=not stack)
    return b"".join(w.to_bytes(4, "little") for w in cv)


class Blake3:
    """Minimal incremental wrapper (buffers input; fallback path only)."""

    def __init__(self):
        self._buf = bytearray()

    def update(self, data):
        self._buf += data
        return self

    def digest(self):
        return blake3_digest(bytes(self._buf))

    def hexdigest(self):
        return self.digest().hex()


BLK = 4096
SB_COPY_OFFSETS = (0, 1 << 20, 64 << 20)
SB_MAGIC = b"NEOFSv1\0"
INO_MAGIC = b"NEOFSINO"
INO_SIZE = 256
DIRENT_SIZE = 64
PTRS_PER_BLOCK = BLK // 8
KINDS = {0: "Regular", 1: "Directory", 2: "Symlink", 3: "Model"}


def u16(b, o):
    return struct.unpack_from("<H", b, o)[0]


def u32(b, o):
    return struct.unpack_from("<I", b, o)[0]


def u64(b, o):
    return struct.unpack_from("<Q", b, o)[0]


# --- BLAKE3 backend selection --------------------------------------------
def _blake3_factory():
    try:
        import blake3
        return (lambda: blake3.blake3()), "native (python-blake3)", True
    except ImportError:
        pass
    try:
        import hashlib
        hashlib.new("blake3")
        return (lambda: hashlib.new("blake3")), "native (hashlib)", True
    except Exception:
        pass
    return (lambda: Blake3()), "PURE-PYTHON FALLBACK (~0.8 MiB/s)", False


NEW_B3, B3_SOURCE, B3_NATIVE = _blake3_factory()
SLOW_LIMIT = 16 * 1024 * 1024
FORCE_SLOW = False


class Dev:
    """Read-only, offset-addressed reader."""

    def __init__(self, path):
        self.f = open(path, "rb")  # read-only, always
        self.path = path

    def at(self, off, n):
        self.f.seek(off)
        b = self.f.read(n)
        if len(b) != n:
            raise IOError(f"short read at offset {off}: wanted {n}, got {len(b)}")
        return b

    def block(self, blk):
        return self.at(blk * BLK, BLK)


def read_superblocks(dev):
    out = []
    for off in SB_COPY_OFFSETS:
        try:
            b = dev.at(off, 512)
        except IOError:
            out.append((off, None, "unreadable (device/image too small)"))
            continue
        if b[:8] != SB_MAGIC:
            out.append((off, None, f"bad magic {b[:8]!r}"))
            continue
        sb = {
            "offset": off,
            "format_version": u16(b, 8),
            "block_size": 1 << u16(b, 10),
            "total_blocks": u64(b, 16),
            "next_free_block": u64(b, 24),
            "generation": u64(b, 32),
            "root_inode_block": u64(b, 40),
            "root_inode_hash": b[48:80],
            "inode_size_bytes": u32(b, 80),
            "free_map_start_block": u64(b, 88),
            "free_map_block_count": u64(b, 96),
            "free_map_hash": b[104:136],
            "checksum": b[480:512],
            "raw": b,
        }
        h = NEW_B3()
        h.update(b[:480])
        note = "ok" if h.digest() == sb["checksum"] else "CHECKSUM MISMATCH"
        out.append((off, sb, note))
    return out


def verify_free_map(dev, sb):
    """BLAKE3 over the free-map blocks vs the hash the superblock records.

    Mirrors read_free_map_bytes in tools/neofs-mkfs/src/format.rs. Cheap even on
    the pure-Python backend: the map is ceil(total_blocks/32768) blocks, so a
    768 MiB volume is 6 blocks / 24 KiB.
    """
    start, count = sb["free_map_start_block"], sb["free_map_block_count"]
    if start == 0 or count == 0:
        return None, "absent (legacy bump-only allocation)"
    data = b"".join(dev.block(start + i) for i in range(count))
    ok = NEW_B3()
    ok.update(data)
    got = ok.digest()
    return got == sb["free_map_hash"], (
        "MATCHES" if got == sb["free_map_hash"] else "FREE-MAP HASH MISMATCH")


def read_inode(dev, blk, total_blocks):
    if blk == 0 or (total_blocks and blk >= total_blocks):
        raise ValueError(f"inode block {blk} out of range (total_blocks={total_blocks})")
    b = dev.block(blk)[:INO_SIZE]
    if b[:8] != INO_MAGIC:
        raise ValueError(f"bad inode magic at block {blk}: {b[:8]!r}")
    ino = {
        "block": blk,
        "kind_id": b[8],
        "kind": KINDS.get(b[8], f"reserved({b[8]})"),
        "generation": u32(b, 12),
        "file_size": u64(b, 16),
        "block_count": u64(b, 24),
        "direct": [u64(b, 32 + i * 8) for i in range(8)],
        "single_indirect": u64(b, 96),
        "content_hash": b[104:136],
        "double_indirect": u64(b, 136),
        "triple_indirect": u64(b, 144),
        "directory_entry_count": u32(b, 156),
        "checksum": b[224:256],
        "checksum_ok": None,
    }
    h = NEW_B3()
    h.update(b[:224])
    ino["checksum_ok"] = h.digest() == ino["checksum"]
    return ino


def logical_block_list(dev, ino):
    """Physical block numbers for logical blocks 0..block_count, in order."""
    n = ino["block_count"]
    out = list(ino["direct"][: min(n, 8)])
    if n <= 8:
        return out

    def ptrs(blk):
        b = dev.block(blk)
        return [u64(b, i * 8) for i in range(PTRS_PER_BLOCK)]

    remaining = n - 8
    # single indirect: logical 8..520
    take = min(remaining, PTRS_PER_BLOCK)
    out += ptrs(ino["single_indirect"])[:take]
    remaining -= take
    if remaining <= 0:
        return out
    # double indirect
    for leaf in ptrs(ino["double_indirect"]):
        if remaining <= 0:
            break
        take = min(remaining, PTRS_PER_BLOCK)
        out += ptrs(leaf)[:take]
        remaining -= take
    if remaining <= 0:
        return out
    # triple indirect
    for l2 in ptrs(ino["triple_indirect"]):
        if remaining <= 0:
            break
        for leaf in ptrs(l2):
            if remaining <= 0:
                break
            take = min(remaining, PTRS_PER_BLOCK)
            out += ptrs(leaf)[:take]
            remaining -= take
    return out


def read_content(dev, ino):
    data = bytearray()
    for blk in logical_block_list(dev, ino):
        data += dev.block(blk)
    return bytes(data[: ino["file_size"]])


def hash_content(dev, ino):
    """Streaming BLAKE3 over the logical bytes -- never holds the file in RAM."""
    h = NEW_B3()
    left = ino["file_size"]
    for blk in logical_block_list(dev, ino):
        chunk = dev.block(blk)
        if left < len(chunk):
            chunk = chunk[:left]
        h.update(chunk)
        left -= len(chunk)
        if left <= 0:
            break
    return h.digest()


def read_dirents(dev, ino):
    if ino["kind"] != "Directory":
        raise ValueError("not a directory")
    if ino["file_size"] == 0:
        return []
    content = read_content(dev, ino)
    ents = []
    for i in range(ino["directory_entry_count"]):
        e = content[i * DIRENT_SIZE : (i + 1) * DIRENT_SIZE]
        name = e[:48].split(b"\0", 1)[0].decode("utf-8", "replace")
        ents.append({"name": name, "child_block": u64(e, 48), "kind_id": e[56],
                     "kind": KINDS.get(e[56], f"reserved({e[56]})")})
    return ents


def walk(dev, ino, total_blocks, path, depth, out, verify):
    for e in read_dirents(dev, ino):
        if e["child_block"] == 0:
            continue
        full = f"{path}/{e['name']}"
        try:
            child = read_inode(dev, e["child_block"], total_blocks)
        except Exception as exc:
            out.append((full, "ERROR", str(exc)))
            continue
        if child["kind_id"] != e["kind_id"]:
            out.append((full, "ERROR",
                        f"dirent kind={e['kind']} but inode kind={child['kind']}"))
            continue
        status = []
        if child["checksum_ok"] is False:
            status.append("INODE CHECKSUM MISMATCH")
        elif child["checksum_ok"] is True:
            status.append("inode ok")
        content_verdict = "not checked"
        if verify and not B3_NATIVE and child["file_size"] > SLOW_LIMIT and not FORCE_SLOW:
            mins = child["file_size"] / 2**20 / 0.78 / 60
            content_verdict = (f"SKIPPED - {child['file_size'] / 2**20:.0f} MiB on the "
                               f"pure-Python backend is ~{mins:.0f} min; install a native "
                               f"blake3 or pass --force-slow")
        elif verify:
            try:
                got = hash_content(dev, child)
                content_verdict = ("CONTENT OK" if got == child["content_hash"]
                                   else "CONTENT HASH MISMATCH")
            except Exception as exc:
                content_verdict = f"content unreadable: {exc}"
        out.append((full, child, ", ".join(status), content_verdict))
        if child["kind"] == "Directory":
            walk(dev, child, total_blocks, full, depth + 1, out, verify)


def main():
    ap = argparse.ArgumentParser(description="Read-only NeoFS v1 inspector")
    ap.add_argument("target", help="block device or image file (opened READ-ONLY)")
    ap.add_argument("--verify", action="store_true",
                    help="recompute BLAKE3 over each file's bytes and compare "
                         "against the hash the inode records (reads all data)")
    ap.add_argument("--force-slow", action="store_true",
                    help="verify large files even on the pure-Python backend")
    ap.add_argument("--extract", metavar="NAME=PATH",
                    help="write one root-level file's logical bytes to PATH")
    args = ap.parse_args()
    global FORCE_SLOW
    FORCE_SLOW = args.force_slow

    dev = Dev(args.target)
    print(f"target: {args.target}  (opened read-only)")
    print(f"blake3: {B3_SOURCE}")
    if not B3_NATIVE:
        print("        metadata checks are instant; large-file content checks are "
              "skipped unless --force-slow")
    print()

    print("=== superblock copies ===")
    sbs = read_superblocks(dev)
    for off, sb, note in sbs:
        if sb is None:
            print(f"  @{off:>10}  ---  {note}")
            continue
        print(f"  @{off:>10}  gen={sb['generation']:<4} total_blocks={sb['total_blocks']:<10} "
              f"root_inode_block={sb['root_inode_block']:<8} {note}")
    valid = [sb for _, sb, n in sbs if sb and "MISMATCH" not in n]
    if not valid:
        print("\nNO VALID SUPERBLOCK -- stopping.")
        return 1
    sel = max(valid, key=lambda s: s["generation"])
    total_bytes = sel["total_blocks"] * sel["block_size"]
    print(f"\nselected: generation={sel['generation']} @offset {sel['offset']}")
    print(f"  block_size          = {sel['block_size']}")
    print(f"  total_blocks        = {sel['total_blocks']}")
    print(f"  FILESYSTEM EXTENT   = {total_bytes} bytes ({total_bytes / 2**20:.1f} MiB)")
    print(f"  next_free_block     = {sel['next_free_block']} "
          f"({sel['next_free_block'] * sel['block_size'] / 2**20:.1f} MiB used)")
    print(f"  root_inode_block    = {sel['root_inode_block']}")
    fm_ok, fm_note = verify_free_map(dev, sel)
    print(f"  free_map            = {fm_note}"
          + (f" ({sel['free_map_block_count']} blocks @ {sel['free_map_start_block']})"
             if sel['free_map_block_count'] else ""))

    root = read_inode(dev, sel["root_inode_block"], sel["total_blocks"])
    h = NEW_B3()
    h.update(dev.block(sel["root_inode_block"])[:INO_SIZE])
    ok = h.digest() == sel["root_inode_hash"]
    print(f"  root_inode_hash     = {'MATCHES' if ok else 'MISMATCH'}")

    print("\n=== directory tree ===")
    out = []
    walk(dev, root, sel["total_blocks"], "", 0, out, args.verify)
    for row in out:
        if row[1] == "ERROR":
            print(f"  {row[0]:<32} ERROR: {row[2]}")
            continue
        path, ino, status, content = row
        print(f"  {path:<32} {ino['kind']:<10} size={ino['file_size']:<12} "
              f"blocks={ino['block_count']:<6} {status}; {content}")
        print(f"  {'':<32}   recorded content_hash = {ino['content_hash'].hex()}")

    if args.extract:
        name, _, dest = args.extract.partition("=")
        for e in read_dirents(dev, root):
            if e["name"] == name:
                ino = read_inode(dev, e["child_block"], sel["total_blocks"])
                left = ino["file_size"]
                with open(dest, "wb") as fh:
                    for blk in logical_block_list(dev, ino):
                        c = dev.block(blk)
                        fh.write(c[:left] if left < len(c) else c)
                        left -= min(left, len(c))
                        if left <= 0:
                            break
                print(f"\nextracted {name} -> {dest} ({ino['file_size']} bytes)")
                break
        else:
            print(f"\nno root entry named {name!r}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
