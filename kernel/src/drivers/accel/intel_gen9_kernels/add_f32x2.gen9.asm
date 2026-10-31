// SPDX-License-Identifier: CC-BY-4.0
// Independent byte oracle for ADD_F32X2_KERNEL_GEN9 (stateless A64 build).
// Produced by `ocloc disasm` (igc-1.0.17193.4 / CR 24.26.30049.6) from
// add_f32x2.cl compiled with -cl-intel-greater-than-4GB-buffer-required; see
// README.md. Shows the real elementwise f32 add via A64 stateless messages
// (`a64 dword gathering read` / `a64 dword scattering write`, BTI 0xFF — no
// `untyped surface` / BTI surface sends) terminating in a `send ... {EOT}`.
L0:
(W)     mov (8|M0)               r3.0<1>:ud    r0.0<1;1,0>:ud
(W)     or (1|M0)                cr0.0<1>:ud   cr0.0<0;1,0>:ud   0x4C0:uw              {Switch}
(W)     mul (1|M0)               r4.0<1>:d     r9.0<0;1,0>:d     r3.1<0;1,0>:d    {Compacted}
(W)     mov (8|M0)               r127.0<1>:ud  r3.0<8;8,1>:ud                   {Compacted}
        add (16|M0)              r5.0<1>:d     r4.0<0;1,0>:d     r1.0<16;16,1>:uw
        add (16|M16)             r10.0<1>:d    r4.0<0;1,0>:d     r2.0<16;16,1>:uw
        add (16|M0)              r5.0<1>:d     r5.0<8;8,1>:d     r7.0<0;1,0>:d    {Compacted}
        add (16|M16)             r10.0<1>:d    r10.0<8;8,1>:d    r7.0<0;1,0>:d   
        shl (8|M0)               r44.0<1>:q    r5.0<8;8,1>:d     2:w              
        shl (8|M8)               r46.0<1>:q    r6.0<8;8,1>:d     2:w              
        shl (8|M16)              r48.0<1>:q    r10.0<8;8,1>:d    2:w              
        shl (8|M24)              r50.0<1>:q    r11.0<8;8,1>:d    2:w              
        add (8|M0)               r12.0<1>:q    r44.0<4;4,1>:q    r8.0<0;1,0>:q   
        add (8|M0)               r24.0<1>:q    r44.0<4;4,1>:q    r8.1<0;1,0>:q   
        add (8|M8)               r14.0<1>:q    r46.0<4;4,1>:q    r8.0<0;1,0>:q   
        add (8|M8)               r26.0<1>:q    r46.0<4;4,1>:q    r8.1<0;1,0>:q   
        add (8|M16)              r16.0<1>:q    r48.0<4;4,1>:q    r8.0<0;1,0>:q   
        add (8|M16)              r28.0<1>:q    r48.0<4;4,1>:q    r8.1<0;1,0>:q   
        add (8|M24)              r18.0<1>:q    r50.0<4;4,1>:q    r8.0<0;1,0>:q   
        add (8|M24)              r30.0<1>:q    r50.0<4;4,1>:q    r8.1<0;1,0>:q   
        add (8|M0)               r36.0<1>:q    r44.0<4;4,1>:q    r8.2<0;1,0>:q   
        send (16|M0)             r20:w    r12:uq  0xC            0x082411FF           // wr:4+0, rd:2; hdc.dc1; a64 dword gathering read
        add (8|M8)               r38.0<1>:q    r46.0<4;4,1>:q    r8.2<0;1,0>:q   
        send (16|M0)             r32:w    r24:uq  0xC            0x082411FF           // wr:4+0, rd:2; hdc.dc1; a64 dword gathering read
        send (16|M16)            r22:w    r16:uq  0xC            0x082411FF           // wr:4+0, rd:2; hdc.dc1; a64 dword gathering read
        add (8|M16)              r40.0<1>:q    r48.0<4;4,1>:q    r8.2<0;1,0>:q   
        send (16|M16)            r34:w    r28:uq  0xC            0x082411FF           // wr:4+0, rd:2; hdc.dc1; a64 dword gathering read
        add (8|M24)              r42.0<1>:q    r50.0<4;4,1>:q    r8.2<0;1,0>:q   
        add (16|M0)              r20.0<1>:f    r20.0<8;8,1>:f    r32.0<8;8,1>:f   {Compacted}
        add (16|M16)             r22.0<1>:f    r22.0<8;8,1>:f    r34.0<8;8,1>:f  
        sends (16|M0)            null:w   r36     r20     0x8C            0x080691FF           // wr:4+2, rd:0; hdc.dc1; a64 dword scattering write
        sends (16|M16)           null:w   r40     r22     0x8C            0x080691FF           // wr:4+2, rd:0; hdc.dc1; a64 dword scattering write
(W)     send (1|M0)              null     r127    0x27            0x02000010           {EOT} // wr:1+0, rd:0; spawner; end of thread
L496:
        illegal                
        illegal                
        illegal                
        illegal                
        illegal                
        illegal                
        illegal                
        illegal                
        illegal                
