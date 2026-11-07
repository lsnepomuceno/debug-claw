//! Path mapping inference: from the `fileuri` of the entry script in the
//! DBGp `init` packet and the local project tree, find the remote prefix
//! and the local root, then translate paths both ways.
//!
//! Paths are kept as a root plus components, so POSIX and Windows paths
//! go through the same code. Nothing here touches the file system.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Style {
    Posix,
    Windows,
}

/// An absolute path split into components.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AbsPath {
    pub style: Style,
    /// Windows only: `C:` (drive letter upper-cased) or `//server/share`
    /// for UNC paths. Empty for POSIX paths.
    pub root: String,
    pub parts: Vec<String>,
}

impl AbsPath {
    pub fn join(&self, rel: &[String]) -> AbsPath {
        let mut parts = self.parts.clone();
        parts.extend(rel.iter().cloned());
        AbsPath { parts, ..self.clone() }
    }

    /// The path as the operating system writes it.
    pub fn native(&self) -> String {
        match self.style {
            Style::Posix => format!("/{}", self.parts.join("/")),
            Style::Windows => {
                let root = self.root.replace('/', "\\");
                format!("{root}\\{}", self.parts.join("\\"))
            }
        }
    }

    /// The path as a `file://` URI, encoded the way Xdebug 3.5 encodes
    /// it (`xdebug_path_to_url` in `src/lib/usefulstuff.c`).
    pub fn uri(&self) -> String {
        let body = self.parts.iter().map(|p| encode(p)).collect::<Vec<_>>().join("/");
        match (self.style, self.root.as_str()) {
            (Style::Posix, _) => format!("file:///{body}"),
            (Style::Windows, root) if root.starts_with("//") => format!("file:{root}/{body}"),
            (Style::Windows, drive) => format!("file:///{drive}/{body}"),
        }
    }
}

/// Percent-encodes one component like Xdebug's `xdebug_raw_url_encode`:
/// letters, digits, `-`, `.`, `_` and `:` stay, everything else is
/// encoded, including `~` and every non-ASCII byte.
fn encode(part: &str) -> String {
    let mut out = String::new();
    for b in part.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b':') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

fn decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or("");
            if let Ok(v) = u8::from_str_radix(hex, 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn is_drive(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 2 && b[0].is_ascii_alphabetic() && b[1] == b':'
}

/// Splits on the separators, drops empty and `.` components and applies
/// `..` lexically.
fn components(rest: &str, seps: &[char]) -> Vec<String> {
    let mut parts: Vec<String> = Vec::new();
    for p in rest.split(|c| seps.contains(&c)) {
        match p {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            p => parts.push(p.to_string()),
        }
    }
    parts
}

/// Parses a `file://` URI as Xdebug sends it. `file:///var/www/x.php`
/// is POSIX, `file:///C:/x.php` is a Windows drive path and
/// `file://server/share/x.php` is a Windows UNC path. Other schemes
/// (`dbgp://` for eval'd code, `phar://`) are not files and are refused.
pub fn parse_file_uri(uri: &str) -> Result<AbsPath, String> {
    let rest = uri
        .strip_prefix("file://")
        .ok_or_else(|| format!("not a file URI: {uri}"))?;
    let rest = decode(rest);
    if let Some(path) = rest.strip_prefix('/') {
        let first = path.split('/').next().unwrap_or("");
        if is_drive(first) {
            return Ok(AbsPath {
                style: Style::Windows,
                root: first.to_ascii_uppercase(),
                parts: components(&path[2..], &['/', '\\']),
            });
        }
        return Ok(AbsPath { style: Style::Posix, root: String::new(), parts: components(path, &['/']) });
    }
    // file://server/share/path: a UNC path.
    let mut it = rest.splitn(3, '/');
    let (server, share) = (it.next().unwrap_or(""), it.next().unwrap_or(""));
    if server.is_empty() || share.is_empty() {
        return Err(format!("bad UNC URI: {uri}"));
    }
    Ok(AbsPath {
        style: Style::Windows,
        root: format!("//{server}/{share}"),
        parts: components(it.next().unwrap_or(""), &['/', '\\']),
    })
}

/// Parses a path written the local system's way: `/home/x` on POSIX,
/// `C:\x`, `c:/x`, `\\server\share\x` or `\\?\C:\x` on Windows.
pub fn parse_native(path: &str, style: Style) -> Result<AbsPath, String> {
    match style {
        Style::Posix => {
            let rest = path.strip_prefix('/').ok_or_else(|| format!("not absolute: {path}"))?;
            Ok(AbsPath { style, root: String::new(), parts: components(rest, &['/']) })
        }
        Style::Windows => {
            let path = path.strip_prefix(r"\\?\").unwrap_or(path);
            let seps = ['/', '\\'];
            if let Some(unc) = path.strip_prefix(r"\\").or_else(|| path.strip_prefix("//")) {
                let mut it = unc.splitn(3, seps);
                let (server, share) = (it.next().unwrap_or(""), it.next().unwrap_or(""));
                if server.is_empty() || share.is_empty() {
                    return Err(format!("bad UNC path: {path}"));
                }
                return Ok(AbsPath {
                    style,
                    root: format!("//{server}/{share}"),
                    parts: components(it.next().unwrap_or(""), &seps),
                });
            }
            if path.len() >= 2 && is_drive(&path[..2]) {
                return Ok(AbsPath {
                    style,
                    root: path[..2].to_ascii_uppercase(),
                    parts: components(&path[2..], &seps),
                });
            }
            Err(format!("not absolute: {path}"))
        }
    }
}

fn eq(a: &str, b: &str, ci: bool) -> bool {
    if ci { a.to_lowercase() == b.to_lowercase() } else { a == b }
}

fn same_root(a: &AbsPath, b: &AbsPath) -> bool {
    a.style == b.style && a.root.eq_ignore_ascii_case(&b.root)
}

/// Is `prefix` a component-wise prefix of `path`?
fn strip<'a>(path: &'a AbsPath, prefix: &AbsPath, ci: bool) -> Option<&'a [String]> {
    if !same_root(path, prefix) || path.parts.len() < prefix.parts.len() {
        return None;
    }
    let ok = prefix.parts.iter().zip(&path.parts).all(|(a, b)| eq(a, b, ci));
    ok.then(|| &path.parts[prefix.parts.len()..])
}

fn common_suffix(a: &[String], b: &[String], ci: bool) -> usize {
    a.iter().rev().zip(b.iter().rev()).take_while(|(x, y)| eq(x, y, ci)).count()
}

/// A learned mapping: `remote` (a prefix inside the container or on the
/// PHP host) corresponds to `local` (a folder in the local tree).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mapping {
    pub remote: AbsPath,
    pub local: AbsPath,
    /// The local file system ignores case (Windows, macOS by default).
    pub local_ci: bool,
}

impl Mapping {
    /// A remote Windows file system ignores case too; a Linux container
    /// does not.
    fn remote_ci(&self) -> bool {
        self.remote.style == Style::Windows
    }

    /// Local path to remote path, keeping the case of the local suffix.
    pub fn to_remote(&self, local: &AbsPath) -> Option<AbsPath> {
        strip(local, &self.local, self.local_ci).map(|rest| self.remote.join(rest))
    }

    pub fn to_local(&self, remote: &AbsPath) -> Option<AbsPath> {
        strip(remote, &self.remote, self.remote_ci()).map(|rest| self.local.join(rest))
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum Inference {
    /// One mapping explains the entry script best.
    Unique { mapping: Mapping, matched: usize },
    /// Several local files match the same number of trailing components.
    /// The caller tries to break the tie (comparing file contents through
    /// the DBGp `source` command) and asks the user if that fails.
    Ambiguous { candidates: Vec<Mapping>, matched: usize },
    /// No local file has the entry script's name.
    NoMatch,
}

/// Infers the mapping by longest common suffix.
///
/// `local_files` are the files under `local_root` whose name matches the
/// entry script's name, as relative components (the tree walker finds
/// them). The candidate sharing the most trailing components with the
/// remote path wins; a tie is reported as ambiguous.
pub fn infer(remote_entry: &AbsPath, local_root: &AbsPath, local_files: &[Vec<String>], local_ci: bool) -> Inference {
    // The suffix is compared with the looser rule of the two sides: a
    // case-insensitive local tree may hold `Public/Index.php` for a
    // remote `public/index.php` on a bind mount from macOS.
    let ci = local_ci || remote_entry.style == Style::Windows;
    let mut best = 0;
    let mut found: Vec<Mapping> = Vec::new();
    for rel in local_files {
        let k = common_suffix(&remote_entry.parts, rel, ci);
        if k == 0 || k < best {
            continue;
        }
        let mapping = Mapping {
            remote: AbsPath {
                parts: remote_entry.parts[..remote_entry.parts.len() - k].to_vec(),
                ..remote_entry.clone()
            },
            local: local_root.join(&rel[..rel.len() - k]),
            local_ci,
        };
        if k > best {
            best = k;
            found.clear();
        }
        if !found.contains(&mapping) {
            found.push(mapping);
        }
    }
    match found.len() {
        0 => Inference::NoMatch,
        1 => Inference::Unique { mapping: found.remove(0), matched: best },
        _ => Inference::Ambiguous { candidates: found, matched: best },
    }
}

/// Mappings learned in earlier sessions, reused while they still explain
/// the entry script of a new session.
#[derive(Default, Debug)]
pub struct Learned {
    pub mappings: Vec<Mapping>,
}

impl Learned {
    /// The mapping with the longest remote prefix that contains `remote`.
    /// The caller still checks that the translated local file exists.
    pub fn lookup(&self, remote: &AbsPath) -> Option<&Mapping> {
        self.mappings
            .iter()
            .filter(|m| m.to_local(remote).is_some())
            .max_by_key(|m| m.remote.parts.len())
    }

    pub fn learn(&mut self, mapping: Mapping) {
        self.mappings.retain(|m| m.remote != mapping.remote);
        self.mappings.push(mapping);
    }

    /// Drops a mapping, for when it stops explaining what Xdebug sends.
    pub fn forget(&mut self, mapping: &Mapping) {
        self.mappings.retain(|m| m != mapping);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn posix(p: &str) -> AbsPath {
        parse_native(p, Style::Posix).unwrap()
    }

    fn win(p: &str) -> AbsPath {
        parse_native(p, Style::Windows).unwrap()
    }

    fn rel(p: &str) -> Vec<String> {
        components(p, &['/', '\\'])
    }

    fn files(list: &[&str]) -> Vec<Vec<String>> {
        list.iter().map(|p| rel(p)).collect()
    }

    /// `fileuri` of an `init` packet in `recorded/`.
    fn recorded(name: &str) -> AbsPath {
        let path = format!("{}/recorded/{name}", env!("CARGO_MANIFEST_DIR"));
        let xml = std::fs::read_to_string(&path).unwrap();
        let uri = crate::dbgp::attrs(&xml, "init").unwrap()["fileuri"].clone();
        parse_file_uri(&uri).unwrap()
    }

    fn unique(inf: Inference) -> Mapping {
        match inf {
            Inference::Unique { mapping, .. } => mapping,
            other => panic!("expected a unique mapping, got {other:?}"),
        }
    }

    /// The local tree of `spikes/testbed/app`, as the walker returns the
    /// files named `index.php`.
    const TESTBED_INDEX: &[&str] = &["index.php", "public/index.php"];

    #[test]
    fn linux_public_layout_from_recorded_init() {
        let remote = recorded("linux-php85-public.xml");
        assert_eq!(remote.native(), "/var/www/html/public/index.php");
        let root = posix("/work/spikes/testbed/app");
        let m = unique(infer(&remote, &root, &files(TESTBED_INDEX), false));
        assert_eq!(m.remote.native(), "/var/www/html");
        assert_eq!(m.local.native(), "/work/spikes/testbed/app");
        let bp = posix("/work/spikes/testbed/app/src/Graph.php");
        assert_eq!(m.to_remote(&bp).unwrap().uri(), "file:///var/www/html/src/Graph.php");
    }

    #[test]
    fn linux_root_layout_is_a_tie_on_suffix_alone() {
        // /srv/app/index.php shares only "index.php" with both local
        // files, so the suffix cannot choose. The listener breaks the tie
        // by comparing contents through `source`.
        let remote = recorded("linux-php83-root.xml");
        assert_eq!(remote.native(), "/srv/app/index.php");
        let root = posix("/work/spikes/testbed/app");
        match infer(&remote, &root, &files(TESTBED_INDEX), false) {
            Inference::Ambiguous { candidates, matched } => {
                assert_eq!(matched, 1);
                let locals: Vec<_> = candidates.iter().map(|m| m.local.native()).collect();
                assert_eq!(locals, ["/work/spikes/testbed/app", "/work/spikes/testbed/app/public"]);
                assert!(candidates.iter().all(|m| m.remote.native() == "/srv/app"));
            }
            other => panic!("expected a tie, got {other:?}"),
        }
    }

    #[test]
    fn longest_suffix_wins_over_a_shorter_one() {
        let remote = parse_file_uri("file:///app/api/public/index.php").unwrap();
        let root = posix("/home/dev/mono");
        let m = unique(infer(&remote, &root, &files(&["api/public/index.php", "admin/public/index.php", "index.php"]), false));
        // Three components match ("api/public/index.php"), so the
        // container's /app is the local mono/ folder.
        assert_eq!(m.local.native(), "/home/dev/mono");
        assert_eq!(m.remote.native(), "/app");
    }

    #[test]
    fn duplicate_relative_path_is_ambiguous() {
        let remote = parse_file_uri("file:///var/www/html/public/index.php").unwrap();
        let root = posix("/home/dev/mono");
        match infer(&remote, &root, &files(&["api/public/index.php", "admin/public/index.php"]), false) {
            Inference::Ambiguous { candidates, matched } => {
                assert_eq!(matched, 2);
                assert_eq!(candidates.len(), 2);
            }
            other => panic!("expected ambiguity, got {other:?}"),
        }
    }

    #[test]
    fn no_local_file_with_that_name() {
        let remote = parse_file_uri("file:///var/www/html/bin/console").unwrap();
        assert_eq!(infer(&remote, &posix("/p"), &[], false), Inference::NoMatch);
    }

    #[test]
    fn whole_remote_path_matches() {
        // Same path on both sides: PHP runs on the host.
        let remote = parse_file_uri("file:///home/dev/shop/public/index.php").unwrap();
        let root = posix("/home/dev/shop");
        let m = unique(infer(&remote, &root, &files(&["public/index.php"]), false));
        assert_eq!(m.remote, m.local);
    }

    // Windows hosts. The init packets in recorded/windows-*.xml are the
    // Linux recording with `fileuri` replaced by what Xdebug 3.5.3's
    // xdebug_path_to_url produces for that path: the test bed cannot
    // run Windows.

    #[test]
    fn windows_host_with_docker_desktop() {
        // PHP in a Linux container, project on C:. The init packet is the
        // same as on Linux; only the local side changes.
        let remote = recorded("linux-php85-public.xml");
        let root = win(r"C:\Users\Dev\Projects\shop");
        let m = unique(infer(&remote, &root, &files(&[r"public\index.php", "index.php"]), true));
        assert_eq!(m.local.native(), r"C:\Users\Dev\Projects\shop");
        // The editor may hand over a lower-case drive or forward slashes.
        let bp = win("c:/users/dev/projects/shop/src/Graph.php");
        assert_eq!(m.to_remote(&bp).unwrap().uri(), "file:///var/www/html/src/Graph.php");
        let back = m.to_local(&parse_file_uri("file:///var/www/html/src/Graph.php").unwrap()).unwrap();
        assert_eq!(back.native(), r"C:\Users\Dev\Projects\shop\src\Graph.php");
    }

    #[test]
    fn windows_native_php_drive_uri() {
        let remote = recorded("windows-native-drive.xml");
        assert_eq!(remote.style, Style::Windows);
        assert_eq!(remote.native(), r"C:\laragon\www\My Shop\public\index.php");
        let root = win(r"c:\Laragon\www\My Shop");
        let m = unique(infer(&remote, &root, &files(&[r"public\index.php"]), true));
        // Native PHP: the mapping is the identity, but it has to compare
        // drive letters and names without case.
        let bp = win(r"C:\laragon\WWW\my shop\src\Graph.php");
        let uri = m.to_remote(&bp).unwrap().uri();
        assert_eq!(uri, "file:///C:/laragon/www/My%20Shop/src/Graph.php");
    }

    #[test]
    fn windows_uri_encoding_matches_xdebug() {
        let p = win(r"D:\dev\café~1\a+b\index.php");
        assert_eq!(p.uri(), "file:///D:/dev/caf%C3%A9%7E1/a%2Bb/index.php");
        assert_eq!(parse_file_uri(&p.uri()).unwrap(), p);
        // A lower-case drive letter in a URI is accepted.
        assert_eq!(parse_file_uri("file:///c:/x/y.php").unwrap(), win(r"C:\x\y.php"));
        // Backslashes that survive in a URI are separators.
        assert_eq!(parse_file_uri("file:///C:/x\\y.php").unwrap(), win(r"C:\x\y.php"));
        // The verbatim prefix that some Windows APIs return.
        assert_eq!(win(r"\\?\C:\x\y.php"), win(r"C:\x\y.php"));
    }

    #[test]
    fn wsl_project_opened_from_windows() {
        // PHP inside WSL 2, editor on Windows opening \\wsl.localhost\...
        let remote = recorded("wsl-php.xml");
        assert_eq!(remote.native(), "/home/dev/shop/public/index.php");
        let root = win(r"\\wsl.localhost\Ubuntu\home\dev\shop");
        let m = unique(infer(&remote, &root, &files(&[r"public\index.php"]), true));
        let bp = win(r"\\wsl.localhost\Ubuntu\home\dev\shop\src\Graph.php");
        assert_eq!(m.to_remote(&bp).unwrap().uri(), "file:///home/dev/shop/src/Graph.php");
    }

    #[test]
    fn windows_unc_uri() {
        let remote = recorded("windows-native-unc.xml");
        assert_eq!(remote.root, "//fileserver/www");
        assert_eq!(remote.native(), r"\\fileserver\www\shop\index.php");
        assert_eq!(remote.uri(), "file://fileserver/www/shop/index.php");
    }

    #[test]
    fn windows_drive_change_does_not_match() {
        let remote = parse_file_uri("file:///C:/www/shop/index.php").unwrap();
        let m = unique(infer(&remote, &win(r"C:\www\shop"), &files(&["index.php"]), true));
        assert!(m.to_remote(&win(r"D:\www\shop\index.php")).is_none());
    }

    // macOS: APFS is case-insensitive by default, and Docker Desktop's
    // bind mounts show the host's names to the container.

    #[test]
    fn macos_docker_case_insensitive_tree() {
        let remote = recorded("macos-docker.xml");
        assert_eq!(remote.native(), "/var/www/html/Public/index.php");
        let root = posix("/Users/dev/Sites/shop");
        // The walker reports the name as stored on disk ("public"), the
        // container reports what PHP was asked to open ("Public").
        let m = unique(infer(&remote, &root, &files(&["public/index.php"]), true));
        assert_eq!(m.remote.native(), "/var/www/html");
        // The editor's path may differ in case from the disk; the suffix
        // keeps the local spelling, so it reaches Xdebug unchanged.
        let bp = posix("/users/dev/sites/shop/src/Graph.php");
        assert_eq!(m.to_remote(&bp).unwrap().native(), "/var/www/html/src/Graph.php");
        let bp = posix("/Users/dev/Sites/shop/src/graph.php");
        assert_eq!(m.to_remote(&bp).unwrap().native(), "/var/www/html/src/graph.php");
    }

    #[test]
    fn case_sensitive_tree_does_not_fold_case() {
        let remote = parse_file_uri("file:///var/www/html/Public/index.php").unwrap();
        let root = posix("/home/dev/shop");
        let m = unique(infer(&remote, &root, &files(&["public/index.php"]), false));
        // Only "index.php" matched, so the local root moves into public/.
        assert_eq!(m.local.native(), "/home/dev/shop/public");
        assert!(m.to_remote(&posix("/HOME/dev/shop/public/x.php")).is_none());
    }

    #[test]
    fn learned_mapping_is_reused_and_forgotten() {
        let mut learned = Learned::default();
        let html = Mapping { remote: posix("/var/www/html"), local: posix("/work/app"), local_ci: false };
        let srv = Mapping { remote: posix("/srv/app"), local: posix("/work/app"), local_ci: false };
        learned.learn(html.clone());
        learned.learn(srv.clone());
        let entry = parse_file_uri("file:///var/www/html/bin/console").unwrap();
        assert_eq!(learned.lookup(&entry), Some(&html));
        assert!(learned.lookup(&parse_file_uri("file:///opt/other/index.php").unwrap()).is_none());
        learned.forget(&html);
        assert!(learned.lookup(&entry).is_none());
        assert_eq!(learned.mappings, [srv]);
    }

    #[test]
    fn eval_and_phar_uris_are_not_files() {
        assert!(parse_file_uri("dbgp://1").is_err());
        assert!(parse_file_uri("phar:///app/x.phar/index.php").is_err());
    }

    #[test]
    fn dot_segments_are_resolved() {
        assert_eq!(posix("/var/www/html/public/../src/./Graph.php").native(), "/var/www/html/src/Graph.php");
    }
}
