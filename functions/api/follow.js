// SPDX-License-Identifier: MPL-2.0
// logansos.com "follow": POST /api/follow stores an email in D1 (binding DB) so one
// announcement can go out when Arthur Preview ships. Turnstile (secret TURNSTILE_SECRET)
// and a honeypot field keep bots out. The reply is the same for new and known addresses.

const json = (body, status = 200) =>
  new Response(JSON.stringify(body), { status, headers: { 'content-type': 'application/json' } });

export async function onRequestPost({ request, env }) {
  const form = await request.formData();
  if (form.get('website')) return json({ ok: true }); // honeypot: bots fill it, people never see it

  const email = String(form.get('email') || '').trim().toLowerCase();
  if (email.length > 254 || !/^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(email)) {
    return json({ ok: false, error: "That doesn't look like an email address." }, 400);
  }

  const check = await fetch('https://challenges.cloudflare.com/turnstile/v0/siteverify', {
    method: 'POST',
    body: new URLSearchParams({
      secret: env.TURNSTILE_SECRET,
      response: String(form.get('cf-turnstile-response') || ''),
      remoteip: request.headers.get('CF-Connecting-IP') || '',
    }),
  }).then((r) => r.json());
  if (!check.success) return json({ ok: false, error: "We couldn't confirm you're human. Please try again." }, 403);

  await env.DB.prepare('INSERT OR IGNORE INTO followers (email) VALUES (?)').bind(email).run();
  return json({ ok: true });
}
