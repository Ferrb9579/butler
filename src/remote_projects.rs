//! Local preparation for "Add project → Git URL" (T3 Code v0.1.11).
//! Parsing and destination preview do not contact a host or touch the filesystem.

use std::{
    fmt, io,
    path::{Component, Path, PathBuf},
};
use url::Url;

const MAX_URL_BYTES: usize = 4096;
const ENTER_URL: &str = "Enter a Git clone URL.";
const HTTPS_ONLY: &str = "Enter an HTTPS Git clone URL or GitHub owner/repository.";
const INVALID_URL: &str = "Enter a valid HTTPS Git clone URL with a repository path.";
const CREDENTIALS: &str = "Git URL cannot include credentials, a query, or a fragment.";
const INVALID_URL_PATH: &str = "The repository URL has an invalid or unsafe path.";

#[derive(Clone, PartialEq, Eq)]
pub struct RemoteProject {
    /// Original input (after trimming), for the Repository summary.
    pub input: String,
    /// Validated HTTPS URL, with recognized provider page routes removed.
    pub url: String,
    /// Suggested final directory name, as written by the repository owner.
    pub leaf: String,
}

// A URL can identify a private repo or embed sensitive path names even when
// it cannot contain URL credentials. Keep debug reports and errors generic.
impl fmt::Debug for RemoteProject {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RemoteProject").finish_non_exhaustive()
    }
}

impl RemoteProject {
    pub fn suggested_destination(&self) -> String {
        format!("~/{}", self.leaf)
    }
}

/// Normalize supported input as a pure operation. Validation doesn't prove
/// repository existence, transport connectivity, or ownership of the host.
pub fn parse(input: &str) -> io::Result<RemoteProject> {
    let input = trim(input);
    if input.is_empty() {
        return Err(invalid(ENTER_URL));
    }
    if input.len() > MAX_URL_BYTES {
        return Err(invalid("The Git URL is too long."));
    }
    if input.contains(['?', '#']) {
        return Err(invalid(CREDENTIALS));
    }
    if input.chars().any(|c| js_space(c) || unsafe_character(c)) {
        return Err(invalid(INVALID_URL));
    }
    if input.contains('\\') {
        return Err(invalid(INVALID_URL_PATH));
    }

    // T3's generic Git URL source accepts owner/repository on the default
    // public host (GitHub); no service/provider lookup is involved.
    let shorthand = if repository_shorthand(input) {
        let suffix = if input.ends_with(".git") { "" } else { ".git" };
        Some(format!("https://github.com/{input}{suffix}"))
    } else {
        None
    };
    let candidate = shorthand.as_deref().unwrap_or(input);
    let (scheme, rest) = candidate
        .split_once("://")
        .ok_or_else(|| invalid(HTTPS_ONLY))?;
    if !scheme.eq_ignore_ascii_case("https") {
        return Err(invalid(HTTPS_ONLY));
    }
    let (authority, path) = rest.split_once('/').ok_or_else(|| invalid(INVALID_URL))?;
    if authority.is_empty() {
        return Err(invalid(INVALID_URL));
    }
    // Reject even empty or escaped userinfo before WHATWG normalization.
    if authority.contains('@') {
        return Err(invalid(CREDENTIALS));
    }
    if authority.contains('%') {
        return Err(invalid(INVALID_URL));
    }

    // Validate the *raw* path before Url::parse, which collapses dot
    // segments (including encoded dots) and treats backslashes as slashes.
    let raw: Vec<_> = path.strip_suffix('/').unwrap_or(path).split('/').collect();
    let decoded = raw
        .iter()
        .map(|segment| decode_url_segment(segment))
        .collect::<io::Result<Vec<_>>>()?;
    let mut url = Url::parse(candidate).map_err(|_| invalid(INVALID_URL))?;
    if url.scheme() != "https" || url.host().is_none() {
        return Err(invalid(INVALID_URL));
    }
    if !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(invalid(CREDENTIALS));
    }

    // Only rewrite routes on the public services we actually recognize. Do
    // not guess a provider from an arbitrary HTTPS hostname or path.
    let kept = if url.port().is_none() {
        match url.host_str() {
            Some("github.com" | "www.github.com") => github_repo_segments(&raw)?,
            Some("gitlab.com") => gitlab_repo_segments(&raw)?,
            _ => raw.len(),
        }
    } else {
        raw.len()
    };
    let slug = decoded
        .get(kept - 1)
        .ok_or_else(|| invalid(INVALID_URL_PATH))?;
    let leaf = slug.strip_suffix(".git").unwrap_or(slug);
    if !safe_component(leaf) {
        return Err(invalid(INVALID_URL_PATH));
    }
    if kept < raw.len() {
        let mut segments = url.path_segments_mut().map_err(|_| invalid(INVALID_URL))?;
        segments.pop_if_empty();
        for _ in kept..raw.len() {
            segments.pop();
        }
    }
    Ok(RemoteProject {
        input: input.to_owned(),
        url: url.into(),
        leaf: leaf.to_owned(),
    })
}

fn repository_shorthand(input: &str) -> bool {
    let Some((owner, repo)) = input.split_once('/') else {
        return false;
    };
    (1..=39).contains(&owner.len())
        && owner.as_bytes()[0].is_ascii_alphanumeric()
        && owner
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'-')
        && !repo.is_empty()
        && repo
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'.' | b'_' | b'-'))
}

fn github_repo_segments(segments: &[&str]) -> io::Result<usize> {
    match segments {
        [_, _] | [_, _, "tree" | "blob", _, ..] => Ok(2),
        _ => Err(invalid(INVALID_URL)),
    }
}

fn gitlab_repo_segments(segments: &[&str]) -> io::Result<usize> {
    match segments.iter().position(|&segment| segment == "-") {
        Some(index) if index >= 2 && index + 1 < segments.len() => Ok(index),
        None if segments.len() >= 2 => Ok(segments.len()),
        _ => Err(invalid(INVALID_URL)),
    }
}

fn decode_url_segment(segment: &str) -> io::Result<String> {
    let mut decoded = Vec::with_capacity(segment.len());
    let mut bytes = segment.bytes();
    while let Some(byte) = bytes.next() {
        if byte == b'%' {
            let high = bytes
                .next()
                .and_then(hex)
                .ok_or_else(|| invalid(INVALID_URL_PATH))?;
            let low = bytes
                .next()
                .and_then(hex)
                .ok_or_else(|| invalid(INVALID_URL_PATH))?;
            decoded.push(high * 16 + low);
        } else {
            decoded.push(byte);
        }
    }
    let decoded = String::from_utf8(decoded).map_err(|_| invalid(INVALID_URL_PATH))?;
    // Deliberately reject double escapes instead of guessing how a later
    // transport or server might interpret a second decoding pass.
    if decoded.contains('%') || !safe_component(&decoded) {
        return Err(invalid(INVALID_URL_PATH));
    }
    Ok(decoded)
}

fn hex(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn safe_component(component: &str) -> bool {
    if component.is_empty()
        || matches!(component, "." | "..")
        || component.eq_ignore_ascii_case(".git")
        || component.ends_with([' ', '.'])
        || component.contains(['/', '\\', ':', '*', '?', '"', '<', '>', '|', '#'])
        || component.chars().any(unsafe_character)
    {
        return false;
    }
    // Portable paths: Windows treats device names specially even if there
    // is an extension or intervening space before the dot.
    let stem = component
        .split('.')
        .next()
        .unwrap_or_default()
        .trim_end_matches(' ');
    let name = stem.to_ascii_lowercase();
    if matches!(
        name.as_str(),
        "con" | "prn" | "aux" | "nul" | "conin$" | "conout$"
    ) {
        return false;
    }
    !["com", "lpt"].iter().any(|prefix| {
        name.strip_prefix(prefix).is_some_and(|n| {
            matches!(
                n,
                "0" | "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
            )
        })
    })
}

fn unsafe_character(c: char) -> bool {
    c.is_control()
        || matches!(c, '\u{061c}' | '\u{200e}' | '\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')
}

fn js_space(c: char) -> bool {
    c == '\u{feff}' || (c.is_whitespace() && c != '\u{0085}')
}

fn trim(input: &str) -> &str {
    input.trim_matches(js_space)
}

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}

/// Resolve an explicitly entered local destination without following symlinks
/// or checking existence. Absolute paths, `~/...`, and `./...` are supported.
/// Parent traversal and network/device paths are excluded from this first slice.
pub fn resolve_destination(
    input: &str,
    home: Option<&Path>,
    active_project: Option<&Path>,
) -> io::Result<PathBuf> {
    let input = trim(input);
    if input.is_empty() {
        return Err(invalid("Choose a destination path before cloning."));
    }
    if input.chars().any(unsafe_character) || input.starts_with("//") || input.starts_with("\\\\") {
        return Err(invalid("Choose a local destination folder."));
    }
    #[cfg(not(windows))]
    if input.contains('\\') {
        return Err(invalid(
            "Windows-style paths are only supported on Windows.",
        ));
    }

    let relative = if let Some(rest) = input.strip_prefix("~/") {
        Some((home, rest, "Home folder is unavailable."))
    } else if cfg!(windows)
        && let Some(rest) = input.strip_prefix("~\\")
    {
        Some((home, rest, "Home folder is unavailable."))
    } else if let Some(rest) = input
        .strip_prefix("./")
        .or_else(|| cfg!(windows).then(|| input.strip_prefix(".\\")).flatten())
    {
        Some((
            active_project,
            rest,
            "Relative paths require an active project.",
        ))
    } else if input == ".."
        || input.starts_with("../")
        || (cfg!(windows) && input.starts_with("..\\"))
    {
        return Err(invalid("Destination paths cannot contain '..'."));
    } else {
        None
    };

    let path = if let Some((base, rest, missing)) = relative {
        let base = base
            .filter(|p| p.is_absolute())
            .ok_or_else(|| invalid(missing))?;
        let rest = Path::new(rest);
        if rest
            .components()
            .any(|c| matches!(c, Component::Prefix(_) | Component::RootDir))
        {
            return Err(invalid("Enter a folder name after the destination prefix."));
        }
        validate_components(rest)?;
        return Ok(base.join(rest));
    } else {
        Path::new(input)
    };
    if !path.is_absolute() {
        return Err(invalid(
            "Enter an absolute destination path or start with ~/ or ./.",
        ));
    }
    validate_components(path)?;
    Ok(path.components().collect())
}

fn validate_components(path: &Path) -> io::Result<()> {
    let mut has_folder = false;
    for component in path.components() {
        match component {
            Component::Normal(part) => {
                if part.to_str().is_none_or(|part| !safe_component(part)) {
                    return Err(invalid(
                        "The destination path contains an unsafe folder name.",
                    ));
                }
                has_folder = true;
            }
            Component::ParentDir => {
                return Err(invalid("Destination paths cannot contain '..'."));
            }
            #[cfg(windows)]
            Component::Prefix(prefix) if !matches!(prefix.kind(), std::path::Prefix::Disk(_)) => {
                return Err(invalid("Choose a local destination folder."));
            }
            _ => {}
        }
    }
    if !has_folder {
        return Err(invalid(
            "Choose a destination folder, not a filesystem root.",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_pasted_https_clone_and_repository_shorthand() {
        let parsed = parse("  \u{feff}Ferrb9579/Butler\n").unwrap();
        assert_eq!(parsed.input, "Ferrb9579/Butler");
        assert_eq!(parsed.url, "https://github.com/Ferrb9579/Butler.git");
        assert_eq!(parsed.suggested_destination(), "~/Butler");
        assert_eq!(parse("Ferrb9579/Butler.git").unwrap().url, parsed.url);
        assert_eq!(
            parse("https://example.com:9443/team/計画.git")
                .unwrap()
                .leaf,
            "計画"
        );
        assert_eq!(
            parse("HTTPS://Code.Example/team/R%C3%A9sum%C3%A9%20Tools.git/")
                .unwrap()
                .suggested_destination(),
            "~/Résumé Tools"
        );
        assert_eq!(
            parse("https://[2001:db8::1]:8443/SomeRepo.git")
                .unwrap()
                .leaf,
            "SomeRepo"
        );
    }

    #[test]
    fn pinned_provider_page_routes_normalize_without_network() {
        for route in ["tree/Main", "blob/Main/readme.md"] {
            let parsed = parse(&format!("https://github.com/Team/Repo/{route}")).unwrap();
            assert_eq!(parsed.url, "https://github.com/Team/Repo");
            assert_eq!(parsed.leaf, "Repo");
        }
        let parsed = parse("https://gitlab.com/Group/Subgroup/Repo/-/merge_requests/12").unwrap();
        assert_eq!(parsed.url, "https://gitlab.com/Group/Subgroup/Repo");
        assert_eq!(parsed.leaf, "Repo");
        let parsed = parse("https://gitlab.com/Group/Repo/-/blob/Main/read%20me.md/").unwrap();
        assert_eq!(parsed.url, "https://gitlab.com/Group/Repo");
        let custom = "https://example.com/group/repo/-/not-a-provider-page.git";
        let parsed = parse(custom).unwrap();
        assert_eq!(parsed.url, custom);
        assert_eq!(parsed.leaf, "not-a-provider-page");
        let encoded = parse("https://gitlab.com/team/R%C3%A9su/-/tree/Main").unwrap();
        assert_eq!(encoded.url, "https://gitlab.com/team/R%C3%A9su");
        assert_eq!(encoded.leaf, "Résu");
    }

    #[test]
    fn rejects_unsafe_transports_ambiguous_routes_credentials_and_paths() {
        for input in [
            "",
            "owner/",
            "owner/.",
            "owner/..",
            "owner/.git",
            "http://example.com/repo.git",
            "ssh://git@example.com/repo.git",
            "git@example.com:team/repo.git",
            "file:///tmp/repo.git",
            "ext::git-upload-pack repo.git",
            "/tmp/repo.git",
            "C:/repo.git",
            "https:/example.com/repo.git",
            "https:///example.com/repo.git",
            "https://example.com",
            "https://example.com/",
            "https://example.com:bad/repo.git",
            "https://@example.com/repo.git",
            "https://user:secret@example.com/repo.git",
            "https://example.com/repo.git?password=secret",
            "https://example.com/repo.git#fragment",
            "https://example.com/repo.git?",
            "https://example.com/repo.git#",
            "https://%65xample.com/repo.git",
            "https://example.com/team/../repo.git",
            "https://example.com/team/.%2e/repo.git",
            "https://example.com/team/..git",
            "https://example.com/team/%2e%2e.git",
            "https://example.com/team/%2Frepo.git",
            "https://example.com/team/%5Crepo.git",
            "https://example.com/team\\repo.git",
            "https://example.com/team/%252f.git",
            "https://example.com/team/%FF.git",
            "https://example.com/team/%G1.git",
            "https://example.com/team/%2.git",
            "https://example.com/team/%00.git",
            "https://example.com/team/%0A.git",
            "https://example.com/team/%3F.git",
            "https://example.com/team/%23.git",
            "https://example.com/team//repo.git",
            "https://example.com/repo.git//",
            "https://example.com/team/CON.git",
            "https://example.com/team/LPT%C2%B9.git",
            "https://example.com/team/repo%20.git",
            "https://github.com/owner/repo/issues/123",
            "https://gitlab.com/group/repo/-/",
            "https://example.com/team/re po.git",
        ] {
            assert!(parse(input).is_err(), "{input:?}");
        }
        let error = parse("https://user:SecretToken@example.com/team/Repo.git").unwrap_err();
        assert_eq!(error.to_string(), CREDENTIALS);
        assert!(!format!("{error:?}").contains("SecretToken"));
        let parsed = parse("https://example.com/team/PrivateProject.git").unwrap();
        assert!(!format!("{parsed:?}").contains("PrivateProject"));
    }

    #[test]
    fn destination_resolves_without_touching_or_resolving_filesystem() {
        let home = if cfg!(windows) {
            Path::new("C:\\UnavailableHome")
        } else {
            Path::new("/unavailable-home")
        };
        let active = home.join("unavailable-active");
        assert_eq!(
            resolve_destination("~/Répo Name", Some(home), Some(&active)).unwrap(),
            home.join("Répo Name")
        );
        assert_eq!(
            resolve_destination("./workspace/Repo", Some(home), Some(&active)).unwrap(),
            active.join("workspace").join("Repo")
        );
        let absolute = home.join("SomeRepo");
        assert_eq!(
            resolve_destination(absolute.to_str().unwrap(), None, None).unwrap(),
            absolute
        );
        assert_eq!(
            resolve_destination("./Repo", Some(home), None)
                .unwrap_err()
                .to_string(),
            "Relative paths require an active project."
        );
        for input in [
            "",
            "Repo",
            "~",
            "~/",
            "./",
            "../Repo",
            "~/../Repo",
            "./nested/../Repo",
            "~/CON",
            "~/.git",
            "////server/something",
            "\\\\server\\share\\Repo",
            "~/repo\nname",
        ] {
            assert!(
                resolve_destination(input, Some(home), Some(&active)).is_err(),
                "{input:?}"
            );
        }
        #[cfg(not(windows))]
        assert!(resolve_destination("C:\\Repo", Some(home), Some(&active)).is_err());
    }
}
