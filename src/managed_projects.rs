//! Offline local project creation, based on T3 Code v0.1.11's managed folders.
//! This module does not register a project or start a thread.

use icu_normalizer::DecomposingNormalizer;
use std::{
    env,
    fs::{self, Metadata, OpenOptions},
    io::{self, Read, Write},
    path::{Path, PathBuf},
    process::{Command, ExitStatus, Stdio},
    thread,
    time::{Duration, Instant},
};

const MAX_ATTEMPTS: usize = 100;
const SHORT_GIT_TIMEOUT: Duration = Duration::from_secs(10);
const COMMIT_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_GIT_OUTPUT: usize = 16 * 1024;

#[derive(Debug)]
pub struct Created {
    pub root: PathBuf,
    pub title: String,
    pub warning: Option<String>,
}

/// Preview the first candidate, without reserving a suffix or writing to disk.
pub fn preview_root(data_root: &Path, name: &str) -> io::Result<PathBuf> {
    if !data_root.is_absolute() {
        return Err(invalid("The Butler data folder must be an absolute path."));
    }
    Ok(data_root.join("projects").join(slug(title(name)?)))
}

/// Claim, initialize, and scaffold a local directory. Failure to initialize or
/// stage is fatal; failure to commit leaves the scaffold and index intact.
pub fn create(data_root: &Path, name: &str) -> io::Result<Created> {
    let candidate = preview_root(data_root, name)?;
    let git = Git::installed()?;
    create_with(&git, data_root, title(name)?, &candidate)
}

fn create_with(git: &Git, data_root: &Path, title: &str, candidate: &Path) -> io::Result<Created> {
    let projects = prepare_parent(data_root)?;
    let claimed = claim(&projects, candidate.file_name().expect("slug is nonempty"))?;
    let created = scaffold(git, &claimed, title);
    if created.is_err() {
        // Delete only directories claimed by this call, never an existing
        // collision or a path that now resolves to a different directory.
        if claimed.verify().is_ok() {
            let _ = fs::remove_dir_all(&claimed.path);
        }
    }
    created
}

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}

fn title(raw: &str) -> io::Result<&str> {
    // ECMAScript String.trim, including BOM but excluding U+0085.
    let title = raw.trim_matches(|c| {
        matches!(
            c,
            '\u{0009}'..='\u{000d}'
                | '\u{0020}'
                | '\u{00a0}'
                | '\u{1680}'
                | '\u{2000}'..='\u{200a}'
                | '\u{2028}'..='\u{2029}'
                | '\u{202f}'
                | '\u{205f}'
                | '\u{3000}'
                | '\u{feff}'
        )
    });
    if title.is_empty() {
        return Err(invalid("Enter a project name."));
    }
    if title.encode_utf16().count() > 200 {
        return Err(invalid("Use a project name of at most 200 characters."));
    }
    Ok(title)
}

fn slug(title: &str) -> String {
    let mut latin = String::with_capacity(title.len());
    for letter in title.chars() {
        latin.push_str(match letter {
            'Ŀ' | 'ŀ' | 'Ł' | 'ł' => "l",
            'ŉ' | 'Ŋ' | 'ŋ' => "n",
            'Đ' | 'đ' | 'Ð' | 'ð' => "d",
            'ß' | 'ẞ' => "ss",
            'Ø' | 'ø' => "o",
            'Æ' | 'æ' => "ae",
            'Œ' | 'œ' => "oe",
            'Þ' | 'þ' => "th",
            'Ħ' | 'ħ' => "h",
            'ı' => "i",
            'ĸ' => "q",
            'Ŧ' | 'ŧ' => "t",
            _ => {
                latin.push(letter);
                continue;
            }
        });
    }
    let mut slug = String::new();
    let mut pending_separator = false;
    for c in DecomposingNormalizer::new_nfkd()
        .normalize_iter(latin.chars())
        .filter(|c| !('\u{0300}'..='\u{036f}').contains(c))
        .flat_map(char::to_lowercase)
    {
        if !c.is_ascii_alphanumeric() {
            pending_separator = !slug.is_empty();
            continue;
        }
        if slug.len() + usize::from(pending_separator) >= 64 {
            break;
        }
        if pending_separator {
            slug.push('-');
            pending_separator = false;
        }
        slug.push(c);
    }
    let bytes = slug.as_bytes();
    if slug.is_empty() {
        slug.push_str("project");
    } else if matches!(slug.as_str(), "con" | "prn" | "aux" | "nul")
        || (bytes.len() == 4
            && matches!(&bytes[..3], b"com" | b"lpt")
            && matches!(bytes[3], b'1'..=b'9'))
    {
        slug.push_str("-project");
    }
    slug
}

fn real_directory(metadata: &Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        // Reject junctions and other reparse points, as well as symlinks.
        if metadata.file_attributes() & 0x400 != 0 {
            return false;
        }
    }
    metadata.file_type().is_dir()
}

fn reject_checkout(path: &Path) -> io::Result<()> {
    for ancestor in path.ancestors() {
        let marker = ancestor.join(".git");
        let metadata = match fs::metadata(&marker) {
            Ok(metadata) => metadata,
            Err(err) if err.kind() == io::ErrorKind::NotFound => continue,
            Err(err) => return Err(err),
        };
        let git_dir = if metadata.is_dir() {
            marker
        } else if metadata.is_file() && metadata.len() <= 16 * 1024 {
            let mut pointer = String::new();
            let read = fs::File::open(&marker)?
                .take(16 * 1024)
                .read_to_string(&mut pointer);
            if read.is_err() {
                continue;
            }
            let Some(target) = pointer.trim_end().strip_prefix("gitdir: ") else {
                continue;
            };
            ancestor.join(target)
        } else {
            continue;
        };
        if git_dir.join("HEAD").is_file() {
            return Err(invalid(
                "Butler's managed project folder must be outside existing Git checkouts.",
            ));
        }
    }
    Ok(())
}

fn prepare_parent(data_root: &Path) -> io::Result<PathBuf> {
    // Check the first existing ancestor before mkdir as well as the resulting
    // canonical path, so a configured data folder cannot write into a checkout.
    let mut existing = data_root;
    loop {
        match existing.canonicalize() {
            Ok(resolved) => {
                reject_checkout(&resolved)?;
                break;
            }
            Err(err) if err.kind() == io::ErrorKind::NotFound => {
                existing = existing.parent().ok_or(err)?;
            }
            Err(err) => return Err(err),
        }
    }
    fs::create_dir_all(data_root)?;
    let data_root = data_root.canonicalize()?;
    reject_checkout(&data_root)?;
    let projects = data_root.join("projects");
    match fs::create_dir(&projects) {
        Ok(()) => {}
        Err(err) if err.kind() == io::ErrorKind::AlreadyExists => {}
        Err(err) => return Err(err),
    }
    if !real_directory(&fs::symlink_metadata(&projects)?) {
        return Err(invalid(
            "Butler's managed projects path is not a real directory.",
        ));
    }
    let projects = projects.canonicalize()?;
    reject_checkout(&projects)?;
    Ok(projects)
}

struct Claimed {
    path: PathBuf,
    #[cfg(unix)]
    identity: (u64, u64),
}

impl Claimed {
    fn verify(&self) -> io::Result<()> {
        let metadata = fs::symlink_metadata(&self.path)?;
        if !real_directory(&metadata) || self.path.canonicalize()? != self.path {
            return Err(invalid("The newly created project folder moved."));
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            if self.identity != (metadata.dev(), metadata.ino()) {
                return Err(invalid("The newly created project folder was replaced."));
            }
        }
        Ok(())
    }
}

fn claim(projects: &Path, stem: &std::ffi::OsStr) -> io::Result<Claimed> {
    let mut builder = fs::DirBuilder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    for attempt in 1..=MAX_ATTEMPTS {
        let candidate = if attempt == 1 {
            stem.to_owned()
        } else {
            let mut candidate = stem.to_owned();
            candidate.push(format!("-{attempt}"));
            candidate
        };
        let path = projects.join(candidate);
        match builder.create(&path) {
            Ok(()) => {
                let claim = Claimed {
                    #[cfg(unix)]
                    identity: {
                        use std::os::unix::fs::MetadataExt;
                        let metadata = fs::symlink_metadata(&path)?;
                        (metadata.dev(), metadata.ino())
                    },
                    path,
                };
                claim.verify()?;
                return Ok(claim);
            }
            Err(err) if err.kind() == io::ErrorKind::AlreadyExists => {}
            Err(err) => return Err(err),
        }
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        format!(
            "Every folder name for \"{}\" is taken.",
            stem.to_string_lossy()
        ),
    ))
}

fn scaffold(git: &Git, claimed: &Claimed, title: &str) -> io::Result<Created> {
    let root = &claimed.path;
    let config = git.read_identity_and_branch(root);
    claimed.verify()?;
    let branch = config.branch.as_deref().unwrap_or("main");
    git.required(
        root,
        "init",
        &["init", "--template=", &format!("--initial-branch={branch}")],
    )?;

    claimed.verify()?;
    write_new(&root.join("README.md"), &named_readme(title))?;
    fs::create_dir(root.join("assets"))?;
    write_new(&root.join("assets/icon.svg"), &named_icon(title))?;
    claimed.verify()?;
    git.required(
        root,
        "add",
        &["add", "--force", "--", "README.md", "assets/icon.svg"],
    )?;

    claimed.verify()?;
    let mut commit_options = vec!["-c".to_owned(), "user.useConfigOnly=true".to_owned()];
    for (key, value) in [("user.name", config.name), ("user.email", config.email)] {
        if let Some(value) = value {
            commit_options.extend(["-c".to_owned(), format!("{key}={value}")]);
        }
    }
    let mut commit: Vec<_> = commit_options.iter().map(String::as_str).collect();
    commit.extend(["commit", "--no-gpg-sign", "--message", "Initial commit"]);
    let warning = match git.run(root, &commit, COMMIT_TIMEOUT, false) {
        Ok(output) if output.status.success() => None,
        Ok(output) => Some(commit_warning(&output.stderr)),
        Err(error) => Some(error.to_string()),
    };
    claimed.verify()?;
    Ok(Created {
        root: root.clone(),
        title: title.to_owned(),
        warning,
    })
}

fn write_new(path: &Path, contents: &str) -> io::Result<()> {
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
    file.write_all(contents.as_bytes())
}

fn named_readme(title: &str) -> String {
    format!(
        "<img src=\"assets/icon.svg\" width=\"64\" height=\"64\" alt=\"\">\n\n# {title}\n\nCreated in [Butler](https://github.com/Ferrb9579/butler).\n"
    )
}

fn named_icon(title: &str) -> String {
    const BACKGROUNDS: [&str; 15] = [
        "#dc2626", "#ea580c", "#d97706", "#16a34a", "#059669", "#0d9488", "#0891b2", "#0284c7",
        "#2563eb", "#4f46e5", "#7c3aed", "#9333ea", "#c026d3", "#db2777", "#e11d48",
    ];
    let initials: String = title
        .split(|character| !is_letter_or_number(character))
        .filter_map(|word| word.chars().next())
        .take(2)
        .flat_map(char::to_uppercase)
        .collect();
    let initials = if initials.is_empty() {
        title.chars().next().unwrap_or('?').to_string()
    } else {
        initials
    };
    let size = if initials.encode_utf16().count() > 1 {
        26
    } else {
        32
    };
    let initials = initials
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;");
    let hash = title.chars().fold(0_u32, |hash, c| {
        hash.wrapping_mul(31).wrapping_add(u32::from(c))
    });
    let background = BACKGROUNDS[(hash % BACKGROUNDS.len() as u32) as usize];
    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 64 64\">\n  <rect width=\"64\" height=\"64\" rx=\"14\" fill=\"{background}\"/>\n  <text x=\"32\" y=\"32\" dy=\"0.35em\" text-anchor=\"middle\" font-family=\"ui-sans-serif, system-ui, -apple-system, sans-serif\" font-size=\"{size}\" font-weight=\"600\" fill=\"#ffffff\">{initials}</text>\n</svg>\n",
    )
}

// Rust's Alphabetic includes additional marks. Exclude the exact set difference
// Unicode 17 Alphabetic − (General_Category Letter ∪ Number) to match T3's regex
// without adding a dependency. The ranges below can be reproduced by iterating
// Unicode scalars with Node's /\p{Alphabetic}/u.test(c) && !/[\p{L}\p{N}]/u.test(c).
fn is_letter_or_number(character: char) -> bool {
    if !character.is_alphanumeric() {
        return false;
    }
    let codepoint = u32::from(character);
    let index = EXTRA_ALPHABETIC.partition_point(|&(_, end)| end < codepoint);
    EXTRA_ALPHABETIC
        .get(index)
        .is_none_or(|&(start, _)| codepoint < start)
}

#[rustfmt::skip]
const EXTRA_ALPHABETIC: &[(u32, u32)] = &[
    (0x345, 0x345), (0x363, 0x36f), (0x5b0, 0x5bd), (0x5bf, 0x5bf),
    (0x5c1, 0x5c2), (0x5c4, 0x5c5), (0x5c7, 0x5c7), (0x610, 0x61a),
    (0x64b, 0x657), (0x659, 0x65f), (0x670, 0x670), (0x6d6, 0x6dc),
    (0x6e1, 0x6e4), (0x6e7, 0x6e8), (0x6ed, 0x6ed), (0x711, 0x711),
    (0x730, 0x73f), (0x7a6, 0x7b0), (0x816, 0x817), (0x81b, 0x823),
    (0x825, 0x827), (0x829, 0x82c), (0x897, 0x897), (0x8d4, 0x8df),
    (0x8e3, 0x8e9), (0x8f0, 0x903), (0x93a, 0x93b), (0x93e, 0x94c),
    (0x94e, 0x94f), (0x955, 0x957), (0x962, 0x963), (0x981, 0x983),
    (0x9be, 0x9c4), (0x9c7, 0x9c8), (0x9cb, 0x9cc), (0x9d7, 0x9d7),
    (0x9e2, 0x9e3), (0xa01, 0xa03), (0xa3e, 0xa42), (0xa47, 0xa48),
    (0xa4b, 0xa4c), (0xa51, 0xa51), (0xa70, 0xa71), (0xa75, 0xa75),
    (0xa81, 0xa83), (0xabe, 0xac5), (0xac7, 0xac9), (0xacb, 0xacc),
    (0xae2, 0xae3), (0xafa, 0xafc), (0xb01, 0xb03), (0xb3e, 0xb44),
    (0xb47, 0xb48), (0xb4b, 0xb4c), (0xb56, 0xb57), (0xb62, 0xb63),
    (0xb82, 0xb82), (0xbbe, 0xbc2), (0xbc6, 0xbc8), (0xbca, 0xbcc),
    (0xbd7, 0xbd7), (0xc00, 0xc04), (0xc3e, 0xc44), (0xc46, 0xc48),
    (0xc4a, 0xc4c), (0xc55, 0xc56), (0xc62, 0xc63), (0xc81, 0xc83),
    (0xcbe, 0xcc4), (0xcc6, 0xcc8), (0xcca, 0xccc), (0xcd5, 0xcd6),
    (0xce2, 0xce3), (0xcf3, 0xcf3), (0xd00, 0xd03), (0xd3e, 0xd44),
    (0xd46, 0xd48), (0xd4a, 0xd4c), (0xd57, 0xd57), (0xd62, 0xd63),
    (0xd81, 0xd83), (0xdcf, 0xdd4), (0xdd6, 0xdd6), (0xdd8, 0xddf),
    (0xdf2, 0xdf3), (0xe31, 0xe31), (0xe34, 0xe3a), (0xe4d, 0xe4d),
    (0xeb1, 0xeb1), (0xeb4, 0xeb9), (0xebb, 0xebc), (0xecd, 0xecd),
    (0xf71, 0xf83), (0xf8d, 0xf97), (0xf99, 0xfbc), (0x102b, 0x1036),
    (0x1038, 0x1038), (0x103b, 0x103e), (0x1056, 0x1059), (0x105e, 0x1060),
    (0x1062, 0x1064), (0x1067, 0x106d), (0x1071, 0x1074), (0x1082, 0x108d),
    (0x108f, 0x108f), (0x109a, 0x109d), (0x1712, 0x1713), (0x1732, 0x1733),
    (0x1752, 0x1753), (0x1772, 0x1773), (0x17b6, 0x17c8), (0x1885, 0x1886),
    (0x18a9, 0x18a9), (0x1920, 0x192b), (0x1930, 0x1938), (0x1a17, 0x1a1b),
    (0x1a55, 0x1a5e), (0x1a61, 0x1a74), (0x1abf, 0x1ac0), (0x1acc, 0x1ace),
    (0x1b00, 0x1b04), (0x1b35, 0x1b43), (0x1b80, 0x1b82), (0x1ba1, 0x1ba9),
    (0x1bac, 0x1bad), (0x1be7, 0x1bf1), (0x1c24, 0x1c36), (0x1dd3, 0x1df4),
    (0x24b6, 0x24e9), (0x2de0, 0x2dff), (0xa674, 0xa67b), (0xa69e, 0xa69f),
    (0xa802, 0xa802), (0xa80b, 0xa80b), (0xa823, 0xa827), (0xa880, 0xa881),
    (0xa8b4, 0xa8c3), (0xa8c5, 0xa8c5), (0xa8ff, 0xa8ff), (0xa926, 0xa92a),
    (0xa947, 0xa952), (0xa980, 0xa983), (0xa9b4, 0xa9bf), (0xa9e5, 0xa9e5),
    (0xaa29, 0xaa36), (0xaa43, 0xaa43), (0xaa4c, 0xaa4d), (0xaa7b, 0xaa7d),
    (0xaab0, 0xaab0), (0xaab2, 0xaab4), (0xaab7, 0xaab8), (0xaabe, 0xaabe),
    (0xaaeb, 0xaaef), (0xaaf5, 0xaaf5), (0xabe3, 0xabea), (0xfb1e, 0xfb1e),
    (0x10376, 0x1037a), (0x10a01, 0x10a03), (0x10a05, 0x10a06), (0x10a0c, 0x10a0f),
    (0x10d24, 0x10d27), (0x10d69, 0x10d69), (0x10eab, 0x10eac), (0x10efa, 0x10efc),
    (0x11000, 0x11002), (0x11038, 0x11045), (0x11073, 0x11074), (0x11080, 0x11082),
    (0x110b0, 0x110b8), (0x110c2, 0x110c2), (0x11100, 0x11102), (0x11127, 0x11132),
    (0x11145, 0x11146), (0x11180, 0x11182), (0x111b3, 0x111bf), (0x111ce, 0x111cf),
    (0x1122c, 0x11234), (0x11237, 0x11237), (0x1123e, 0x1123e), (0x11241, 0x11241),
    (0x112df, 0x112e8), (0x11300, 0x11303), (0x1133e, 0x11344), (0x11347, 0x11348),
    (0x1134b, 0x1134c), (0x11357, 0x11357), (0x11362, 0x11363), (0x113b8, 0x113c0),
    (0x113c2, 0x113c2), (0x113c5, 0x113c5), (0x113c7, 0x113ca), (0x113cc, 0x113cd),
    (0x11435, 0x11441), (0x11443, 0x11445), (0x114b0, 0x114c1), (0x115af, 0x115b5),
    (0x115b8, 0x115be), (0x115dc, 0x115dd), (0x11630, 0x1163e), (0x11640, 0x11640),
    (0x116ab, 0x116b5), (0x1171d, 0x1172a), (0x1182c, 0x11838), (0x11930, 0x11935),
    (0x11937, 0x11938), (0x1193b, 0x1193c), (0x11940, 0x11940), (0x11942, 0x11942),
    (0x119d1, 0x119d7), (0x119da, 0x119df), (0x119e4, 0x119e4), (0x11a01, 0x11a0a),
    (0x11a35, 0x11a39), (0x11a3b, 0x11a3e), (0x11a51, 0x11a5b), (0x11a8a, 0x11a97),
    (0x11b60, 0x11b67), (0x11c2f, 0x11c36), (0x11c38, 0x11c3e), (0x11c92, 0x11ca7),
    (0x11ca9, 0x11cb6), (0x11d31, 0x11d36), (0x11d3a, 0x11d3a), (0x11d3c, 0x11d3d),
    (0x11d3f, 0x11d41), (0x11d43, 0x11d43), (0x11d47, 0x11d47), (0x11d8a, 0x11d8e),
    (0x11d90, 0x11d91), (0x11d93, 0x11d96), (0x11ef3, 0x11ef6), (0x11f00, 0x11f01),
    (0x11f03, 0x11f03), (0x11f34, 0x11f3a), (0x11f3e, 0x11f40), (0x1611e, 0x1612e),
    (0x16f4f, 0x16f4f), (0x16f51, 0x16f87), (0x16f8f, 0x16f92), (0x16ff0, 0x16ff1),
    (0x1bc9e, 0x1bc9e), (0x1e000, 0x1e006), (0x1e008, 0x1e018), (0x1e01b, 0x1e021),
    (0x1e023, 0x1e024), (0x1e026, 0x1e02a), (0x1e08f, 0x1e08f), (0x1e6e3, 0x1e6e3),
    (0x1e6e6, 0x1e6e6), (0x1e6ee, 0x1e6ef), (0x1e6f5, 0x1e6f5), (0x1e947, 0x1e947),
    (0x1f130, 0x1f149), (0x1f150, 0x1f169), (0x1f170, 0x1f189),
];

#[derive(Default)]
struct IdentityAndBranch {
    branch: Option<String>,
    name: Option<String>,
    email: Option<String>,
}

struct Git {
    executable: PathBuf,
    config_environment: Vec<(&'static str, PathBuf)>,
}

impl Git {
    fn installed() -> io::Result<Self> {
        let program = if cfg!(windows) { "git.exe" } else { "git" };
        let path = env::var_os("PATH").unwrap_or_default();
        let executable = env::split_paths(&path)
            .filter(|directory| directory.is_absolute())
            .filter_map(|directory| directory.join(program).canonicalize().ok())
            .find(|candidate| {
                let Ok(metadata) = fs::metadata(candidate) else {
                    return false;
                };
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    if metadata.permissions().mode() & 0o111 == 0 {
                        return false;
                    }
                }
                metadata.is_file()
            })
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::NotFound,
                    "Git isn't installed on this machine. Install Git to create a project.",
                )
            })?;
        let config_environment = ["HOME", "XDG_CONFIG_HOME", "USERPROFILE"]
            .into_iter()
            .filter_map(|name| {
                let path = PathBuf::from(env::var_os(name)?);
                path.is_absolute().then_some((name, path))
            })
            .collect();
        Ok(Self {
            executable,
            config_environment,
        })
    }

    fn read_identity_and_branch(&self, root: &Path) -> IdentityAndBranch {
        let mut selected = IdentityAndBranch::default();
        let Ok(output) = self.run(
            root,
            &[
                "config",
                "--null",
                "--get-regexp",
                r"^(init\.defaultbranch|user\.(name|email))$",
            ],
            SHORT_GIT_TIMEOUT,
            true,
        ) else {
            return selected;
        };
        if output.status.success() {
            for entry in output.stdout.split(|&byte| byte == b'\0') {
                let Some((key, value)) = std::str::from_utf8(entry)
                    .ok()
                    .and_then(|entry| entry.split_once('\n'))
                else {
                    continue;
                };
                match key {
                    "init.defaultbranch" => selected.branch = Some(value.to_owned()),
                    "user.name" => selected.name = Some(value.to_owned()),
                    "user.email" => selected.email = Some(value.to_owned()),
                    _ => {}
                }
            }
        }
        selected
    }

    fn required(&self, root: &Path, operation: &'static str, args: &[&str]) -> io::Result<()> {
        let output = self.run(root, args, SHORT_GIT_TIMEOUT, false)?;
        if output.status.success() {
            Ok(())
        } else {
            Err(io::Error::other(format!(
                "Git {operation} failed: {}",
                last_message(&output.stderr).unwrap_or_else(|| output.status.to_string())
            )))
        }
    }

    fn run(
        &self,
        root: &Path,
        args: &[&str],
        timeout: Duration,
        read_config: bool,
    ) -> io::Result<GitOutput> {
        let mut command = Command::new(&self.executable);
        command
            .env_clear()
            .env("LC_ALL", "C")
            .env("GIT_DIR", ".git")
            .env("GIT_WORK_TREE", ".")
            .env("GIT_TERMINAL_PROMPT", "0")
            .env("GIT_ALLOW_PROTOCOL", "")
            .env("GIT_ATTR_NOSYSTEM", "1")
            .current_dir(root)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        // Required infrastructure for Git for Windows, not Git behavior settings.
        #[cfg(windows)]
        for key in ["SystemRoot", "WINDIR"] {
            if let Some(value) = env::var_os(key) {
                command.env(key, value);
            }
        }
        if read_config {
            command.envs(
                self.config_environment
                    .iter()
                    .map(|(key, path)| (*key, path)),
            );
        } else {
            // Mutation commands never consume the machine, user, or inherited
            // process config. Start with no templates, no attributes, no hooks,
            // no signing, no filters and no background repository maintenance.
            command
                .env("GIT_CONFIG_NOSYSTEM", "1")
                .env("GIT_CONFIG_GLOBAL", ".git/butler-no-global-config")
                .args([
                    "-c",
                    "core.hooksPath=.git/butler-no-hooks",
                    "-c",
                    "core.attributesFile=.git/butler-no-attributes",
                    "-c",
                    "core.safecrlf=false",
                    "-c",
                    "core.autocrlf=false",
                    "-c",
                    "commit.gpgSign=false",
                    "-c",
                    "maintenance.auto=false",
                    "-c",
                    "gc.auto=0",
                ]);
        }
        let mut child = command.args(args).spawn()?;
        // Drain both pipes concurrently, holding only a limited prefix. These
        // fixed Git built-ins use no external commands, so kill closes the pipes.
        let stdout = child.stdout.take().expect("piped stdout");
        let stderr = child.stderr.take().expect("piped stderr");
        thread::scope(|scope| -> io::Result<GitOutput> {
            let out = scope.spawn(|| drain(stdout));
            let err = scope.spawn(|| drain(stderr));
            let start = Instant::now();
            let status = loop {
                match child.try_wait() {
                    Ok(Some(status)) => break Ok(status),
                    Ok(None) if start.elapsed() < timeout => {
                        thread::sleep(Duration::from_millis(10))
                    }
                    Ok(None) => {
                        break Err(io::Error::new(io::ErrorKind::TimedOut, "Git timed out."));
                    }
                    Err(err) => break Err(err),
                }
            };
            if status.is_err() {
                let _ = child.kill();
                let _ = child.wait();
            }
            let stdout = out
                .join()
                .map_err(|_| io::Error::other("Git output reader stopped."))??;
            let stderr = err
                .join()
                .map_err(|_| io::Error::other("Git error reader stopped."))??;
            Ok(GitOutput {
                status: status?,
                stdout,
                stderr,
            })
        })
    }
}

struct GitOutput {
    status: ExitStatus,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

fn drain(mut reader: impl Read) -> io::Result<Vec<u8>> {
    let mut output = Vec::new();
    let mut buffer = [0; 4096];
    loop {
        match reader.read(&mut buffer) {
            Ok(0) => return Ok(output),
            Ok(read) => {
                output.extend_from_slice(&buffer[..read.min(MAX_GIT_OUTPUT - output.len())])
            }
            Err(err) if err.kind() == io::ErrorKind::Interrupted => continue,
            Err(err) => return Err(err),
        }
    }
}

fn last_message(bytes: &[u8]) -> Option<String> {
    String::from_utf8_lossy(bytes)
        .lines()
        .rev()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(|line| line.chars().take(512).collect())
}

fn commit_warning(bytes: &[u8]) -> String {
    let stderr = String::from_utf8_lossy(bytes).to_ascii_lowercase();
    if [
        "identity unknown",
        "tell me who you are",
        "no name was given",
        "no email was given",
    ]
    .iter()
    .any(|needle| stderr.contains(needle))
    {
        "Git has no name or email on this machine. Set user.name and user.email, then commit."
            .into()
    } else {
        last_message(bytes).unwrap_or_else(|| "Git could not make the first commit.".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    struct Temp(PathBuf);

    impl Temp {
        fn new() -> Self {
            let path = env::temp_dir().join(format!("butler-managed-{}", Uuid::new_v4().simple()));
            fs::create_dir(&path).unwrap();
            Self(path.canonicalize().unwrap())
        }

        fn git(&self, user_config: &str) -> Git {
            // The previous tests use normal filesystem paths but must never rely
            // on or change the developer's Git configuration.
            let home = self.0.join("git-home");
            fs::create_dir(&home).unwrap();
            fs::write(home.join(".gitconfig"), user_config).unwrap();
            Git {
                executable: Git::installed()
                    .expect("Git is required for scaffold tests")
                    .executable,
                config_environment: vec![
                    ("GIT_CONFIG_NOSYSTEM", PathBuf::from("1")),
                    ("HOME", home.clone()),
                    ("XDG_CONFIG_HOME", home),
                ],
            }
        }
    }

    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn project_validation_and_slug_match_reference() {
        assert_eq!(
            title("\u{feff}\u{a0} Fancy Café \u{feff}\n").unwrap(),
            "Fancy Café"
        );
        assert_eq!(title("\u{85}").unwrap(), "\u{85}");
        assert!(title("\u{feff}  ").is_err());
        assert!(title(&"😀".repeat(100)).is_ok());
        assert!(title(&"😀".repeat(101)).is_err());
        assert_eq!(
            slug("Déjà vu — Straße Øresund 👋"),
            "deja-vu-strasse-oresund"
        );
        assert_eq!(slug("ＣＯＭ９"), "com9-project");
        assert_eq!(slug("..///😺"), "project");
        assert_eq!(slug("ŁŒڤÞ"), "loe-th");
        assert_eq!(slug(&format!("{}-tail", "a".repeat(63))), "a".repeat(63));
        assert_eq!(
            slug(&format!("{}tail", "a".repeat(63))),
            format!("{}t", "a".repeat(63))
        );
    }

    #[test]
    fn preview_is_unsuffixed_and_claims_never_touch_collisions() {
        let temp = Temp::new();
        let profile = temp.0.join("not-yet-created");
        assert_eq!(
            preview_root(&profile, " My App ").unwrap(),
            profile.join("projects/my-app")
        );
        assert!(!profile.exists());
        let parent = prepare_parent(&profile).unwrap();
        fs::write(parent.join("my-app"), "existing").unwrap();
        let paths = thread::scope(|scope| {
            let a = scope.spawn(|| claim(&parent, "my-app".as_ref()).unwrap().path);
            let b = scope.spawn(|| claim(&parent, "my-app".as_ref()).unwrap().path);
            [a.join().unwrap(), b.join().unwrap()]
        });
        assert!(paths.contains(&parent.join("my-app-2")));
        assert!(paths.contains(&parent.join("my-app-3")));
        assert_eq!(
            fs::read_to_string(parent.join("my-app")).unwrap(),
            "existing"
        );
        for i in 4..=100 {
            fs::create_dir(parent.join(format!("my-app-{i}"))).unwrap();
        }
        assert_eq!(
            claim(&parent, "my-app".as_ref()).err().unwrap().kind(),
            io::ErrorKind::AlreadyExists
        );
    }

    #[test]
    fn parent_cannot_be_in_an_existing_git_checkout() {
        let temp = Temp::new();
        let alternative_git_dir = temp.0.join("fake-git-dir");
        fs::create_dir(&alternative_git_dir).unwrap();
        fs::write(alternative_git_dir.join("HEAD"), "ref: refs/heads/main\n").unwrap();
        fs::write(temp.0.join(".git"), "gitdir: fake-git-dir\n").unwrap();
        let profile = temp.0.join("profile");
        assert!(prepare_parent(&profile).is_err());
        assert!(!profile.exists());
    }

    #[cfg(unix)]
    #[test]
    fn managed_container_cannot_be_a_symlink() {
        let temp = Temp::new();
        let target = temp.0.join("target");
        fs::create_dir(&target).unwrap();
        std::os::unix::fs::symlink(&target, temp.0.join("projects")).unwrap();
        assert!(prepare_parent(&temp.0).is_err());
        assert_eq!(fs::read_dir(target).unwrap().count(), 0);
    }

    #[test]
    fn scaffold_initials_follow_unicode_category_uppercase_and_escaping() {
        assert!(named_icon("Café Déjà").contains(">CD</text>"));
        assert!(named_icon("ज़ data").contains(">जD</text>"));
        assert!(named_icon("ß").contains("font-size=\"26\""));
        assert!(named_icon("ß").contains(">SS</text>"));
        assert!(named_icon("&<").contains(">&amp;</text>"));
        assert!(named_icon("😀").contains("font-size=\"26\""));
        assert!(named_icon("🚂").contains(">🚂</text>"));
    }

    #[test]
    fn scaffold_initial_commit_uses_only_identity_and_branch_from_configuration() {
        let temp = Temp::new();
        let template = temp.0.join("template");
        fs::create_dir(&template).unwrap();
        fs::write(template.join("outside-template"), "should not be copied").unwrap();
        let hook_source = temp.0.join("hooks");
        fs::create_dir(&hook_source).unwrap();
        fs::write(hook_source.join("pre-commit"), "#!/bin/sh\nexit 1\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(
                hook_source.join("pre-commit"),
                fs::Permissions::from_mode(0o755),
            )
            .unwrap();
        }
        let attrs = temp.0.join("attributes");
        fs::write(&attrs, "* filter=disallowed\n").unwrap();
        let git = temp.git(&format!(
            "[init]\n   defaultBranch = trunk\n   templateDir = {}\n[user]\n   name = Local Developer\n   email = developer@example.invalid\n[core]\n   hooksPath = {}\n   attributesFile = {}\n[commit]\n   gpgsign = true\n[gpg]\n   program = intentionally-missing-gpg\n[filter \"disallowed\"]\n   clean = intentionally-missing-filter\n   required = true\n",
            template.display(), hook_source.display(), attrs.display()
        ));
        let parent = prepare_parent(&temp.0.join("profile")).unwrap();
        let folder = claim(&parent, "cafe-deja".as_ref()).unwrap();
        let project = scaffold(&git, &folder, "Café Déjà").unwrap();
        assert_eq!(project.root, folder.path);
        assert_eq!(project.title, "Café Déjà");
        assert_eq!(project.warning, None);
        assert_eq!(
            fs::read_to_string(project.root.join(".git/HEAD")).unwrap(),
            "ref: refs/heads/trunk\n"
        );
        assert!(!project.root.join(".git/outside-template").exists());
        assert!(!project.root.join(".git/hooks").exists());
        assert_eq!(
            fs::read_to_string(project.root.join("README.md")).unwrap(),
            named_readme("Café Déjà"),
        );
        assert!(
            fs::read_to_string(project.root.join("assets/icon.svg"))
                .unwrap()
                .contains(">CD</text>")
        );
        let tracked = git
            .run(
                &project.root,
                &["ls-tree", "-r", "--name-only", "HEAD"],
                SHORT_GIT_TIMEOUT,
                false,
            )
            .unwrap();
        assert!(tracked.status.success());
        assert_eq!(tracked.stdout, b"README.md\nassets/icon.svg\n");
        let commit = git
            .run(
                &project.root,
                &["log", "-1", "--format=%s|%an|%ae"],
                SHORT_GIT_TIMEOUT,
                false,
            )
            .unwrap();
        assert!(commit.status.success());
        assert_eq!(
            commit.stdout,
            b"Initial commit|Local Developer|developer@example.invalid\n"
        );
    }

    #[test]
    fn missing_identity_leaves_staged_files_and_a_warning() {
        let temp = Temp::new();
        let git = temp.git("");
        let parent = prepare_parent(&temp.0.join("profile")).unwrap();
        let folder = claim(&parent, "example".as_ref()).unwrap();
        let project = scaffold(&git, &folder, "Example").unwrap();
        assert_eq!(
            project.warning.as_deref(),
            Some(
                "Git has no name or email on this machine. Set user.name and user.email, then commit."
            )
        );
        assert_eq!(
            fs::read_to_string(project.root.join(".git/HEAD")).unwrap(),
            "ref: refs/heads/main\n"
        );
        let entries = git
            .run(&project.root, &["ls-files"], SHORT_GIT_TIMEOUT, false)
            .unwrap();
        assert!(entries.status.success());
        assert_eq!(entries.stdout, b"README.md\nassets/icon.svg\n");
    }

    #[test]
    fn fatal_init_failure_removes_only_its_own_claim() {
        let temp = Temp::new();
        let git = temp.git("[init]\n defaultBranch = not a valid branch\n");
        let profile = temp.0.join("profile");
        let parent = prepare_parent(&profile).unwrap();
        let candidate = preview_root(&profile, "Example").unwrap();
        fs::write(&candidate, "pre-existing").unwrap();
        let error = create_with(&git, &profile, "Example", &candidate).unwrap_err();
        assert!(error.to_string().starts_with("Git init failed"));
        assert_eq!(fs::read_to_string(&candidate).unwrap(), "pre-existing");
        assert!(!parent.join("example-2").exists());
    }
}
