# SPDX-License-Identifier: MPL-2.0
# Regenerates src/frames.rs: python3 -I gen_frames.py > (strip into) src/frames.rs
# Independent oracle: builds the KAT frames with RFC 1071 checksums.
import struct
def csum(b):
    if len(b)%2: b+=b'\0'
    s=sum(struct.unpack('!%dH'%(len(b)//2),b))
    while s>>16: s=(s&0xffff)+(s>>16)
    return (~s)&0xffff
US=bytes.fromhex('525400123456'); PEER=bytes.fromhex('525400123457')
def ip(src,dst,proto,payload,ident=0x1234,frag=0):
    h=struct.pack('!BBHHHBBH4s4s',0x45,0,20+len(payload),ident,frag,64,proto,0,bytes(src),bytes(dst))
    h=h[:10]+struct.pack('!H',csum(h))+h[12:]
    return h+payload
def udp(src,dst,sp,dp,data):
    l=8+len(data); u=struct.pack('!HHHH',sp,dp,l,0)+data
    ph=bytes(src)+bytes(dst)+struct.pack('!BBH',0,17,l)
    c=csum(ph+u) or 0xffff
    return u[:6]+struct.pack('!H',c)+u[8:]
def tcp(src,dst,sp,dp,seq,flags):
    t=struct.pack('!HHIIBBHHH',sp,dp,seq,0,5<<4,flags,4096,0,0)
    ph=bytes(src)+bytes(dst)+struct.pack('!BBH',0,6,len(t))
    return t[:16]+struct.pack('!H',csum(ph+t))+t[18:]
def icmp(t,ident,seq,data):
    m=struct.pack('!BBHHH',t,0,0,ident,seq)+data
    return m[:2]+struct.pack('!H',csum(m))+m[4:]
def eth(dst,src,et,p): return dst+src+struct.pack('!H',et)+p
A=[10,0,0,2]; B=[10,0,0,10]
frames={
 'UDP': eth(US,PEER,0x0800, ip(A,B,17,udp(A,B,40000,7001,b'hello'))),
 'TCP_SYN': eth(US,PEER,0x0800, ip(A,B,6,tcp(A,B,40001,7002,0x01020304,0x02))),
 'ICMP_ECHO': eth(US,PEER,0x0800, ip(A,B,1,icmp(8,1,1,b'ping'))),
 'ICMP_REPLY_MSG': icmp(0,1,1,b'ping'),
 'DHCP': eth(b'\xff'*6,PEER,0x0800, ip([10,0,0,1],[255]*4,17,udp([10,0,0,1],[255]*4,67,68,b'\x02\x01\x06\x00'))),
}
for k,v in frames.items():
    print(f"pub const {k}: [u8; {len(v)}] = [")
    for i in range(0,len(v),12): print("    "+", ".join(f"0x{x:02x}" for x in v[i:i+12])+",")
    print("];")
def arp(op,sha,spa,tha,tpa,edst,esrc):
    return edst+esrc+b'\x08\x06'+struct.pack('!HHBBH',1,0x0800,6,4,op)+sha+bytes(spa)+tha+bytes(tpa)
extra={
 'ARP_REQUEST': arp(1,PEER,A,b'\0'*6,B,b'\xff'*6,PEER),
 'ARP_REPLY': arp(2,US,B,PEER,A,PEER,US),
}
for k,v in extra.items():
    print(f"pub const {k}: [u8; {len(v)}] = [")
    for i in range(0,len(v),12): print("    "+", ".join(f"0x{x:02x}" for x in v[i:i+12])+",")
    print("];")
