// SPDX-License-Identifier: CC-BY-4.0
L0:
(W)     mov (8|M0)               r32.0<1>:ud   r0.0<1;1,0>:ud                  
(W)     or (1|M0)                cr0.0<1>:ud   cr0.0<0;1,0>:ud   0x4C0:uw              {Switch}
(W)     mul (1|M0)               r5.0<1>:d     r9.3<0;1,0>:d     r32.1<0;1,0>:d   {Compacted}
(W)     mul (1|M0)               r6.0<1>:d     r9.4<0;1,0>:d     r32.6<0;1,0>:d  
        add (16|M16)             r12.0<1>:d    r5.0<0;1,0>:d     r2.0<16;16,1>:uw
        add (16|M16)             r16.0<1>:d    r6.0<0;1,0>:d     r4.0<16;16,1>:uw
        add (16|M0)              r14.0<1>:d    r6.0<0;1,0>:d     r3.0<16;16,1>:uw
        add (16|M0)              r10.0<1>:d    r5.0<0;1,0>:d     r1.0<16;16,1>:uw
        add (16|M16)             r36.0<1>:d    r12.0<8;8,1>:d    r7.0<0;1,0>:d   
        add (16|M16)             r40.0<1>:d    r16.0<8;8,1>:d    r7.1<0;1,0>:d   
(W)     mov (1|M0)               r5.0<1>:uw    0x1:uw                             
        add (16|M0)              r38.0<1>:d    r14.0<8;8,1>:d    r7.1<0;1,0>:d    {Compacted}
        cmp (16|M16)  (lt)f1.0   null<1>:d     r36.0<8;8,1>:ud   r8.6<0;1,0>:ud  
        cmp (16|M16)  (lt)f0.0   null<1>:d     r40.0<8;8,1>:ud   r8.7<0;1,0>:ud  
        add (16|M0)              r34.0<1>:d    r10.0<8;8,1>:d    r7.0<0;1,0>:d    {Compacted}
        cmp (16|M0)   (lt)f1.0   null<1>:d     r38.0<8;8,1>:ud   r8.7<0;1,0>:ud  
(f1.0)  sel (16|M16)             r10.0<1>:uw   r5.0<0;1,0>:uw    0x0:uw             
(f0.0)  sel (16|M16)             r11.0<1>:uw   r5.0<0;1,0>:uw    0x0:uw             
(f1.0)  cmp (16|M0)   (lt)f1.0   null<1>:d     r34.0<8;8,1>:ud   r8.6<0;1,0>:ud  
        and (16|M16)  (ne)f1.0   null<1>:uw    r10.0<16;16,1>:uw  r11.0<16;16,1>:uw
(f1.0)  if (32|M0)                           L4560                  L4560                
L312:
        mul (16|M0)              r42.0<1>:d    r38.0<8;8,1>:d    r9.0<0;1,0>:d    {Compacted}
        mul (16|M16)             r44.0<1>:d    r40.0<8;8,1>:d    r9.0<0;1,0>:d   
        shl (16|M0)              r46.0<1>:d    r34.0<8;8,1>:d    2:w              
        shl (16|M16)             r48.0<1>:d    r36.0<8;8,1>:d    2:w              
(W)     cmp (16|M0)   (lt)f0.0   null<1>:d     r9.1<0;1,0>:d     0:w              
(W)     cmp (16|M16)  (lt)f0.0   null<1>:d     r9.1<0;1,0>:d     0:w              
        add (16|M0)              r50.0<1>:d    r42.0<8;8,1>:d    r46.0<8;8,1>:d   {Compacted}
        add (16|M16)             r52.0<1>:d    r44.0<8;8,1>:d    r48.0<8;8,1>:d  
(W&f0.0) jmpi                                L3968                                
L440:
(W)     cmp (16|M0)   (eq)f0.0   null<1>:d     r9.2<0;1,0>:d     0:w              
        mov (16|M0)              r54.0<1>:d    0:w                              
        mov (16|M16)             r56.0<1>:d    0:w                              
(W)     shl (1|M0)               r9.6<1>:d     r9.1<0;1,0>:d     1:w              
(W)     mov (1|M0)               r33.7<1>:ud   f0.0<0;1,0>:ud                  
(W)     add (1|M0)               r9.7<1>:d     r8.7<0;1,0>:d     -1:w              
(W)     add (1|M0)               r33.0<1>:d    r8.6<0;1,0>:d     1073741823:d              
(W)     mov (1|M0)               r33.1<1>:d    0:w                              
(W)     mov (1|M0)               f1.0<1>:ud    r33.7<0;1,0>:ud                 
(W)     cmp (16|M16)  (eq)f1.0   null<1>:d     r9.2<0;1,0>:d     0:w              
(W)     mov (1|M0)               r33.7<1>:ud   f1.0<0;1,0>:ud                  
L616:
(W)     mov (1|M0)               f0.0<1>:ud    r33.7<0;1,0>:ud                 
(W)     add (1|M0)               r33.2<1>:d    r33.1<0;1,0>:d    -r9.1<0;1,0>:d   {Compacted}
(W&f0.0) jmpi                                L840                                
L656:
        add (16|M0)   (lt)f0.0   r10.0<1>:d    r38.0<8;8,1>:d    r33.2<0;1,0>:d   {Compacted}
        add (16|M16)  (lt)f0.0   r12.0<1>:d    r40.0<8;8,1>:d    r33.2<0;1,0>:d  
        cmp (16|M0)   (lt)f1.0   null<1>:d     r10.0<8;8,1>:d    r8.7<0;1,0>:d   
        cmp (16|M16)  (lt)f1.0   null<1>:d     r12.0<8;8,1>:d    r8.7<0;1,0>:d   
(f1.0)  sel (16|M0)              r14.0<1>:d    r10.0<8;8,1>:d    r9.7<0;1,0>:d   
(f1.0)  sel (16|M16)             r16.0<1>:d    r12.0<8;8,1>:d    r9.7<0;1,0>:d   
(f0.0)  mov (16|M0)              r14.0<1>:d    0:w                              
(f0.0)  mov (16|M16)             r16.0<1>:d    0:w                              
        mul (16|M0)              r14.0<1>:d    r14.0<8;8,1>:d    r9.0<0;1,0>:d    {Compacted}
        mul (16|M16)             r16.0<1>:d    r16.0<8;8,1>:d    r9.0<0;1,0>:d   
        add (16|M0)              r58.0<1>:d    r14.0<8;8,1>:d    r46.0<8;8,1>:d   {Compacted}
        add (16|M16)             r60.0<1>:d    r16.0<8;8,1>:d    r48.0<8;8,1>:d  
(W)     jmpi                                 L1016                                
L840:
        add (16|M0)   (lt)f0.0   r10.0<1>:d    r34.0<8;8,1>:d    r33.2<0;1,0>:d   {Compacted}
        add (16|M16)  (lt)f0.0   r12.0<1>:d    r36.0<8;8,1>:d    r33.2<0;1,0>:d  
        cmp (16|M0)   (lt)f1.0   null<1>:d     r10.0<8;8,1>:d    r8.6<0;1,0>:d   
        cmp (16|M16)  (lt)f1.0   null<1>:d     r12.0<8;8,1>:d    r8.6<0;1,0>:d   
(f1.0)  sel (16|M0)              r14.0<1>:d    r10.0<8;8,1>:d    r33.0<0;1,0>:d  
(f1.0)  sel (16|M16)             r16.0<1>:d    r12.0<8;8,1>:d    r33.0<0;1,0>:d  
        shl (16|M0)              r14.0<1>:d    r14.0<8;8,1>:d    2:w              
        shl (16|M16)             r16.0<1>:d    r16.0<8;8,1>:d    2:w              
(f0.0)  mov (16|M0)              r14.0<1>:d    0:w                              
(f0.0)  mov (16|M16)             r16.0<1>:d    0:w                              
        add (16|M0)              r58.0<1>:d    r42.0<8;8,1>:d    r14.0<8;8,1>:d   {Compacted}
        add (16|M16)             r60.0<1>:d    r44.0<8;8,1>:d    r16.0<8;8,1>:d  
L1016:
(W)     shl (1|M0)               r5.0<1>:q     r33.1<0;1,0>:ud   2:w              
        add (8|M0)               r10.0<1>:q    r8.0<0;1,0>:q     r58.0<8;8,1>:ud 
        add (8|M8)               r12.0<1>:q    r8.0<0;1,0>:q     r59.0<8;8,1>:ud 
        add (8|M16)              r14.0<1>:q    r8.0<0;1,0>:q     r60.0<8;8,1>:ud 
        add (8|M24)              r16.0<1>:q    r8.0<0;1,0>:q     r61.0<8;8,1>:ud 
(W)     cmp (16|M0)   (lt)f0.0   null<1>:d     r33.1<0;1,0>:d    r9.6<0;1,0>:d   
(W)     add (1|M0)               r6.0<1>:q     r5.0<0;1,0>:q     r8.2<0;1,0>:q   
(W)     cmp (16|M16)  (lt)f0.0   null<1>:d     r33.1<0;1,0>:d    r9.6<0;1,0>:d   
        send (16|M0)             r20:w    r10:uq  0xC            0x082410FF           // wr:4+0, rd:2; hdc.dc1; a64 byte gathering read 8b
        send (16|M16)            r22:w    r14:uq  0xC            0x082410FF           // wr:4+0, rd:2; hdc.dc1; a64 byte gathering read 8b
(W)     send (1|M0)              r18      r6:uq   0xC            0x041401FF           // wr:2+0, rd:1; hdc.dc1; a64 dword gathering read
        mul (16|M0)              r24.0<1>:d    r18.0<0;1,0>:d    r20.0<4;1,0>:ub 
        mul (16|M16)             r26.0<1>:d    r18.0<0;1,0>:d    r22.0<4;1,0>:ub 
        add (16|M0)              r54.0<1>:d    r54.0<8;8,1>:d    r24.0<8;8,1>:d   {Compacted}
        add (16|M16)             r56.0<1>:d    r56.0<8;8,1>:d    r26.0<8;8,1>:d  
(~f0.0) break (32|M0)                        L1280                  L1280                
L1264:
(W)     add (1|M0)               r33.1<1>:d    r33.1<0;1,0>:d    1:w              
L1280:
        while (32|M0)                        L616                                
L1296:
        add (16|M0)              r10.0<1>:d    r54.0<8;8,1>:d    0x8000:uw             
        add (16|M16)             r12.0<1>:d    r56.0<8;8,1>:d    0x8000:uw             
        cmp (16|M0)   (gt)f0.0   null<1>:d     r54.0<8;8,1>:d    16744447:d              
        cmp (16|M16)  (gt)f0.0   null<1>:d     r56.0<8;8,1>:d    16744447:d              
        shr (16|M0)              r10.0<1>:ud   r10.0<8;8,1>:ud   16:w              
        cmp (16|M0)   (lt)f1.0   null<1>:d     r54.0<8;8,1>:d    -32768:w              
        shr (16|M16)             r12.0<1>:ud   r12.0<8;8,1>:ud   16:w              
        cmp (16|M16)  (lt)f1.0   null<1>:d     r56.0<8;8,1>:d    -32768:w              
(~f0.0) sel (16|M0)              r5.0<2>:b     r10.0<4;1,0>:b    -1:w              
        add (8|M0)               r14.0<1>:q    r8.1<0;1,0>:q     r50.0<8;8,1>:ud 
(f1.0)  mov (16|M0)              r5.0<2>:b     0:w                              
(~f0.0) sel (16|M16)             r6.0<2>:b     r12.0<4;1,0>:b    -1:w              
        add (8|M8)               r16.0<1>:q    r8.1<0;1,0>:q     r51.0<8;8,1>:ud 
(f1.0)  mov (16|M16)             r6.0<2>:b     0:w                              
        mov (16|M0)              r22.0<1>:ud   r5.0<2;1,0>:ub                  
        add (8|M16)              r18.0<1>:q    r8.1<0;1,0>:q     r52.0<8;8,1>:ud 
        add (8|M24)              r20.0<1>:q    r8.1<0;1,0>:q     r53.0<8;8,1>:ud 
(W)     mov (1|M0)               f1.0<1>:ud    0x0:uw                             
        mov (16|M16)             r10.0<1>:ud   r6.0<2;1,0>:ub                  
        sends (16|M0)            null:w   r14     r22     0x8C            0x080690FF           // wr:4+2, rd:0; hdc.dc1; a64 byte scattering write 8b
        sends (16|M16)           null:w   r18     r10     0x8C            0x080690FF           // wr:4+2, rd:0; hdc.dc1; a64 byte scattering write 8b
(~f1.0) goto (32|M0)                         L4096                  L4096                
L1648:
        mov (16|M0)              r62.0<1>:d    0:w                              
        mov (16|M16)             r64.0<1>:d    0:w                              
(W)     mov (1|M0)               r33.3<1>:d    0:w                              
L1696:
(W)     mov (1|M0)               f1.0<1>:ud    r33.7<0;1,0>:ud                 
(W)     add (1|M0)               r33.4<1>:d    r33.3<0;1,0>:d    -r9.1<0;1,0>:d  
(W&f1.0) jmpi                                L1928                                
L1744:
        add (16|M0)   (lt)f0.0   r10.0<1>:d    r38.0<8;8,1>:d    r33.4<0;1,0>:d   {Compacted}
        add (16|M16)  (lt)f0.0   r12.0<1>:d    r40.0<8;8,1>:d    r33.4<0;1,0>:d  
        cmp (16|M0)   (lt)f1.0   null<1>:d     r10.0<8;8,1>:d    r8.7<0;1,0>:d   
        cmp (16|M16)  (lt)f1.0   null<1>:d     r12.0<8;8,1>:d    r8.7<0;1,0>:d   
(f1.0)  sel (16|M0)              r14.0<1>:d    r10.0<8;8,1>:d    r9.7<0;1,0>:d   
(f1.0)  sel (16|M16)             r16.0<1>:d    r12.0<8;8,1>:d    r9.7<0;1,0>:d   
(f0.0)  mov (16|M0)              r14.0<1>:d    0:w                              
(f0.0)  mov (16|M16)             r16.0<1>:d    0:w                              
        mul (16|M0)              r14.0<1>:d    r14.0<8;8,1>:d    r9.0<0;1,0>:d    {Compacted}
        mul (16|M16)             r16.0<1>:d    r16.0<8;8,1>:d    r9.0<0;1,0>:d   
        add (16|M0)              r66.0<1>:d    r14.0<8;8,1>:d    r46.0<8;8,1>:d   {Compacted}
        add (16|M16)             r68.0<1>:d    r16.0<8;8,1>:d    r48.0<8;8,1>:d  
(W)     jmpi                                 L2104                                
L1928:
        add (16|M0)   (lt)f0.0   r10.0<1>:d    r34.0<8;8,1>:d    r33.4<0;1,0>:d   {Compacted}
        add (16|M16)  (lt)f0.0   r12.0<1>:d    r36.0<8;8,1>:d    r33.4<0;1,0>:d  
        cmp (16|M0)   (lt)f1.0   null<1>:d     r10.0<8;8,1>:d    r8.6<0;1,0>:d   
        cmp (16|M16)  (lt)f1.0   null<1>:d     r12.0<8;8,1>:d    r8.6<0;1,0>:d   
(f1.0)  sel (16|M0)              r14.0<1>:d    r10.0<8;8,1>:d    r33.0<0;1,0>:d  
(f1.0)  sel (16|M16)             r16.0<1>:d    r12.0<8;8,1>:d    r33.0<0;1,0>:d  
        shl (16|M0)              r14.0<1>:d    r14.0<8;8,1>:d    2:w              
        shl (16|M16)             r16.0<1>:d    r16.0<8;8,1>:d    2:w              
(f0.0)  mov (16|M0)              r14.0<1>:d    0:w                              
(f0.0)  mov (16|M16)             r16.0<1>:d    0:w                              
        add (16|M0)              r66.0<1>:d    r42.0<8;8,1>:d    r14.0<8;8,1>:d   {Compacted}
        add (16|M16)             r68.0<1>:d    r44.0<8;8,1>:d    r16.0<8;8,1>:d  
L2104:
        add (16|M0)              r10.0<1>:d    r66.0<8;8,1>:d    1:w              
        add (16|M16)             r12.0<1>:d    r68.0<8;8,1>:d    1:w              
(W)     shl (1|M0)               r5.0<1>:q     r33.3<0;1,0>:ud   2:w              
(W)     cmp (16|M0)   (lt)f1.0   null<1>:d     r33.3<0;1,0>:d    r9.6<0;1,0>:d   
        add (8|M0)               r14.0<1>:q    r8.0<0;1,0>:q     r10.0<8;8,1>:ud 
        add (8|M8)               r16.0<1>:q    r8.0<0;1,0>:q     r11.0<8;8,1>:ud 
        add (8|M16)              r18.0<1>:q    r8.0<0;1,0>:q     r12.0<8;8,1>:ud 
        add (8|M24)              r20.0<1>:q    r8.0<0;1,0>:q     r13.0<8;8,1>:ud 
(W)     add (1|M0)               r6.0<1>:q     r5.0<0;1,0>:q     r8.2<0;1,0>:q   
(W)     cmp (16|M16)  (lt)f1.0   null<1>:d     r33.3<0;1,0>:d    r9.6<0;1,0>:d   
        send (16|M0)             r24:w    r14:uq  0xC            0x082410FF           // wr:4+0, rd:2; hdc.dc1; a64 byte gathering read 8b
        send (16|M16)            r26:w    r18:uq  0xC            0x082410FF           // wr:4+0, rd:2; hdc.dc1; a64 byte gathering read 8b
(W)     send (1|M0)              r22      r6:uq   0xC            0x041401FF           // wr:2+0, rd:1; hdc.dc1; a64 dword gathering read
        mul (16|M0)              r28.0<1>:d    r22.0<0;1,0>:d    r24.0<4;1,0>:ub 
        mul (16|M16)             r30.0<1>:d    r22.0<0;1,0>:d    r26.0<4;1,0>:ub 
        add (16|M0)              r62.0<1>:d    r62.0<8;8,1>:d    r28.0<8;8,1>:d   {Compacted}
        add (16|M16)             r64.0<1>:d    r64.0<8;8,1>:d    r30.0<8;8,1>:d  
(~f1.0) break (32|M0)                        L2400                  L2400                
L2384:
(W)     add (1|M0)               r33.3<1>:d    r33.3<0;1,0>:d    1:w              
L2400:
        while (32|M0)                        L1696                                
L2416:
        add (16|M0)              r10.0<1>:d    r62.0<8;8,1>:d    0x8000:uw             
        add (16|M16)             r12.0<1>:d    r64.0<8;8,1>:d    0x8000:uw             
        cmp (16|M0)   (lt)f0.0   null<1>:d     r62.0<8;8,1>:d    -32768:w              
        cmp (16|M16)  (lt)f0.0   null<1>:d     r64.0<8;8,1>:d    -32768:w              
        shr (16|M0)              r10.0<1>:ud   r10.0<8;8,1>:ud   16:w              
        cmp (16|M0)   (gt)f1.0   null<1>:d     r62.0<8;8,1>:d    16744447:d              
        shr (16|M16)             r12.0<1>:ud   r12.0<8;8,1>:ud   16:w              
        cmp (16|M16)  (gt)f1.0   null<1>:d     r64.0<8;8,1>:d    16744447:d              
(~f0.0) sel (16|M0)              r5.0<2>:b     r10.0<4;1,0>:b    0:w              
        add (16|M0)              r14.0<1>:d    r50.0<8;8,1>:d    1:w              
(f1.0)  mov (16|M0)              r5.0<2>:b     -1:w                              
(~f0.0) sel (16|M16)             r6.0<2>:b     r12.0<4;1,0>:b    0:w              
        add (16|M16)             r16.0<1>:d    r52.0<8;8,1>:d    1:w              
(f1.0)  mov (16|M16)             r6.0<2>:b     -1:w                              
        add (8|M0)               r18.0<1>:q    r8.1<0;1,0>:q     r14.0<8;8,1>:ud 
        add (8|M8)               r20.0<1>:q    r8.1<0;1,0>:q     r15.0<8;8,1>:ud 
        mov (16|M0)              r10.0<1>:ud   r5.0<2;1,0>:ub                  
        add (8|M16)              r22.0<1>:q    r8.1<0;1,0>:q     r16.0<8;8,1>:ud 
        add (8|M24)              r24.0<1>:q    r8.1<0;1,0>:q     r17.0<8;8,1>:ud 
        mov (16|M16)             r12.0<1>:ud   r6.0<2;1,0>:ub                  
(W)     mov (1|M0)               f0.0<1>:ud    0x0:uw                             
        sends (16|M0)            null:w   r18     r10     0x8C            0x080690FF           // wr:4+2, rd:0; hdc.dc1; a64 byte scattering write 8b
        sends (16|M16)           null:w   r22     r12     0x8C            0x080690FF           // wr:4+2, rd:0; hdc.dc1; a64 byte scattering write 8b
(f0.0)  if (32|M0)                           L2864                  L3920                
L2800:
(W)     mov (1|M0)               r5.0<2>:b     0:w                              
        mov (16|M0)              r70.0<1>:b    r5.0<0;1,0>:b                   
        mov (16|M16)             r71.0<1>:b    r5.0<0;1,0>:b                   
        else (32|M0)                         L3920                  L3920                
L2864:
        mov (16|M0)              r72.0<1>:d    0:w                              
        mov (16|M16)             r74.0<1>:d    0:w                              
(W)     mov (1|M0)               r33.5<1>:d    0:w                              
L2912:
(W)     mov (1|M0)               f0.0<1>:ud    r33.7<0;1,0>:ud                 
(W)     add (1|M0)               r33.6<1>:d    r33.5<0;1,0>:d    -r9.1<0;1,0>:d  
(W&f0.0) jmpi                                L3144                                
L2960:
        add (16|M0)   (lt)f0.0   r10.0<1>:d    r38.0<8;8,1>:d    r33.6<0;1,0>:d   {Compacted}
        add (16|M16)  (lt)f0.0   r12.0<1>:d    r40.0<8;8,1>:d    r33.6<0;1,0>:d  
        cmp (16|M0)   (lt)f1.0   null<1>:d     r10.0<8;8,1>:d    r8.7<0;1,0>:d   
        cmp (16|M16)  (lt)f1.0   null<1>:d     r12.0<8;8,1>:d    r8.7<0;1,0>:d   
(f1.0)  sel (16|M0)              r14.0<1>:d    r10.0<8;8,1>:d    r9.7<0;1,0>:d   
(f1.0)  sel (16|M16)             r16.0<1>:d    r12.0<8;8,1>:d    r9.7<0;1,0>:d   
(f0.0)  mov (16|M0)              r14.0<1>:d    0:w                              
(f0.0)  mov (16|M16)             r16.0<1>:d    0:w                              
        mul (16|M0)              r14.0<1>:d    r14.0<8;8,1>:d    r9.0<0;1,0>:d    {Compacted}
        mul (16|M16)             r16.0<1>:d    r16.0<8;8,1>:d    r9.0<0;1,0>:d   
        add (16|M0)              r76.0<1>:d    r14.0<8;8,1>:d    r46.0<8;8,1>:d   {Compacted}
        add (16|M16)             r78.0<1>:d    r16.0<8;8,1>:d    r48.0<8;8,1>:d  
(W)     jmpi                                 L3320                                
L3144:
        add (16|M0)   (lt)f0.0   r10.0<1>:d    r34.0<8;8,1>:d    r33.6<0;1,0>:d   {Compacted}
        add (16|M16)  (lt)f0.0   r12.0<1>:d    r36.0<8;8,1>:d    r33.6<0;1,0>:d  
        cmp (16|M0)   (lt)f1.0   null<1>:d     r10.0<8;8,1>:d    r8.6<0;1,0>:d   
        cmp (16|M16)  (lt)f1.0   null<1>:d     r12.0<8;8,1>:d    r8.6<0;1,0>:d   
(f1.0)  sel (16|M0)              r14.0<1>:d    r10.0<8;8,1>:d    r33.0<0;1,0>:d  
(f1.0)  sel (16|M16)             r16.0<1>:d    r12.0<8;8,1>:d    r33.0<0;1,0>:d  
        shl (16|M0)              r14.0<1>:d    r14.0<8;8,1>:d    2:w              
        shl (16|M16)             r16.0<1>:d    r16.0<8;8,1>:d    2:w              
(f0.0)  mov (16|M0)              r14.0<1>:d    0:w                              
(f0.0)  mov (16|M16)             r16.0<1>:d    0:w                              
        add (16|M0)              r76.0<1>:d    r42.0<8;8,1>:d    r14.0<8;8,1>:d   {Compacted}
        add (16|M16)             r78.0<1>:d    r44.0<8;8,1>:d    r16.0<8;8,1>:d  
L3320:
        add (16|M0)              r10.0<1>:d    r76.0<8;8,1>:d    2:w              
        add (16|M16)             r12.0<1>:d    r78.0<8;8,1>:d    2:w              
(W)     shl (1|M0)               r5.0<1>:q     r33.5<0;1,0>:ud   2:w              
(W)     cmp (16|M0)   (lt)f0.0   null<1>:d     r33.5<0;1,0>:d    r9.6<0;1,0>:d   
        add (8|M0)               r14.0<1>:q    r8.0<0;1,0>:q     r10.0<8;8,1>:ud 
        add (8|M8)               r16.0<1>:q    r8.0<0;1,0>:q     r11.0<8;8,1>:ud 
        add (8|M16)              r18.0<1>:q    r8.0<0;1,0>:q     r12.0<8;8,1>:ud 
        add (8|M24)              r20.0<1>:q    r8.0<0;1,0>:q     r13.0<8;8,1>:ud 
(W)     add (1|M0)               r6.0<1>:q     r5.0<0;1,0>:q     r8.2<0;1,0>:q   
(W)     cmp (16|M16)  (lt)f0.0   null<1>:d     r33.5<0;1,0>:d    r9.6<0;1,0>:d   
        send (16|M0)             r24:w    r14:uq  0xC            0x082410FF           // wr:4+0, rd:2; hdc.dc1; a64 byte gathering read 8b
        send (16|M16)            r26:w    r18:uq  0xC            0x082410FF           // wr:4+0, rd:2; hdc.dc1; a64 byte gathering read 8b
(W)     send (1|M0)              r22      r6:uq   0xC            0x041401FF           // wr:2+0, rd:1; hdc.dc1; a64 dword gathering read
        mul (16|M0)              r28.0<1>:d    r22.0<0;1,0>:d    r24.0<4;1,0>:ub 
        mul (16|M16)             r30.0<1>:d    r22.0<0;1,0>:d    r26.0<4;1,0>:ub 
        add (16|M0)              r72.0<1>:d    r72.0<8;8,1>:d    r28.0<8;8,1>:d   {Compacted}
        add (16|M16)             r74.0<1>:d    r74.0<8;8,1>:d    r30.0<8;8,1>:d  
(~f0.0) break (32|M0)                        L3616                  L3616                
L3600:
(W)     add (1|M0)               r33.5<1>:d    r33.5<0;1,0>:d    1:w              
L3616:
        while (32|M0)                        L2912                                
L3632:
        cmp (16|M0)   (gt)f1.0   null<1>:d     r72.0<8;8,1>:d    16744447:d              
        cmp (16|M16)  (gt)f1.0   null<1>:d     r74.0<8;8,1>:d    16744447:d              
(f1.0)  if (32|M0)                           L3744                  L3904                
L3680:
(W)     mov (1|M0)               r5.0<2>:b     -1:w                              
        mov (16|M0)              r70.0<1>:b    r5.0<0;1,0>:b                   
        mov (16|M16)             r71.0<1>:b    r5.0<0;1,0>:b                   
        else (32|M0)                         L3904                  L3904                
L3744:
        add (16|M0)              r10.0<1>:d    r72.0<8;8,1>:d    0x8000:uw             
        add (16|M16)             r12.0<1>:d    r74.0<8;8,1>:d    0x8000:uw             
        cmp (16|M0)   (lt)f0.0   null<1>:d     r72.0<8;8,1>:d    -32768:w              
        cmp (16|M16)  (lt)f0.0   null<1>:d     r74.0<8;8,1>:d    -32768:w              
        shr (16|M0)              r10.0<1>:ud   r10.0<8;8,1>:ud   16:w              
        shr (16|M16)             r12.0<1>:ud   r12.0<8;8,1>:ud   16:w              
(~f0.0) sel (16|M0)              r5.0<2>:b     r10.0<4;1,0>:b    0:w              
(~f0.0) sel (16|M16)             r6.0<2>:b     r12.0<4;1,0>:b    0:w              
        mov (16|M0)              r70.0<1>:b    r5.0<2;1,0>:b                   
        mov (16|M16)             r71.0<1>:b    r6.0<2;1,0>:b                   
L3904:
        endif (32|M0)                        L3920                                
L3920:
        endif (32|M0)                        L3936                                
L3936:
(W)     mov (1|M0)               f1.0<1>:ud    0x0:uw                             
(f1.0)  goto (32|M0)                         L4096                  L4320                
L3968:
        add (8|M0)               r10.0<1>:q    r8.1<0;1,0>:q     r50.0<8;8,1>:ud 
        add (8|M8)               r12.0<1>:q    r8.1<0;1,0>:q     r51.0<8;8,1>:ud 
        mov (16|M0)              r18.0<1>:ud   0x0:uw                             
        add (8|M16)              r14.0<1>:q    r8.1<0;1,0>:q     r52.0<8;8,1>:ud 
        add (8|M24)              r16.0<1>:q    r8.1<0;1,0>:q     r53.0<8;8,1>:ud 
        mov (16|M16)             r20.0<1>:ud   0x0:uw                             
        sends (16|M0)            null:w   r10     r18     0x8C            0x080690FF           // wr:4+2, rd:0; hdc.dc1; a64 byte scattering write 8b
        sends (16|M16)           null:w   r14     r20     0x8C            0x080690FF           // wr:4+2, rd:0; hdc.dc1; a64 byte scattering write 8b
L4096:
        join (32|M0)                         L4320                                
L4112:
        add (16|M0)              r10.0<1>:d    r50.0<8;8,1>:d    1:w              
        add (16|M16)             r12.0<1>:d    r52.0<8;8,1>:d    1:w              
        mov (16|M0)              r22.0<1>:ud   0x0:uw                             
        mov (16|M16)             r24.0<1>:ud   0x0:uw                             
        add (8|M0)               r14.0<1>:q    r8.1<0;1,0>:q     r10.0<8;8,1>:ud 
        add (8|M8)               r16.0<1>:q    r8.1<0;1,0>:q     r11.0<8;8,1>:ud 
(W)     mov (1|M0)               r5.0<2>:b     0:w                              
        add (8|M16)              r18.0<1>:q    r8.1<0;1,0>:q     r12.0<8;8,1>:ud 
        add (8|M24)              r20.0<1>:q    r8.1<0;1,0>:q     r13.0<8;8,1>:ud 
        sends (16|M0)            null:w   r14     r22     0x8C            0x080690FF           // wr:4+2, rd:0; hdc.dc1; a64 byte scattering write 8b
        mov (16|M0)              r70.0<1>:b    r5.0<0;1,0>:b                   
        sends (16|M16)           null:w   r18     r24     0x8C            0x080690FF           // wr:4+2, rd:0; hdc.dc1; a64 byte scattering write 8b
        mov (16|M16)             r71.0<1>:b    r5.0<0;1,0>:b                   
L4320:
        join (32|M0)                         L4320                                
L4336:
        mov (16|M0)              r5.1<2>:b     -1:w                              
        add (16|M0)              r10.0<1>:d    r50.0<8;8,1>:d    2:w              
        mov (16|M16)             r6.1<2>:b     -1:w                              
        mov (16|M0)              r5.0<2>:b     r70.0<8;8,1>:b                  
        add (16|M16)             r12.0<1>:d    r52.0<8;8,1>:d    2:w              
        mov (16|M16)             r6.0<2>:b     r71.0<8;8,1>:b                  
        add (8|M0)               r14.0<1>:q    r8.1<0;1,0>:q     r10.0<8;8,1>:ud 
        add (8|M8)               r16.0<1>:q    r8.1<0;1,0>:q     r11.0<8;8,1>:ud 
        mov (16|M0)              r22.0<1>:ud   r5.0<16;16,1>:uw                
        add (8|M16)              r18.0<1>:q    r8.1<0;1,0>:q     r12.0<8;8,1>:ud 
        add (8|M24)              r20.0<1>:q    r8.1<0;1,0>:q     r13.0<8;8,1>:ud 
        mov (16|M16)             r24.0<1>:ud   r6.0<16;16,1>:uw                
        sends (16|M0)            null:w   r14     r22     0x8C            0x080694FF           // wr:4+2, rd:0; hdc.dc1; a64 byte scattering write 16b
        sends (16|M16)           null:w   r18     r24     0x8C            0x080694FF           // wr:4+2, rd:0; hdc.dc1; a64 byte scattering write 16b
L4560:
        endif (32|M0)                        L4576                                
L4576:
(W)     mov (8|M0)               r127.0<1>:ud  r32.0<8;8,1>:ud                  {Compacted}
(W)     send (1|M0)              null     r127    0x27            0x02000010           {EOT} // wr:1+0, rd:0; spawner; end of thread
L4600:
        illegal                
        illegal                
        illegal                
        illegal                
        illegal                
        illegal                
        illegal                
        illegal                
