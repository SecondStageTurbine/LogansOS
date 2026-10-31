# SPDX-License-Identifier: MPL-2.0
# OT-90 THINK §3.2 prototype: a parse-only namespace walk of AML definition
# blocks that finds the root \_S5 (a model for P1, which ports it to Rust).
# Usage: python3 -I docs/THINK-2026-10-27-ot90-s5-walk.py fixtures/acpi
# It reads only the table bytes; it executes nothing.
#
# Revision 4: every table of a set is walked to its end (no early return), and
# every name used as a term at table level must resolve to a declaration whose
# argument count is known, or the whole decode fails closed. Declarations come
# from a fixed-point pre-pass over all tables (forward references and
# declarations inside table-level If/Else/While bodies included).
#
# Revision 5: a definition inside an If/Else/While body is recorded as conditional,
# keyed by that body. It resolves only for a use inside the same body (or one
# nested in it); a use anywhere else fails the decode closed, and its arity never
# feeds an unconditional call. External alone is collected from conditional bodies
# as unconditional: it creates nothing, and iASL wraps its External block in If (Zero).
import sys,glob,os
class Fail(Exception): pass
SEGC=set(b'ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_')
def pkglen(b,i):
    lead=b[i]; n=lead>>6
    if n==0: return lead&0x3f, i+1
    if lead&0x30: raise Fail("pkglen lead @%x"%i)
    v=lead&0x0f
    for k in range(n): v|=b[i+1+k]<<(4+8*k)
    return v, i+1+n
def seg(b,i):
    s=b[i:i+4]
    if len(s)<4 or not all(c in SEGC for c in s) or 48<=s[0]<=57: raise Fail("nameseg @%x"%i)
    return s.decode(), i+4
def namestring(b,i):
    root=False; up=0
    if b[i]==0x5c: root=True; i+=1
    else:
        while b[i]==0x5e: up+=1; i+=1
    if b[i]==0x00: return (root,up,[]), i+1
    if b[i]==0x2e: a,i=seg(b,i+1); c,i=seg(b,i); return (root,up,[a,c]),i
    if b[i]==0x2f:
        n=b[i+1]; i+=2; segs=[]
        for _ in range(n): x,i=seg(b,i); segs.append(x)
        return (root,up,segs),i
    x,i=seg(b,i); return (root,up,[x]),i
def is_name_start(c): return c in (0x5c,0x5e,0x2e,0x2f) or (0x41<=c<=0x5a) or c==0x5f
def resolve(scope,ns):
    root,up,segs=ns
    base=[] if root else list(scope)
    if up>len(base): raise Fail("^ past root")
    base=base[:len(base)-up] if up else base
    return tuple(base+segs)
# opcode -> argument shape. 'T' TermArg, 'S' SuperName/Target, 'N' a NameString it declares,
# 'B' byte, 'W' word, 'D' dword
ONE={
 0x70:'TS',0x71:'TS',0x72:'TTS',0x73:'TTS',0x74:'TTS',0x75:'S',0x76:'S',0x77:'TTS',0x78:'TTSS',0x79:'TTS',0x7a:'TTS',
 0x7b:'TTS',0x7c:'TTS',0x7d:'TTS',0x7e:'TTS',0x7f:'TTS',0x80:'TS',0x81:'TS',0x82:'TS',0x83:'T',0x84:'TTS',0x85:'TTS',
 0x86:'ST',0x87:'S',0x88:'TTS',0x89:'TBTBTT',0x8e:'S',0x90:'TT',0x91:'TT',0x92:'T',0x93:'TT',0x94:'TT',0x95:'TT',
 0x96:'TS',0x97:'TS',0x98:'TS',0x99:'TS',0x9c:'TS',0x9d:'TS',0x9e:'TTS',0x9f:'',0xa3:'',0xa4:'T',0xa5:'',0xcc:'',
 0x8a:'TTN',0x8b:'TTN',0x8c:'TTN',0x8d:'TTN',0x8f:'TTN',
}
EXT={0x12:'S',0x13:'TTTN',0x1f:'S',0x20:'S',0x21:'T',0x22:'T',0x23:'SW',0x24:'S',0x25:'S',0x26:'S',0x27:'S',0x28:'TS',0x29:'TS',0x2a:'TT',0x30:'',0x33:'',0x88:'NTTT'}
# Objects the OS predefines (ACPI 6.5 §5.3, §5.7): None is a non-method, n is a method's argument count.
PREDEF={('_SB_',):None,('_GPE',):None,('_PR_',):None,('_TZ_',):None,('_SI_',):None,('_GL_',):None,
        ('_OS_',):None,('_REV',):None,('_OSI',):1}
class W:
    # decls: (unconditional, conditional). unconditional maps path -> None (not a
    # method) or a method's argument count; conditional maps (path, body) -> the same,
    # where body names the If/Else/While body holding the definition. strict: an
    # unresolved table-level name fails closed; otherwise (the pre-pass) it takes 0 args.
    def __init__(s,decls,strict):
        s.decls=decls[0]; s.cdecls={}
        for (p,bd),a in decls[1].items(): s.cdecls.setdefault(p,{})[bd]=a
        s.strict=strict; s.seen={}; s.cseen={}; s.found=[]; s.cur=None; s.stack=()
    def declare(s,path,arity,external=False):
        if s.stack and not external: d,key=s.cseen,(path,s.stack[-1])
        else: d,key=s.seen,path
        old=d.get(key)
        if old is not None and arity is not None and old!=arity:
            raise Fail("conflicting arity for %s"%'.'.join(path))
        if old is None: d[key]=arity
    def termarg(s,b,i,scope):
        c=b[i]
        if c in (0x00,0x01,0xff): return i+1
        if c==0x0a: return i+2
        if c==0x0b: return i+3
        if c==0x0c: return i+5
        if c==0x0e: return i+9
        if c==0x0d:
            j=i+1
            while b[j]!=0:
                if b[j]>0x7f: raise Fail("string byte @%x"%j)
                j+=1
            return j+1
        if c in (0x11,0x12,0x13): l,_=pkglen(b,i+1); return i+1+l
        if 0x60<=c<=0x6e: return i+1
        if c==0x5b and b[i+1]==0x30: return i+2  # Revision
        if c==0x5b and b[i+1]==0x31: return i+2  # Debug
        if is_name_start(c):
            ns,j=namestring(b,i)
            n=s.arity(scope,ns,i)
            for _ in range(n): j=s.termarg(b,j,scope)
            return j
        if c in ONE:
            return s.args(b,i+1,ONE[c],scope)
        if c==0x5b and b[i+1] in EXT:
            return s.args(b,i+2,EXT[b[i+1]],scope)
        raise Fail("termarg op %02x @%x"%(c,i))
    def supername(s,b,i,scope):
        c=b[i]
        if c==0x00: return i+1  # NullName target
        if 0x60<=c<=0x6e: return i+1
        if c==0x5b and b[i+1]==0x31: return i+2
        if is_name_start(c): _,j=namestring(b,i); return j
        return s.termarg(b,i,scope)  # RefOf/DerefOf/Index as supername
    def args(s,b,i,shape,scope):
        for k in shape:
            if k=='T': i=s.termarg(b,i,scope)
            elif k=='S': i=s.supername(b,i,scope)
            elif k=='N': ns,i=namestring(b,i); s.declare(resolve(scope,ns),None)
            elif k=='B': i+=1
            elif k=='W': i+=2
            elif k=='D': i+=4
        return i
    def arity(s,scope,ns,at):
        # ACPI 6.5 §5.3 search rules: a lone NameSeg searches up toward the root.
        root,up,segs=ns
        if root or up or len(segs)!=1: cands=[resolve(scope,ns)]
        else:
            sc=list(scope); cands=[]
            while True:
                cands.append(tuple(sc+segs))
                if not sc: break
                sc.pop()
        for p in cands:
            if p in s.decls: return s.decls[p] or 0
            if p in PREDEF: return PREDEF[p] or 0
            if p in s.cdecls:
                live=[bd for bd in s.stack if bd in s.cdecls[p]]
                if live: return s.cdecls[p][live[-1]] or 0
                if s.strict: raise Fail("conditional-only name %s used outside its body @%x"%('.'.join(p),at))
                return 0
        if s.strict: raise Fail("unresolved name %s @%x"%('.'.join(segs) or '\\',at))
        return 0
    def fieldlist(s,b,i,end,scope):
        while i<end:
            c=b[i]
            if c==0x00: _,i=pkglen(b,i+1)
            elif c==0x01: i+=3
            elif c==0x02:
                if b[i+1]==0x11: l,_=pkglen(b,i+2); i=i+2+l
                else: _,i=namestring(b,i+1)
            elif c==0x03: i+=4
            else:
                x,i=seg(b,i); s.declare(tuple(list(scope)+[x]),None); _,i=pkglen(b,i)
        if i!=end: raise Fail("field list overruns @%x"%i)
    def walk(s,b,i,end,scope,stack):
        # stack: the conditional bodies this walk is inside, innermost last (empty: unconditional).
        while i<end:
            s.stack=stack; c=b[i]
            if c==0x10 or (c==0x5b and b[i+1] in (0x82,0x83,0x84,0x85)):
                j=i+1 if c==0x10 else i+2
                l,body=pkglen(b,j); pend=j+l
                ns,k=namestring(b,body); path=resolve(scope,ns)
                if c==0x5b: s.declare(path,None)
                if c==0x5b and b[i+1]==0x83: k+=6
                if c==0x5b and b[i+1]==0x84: k+=3
                s.walk(b,k,pend,path,stack); i=pend; continue
            if c==0x14:
                l,body=pkglen(b,i+1); pend=i+1+l
                ns,k=namestring(b,body); path=resolve(scope,ns)
                if path==('_S5_',): raise Fail("Method \\_S5")
                s.declare(path,b[k]&7)
                i=pend; continue  # method bodies are never walked
            if c==0xa0 or c==0xa2:  # If/While: predicate, then a conditional body
                l,body=pkglen(b,i+1); pend=i+1+l
                k=s.termarg(b,body,scope); s.walk(b,k,pend,scope,stack+((s.cur[0],i),)); i=pend; continue
            if c==0xa1:  # Else
                l,body=pkglen(b,i+1); pend=i+1+l; s.walk(b,body,pend,scope,stack+((s.cur[0],i),)); i=pend; continue
            if c in (0x11,0x12,0x13):
                l,_=pkglen(b,i+1); i=i+1+l; continue
            if c==0x5b and b[i+1] in (0x81,0x86,0x87):  # Field/IndexField/BankField
                l,body=pkglen(b,i+2); pend=i+2+l
                _,k=namestring(b,body)
                if b[i+1]!=0x81: _,k=namestring(b,k)
                if b[i+1]==0x87: k=s.termarg(b,k,scope)
                s.fieldlist(b,k+1,pend,scope); i=pend; continue
            if c==0x08:
                ns,k=namestring(b,i+1); path=resolve(scope,ns)
                if path==('_S5_',):
                    if stack: raise Fail("conditional \\_S5")
                    s.found.append((s.cur[0],s.cur[1],i,k))
                s.declare(path,None)
                i=s.termarg(b,k,scope); continue
            if c==0x15:  # External: ObjectType 8 is a method, with its argument count
                ns,k=namestring(b,i+1)
                try: path=resolve(scope,ns)
                except Fail: path=None  # e.g. iASL's External (^GFX0.CLID) at the root: declares nothing
                if path: s.declare(path,(b[k+1]&7) if b[k]==8 else None,external=True)
                i=k+2; continue
            if c==0x06: _,k=namestring(b,i+1); ns,k=namestring(b,k); s.declare(resolve(scope,ns),None); i=k; continue
            if c==0x5b and b[i+1]==0x80:
                ns,k=namestring(b,i+2); s.declare(resolve(scope,ns),None)
                k+=1; k=s.termarg(b,k,scope); i=s.termarg(b,k,scope); continue
            if c==0x5b and b[i+1] in (0x01,0x02):  # Mutex (SyncFlags byte), Event
                ns,k=namestring(b,i+2); s.declare(resolve(scope,ns),None); i=k+(b[i+1]==0x01); continue
            # any other Type1/Type2 statement (CreateXField and DataRegion declare through 'N')
            i=s.termarg(b,i,scope)
        if i!=end: raise Fail("body overruns @%x"%i)
def walk_all(tables,decls,strict):
    w=W(decls,strict); per=[]
    for n,b in tables:
        w.cur=(n,b)
        try: w.walk(b,36,len(b),(),()); per.append((n,'OK'))
        except Fail as e: per.append((n,'FAIL %s'%e))
        except IndexError: per.append((n,'FAIL truncated'))
    return w,per
def decode(tables):
    # Fixed-point pre-pass: collect declarations until the set is stable.
    decls=({},{})
    for _ in range(8):
        w,_per=walk_all(tables,decls,False)
        if (w.seen,w.cseen)==decls: break
        decls=(dict(w.seen),dict(w.cseen))
    else: return 'FAIL-CLOSED: declarations do not settle',[]
    # Strict pass: every table-level name must resolve, and it must reproduce the same declarations.
    w,per=walk_all(tables,decls,True)
    bad=[p for p in per if p[1]!='OK']
    if bad: return 'FAIL-CLOSED(%s: %s)'%bad[0],per
    if (w.seen,w.cseen)!=decls: return 'FAIL-CLOSED: strict pass disagrees with pre-pass',per
    occ=sum(b.count(b'_S5_',36) for _,b in tables)
    if occ!=1: return 'FAIL-CLOSED: %d _S5_ occurrences'%occ,per
    if len(w.found)!=1: return 'FAIL-CLOSED: %d root \\_S5 definitions'%len(w.found),per
    n,b,i,k=w.found[0]
    if b[k]!=0x12 or b[k+1]>>6: return 'FAIL-CLOSED: not a short Package',per
    j=k+2; cnt=b[j]; j+=1; vals=[]
    for _ in range(2):
        op=b[j]
        if op==0: vals.append(0); j+=1
        elif op==1: vals.append(1); j+=1
        elif op==0x0a: vals.append(b[j+1]); j+=2
        else: return 'FAIL-CLOSED: element',per
    if cnt<2 or max(vals)>7: return 'FAIL-CLOSED: bad package',per
    return ('OK',n,hex(i),vals),per
def load(d):
    tabs=[(os.path.basename(f).lower(),open(f,'rb').read()) for f in sorted(glob.glob(os.path.join(d,'*')))
          if os.path.basename(f).lower().startswith('dsdt') or 'ssdt' in os.path.basename(f).lower()]
    tabs.sort(key=lambda t: (0 if t[0].startswith('dsdt') else 1, t[0]))
    return tabs
if __name__=='__main__':
    nt=ok=ns=nok=0
    for d in sorted(glob.glob(os.path.join(sys.argv[1],'*',''))):
        name=os.path.basename(os.path.normpath(d)); tabs=load(d)
        if not tabs: continue
        r,per=decode(tabs); ns+=1; nok+=isinstance(r,tuple)
        for t,st in per:
            nt+=1; ok+=st=='OK'; print('  %-40s %-12s %s'%(name,t,st))
        print('%s -> %s'%(name,r))
    print('tables walked to end: %d/%d; sets decoded: %d/%d'%(ok,nt,nok,ns))
