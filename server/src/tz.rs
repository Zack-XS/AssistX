//! Time zones from the system tz database (TZif files + POSIX TZ footer), so schedules keep
//! their local time across daylight saving changes (docs/00 Scheduler). No extra crates.

use std::path::Path;

const ZONEINFO: &[&str] = &["/usr/share/zoneinfo", "/var/db/timezone/zoneinfo"];

/// Zone name from /etc/localtime, e.g. "Europe/Berlin".
pub fn system_zone() -> Option<String> {
    let target = std::fs::read_link("/etc/localtime").ok()?;
    let s = target.to_string_lossy();
    let i = s.find("zoneinfo/")?;
    Some(s[i + 9..].to_string())
}

#[derive(Clone, Debug)]
struct Rule {
    std_off: i64,
    dst: Option<(i64, RuleDate, i64, RuleDate, i64)>, // dst_off, start, start_secs, end, end_secs
}

#[derive(Clone, Copy, Debug)]
struct RuleDate {
    month: u32,
    week: u32,
    weekday: u32,
}

#[derive(Clone, Debug)]
pub struct Zone {
    transitions: Vec<(i64, i64)>, // (utc time, offset after it)
    first_off: i64,
    footer: Option<Rule>,
}

fn be32(b: &[u8], i: usize) -> Option<u32> {
    b.get(i..i + 4).map(|x| u32::from_be_bytes([x[0], x[1], x[2], x[3]]))
}

impl Zone {
    pub fn utc() -> Zone {
        Zone { transitions: vec![], first_off: 0, footer: None }
    }

    /// Load a zone by IANA name. Names with `..` are refused.
    pub fn load(name: &str) -> Option<Zone> {
        if name == "UTC" || name == "Etc/UTC" {
            return Some(Zone::utc());
        }
        if name.contains("..") || name.starts_with('/') {
            return None;
        }
        let bytes = ZONEINFO.iter().find_map(|d| std::fs::read(Path::new(d).join(name)).ok())?;
        Zone::parse(&bytes)
    }

    fn parse(b: &[u8]) -> Option<Zone> {
        if b.get(0..4)? != b"TZif" {
            return None;
        }
        let version = *b.get(4)?;
        let counts = |at: usize| -> Option<[usize; 6]> {
            let mut c = [0usize; 6];
            for (k, v) in c.iter_mut().enumerate() {
                *v = be32(b, at + 20 + k * 4)? as usize;
            }
            Some(c)
        };
        let [isut, isstd, leap, timecnt, typecnt, charcnt] = counts(0)?;
        let v1_len = timecnt * 5 + typecnt * 6 + charcnt + leap * 8 + isstd + isut;
        let (base, wide) = if version >= b'2' { (44 + v1_len, true) } else { (0, false) };
        let [isut, isstd, leap, timecnt, typecnt, charcnt] = counts(base)?;
        let tsz = if wide { 8 } else { 4 };
        let mut p = base + 44;
        let mut times = Vec::with_capacity(timecnt);
        for k in 0..timecnt {
            let at = p + k * tsz;
            let t = if wide {
                i64::from_be_bytes(b.get(at..at + 8)?.try_into().ok()?)
            } else {
                be32(b, at)? as i32 as i64
            };
            times.push(t);
        }
        p += timecnt * tsz;
        let idx: Vec<usize> = b.get(p..p + timecnt)?.iter().map(|x| *x as usize).collect();
        p += timecnt;
        let mut offs = Vec::with_capacity(typecnt);
        for k in 0..typecnt {
            offs.push(be32(b, p + k * 6)? as i32 as i64);
        }
        p += typecnt * 6 + charcnt + leap * (tsz + 4) + isstd + isut;
        let footer = if wide {
            b.get(p..).and_then(|rest| std::str::from_utf8(rest).ok()).and_then(|s| parse_posix(s.trim_matches('\n')))
        } else {
            None
        };
        let transitions = times.into_iter().zip(idx).filter_map(|(t, i)| offs.get(i).map(|o| (t, *o))).collect();
        Some(Zone { transitions, first_off: *offs.first().unwrap_or(&0), footer })
    }

    /// Seconds east of UTC at a UTC instant.
    pub fn offset_at(&self, utc: i64) -> i64 {
        match self.transitions.last() {
            Some((t, _)) if utc >= *t && self.footer.is_some() => self.footer.as_ref().unwrap().offset_at(utc),
            _ => match self.transitions.iter().rposition(|(t, _)| *t <= utc) {
                Some(i) => self.transitions[i].1,
                None => self.footer.as_ref().filter(|_| self.transitions.is_empty()).map(|f| f.offset_at(utc)).unwrap_or(self.first_off),
            },
        }
    }

    /// UTC instant for a local wall time. In a DST gap, moves forward; in an overlap, takes the first.
    pub fn local_to_utc(&self, local: i64) -> i64 {
        let guess = local - self.offset_at(local);
        let a = local - self.offset_at(guess);
        let b = local - self.offset_at(a);
        if a + self.offset_at(a) == local { a.min(if b + self.offset_at(b) == local { b } else { a }) } else { b }
    }
}

impl Rule {
    fn offset_at(&self, utc: i64) -> i64 {
        let Some((dst_off, start, start_s, end, end_s)) = self.dst else { return self.std_off };
        let year = civil(utc + self.std_off).0;
        // Rule times are local wall times: start in std time, end in dst time.
        let s = rule_day(year, start) + start_s - self.std_off;
        let e = rule_day(year, end) + end_s - dst_off;
        let in_dst = if s < e { utc >= s && utc < e } else { utc >= s || utc < e };
        if in_dst { dst_off } else { self.std_off }
    }
}

fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m as i64 + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// (year, month, day, seconds of day) for a timestamp.
pub fn civil(ts: i64) -> (i64, u32, u32, i64) {
    let days = ts.div_euclid(86_400);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (yoe + era * 400 + if m <= 2 { 1 } else { 0 }, m, d, ts.rem_euclid(86_400))
}

/// Seconds at local midnight, counted as if local time were UTC.
pub fn local_midnight(y: i64, m: u32, d: u32) -> i64 {
    days_from_civil(y, m, d) * 86_400
}

fn rule_day(year: i64, r: RuleDate) -> i64 {
    // Mm.w.d: day d (0 = Sunday) of week w (5 = last) of month m.
    let first = days_from_civil(year, r.month, 1);
    let wd_first = (first + 4).rem_euclid(7) as u32; // 1970-01-01 was a Thursday
    let mut day = 1 + (r.weekday + 7 - wd_first) % 7 + (r.week.saturating_sub(1)) * 7;
    let dim = [31, if (year % 4 == 0 && year % 100 != 0) || year % 400 == 0 { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31][(r.month - 1) as usize];
    while day > dim {
        day -= 7;
    }
    days_from_civil(year, r.month, day) * 86_400
}

/// Parse a POSIX TZ string like "CET-1CEST,M3.5.0,M10.5.0/3". Only M-rules are supported.
fn parse_posix(s: &str) -> Option<Rule> {
    let mut p = s;
    let name = |p: &mut &str| -> Option<()> {
        if let Some(rest) = p.strip_prefix('<') {
            let e = rest.find('>')?;
            *p = &rest[e + 1..];
        } else {
            let e = p.find(|c: char| !c.is_ascii_alphabetic()).unwrap_or(p.len());
            if e < 3 { return None; }
            *p = &p[e..];
        }
        Some(())
    };
    let offset = |p: &mut &str| -> Option<i64> {
        let e = p.find(|c: char| !(c.is_ascii_digit() || c == ':' || c == '+' || c == '-')).unwrap_or(p.len());
        let (txt, rest) = p.split_at(e);
        *p = rest;
        hms(txt)
    };
    name(&mut p)?;
    let std_off = -offset(&mut p)?;
    if p.is_empty() {
        return Some(Rule { std_off, dst: None });
    }
    name(&mut p)?;
    let dst_off = if p.starts_with(',') { std_off + 3600 } else { -offset(&mut p)? };
    let rules: Vec<&str> = p.trim_start_matches(',').split(',').collect();
    if rules.len() != 2 {
        return Some(Rule { std_off, dst: None });
    }
    let date = |r: &str| -> Option<(RuleDate, i64)> {
        let (d, t) = r.split_once('/').map(|(a, b)| (a, Some(b))).unwrap_or((r, None));
        let mut it = d.strip_prefix('M')?.split('.');
        let rd = RuleDate { month: it.next()?.parse().ok()?, week: it.next()?.parse().ok()?, weekday: it.next()?.parse().ok()? };
        if !(1..=12).contains(&rd.month) { return None; }
        Some((rd, t.map(hms).unwrap_or(Some(7200))?))
    };
    let (s, ss) = date(rules[0])?;
    let (e, es) = date(rules[1])?;
    Some(Rule { std_off, dst: Some((dst_off, s, ss, e, es)) })
}

fn hms(t: &str) -> Option<i64> {
    let (sign, t) = match t.strip_prefix('-') {
        Some(r) => (-1, r),
        None => (1, t.trim_start_matches('+')),
    };
    let mut parts = t.split(':');
    let h: i64 = parts.next()?.parse().ok()?;
    let m: i64 = parts.next().map(|x| x.parse().ok()).unwrap_or(Some(0))?;
    let s: i64 = parts.next().map(|x| x.parse().ok()).unwrap_or(Some(0))?;
    Some(sign * (h * 3600 + m * 60 + s))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn posix_rule_berlin() {
        let r = parse_posix("CET-1CEST,M3.5.0,M10.5.0/3").unwrap();
        // 2026-01-15 12:00Z winter, 2026-07-15 12:00Z summer.
        assert_eq!(r.offset_at(local_midnight(2026, 1, 15) + 43200), 3600);
        assert_eq!(r.offset_at(local_midnight(2026, 7, 15) + 43200), 7200);
        // DST starts 2026-03-29 01:00Z.
        assert_eq!(r.offset_at(local_midnight(2026, 3, 29) + 3599), 3600);
        assert_eq!(r.offset_at(local_midnight(2026, 3, 29) + 3600), 7200);
    }

    #[test]
    fn system_zone_file_if_present() {
        if let Some(z) = Zone::load("America/New_York") {
            assert_eq!(z.offset_at(local_midnight(2026, 1, 15) + 43200), -5 * 3600);
            assert_eq!(z.offset_at(local_midnight(2026, 7, 15) + 43200), -4 * 3600);
            assert_eq!(z.offset_at(local_midnight(2040, 7, 15) + 43200), -4 * 3600, "footer rule beyond table");
            let local_8am = local_midnight(2026, 7, 15) + 8 * 3600;
            assert_eq!(z.local_to_utc(local_8am), local_8am + 4 * 3600);
        }
        assert!(Zone::load("../etc/passwd").is_none());
    }
}
