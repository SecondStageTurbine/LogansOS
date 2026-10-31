# SPDX-License-Identifier: MPL-2.0
# Generates fixtures/fadt-oracle.txt: each corpus set's FADT power fields as
# ACPICA's disassembler reads them, the independent oracle for parse_fadt.
# Usage (iasl from ACPICA, any recent release):
#   python3 -I tools/acpi-power-logic-tests/gen-fadt-oracle.py <iasl> fixtures/acpi \
#     > tools/acpi-power-logic-tests/fixtures/fadt-oracle.txt
# A register is its X_ GAS when that address is non-zero, else the 32-bit
# block (SystemIO, LEN * 8 bits): ACPICA's AcpiTbConvertFadt rule.
import glob,os,re,subprocess,sys,tempfile
def dis(iasl,path):
    with tempfile.TemporaryDirectory() as t:
        subprocess.run([iasl,'-d','-p',os.path.join(t,'f'),path],check=True,capture_output=True)
        return open(os.path.join(t,'f.dsl')).read().splitlines()
def fields(lines):
    kv={}; gas={}; cur=None
    for l in lines:
        m=re.match(r'\[\w+h \d+ \w+h\]\s+(.*?) : (.*)',l)
        if not m: continue
        name,val=m.group(1).strip(),m.group(2).strip()
        if val.startswith('[Generic Address Structure]'): cur=name; gas[cur]={}; continue
        if cur and name in ('Space ID','Bit Width','Address'):
            gas[cur][name]=int(val.split()[0],16)
            if name=='Address': cur=None
            continue
        kv[name]=int(val.split()[0],16) if re.match(r'[0-9A-Fa-f]+$',val.split()[0]) else val
    return kv,gas
def main(iasl,root):
    print('# set revision flags smi_cmd acpi_enable pm1a_evt pm1b_evt pm1_evt_len pm1a_cnt pm1b_cnt pm1_cnt_len '
          'gpe0 gpe0_len reset_space reset_addr reset_value sleep_control sleep_status pm_timer')
    print('# a register is space:width:address, or - when absent; numbers are hex')
    for d in sorted(glob.glob(os.path.join(root,'*',''))):
        f=[x for x in sorted(os.listdir(d)) if open(os.path.join(d,x),'rb').read(4)==b'FACP']
        kv,gas=fields(dis(iasl,os.path.join(d,f[0])))
        def g(name):
            x=gas.get(name)
            return '%x:%x:%x'%(x['Space ID'],x['Bit Width'],x['Address']) if x and x['Address'] else '-'
        def reg(a32,lenk,gname):
            x=g(gname)
            if x!='-': return x
            a=kv.get(a32,0); return '1:%x:%x'%(kv[lenk]*8,a) if a else '-'
        r=gas['Reset Register']
        row=[os.path.basename(os.path.normpath(d)),kv['Revision'],kv['Flags (decoded below)'],kv['SMI Command Port'],
             kv['ACPI Enable Value'],
             reg('PM1A Event Block Address','PM1 Event Block Length','PM1A Event Block'),
             reg('PM1B Event Block Address','PM1 Event Block Length','PM1B Event Block'),kv['PM1 Event Block Length'],
             reg('PM1A Control Block Address','PM1 Control Block Length','PM1A Control Block'),
             reg('PM1B Control Block Address','PM1 Control Block Length','PM1B Control Block'),kv['PM1 Control Block Length'],
             reg('GPE0 Block Address','GPE0 Block Length','GPE0 Block'),kv['GPE0 Block Length'],
             r['Space ID'],r['Address'],kv['Value to cause reset'],
             g('Sleep Control Register'),g('Sleep Status Register'),
             reg('PM Timer Block Address','PM Timer Block Length','PM Timer Block')]
        print(' '.join(x if isinstance(x,str) else '%x'%x for x in row))
if __name__=='__main__': main(sys.argv[1],sys.argv[2])
