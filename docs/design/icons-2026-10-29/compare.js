// SPDX-License-Identifier: MPL-2.0
// Builds compare.png from the two seats' SVGs: node compare.js <icons dir> <out.png> (needs playwright).
const { chromium } = require('playwright');
const fs = require('fs'); const path = require('path');
const D = process.argv[2], out = process.argv[3];
const names = ['arthur','lens','terminal','files','settings','install','browser'];
const svg = (seat, n, sz) => {
  const f = path.join(D, seat, 'svg', sz === 40 ? `${n}.svg` : `${n}-${sz}.svg`);
  const p = fs.existsSync(f) ? f : path.join(D, seat, 'svg', `${n}.svg`);
  let s = fs.readFileSync(p, 'utf8').replace(/<\?xml[^>]*\?>/, '');
  return s.replace(/<svg /, `<svg style="width:${sz}px;height:${sz}px;display:block" `);
};
const cell = (seat, n, bg) => `<div class="c" style="background:${bg}">${[40,24,16].map(s=>`<div class="i">${svg(seat,n,s)}</div>`).join('')}</div>`;
let rows = names.map(n => `<tr><th>${n[0].toUpperCase()+n.slice(1)}</th>
<td>${cell('s1',n,'#0a0e27')}</td><td>${cell('s1',n,'#38485e')}</td>
<td class="gap"></td>
<td>${cell('jarvis',n,'#0a0e27')}</td><td>${cell('jarvis',n,'#38485e')}</td></tr>`).join('');
const html = `<html><body style="margin:0;background:#0a0e27;font-family:Inter,Arial,sans-serif;color:#a8adb2">
<style>table{border-collapse:separate;border-spacing:10px 8px;padding:16px} th{color:#fff;font-weight:600;text-align:left;font-size:15px;padding-right:12px}
.h{color:#fff;font-size:15px;font-weight:600;text-align:center}.s{font-size:12px;text-align:center}
.c{display:flex;align-items:center;gap:18px;padding:12px 18px;border-radius:10px}.i{display:flex;align-items:center}.gap{width:24px;border-left:1px solid #2d3748}</style>
<table><tr><th></th><td colspan=2 class="h">A — S1 (Claude family)</td><td class="gap"></td><td colspan=2 class="h">B — Jarvis (Codex family)</td></tr>
<tr><th></th><td class="s">on the Void · 40 / 24 / 16</td><td class="s">on the frosted chip</td><td class="gap"></td><td class="s">on the Void · 40 / 24 / 16</td><td class="s">on the frosted chip</td></tr>
${rows}</table></body></html>`;
(async () => {
  const b = await chromium.launch(); const p = await b.newPage({ deviceScaleFactor: 2 });
  await p.setContent(html); const t = await p.$('table');
  await t.screenshot({ path: out }); await b.close();
})();
