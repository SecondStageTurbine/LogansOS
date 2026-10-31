# SPDX-License-Identifier: MPL-2.0
# OT-90 THINK §3.2: the hand-laid probes for the \_S5 walk prototype.
# Usage: python3 -I docs/THINK-2026-10-27-ot90-s5-probes.py
# The iasl-built negatives are planned P1 fixtures; these cases need hand-laid
# bytes because iasl refuses a method call in a Name or a forward call.
import os,importlib.util
here=os.path.dirname(os.path.abspath(__file__))
sp=importlib.util.spec_from_file_location('walk',os.path.join(here,'THINK-2026-10-27-ot90-s5-walk.py'))
m=importlib.util.module_from_spec(sp); sp.loader.exec_module(m)
def tbl(sig,body): return sig+b'\0'*32+body  # a 36-byte header; the walk starts at offset 36
def method(name,argc,body): return b'\x14'+bytes([len(name)+1+len(body)+1])+name+bytes([argc])+body
S5=b'\x08_S5_\x12\x05\x02\x0a\x05\x00'  # Name (_S5, Package (2) { 5, 0 })
RET0=b'\xa4\x00'                        # Return (Zero)
CASES=[
 ('Janus #119: Name (VAL0, CALL (Zero)), then Method (CALL, 1), then a root _S5',
  [('dsdt',tbl(b'DSDT',b'\x08VAL0CALL\x00'+method(b'CALL',1,RET0)+S5))],'OK'),
 ('a forward Method (CALL, 2) called with one argument, ahead of the root Name (_S5)',
  [('dsdt',tbl(b'DSDT',b'\x08VAL0CALL\x00'+S5+method(b'CALL',2,RET0)))],'FAIL'),
 ('Store (UNKN (Zero), VAL0), with UNKN declared nowhere',
  [('dsdt',tbl(b'DSDT',b'\x08VAL0\x00'+b'\x70UNKN\x00VAL0'+S5))],'FAIL'),
 ('the Zenbook shape: If (Zero) { External (\\ADBG, MethodObj, 1) }, then ADBG ("x")',
  [('dsdt',tbl(b'DSDT',b'\xa0\x0a\x00\x15\\ADBG\x08\x01'+b'ADBG\x0dx\x00'+S5))],'OK'),
 ('Janus #119 probe: a valid DSDT root _S5, then an SSDT holding an unknown opcode',
  [('dsdt',tbl(b'DSDT',S5)),('ssdt',tbl(b'SSDT',b'\x02'))],'FAIL'),
 ('the same method declared with two different arities',
  [('dsdt',tbl(b'DSDT',method(b'CALL',1,RET0)+method(b'CALL',2,RET0)+S5))],'FAIL'),
 ('Janus #120: If (Zero) { Name (VAL0, One) }, then Store (VAL0, Zero), then a root _S5',
  [('dsdt',tbl(b'DSDT',b'\xa0\x08\x00\x08VAL0\x01'+b'\x70VAL0\x00'+S5))],'FAIL'),
 ('the same without the conditional Name',
  [('dsdt',tbl(b'DSDT',b'\x70VAL0\x00'+S5))],'FAIL'),
 ('If (Zero) { Method (CALL, 1) }, then Store (CALL (Zero), VAL1) at table level',
  [('dsdt',tbl(b'DSDT',b'\xa0\x0b\x00'+method(b'CALL',1,RET0)+b'\x08VAL1\x00'+b'\x70CALL\x00VAL1'+S5))],'FAIL'),
 ('If (One) { Method (CALL, 1); Store (CALL (Zero), VAL1) }: the call inside the same body',
  [('dsdt',tbl(b'DSDT',b'\x08VAL1\x00'+b'\xa0\x15\x01'+method(b'CALL',1,RET0)+b'\x70CALL\x00VAL1'+S5))],'OK'),
]
if __name__=='__main__':
    bad=0
    for name,tabs,want in CASES:
        r,_=m.decode(tabs); got='OK' if isinstance(r,tuple) else 'FAIL'
        bad+=got!=want; print('%-4s %s\n     -> %s'%('ok' if got==want else 'BAD',name,r))
    print('%d/%d as expected'%(len(CASES)-bad,len(CASES)))
