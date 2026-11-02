// SPDX-License-Identifier: MPL-2.0
//! `cargo xtask tzdata --src <tz checkout>`: generate
//! `userland/libneo/src/tz_table.rs` from an IANA tz release
//! (`docs/THINK-2026-11-02-time-zones.md` §2.4).
//!
//! The source is a checkout of the tz repository at a release tag (the
//! `eggert/tz` GitHub mirror; `data.iana.org` is not reachable from every
//! build host). The generator records the tag and commit, compiles the data
//! with the host's `zic`, takes each zone's TZif footer (its final POSIX TZ
//! rule) and writes the parsed rules as Rust constants. Ids are stable: a
//! zone keeps the index it had in the existing table, new zones are appended
//! in name order, and a zone that disappears stops the run (it must become an
//! alias by hand, so its id is never reused). Id 0 is `Etc/UTC`.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The tz source files `zic` compiles (the `factory` zone is left out).
const SOURCES: [&str; 9] = [
    "africa",
    "antarctica",
    "asia",
    "australasia",
    "europe",
    "northamerica",
    "southamerica",
    "etcetera",
    "backward",
];

/// One DST transition (`Mm.w.d/time`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Transition {
    pub month: u8,
    pub week: u8,
    pub weekday: u8,
    pub time: i32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Dst {
    pub offset: i32,
    pub abbr: String,
    pub start: Transition,
    pub end: Transition,
}

/// A parsed POSIX TZ string; offsets are seconds east of UTC.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rule {
    pub std_offset: i32,
    pub std_abbr: String,
    pub dst: Option<Dst>,
}

struct Cursor<'a> {
    s: &'a [u8],
    i: usize,
}

impl Cursor<'_> {
    fn peek(&self) -> Option<u8> {
        self.s.get(self.i).copied()
    }

    fn eat(&mut self, b: u8) -> bool {
        if self.peek() == Some(b) {
            self.i += 1;
            true
        } else {
            false
        }
    }

    fn abbr(&mut self) -> Result<String, String> {
        if self.eat(b'<') {
            let start = self.i;
            while self.peek().is_some_and(|b| b != b'>') {
                self.i += 1;
            }
            let name = String::from_utf8_lossy(&self.s[start..self.i]).into_owned();
            if !self.eat(b'>') {
                return Err("unterminated <abbr>".into());
            }
            Ok(name)
        } else {
            let start = self.i;
            while self.peek().is_some_and(|b| b.is_ascii_alphabetic()) {
                self.i += 1;
            }
            if self.i - start < 3 {
                return Err("abbreviation shorter than 3".into());
            }
            Ok(String::from_utf8_lossy(&self.s[start..self.i]).into_owned())
        }
    }

    fn number(&mut self) -> Result<i32, String> {
        let start = self.i;
        while self.peek().is_some_and(|b| b.is_ascii_digit()) {
            self.i += 1;
        }
        std::str::from_utf8(&self.s[start..self.i])
            .unwrap_or("")
            .parse()
            .map_err(|_| format!("number expected at byte {start}"))
    }

    /// `[+-]hh[:mm[:ss]]` as signed seconds (sign as written).
    fn hms(&mut self, max_hours: i32) -> Result<i32, String> {
        let sign = if self.eat(b'-') {
            -1
        } else {
            self.eat(b'+');
            1
        };
        let h = self.number()?;
        if h > max_hours {
            return Err(format!("hour {h} out of range"));
        }
        let mut secs = h * 3600;
        if self.eat(b':') {
            secs += self.number()? * 60;
            if self.eat(b':') {
                secs += self.number()?;
            }
        }
        Ok(sign * secs)
    }

    fn transition(&mut self) -> Result<Transition, String> {
        if !self.eat(b'M') {
            return Err("only Mm.w.d rules are supported".into());
        }
        let month = self.number()?;
        if !self.eat(b'.') {
            return Err("expected `.`".into());
        }
        let week = self.number()?;
        if !self.eat(b'.') {
            return Err("expected `.`".into());
        }
        let weekday = self.number()?;
        if !(1..=12).contains(&month) || !(1..=5).contains(&week) || weekday > 6 {
            return Err("Mm.w.d out of range".into());
        }
        // RFC 8536 §3.3.1 extends the time to -167..=167 hours.
        let time = if self.eat(b'/') { self.hms(167)? } else { 7200 };
        Ok(Transition {
            month: month as u8,
            week: week as u8,
            weekday: weekday as u8,
            time,
        })
    }
}

/// Parse a TZif footer (a POSIX TZ string with the RFC 8536 extensions).
pub fn parse_posix(tz: &str) -> Result<Rule, String> {
    let mut c = Cursor {
        s: tz.as_bytes(),
        i: 0,
    };
    let std_abbr = c.abbr()?;
    // POSIX offsets are west of UTC: "EST5" is UTC-5.
    let std_offset = -c.hms(24)?;
    if c.peek().is_none() {
        return Ok(Rule {
            std_offset,
            std_abbr,
            dst: None,
        });
    }
    let dst_abbr = c.abbr()?;
    let dst_offset = if c.peek().is_some_and(|b| b != b',') {
        -c.hms(24)?
    } else {
        std_offset + 3600
    };
    if !c.eat(b',') {
        return Err("DST without a rule".into());
    }
    let start = c.transition()?;
    if !c.eat(b',') {
        return Err("expected `,`".into());
    }
    let end = c.transition()?;
    if c.peek().is_some() {
        return Err("trailing bytes".into());
    }
    Ok(Rule {
        std_offset,
        std_abbr,
        dst: Some(Dst {
            offset: dst_offset,
            abbr: dst_abbr,
            start,
            end,
        }),
    })
}

/// The footer of a TZif v2+ file: the last line, between the final newlines.
pub fn tzif_footer(bytes: &[u8]) -> Result<String, String> {
    if !bytes.starts_with(b"TZif") || bytes.get(4).is_none_or(|v| *v < b'2') {
        return Err("not a TZif v2+ file".into());
    }
    let body = bytes.strip_suffix(b"\n").ok_or("no footer")?;
    let start = body.iter().rposition(|&b| b == b'\n').ok_or("no footer")? + 1;
    Ok(String::from_utf8_lossy(&body[start..]).into_owned())
}

/// `Zone` and `Link` names declared in the tz source files.
fn read_names(src: &Path) -> Result<(BTreeSet<String>, BTreeMap<String, String>), String> {
    let mut zones = BTreeSet::new();
    let mut links = BTreeMap::new();
    for file in SOURCES {
        let text = fs::read_to_string(src.join(file)).map_err(|e| format!("read {file}: {e}"))?;
        for line in text.lines() {
            let mut f = line.split_whitespace();
            match f.next() {
                Some("Zone") => {
                    zones.insert(f.next().ok_or("Zone without a name")?.to_string());
                }
                Some("Link") => {
                    let target = f.next().ok_or("Link without a target")?;
                    let name = f.next().ok_or("Link without a name")?;
                    links.insert(name.to_string(), target.to_string());
                }
                _ => {}
            }
        }
    }
    Ok((zones, links))
}

/// Zone names in the order of an existing generated table (its ids).
fn existing_ids(table: &str) -> Vec<String> {
    table
        .lines()
        .filter_map(|l| l.trim().strip_prefix("Zone { name: \""))
        .filter_map(|l| l.split('"').next())
        .map(str::to_string)
        .collect()
}

fn git(src: &Path, args: &[&str]) -> Result<String, String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(src)
        .args(args)
        .output()
        .map_err(|e| format!("git: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "git {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

fn transition_lit(t: &Transition) -> String {
    format!(
        "Transition {{ month: {}, week: {}, weekday: {}, time: {} }}",
        t.month, t.week, t.weekday, t.time
    )
}

/// Render the table source. `zones` is in id order.
pub fn render(
    version: &str,
    commit: &str,
    zones: &[(String, Rule)],
    links: &[(String, u16)],
) -> String {
    let mut out = String::new();
    out.push_str("// SPDX-License-Identifier: MPL-2.0\n");
    out.push_str(
        "// GENERATED by `cargo xtask tzdata` from the IANA tz database (public domain).\n",
    );
    out.push_str("// Do not edit by hand. Ids (indices into ZONES) are append-only.\n\n");
    out.push_str(&format!(
        "pub const TZDATA_VERSION: &str = \"{version}\";\n"
    ));
    out.push_str(&format!(
        "pub const TZDATA_COMMIT: &str = \"{commit}\";\n\n"
    ));
    out.push_str("// One zone per line: (id order) name, standard offset and abbreviation, DST.\n");
    out.push_str("#[rustfmt::skip]\n");
    out.push_str(&format!("pub static ZONES: [Zone; {}] = [\n", zones.len()));
    for (name, r) in zones {
        let dst = match &r.dst {
            None => "None".to_string(),
            Some(d) => format!(
                "Some(Dst {{ offset: {}, abbr: \"{}\", start: {}, end: {} }})",
                d.offset,
                d.abbr,
                transition_lit(&d.start),
                transition_lit(&d.end)
            ),
        };
        out.push_str(&format!(
            "    Zone {{ name: \"{name}\", rule: Rule {{ std_offset: {}, std_abbr: \"{}\", dst: {dst} }} }},\n",
            r.std_offset, r.std_abbr
        ));
    }
    out.push_str("];\n\n");
    out.push_str("#[rustfmt::skip]\n");
    out.push_str(&format!(
        "pub static LINKS: [(&str, ZoneId); {}] = [\n",
        links.len()
    ));
    for (name, id) in links {
        out.push_str(&format!("    (\"{name}\", {id}),\n"));
    }
    out.push_str("];\n");
    out
}

pub fn cmd_tzdata(args: &[String], workspace: &Path) -> Result<(), String> {
    let mut src: Option<PathBuf> = None;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--src" => src = it.next().map(PathBuf::from),
            other => return Err(format!("tzdata: unknown argument `{other}`")),
        }
    }
    let src = src.ok_or("tzdata: --src <tz checkout at a release tag> is required")?;
    let version = git(&src, &["describe", "--tags", "--exact-match"])
        .map_err(|e| format!("the tz checkout must be at a release tag: {e}"))?;
    let commit = git(&src, &["rev-parse", "HEAD"])?;

    let out_dir = std::env::temp_dir().join(format!("xtask-tzdata-{}", std::process::id()));
    let _ = fs::remove_dir_all(&out_dir);
    let status = Command::new("zic")
        .arg("-b")
        .arg("slim")
        .arg("-d")
        .arg(&out_dir)
        .args(SOURCES)
        .current_dir(&src)
        .status()
        .map_err(|e| format!("zic: {e}"))?;
    if !status.success() {
        return Err("zic failed".into());
    }

    let (zones, links) = read_names(&src)?;
    let table_path = workspace.join("userland/libneo/src/tz_table.rs");
    let mut order = fs::read_to_string(&table_path)
        .map(|t| existing_ids(&t))
        .unwrap_or_default();
    if order.is_empty() {
        order.push("Etc/UTC".to_string());
    }
    if let Some(gone) = order.iter().find(|z| !zones.contains(*z)) {
        return Err(format!(
            "zone `{gone}` left the database: keep its id as an alias of its replacement by hand"
        ));
    }
    let known: BTreeSet<String> = order.iter().cloned().collect();
    order.extend(zones.iter().filter(|z| !known.contains(*z)).cloned());

    let mut rows = Vec::new();
    for name in &order {
        let bytes = fs::read(out_dir.join(name)).map_err(|e| format!("read TZif {name}: {e}"))?;
        let footer = tzif_footer(&bytes).map_err(|e| format!("{name}: {e}"))?;
        let rule = parse_posix(&footer).map_err(|e| format!("{name}: `{footer}`: {e}"))?;
        rows.push((name.clone(), rule));
    }
    let index: BTreeMap<&str, u16> = order
        .iter()
        .enumerate()
        .map(|(i, n)| (n.as_str(), i as u16))
        .collect();
    let mut link_rows = Vec::new();
    for (alias, target) in &links {
        // A link to a link: follow it to the zone.
        let mut t = target.as_str();
        while let Some(next) = links.get(t) {
            t = next;
        }
        let id = *index
            .get(t)
            .ok_or(format!("link {alias} -> {target}: no such zone"))?;
        link_rows.push((alias.clone(), id));
    }
    fs::write(&table_path, render(&version, &commit, &rows, &link_rows))
        .map_err(|e| format!("write {}: {e}", table_path.display()))?;
    let _ = fs::remove_dir_all(&out_dir);
    println!(
        "[xtask] tzdata {version} ({commit}): {} zones, {} links -> {}",
        rows.len(),
        link_rows.len(),
        table_path.display()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Footers written out by hand from POSIX.1-2017 §8.3 and RFC 8536.
    #[test]
    fn posix_footers_parse() {
        let ny = parse_posix("EST5EDT,M3.2.0,M11.1.0").unwrap();
        assert_eq!(ny.std_offset, -18000);
        assert_eq!(ny.std_abbr, "EST");
        let d = ny.dst.unwrap();
        assert_eq!(d.offset, -14400);
        assert_eq!(d.abbr, "EDT");
        assert_eq!(
            d.start,
            Transition {
                month: 3,
                week: 2,
                weekday: 0,
                time: 7200
            }
        );
        assert_eq!(d.end.month, 11);

        let syd = parse_posix("AEST-10AEDT,M10.1.0,M4.1.0/3").unwrap();
        assert_eq!(syd.std_offset, 36000);
        assert_eq!(syd.dst.as_ref().unwrap().offset, 39600);
        assert_eq!(syd.dst.as_ref().unwrap().end.time, 10800);

        let kol = parse_posix("IST-5:30").unwrap();
        assert_eq!((kol.std_offset, kol.dst), (19800, None));

        let cas = parse_posix("<+00>0").unwrap();
        assert_eq!((cas.std_offset, cas.std_abbr.as_str()), (0, "+00"));

        // Negative DST (Dublin: IST is standard, GMT is "daylight").
        let dub = parse_posix("IST-1GMT0,M10.5.0,M3.5.0/1").unwrap();
        assert_eq!(dub.std_offset, 3600);
        assert_eq!(dub.dst.as_ref().unwrap().offset, 0);

        // RFC 8536 times: negative, and past 24 h.
        let nuuk = parse_posix("<-02>2<-01>,M3.5.0/-1,M10.5.0/0").unwrap();
        assert_eq!(nuuk.dst.as_ref().unwrap().start.time, -3600);
        let gaza = parse_posix("EET-2EEST,M3.4.4/50,M10.4.4/50").unwrap();
        assert_eq!(gaza.dst.as_ref().unwrap().start.time, 50 * 3600);

        assert!(parse_posix("EST5EDT,J60,J300").is_err());
        assert!(parse_posix("ES5").is_err());
    }

    #[test]
    fn footer_is_the_last_line() {
        assert_eq!(
            tzif_footer(b"TZif2....\nEST5EDT,M3.2.0,M11.1.0\n").unwrap(),
            "EST5EDT,M3.2.0,M11.1.0"
        );
        assert!(tzif_footer(b"TZif\0....\n\n").is_err());
    }
}
