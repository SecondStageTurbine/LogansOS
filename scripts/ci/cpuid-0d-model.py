#!/usr/bin/env python3
# SPDX-License-Identifier: MPL-2.0
"""An emulator-side CPUID model for the product-kernel XSAVE witness.

QEMU under Windows Hypervisor Platform (WHPX) answered CPUID.(EAX=0DH,ECX=0)
with EBX = 576 (the x87|SSE area) after the guest had enabled AVX in XCR0
(the preview, 2026-11-02; #52). TCG answers correctly (832), and QEMU has no
option to change one CPUID output. This script makes the emulator answer the
way WHPX did, without any hook in the kernel: it attaches to QEMU's gdbstub,
puts a breakpoint on every `cpuid` instruction in the kernel's text, and after
each CPUID.(0DH,0) on any vCPU sets EBX to 576 before the guest reads it.
Every other leaf, and every other register, is left as the CPU computed it.

The kernel must run at its link address (`iso --product-kernel --no-kaslr`),
so the ELF's `cpuid` addresses are the run-time ones. QEMU is started with
`-S -gdb tcp::PORT`; this script resumes it and exits when QEMU does.

Usage: cpuid-0d-model.py PORT ADDRESS_FILE LOG_FILE
  ADDRESS_FILE: one hex address per line (`objdump -d` of the kernel ELF).
"""

import socket
import sys
import time

EBX_576 = 576
REG_RAX, REG_RBX, REG_RCX, REG_RIP = 0, 1, 2, 16


class Rsp:
    """A minimal GDB remote serial protocol client (no-ack mode)."""

    def __init__(self, port):
        deadline = time.time() + 30
        while True:
            try:
                self.sock = socket.create_connection(("127.0.0.1", port))
                break
            except OSError:
                if time.time() > deadline:
                    raise
                time.sleep(0.2)
        self.buf = b""
        self.ack = True

    def _send(self, payload):
        data = payload.encode()
        csum = sum(data) & 0xFF
        self.sock.sendall(b"$" + data + b"#" + b"%02x" % csum)

    def _recv_packet(self):
        while True:
            start = self.buf.find(b"$")
            if start >= 0:
                end = self.buf.find(b"#", start)
                if end >= 0 and len(self.buf) >= end + 3:
                    payload = self.buf[start + 1 : end]
                    self.buf = self.buf[end + 3 :]
                    if self.ack:
                        self.sock.sendall(b"+")
                    return payload.decode()
            chunk = self.sock.recv(65536)
            if not chunk:
                raise EOFError("gdbstub closed")
            self.buf += chunk

    def cmd(self, payload):
        self._send(payload)
        return self._recv_packet()

    def no_ack(self):
        if self.cmd("QStartNoAckMode") == "OK":
            self.ack = False
            # Drop the '+' acks QEMU sent before the mode switch.
            self.buf = self.buf.replace(b"+", b"")

    def read_reg(self, n):
        raw = bytes.fromhex(self.cmd("p%x" % n))
        return int.from_bytes(raw, "little")

    def write_reg(self, n, value):
        reply = self.cmd("P%x=%s" % (n, value.to_bytes(8, "little").hex()))
        if reply != "OK":
            raise RuntimeError("register write P%x failed: %r" % (n, reply))


def stop_thread(reply):
    """The thread id in a T stop reply, or None (W/X: QEMU exited)."""
    if not reply.startswith("T"):
        return None
    for field in reply[3:].split(";"):
        if field.startswith("thread:"):
            return field[len("thread:") :]
    raise RuntimeError("stop reply without a thread: %r" % reply)


def main():
    port, address_file, log_file = int(sys.argv[1]), sys.argv[2], sys.argv[3]
    with open(address_file) as f:
        addrs = sorted({int(line, 16) for line in f if line.strip()})
    log = open(log_file, "w", buffering=1)
    log.write("cpuid-0d-model: %d kernel cpuid sites\n" % len(addrs))

    rsp = Rsp(port)
    rsp.no_ack()
    rsp.cmd("?")
    for a in addrs:
        reply = rsp.cmd("Z0,%x,1" % a)
        if reply != "OK":
            raise RuntimeError("breakpoint at %#x refused: %r" % (a, reply))

    hits = 0
    overrides = {}
    reply = rsp.cmd("c")
    while True:
        thread = stop_thread(reply)
        if thread is None:
            log.write("cpuid-0d-model: QEMU stopped (%s)\n" % reply)
            break
        rsp.cmd("Hg" + thread)
        rip = rsp.read_reg(REG_RIP)
        if rip not in addrs:
            # Not ours (e.g. a stop with no breakpoint): carry on.
            reply = rsp.cmd("c")
            continue
        hits += 1
        leaf = rsp.read_reg(REG_RAX) & 0xFFFFFFFF
        subleaf = rsp.read_reg(REG_RCX) & 0xFFFFFFFF
        # Step this vCPU over the cpuid (the others stay stopped), then
        # rewrite EBX before anything reads it.
        rsp.cmd("z0,%x,1" % rip)
        step = rsp.cmd("vCont;s:" + thread)
        if stop_thread(step) is None:
            log.write("cpuid-0d-model: QEMU stopped mid-step (%s)\n" % step)
            break
        rsp.cmd("Z0,%x,1" % rip)
        rsp.cmd("Hg" + thread)
        if leaf == 0xD and subleaf == 0:
            ebx = rsp.read_reg(REG_RBX) & 0xFFFFFFFF
            rsp.write_reg(REG_RBX, EBX_576)
            overrides[thread] = overrides.get(thread, 0) + 1
            log.write(
                "cpuid-0d-model: vcpu thread %s rip=%#x CPUID.(0DH,0):EBX %d -> %d\n"
                % (thread, rip, ebx, EBX_576)
            )
        reply = rsp.cmd("c")

    log.write(
        "cpuid-0d-model: done hits=%d overrides=%d vcpus=%d\n"
        % (hits, sum(overrides.values()), len(overrides))
    )


if __name__ == "__main__":
    try:
        main()
    except (EOFError, ConnectionError) as e:
        print("cpuid-0d-model: gdbstub closed: %s" % e, file=sys.stderr)
