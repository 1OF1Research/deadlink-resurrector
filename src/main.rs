//! DeadLink Resurrector — Resurrection category, Kiroween 2026.
//!
//! Reads a list of URLs, probes each one (http over a plain TCP stream,
//! https through the system `openssl s_client` when available), classifies
//! the result, and writes a Markdown resurrection report: for every dead
//! or moved link, the exact observed status and a concrete next step.
//! When no TLS route is available, https:// URLs are reported honestly
//! as `needs-tls-probe` — never guessed dead. Probes run concurrently
//! (std threads, bounded), report order matches input order.
//! v0.3: optional `--follow` follows exactly one redirect hop and reports
//! the final verdict with the hop kept in the note; dead-client rows carry
//! a deterministic Wayback Machine lookup hint (no extra network call).
//! v0.4: optional `--json` writes the same rows as a JSON document
//! (checked / needing_resurrection totals plus one object per URL) for
//! machine use; Markdown stays the default and is byte-identical.
//! v0.5: optional `--html` writes the same rows as one self-contained
//! HTML page (no external assets) — the demo page shape a reader can
//! open in a browser as the working-app page.

use std::env;
use std::fs;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::process::{Command, ExitCode, Stdio};
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    Alive,
    Moved,
    DeadClient,
    DeadServer,
    Unreachable,
    NeedsTlsProbe,
    Invalid,
}

impl Verdict {
    pub fn label(&self) -> &'static str {
        match self {
            Verdict::Alive => "alive",
            Verdict::Moved => "moved",
            Verdict::DeadClient => "dead-client",
            Verdict::DeadServer => "dead-server",
            Verdict::Unreachable => "unreachable",
            Verdict::NeedsTlsProbe => "needs-tls-probe",
            Verdict::Invalid => "invalid",
        }
    }
    pub fn next_step(&self) -> &'static str {
        match self {
            Verdict::Alive => "Keep as-is; re-check on the next pass.",
            Verdict::Moved => "Update the link to the redirect target shown.",
            Verdict::DeadClient => "Find an archived copy or a successor page, then replace the link.",
            Verdict::DeadServer => "Retry later; if it stays 5xx, treat as dead and find a successor.",
            Verdict::Unreachable => "Check the hostname; if it no longer resolves, find a successor page.",
            Verdict::NeedsTlsProbe => "No TLS route was available on this machine (openssl not found); re-run where openssl exists — do not call this link dead yet.",
            Verdict::Invalid => "Fix the URL text itself — it does not parse as an http(s) URL.",
        }
    }
}

/// Classify an observed HTTP status code. 0 means the probe never got a response.
pub fn classify(status: Option<u16>) -> Verdict {
    match status {
        None => Verdict::Unreachable,
        Some(200..=299) => Verdict::Alive,
        Some(300..=399) => Verdict::Moved,
        Some(400..=499) => Verdict::DeadClient,
        Some(500..=599) => Verdict::DeadServer,
        Some(_) => Verdict::Invalid,
    }
}

/// Parse the host and path out of an http:// URL. Returns None if malformed.
pub fn parse_http_url(url: &str) -> Option<(String, String)> {
    let rest = url.strip_prefix("http://")?;
    if rest.is_empty() {
        return None;
    }
    let (host, path) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, "/"),
    };
    if host.is_empty() || host.contains(char::is_whitespace) {
        return None;
    }
    Some((host.to_string(), path.to_string()))
}

/// Parse the host and path out of an https:// URL. Returns None if malformed.
pub fn parse_https_url(url: &str) -> Option<(String, String)> {
    let rest = url.strip_prefix("https://")?;
    if rest.is_empty() {
        return None;
    }
    let (host, path) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, "/"),
    };
    if host.is_empty() || host.contains(char::is_whitespace) {
        return None;
    }
    Some((host.to_string(), path.to_string()))
}

/// Extract a Location header value, preserving its original case.
pub fn extract_location(response: &str) -> Option<String> {
    response.lines().find_map(|line| {
        let (name, value) = line.split_once(':')?;
        if name.trim().eq_ignore_ascii_case("location") {
            Some(value.trim().to_string())
        } else {
            None
        }
    })
}

/// Extract the HTTP status code from a response head.
pub fn extract_status(response: &str) -> Option<u16> {
    response
        .lines()
        .find(|l| l.starts_with("HTTP/"))
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|s| s.parse::<u16>().ok())
}

/// Probe one https:// URL through the system openssl s_client.
/// Returns (status, location, tls_available). tls_available=false means
/// no TLS route exists on this machine — the caller must stay honest.
pub fn probe_https(url: &str) -> (Option<u16>, Option<String>, bool) {
    let Some((host, path)) = parse_https_url(url) else {
        return (None, None, true);
    };
    let host_only = host.split(':').next().unwrap_or(&host).to_string();
    let connect = if host.contains(':') { host.clone() } else { format!("{host}:443") };
    let mut child = match Command::new("timeout")
        .args(["15", "openssl", "s_client", "-quiet", "-connect", &connect, "-servername", &host_only])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(c) => c,
        Err(_) => return (None, None, false),
    };
    if let Some(mut stdin) = child.stdin.take() {
        let req = format!("GET {path} HTTP/1.0\r\nHost: {host}\r\nUser-Agent: deadlink-resurrector/0.5\r\n\r\n");
        let _ = stdin.write_all(req.as_bytes());
    }
    let out = match child.wait_with_output() {
        Ok(o) => o,
        Err(_) => return (None, None, false),
    };
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    // openssl missing exits through `timeout` with no HTTP head at all and
    // s_client prints handshake chatter; only an HTTP/ head counts as a probe.
    if !text.contains("HTTP/") {
        // Distinguish "openssl absent" (command not found -> spawn of timeout
        // still succeeds, exit 127) from "host unreachable" (no response).
        if out.status.code() == Some(127) {
            return (None, None, false);
        }
        return (None, None, true);
    }
    (extract_status(&text), extract_location(&text), true)
}

/// Probe one http:// URL with a real GET. Returns (status, location-header).
pub fn probe_http(url: &str) -> (Option<u16>, Option<String>) {
    let Some((host, path)) = parse_http_url(url) else {
        return (None, None);
    };
    let addr = if host.contains(':') { host.clone() } else { format!("{host}:80") };
    let Ok(mut stream) = TcpStream::connect(&addr) else {
        return (None, None);
    };
    let _ = stream.set_read_timeout(Some(Duration::from_secs(10)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(10)));
    let req = format!("GET {path} HTTP/1.0\r\nHost: {host}\r\nUser-Agent: deadlink-resurrector/0.5\r\n\r\n");
    if stream.write_all(req.as_bytes()).is_err() {
        return (None, None);
    }
    let mut buf = String::new();
    if stream.read_to_string(&mut buf).is_err() {
        return (None, None);
    }
    (extract_status(&buf), extract_location(&buf))
}

/// Deterministic Wayback Machine lookup URL for a dead link.
/// Pure string construction — no network call, so the hint is always available.
pub fn wayback_hint(url: &str) -> String {
    format!("https://web.archive.org/web/*/{url}")
}

/// Resolve a redirect Location against the original URL.
/// Absolute http(s) targets pass through; root-relative and path-relative
/// targets are joined against the original scheme/host/path.
pub fn resolve_redirect(original: &str, location: &str) -> Option<String> {
    if location.starts_with("http://") || location.starts_with("https://") {
        return Some(location.to_string());
    }
    let (scheme, rest) = original.split_once("://")?;
    let (host, path) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, "/"),
    };
    if let Some(stripped) = location.strip_prefix('/') {
        return Some(format!("{scheme}://{host}/{stripped}"));
    }
    // Path-relative: replace the last path segment of the original.
    let dir = match path.rfind('/') {
        Some(i) => &path[..=i],
        None => "/",
    };
    Some(format!("{scheme}://{host}{dir}{location}"))
}

/// Follow exactly one redirect hop. Returns (final_verdict, final_status, note).
/// If the first probe was not `moved` with a resolvable target, it is returned unchanged.
pub fn follow_one_hop(url: &str, verdict: Verdict, status: Option<u16>, location: Option<String>) -> (Verdict, Option<u16>, Option<String>) {
    if verdict != Verdict::Moved {
        return (verdict, status, location);
    }
    let Some(loc) = location else { return (verdict, status, None) };
    let Some(target) = resolve_redirect(url, &loc) else { return (verdict, status, Some(loc)) };
    let (v2, s2, _loc2) = probe(&target);
    let note = format!("{loc} -> final: {} ({})", v2.label(), s2.map(|s| s.to_string()).unwrap_or_else(|| "—".into()));
    (v2, s2, Some(note))
}

pub fn probe(url: &str) -> (Verdict, Option<u16>, Option<String>) {
    if url.starts_with("https://") {
        if parse_https_url(url).is_none() {
            return (Verdict::Invalid, None, None);
        }
        let (status, location, tls_available) = probe_https(url);
        if !tls_available {
            return (Verdict::NeedsTlsProbe, None, None);
        }
        return (classify(status), status, location);
    }
    if !url.starts_with("http://") {
        return (Verdict::Invalid, None, None);
    }
    if parse_http_url(url).is_none() {
        return (Verdict::Invalid, None, None);
    }
    let (status, location) = probe_http(url);
    (classify(status), status, location)
}

/// Escape a string for inclusion in a JSON string literal.
pub fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

/// Render rows as JSON. Same data as the Markdown report: totals plus
/// one object per URL, in input order. `status`/`note` are null when absent.
pub fn render_json(rows: &[(String, Verdict, Option<u16>, Option<String>)]) -> String {
    let dead = rows.iter().filter(|r| matches!(r.1, Verdict::DeadClient | Verdict::DeadServer | Verdict::Unreachable)).count();
    let mut out = String::new();
    out.push_str("{\n");
    out.push_str(&format!("  \"checked\": {},\n  \"needing_resurrection\": {},\n  \"results\": [\n", rows.len(), dead));
    for (i, (url, verdict, status, location)) in rows.iter().enumerate() {
        let step = if *verdict == Verdict::DeadClient {
            format!("{} Archived-copy lookup: {}", verdict.next_step(), wayback_hint(url))
        } else {
            verdict.next_step().to_string()
        };
        let status_s = status.map(|s| s.to_string()).unwrap_or_else(|| "null".into());
        let note_s = match location {
            Some(l) => format!("\"{}\"", json_escape(l)),
            None => "null".into(),
        };
        out.push_str(&format!(
            "    {{\"url\": \"{}\", \"verdict\": \"{}\", \"status\": {}, \"note\": {}, \"next_step\": \"{}\"}}{}\n",
            json_escape(url), verdict.label(), status_s, note_s, json_escape(&step),
            if i + 1 < rows.len() { "," } else { "" }
        ));
    }
    out.push_str("  ]\n}\n");
    out
}

/// Escape a string for inclusion in HTML text or attribute content.
pub fn html_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            c => out.push(c),
        }
    }
    out
}

/// Render rows as one self-contained HTML page. Same data as the
/// Markdown and JSON reports; no external assets, so the file can be
/// opened directly or served as a static demo page.
pub fn render_html(rows: &[(String, Verdict, Option<u16>, Option<String>)]) -> String {
    let dead = rows.iter().filter(|r| matches!(r.1, Verdict::DeadClient | Verdict::DeadServer | Verdict::Unreachable)).count();
    let mut out = String::new();
    out.push_str("<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n");
    out.push_str("<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n");
    out.push_str("<title>DeadLink Resurrector — report</title>\n");
    out.push_str("<style>body{font-family:system-ui,sans-serif;margin:2rem;line-height:1.5;color:#1a1a1a}table{border-collapse:collapse;width:100%}th,td{border:1px solid #ccc;padding:.45rem .6rem;text-align:left;vertical-align:top}th{background:#f2f2f2}code{word-break:break-all}</style>\n");
    out.push_str("</head>\n<body>\n<h1>DeadLink Resurrector — report</h1>\n");
    out.push_str(&format!("<p>Total: {} checked, {} needing resurrection.</p>\n", rows.len(), dead));
    out.push_str("<table>\n<thead><tr><th>URL</th><th>Verdict</th><th>Status</th><th>Redirect / note</th><th>Next step</th></tr></thead>\n<tbody>\n");
    for (url, verdict, status, location) in rows {
        let step = if *verdict == Verdict::DeadClient {
            format!("{} Archived-copy lookup: {}", verdict.next_step(), wayback_hint(url))
        } else {
            verdict.next_step().to_string()
        };
        out.push_str(&format!(
            "<tr><td><code>{}</code></td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>\n",
            html_escape(url),
            html_escape(verdict.label()),
            status.map(|s| s.to_string()).unwrap_or_else(|| "—".into()),
            html_escape(&location.clone().unwrap_or_else(|| "—".into())),
            html_escape(&step)
        ));
    }
    out.push_str("</tbody>\n</table>\n</body>\n</html>\n");
    out
}

fn main() -> ExitCode {
    let raw_args: Vec<String> = env::args().collect();
    let follow = raw_args.iter().any(|a| a == "--follow");
    let json = raw_args.iter().any(|a| a == "--json");
    let html = raw_args.iter().any(|a| a == "--html");
    let args: Vec<String> = raw_args.into_iter().filter(|a| a != "--follow" && a != "--json" && a != "--html").collect();
    if args.len() != 3 || (json && html) {
        eprintln!("usage: deadlink-resurrector [--follow] [--json|--html] <urls.txt> <report.md|report.json|report.html>");
        return ExitCode::from(2);
    }
    let input = match fs::read_to_string(&args[1]) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("cannot read {}: {e}", args[1]);
            return ExitCode::from(2);
        }
    };
    let urls: Vec<String> = input
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .collect();
    // Bounded concurrency: chunks of 8 scoped threads, input order preserved.
    let mut rows = Vec::new();
    for chunk in urls.chunks(8) {
        let probed: Vec<_> = std::thread::scope(|scope| {
            let handles: Vec<_> = chunk
                .iter()
                .map(|url| scope.spawn(|| {
                    let (v, s, loc) = probe(url);
                    if follow { follow_one_hop(url, v, s, loc) } else { (v, s, loc) }
                }))
                .collect();
            handles
                .into_iter()
                .map(|h| h.join().unwrap_or((Verdict::Unreachable, None, None)))
                .collect()
        });
        for (url, (verdict, status, location)) in chunk.iter().zip(probed) {
            rows.push((url.clone(), verdict, status, location));
        }
    }
    let dead = rows.iter().filter(|r| matches!(r.1, Verdict::DeadClient | Verdict::DeadServer | Verdict::Unreachable)).count();
    let out = if json {
        render_json(&rows)
    } else if html {
        render_html(&rows)
    } else {
        let mut md = String::from("# DeadLink Resurrector — report\n\n");
        md.push_str("| URL | Verdict | Status | Redirect / note | Next step |\n|---|---|---|---|---|\n");
        for (url, verdict, status, location) in &rows {
            let step = if *verdict == Verdict::DeadClient {
                format!("{} Archived-copy lookup: {}", verdict.next_step(), wayback_hint(url))
            } else {
                verdict.next_step().to_string()
            };
            md.push_str(&format!(
                "| {} | {} | {} | {} | {} |\n",
                url,
                verdict.label(),
                status.map(|s| s.to_string()).unwrap_or_else(|| "—".into()),
                location.clone().unwrap_or_else(|| "—".into()),
                step
            ));
        }
        md.push_str(&format!("\nTotal: {} checked, {} needing resurrection.\n", rows.len(), dead));
        md
    };
    if let Err(e) = fs::write(&args[2], out) {
        eprintln!("cannot write {}: {e}", args[2]);
        return ExitCode::from(2);
    }
    println!("checked={} needing_resurrection={}", rows.len(), dead);
    ExitCode::SUCCESS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_boundaries() {
        assert_eq!(classify(Some(200)), Verdict::Alive);
        assert_eq!(classify(Some(299)), Verdict::Alive);
        assert_eq!(classify(Some(301)), Verdict::Moved);
        assert_eq!(classify(Some(404)), Verdict::DeadClient);
        assert_eq!(classify(Some(410)), Verdict::DeadClient);
        assert_eq!(classify(Some(503)), Verdict::DeadServer);
        assert_eq!(classify(None), Verdict::Unreachable);
    }

    #[test]
    fn parse_http() {
        assert_eq!(parse_http_url("http://example.com/a/b"), Some(("example.com".into(), "/a/b".into())));
        assert_eq!(parse_http_url("http://example.com"), Some(("example.com".into(), "/".into())));
        assert_eq!(parse_http_url("http://"), None);
        assert_eq!(parse_http_url("https://example.com/"), None);
    }

    #[test]
    fn parse_https() {
        assert_eq!(parse_https_url("https://example.com/a"), Some(("example.com".into(), "/a".into())));
        assert_eq!(parse_https_url("https://example.com"), Some(("example.com".into(), "/".into())));
        assert_eq!(parse_https_url("https://"), None);
        assert_eq!(parse_https_url("http://example.com/"), None);
    }

    #[test]
    fn location_preserves_case() {
        let resp = "HTTP/1.1 301 Moved\r\nLocation: https://Example.COM/NewPath\r\n\r\n";
        assert_eq!(extract_location(resp), Some("https://Example.COM/NewPath".into()));
        assert_eq!(extract_status(resp), Some(301));
        assert_eq!(extract_location("HTTP/1.1 200 OK\r\n\r\n"), None);
    }

    #[test]
    fn https_malformed_is_invalid() {
        let (v, _, _) = probe("https://");
        assert_eq!(v, Verdict::Invalid);
    }

    #[test]
    fn garbage_is_invalid() {
        let (v, _, _) = probe("not a url");
        assert_eq!(v, Verdict::Invalid);
    }

    #[test]
    fn wayback_hint_is_deterministic() {
        assert_eq!(wayback_hint("http://example.com/gone"), "https://web.archive.org/web/*/http://example.com/gone");
    }

    #[test]
    fn resolve_redirect_absolute_and_relative() {
        assert_eq!(resolve_redirect("http://example.com/a", "https://Example.COM/New"), Some("https://Example.COM/New".into()));
        assert_eq!(resolve_redirect("http://example.com/a/b", "/new"), Some("http://example.com/new".into()));
        assert_eq!(resolve_redirect("http://example.com/a/b", "c"), Some("http://example.com/a/c".into()));
        assert_eq!(resolve_redirect("https://example.com/a/", "c"), Some("https://example.com/a/c".into()));
    }

    #[test]
    fn json_escape_handles_quotes_and_controls() {
        assert_eq!(json_escape("a\"b\\c"), "a\\\"b\\\\c");
        assert_eq!(json_escape("x\ny\t"), "x\\ny\\t");
        assert_eq!(json_escape("plain"), "plain");
    }

    #[test]
    fn render_json_shape_and_nulls() {
        let rows = vec![
            ("http://example.com/".to_string(), Verdict::Alive, Some(200), None),
            ("not a url".to_string(), Verdict::Invalid, None, None),
        ];
        let out = render_json(&rows);
        assert!(out.contains("\"checked\": 2"));
        assert!(out.contains("\"needing_resurrection\": 0"));
        assert!(out.contains("\"verdict\": \"alive\", \"status\": 200, \"note\": null"));
        assert!(out.contains("\"verdict\": \"invalid\", \"status\": null, \"note\": null"));
    }

    #[test]
    fn html_escape_handles_markup() {
        assert_eq!(html_escape("a<b>&\"'"), "a&lt;b&gt;&amp;&quot;&#39;");
        assert_eq!(html_escape("plain text"), "plain text");
    }

    #[test]
    fn render_html_shape_and_escaping() {
        let rows = vec![
            ("http://example.com/?a=1&b=2".to_string(), Verdict::Alive, Some(200), None),
            ("not a url".to_string(), Verdict::Invalid, None, None),
        ];
        let out = render_html(&rows);
        assert!(out.starts_with("<!DOCTYPE html>"));
        assert!(out.contains("Total: 2 checked, 0 needing resurrection."));
        assert!(out.contains("http://example.com/?a=1&amp;b=2"));
        assert!(out.contains("<td>alive</td><td>200</td>"));
        assert!(out.contains("<td>invalid</td><td>—</td>"));
        assert!(!out.contains("src="));
        assert!(!out.contains("http://example.com/?a=1&b=2\""));
    }

    #[test]
    fn follow_non_moved_is_unchanged() {
        let (v, s, loc) = follow_one_hop("http://example.com/", Verdict::Alive, Some(200), None);
        assert_eq!((v, s, loc), (Verdict::Alive, Some(200), None));
    }
}
