// SPDX-License-Identifier: CC-BY-4.0
// Independent byte oracle for NOOP_KERNEL_GEN9. Produced by `ocloc disasm`
// (igc-1.0.17193.4 / CR 24.26.30049.6) from noop_eot.cl; see README.md. The
// terminating `send ... {EOT}` is the proof this thread retires.
L0:
(W)     mov (8|M0)               r2.0<1>:ud    r0.0<1;1,0>:ud
(W)     or (1|M0)                cr0.0<1>:ud   cr0.0<0;1,0>:ud   0x4C0:uw              {Switch}
(W)     mov (8|M0)               r127.0<1>:ud  r2.0<8;8,1>:ud                   {Compacted}
(W)     send (1|M0)              null     r127    0x27            0x02000010           {EOT} // wr:1+0, rd:0; spawner; end of thread
L56:
        illegal                
        illegal                
        illegal                
        illegal                
        illegal                
        illegal                
        illegal                
        illegal                
