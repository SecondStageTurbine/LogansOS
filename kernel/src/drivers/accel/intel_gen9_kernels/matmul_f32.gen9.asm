// SPDX-License-Identifier: CC-BY-4.0
L0:
(W)     mov (8|M0)               r3.0<1>:ud    r0.0<1;1,0>:ud                  
(W)     or (1|M0)                cr0.0<1>:ud   cr0.0<0;1,0>:ud   0x4C0:uw              {Switch}
(W)     mul (1|M0)               r126.0<1>:d   r9.1<0;1,0>:d     r3.1<0;1,0>:d   
(W)     mul (1|M0)               r6.0<1>:d     r8.6<0;1,0>:d     r8.7<0;1,0>:d   
        add (16|M0)              r4.0<1>:d     r126.0<0;1,0>:d   r1.0<16;16,1>:uw
        add (16|M16)             r124.0<1>:d   r126.0<0;1,0>:d   r2.0<16;16,1>:uw
        add (16|M0)              r10.0<1>:d    r4.0<8;8,1>:d     r7.0<0;1,0>:d    {Compacted}
        add (16|M16)             r122.0<1>:d   r124.0<8;8,1>:d   r7.0<0;1,0>:d   
        cmp (16|M0)   (lt)f0.0   null<1>:d     r10.0<8;8,1>:d    r6.0<0;1,0>:d    {Compacted}
        cmp (16|M16)  (lt)f0.0   null<1>:d     r122.0<8;8,1>:d   r6.0<0;1,0>:d   
(f0.0)  if (32|M0)                           L1040                  L1040                
L160:
(W)     cmp (16|M0)   (gt)f1.0   null<1>:d     r9.0<0;1,0>:d     0:w              
(W)     cmp (16|M16)  (gt)f1.0   null<1>:d     r9.0<0;1,0>:d     0:w              
(W&f1.0) jmpi                                L256                                
L208:
        mov (16|M0)              r120.0<1>:f   0.0:f                              
        mov (16|M16)             r12.0<1>:f    0.0:f                              
(W)     jmpi                                 L880                                
L256:
        math.iqot (8|M0)         r118.0<1>:d   r10.0<8;8,1>:d    r8.7<0;1,0>:d    {Compacted}
        math.iqot (8|M8)         r119.0<1>:d   r11.0<8;8,1>:d    r8.7<0;1,0>:d    {Compacted}
        math.iqot (8|M16)        r14.0<1>:d    r122.0<8;8,1>:d   r8.7<0;1,0>:d   
        math.iqot (8|M24)        r15.0<1>:d    r123.0<8;8,1>:d   r8.7<0;1,0>:d   
        math.irem (8|M0)         r116.0<1>:d   r10.0<8;8,1>:d    r8.7<0;1,0>:d    {Compacted}
        math.irem (8|M8)         r117.0<1>:d   r11.0<8;8,1>:d    r8.7<0;1,0>:d    {Compacted}
        mov (16|M0)              r120.0<1>:f   0.0:f                              
        math.irem (8|M16)        r16.0<1>:d    r122.0<8;8,1>:d   r8.7<0;1,0>:d   
        mov (16|M16)             r12.0<1>:f    0.0:f                              
        math.irem (8|M24)        r17.0<1>:d    r123.0<8;8,1>:d   r8.7<0;1,0>:d   
        mul (16|M0)              r114.0<1>:d   r118.0<8;8,1>:d   r9.0<0;1,0>:d    {Compacted}
        mul (16|M16)             r18.0<1>:d    r14.0<8;8,1>:d    r9.0<0;1,0>:d   
(W)     mov (1|M0)               r126.1<1>:d   0:w                              
L424:
(W)     mul (1|M0)               r126.2<1>:d   r126.1<0;1,0>:d   r8.7<0;1,0>:d   
        add (16|M0)              r20.0<1>:d    r114.0<8;8,1>:d   r126.1<0;1,0>:d  {Compacted}
        add (16|M16)             r112.0<1>:d   r18.0<8;8,1>:d    r126.1<0;1,0>:d 
(W)     add (1|M0)               r126.1<1>:d   r126.1<0;1,0>:d   1:w              
        add (16|M0)              r30.0<1>:d    r126.2<0;1,0>:d   r116.0<8;8,1>:d  {Compacted}
        add (16|M16)             r106.0<1>:d   r126.2<0;1,0>:d   r16.0<8;8,1>:d  
        shl (8|M0)               r40.0<1>:q    r20.0<8;8,1>:d    2:w              
        shl (8|M8)               r91.0<1>:q    r21.0<8;8,1>:d    2:w              
        shl (8|M16)              r42.0<1>:q    r112.0<8;8,1>:d   2:w              
        shl (8|M24)              r89.0<1>:q    r113.0<8;8,1>:d   2:w              
        shl (8|M0)               r44.0<1>:q    r30.0<8;8,1>:d    2:w              
        shl (8|M8)               r87.0<1>:q    r31.0<8;8,1>:d    2:w              
        shl (8|M16)              r46.0<1>:q    r106.0<8;8,1>:d   2:w              
        shl (8|M24)              r85.0<1>:q    r107.0<8;8,1>:d   2:w              
        add (8|M0)               r22.0<1>:q    r40.0<4;4,1>:q    r8.0<0;1,0>:q   
        add (8|M8)               r24.0<1>:q    r91.0<4;4,1>:q    r8.0<0;1,0>:q   
        add (8|M16)              r108.0<1>:q   r42.0<4;4,1>:q    r8.0<0;1,0>:q   
        add (8|M24)              r110.0<1>:q   r89.0<4;4,1>:q    r8.0<0;1,0>:q   
        add (8|M0)               r32.0<1>:q    r44.0<4;4,1>:q    r8.1<0;1,0>:q   
        add (8|M8)               r34.0<1>:q    r87.0<4;4,1>:q    r8.1<0;1,0>:q   
        add (8|M16)              r102.0<1>:q   r46.0<4;4,1>:q    r8.1<0;1,0>:q   
        add (8|M24)              r104.0<1>:q   r85.0<4;4,1>:q    r8.1<0;1,0>:q   
        send (16|M0)             r26:w    r22:uq  0xC            0x082411FF           // wr:4+0, rd:2; hdc.dc1; a64 dword gathering read
(W)     cmp (16|M0)   (lt)f0.0   null<1>:d     r126.1<0;1,0>:d   r9.0<0;1,0>:d    {Compacted}
        send (16|M16)            r28:w    r108:uq  0xC           0x082411FF           // wr:4+0, rd:2; hdc.dc1; a64 dword gathering read
        send (16|M0)             r99:w    r32:uq  0xC            0x082411FF           // wr:4+0, rd:2; hdc.dc1; a64 dword gathering read
        send (16|M16)            r97:w    r102:uq  0xC           0x082411FF           // wr:4+0, rd:2; hdc.dc1; a64 dword gathering read
(W)     cmp (16|M16)  (lt)f0.0   null<1>:d     r126.1<0;1,0>:d   r9.0<0;1,0>:d   
        mad (16|M0)              r120.0<1>:f   r120.0<2;1>:f     r26.0<2;1>:f      r99.0<1>:f       {Compacted}
        mad (16|M16)             r12.0<1>:f    r12.0<2;1>:f      r28.0<2;1>:f      r97.0<1>:f       {Compacted}
(W&f0.0) jmpi                                L424                                
L880:
        shl (8|M0)               r48.0<1>:q    r10.0<8;8,1>:d    2:w              
        shl (8|M8)               r83.0<1>:q    r11.0<8;8,1>:d    2:w              
        shl (8|M16)              r50.0<1>:q    r122.0<8;8,1>:d   2:w              
        shl (8|M24)              r81.0<1>:q    r123.0<8;8,1>:d   2:w              
        add (8|M0)               r36.0<1>:q    r48.0<4;4,1>:q    r8.2<0;1,0>:q   
        add (8|M8)               r38.0<1>:q    r83.0<4;4,1>:q    r8.2<0;1,0>:q   
        add (8|M16)              r93.0<1>:q    r50.0<4;4,1>:q    r8.2<0;1,0>:q   
        add (8|M24)              r95.0<1>:q    r81.0<4;4,1>:q    r8.2<0;1,0>:q   
        sends (16|M0)            null:w   r36     r120    0x8C            0x080691FF           // wr:4+2, rd:0; hdc.dc1; a64 dword scattering write
        sends (16|M16)           null:w   r93     r12     0x8C            0x080691FF           // wr:4+2, rd:0; hdc.dc1; a64 dword scattering write
L1040:
        endif (32|M0)                        L1056                                
L1056:
(W)     mov (8|M0)               r127.0<1>:ud  r3.0<8;8,1>:ud                   {Compacted}
(W)     send (1|M0)              null     r127    0x27            0x02000010           {EOT} // wr:1+0, rd:0; spawner; end of thread
L1080:
        illegal                
        illegal                
        illegal                
        illegal                
        illegal                
        illegal                
        illegal                
        illegal                
