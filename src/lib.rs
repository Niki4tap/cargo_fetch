#![doc = include_str!("../README.md")]

use cargo::{
    core::{PackageId, PackageSet, SourceId, SourceMap},
    sources::CRATES_IO_INDEX,
    util::IntoUrl,
};
use semver::Version;
use std::{collections::HashSet, io::Write, path::PathBuf, str::FromStr};
use url::Url;

/// Main API of this library.
///
/// Contains cargo config to drive package fetching.
///
/// You can construct default instance of this struct by using [`PackageFetcher::default`],
/// note however, that this function is *not* from [`Default`] trait, and can return an error.
///
/// With default [`PackageFetcher`], cargo will try to output status and errors to the `stdout` and `stderr` of
/// current process. If that is not desirable, you can construct it with
/// [`PackageFetcher::with_out`], to intercept cargo `write` calls.
///
/// After constructing, you can use [`PackageFetcher::fetch`], or [`PackageFetcher::fetch_many`]
/// methods to fetch packages, see them for more documentation.
#[derive(Debug)]
pub struct PackageFetcher {
    config: cargo::Config,
}

impl PackageFetcher {
    /// Constructs [`PackageFetcher`] with default cargo configuration.
    ///
    /// Cargo will output its colored status to the `stdout` and `stderr` of the current process by default, if that is not desirable, see
    /// [`PackageFetcher::with_out`].
    #[allow(clippy::should_implement_trait)]
    pub fn default() -> Result<Self, String> {
        Ok(Self {
            config: cargo::Config::default().map_err(|e| e.to_string())?,
        })
    }

    /// Constructs [`PackageFetcher`] with user-provided stream for cargo to output status to.
    ///
    /// Optionally also accepts [`Verbosity`], which is set to [`Verbosity::Verbose`] if [`None`] is provided.
    pub fn with_out(out: Box<dyn Write>, verbosity: Option<Verbosity>) -> Result<Self, String> {
        let mut shell = cargo::core::Shell::from_write(out);
        shell.set_verbosity(verbosity.unwrap_or_default().into());
        let new_self = Self::default()?;
        let mut sh = new_self.config.shell();
        *sh = shell;
        drop(sh);
        Ok(new_self)
    }

    /// Fetches a single package, and returns the [`PathBuf`] to the root of it.
    pub fn fetch(
        &mut self,
        package: Package,
        yanked_whitelist: Option<HashSet<Package>>,
    ) -> Result<PathBuf, String> {
        let _lock = self
            .config
            .acquire_package_cache_lock()
            .map_err(|e| e.to_string())?;
        let mut map = SourceMap::new();

        let whitelist: HashSet<PackageId>;

        if let Some(wl) = yanked_whitelist {
            whitelist = wl.iter().map(|p| p.package_id).collect();
        } else {
            whitelist = Default::default();
        };

        let mut source = package
            .package_id
            .source_id()
            .load(&self.config, &whitelist)
            .map_err(|e| e.to_string())?;

        source.block_until_ready().map_err(|e| e.to_string())?;

        map.insert(source);

        let package_set =
            PackageSet::new(&[package.package_id], map, &self.config).map_err(|e| e.to_string())?;
        Ok(package_set
            .get_one(package.package_id)
            .map_err(|e| e.to_string())?
            .root()
            .into())
    }

    /// Fetches multiple packages, and returns the [`PathBuf`]s to their roots.
    ///
    /// **Warning**
    ///
    /// This is not guaranteed to return the same amount of roots as requested packages,
    /// as packages passed in might be from the same source, with the same version, in
    /// which case cargo will cache the package sources, and return the root only once,
    /// no matter what amount of duplicate packages was passed.
    ///
    /// Errors, if any of the requested packages cannot be fetched.
    pub fn fetch_many(
        &mut self,
        packages: &[Package],
        yanked_whitelist: Option<HashSet<Package>>,
    ) -> Result<Vec<PathBuf>, String> {
        let _lock = self
            .config
            .acquire_package_cache_lock()
            .map_err(|e| e.to_string())?;
        let mut map = SourceMap::new();

        let whitelist: HashSet<PackageId>;

        if let Some(wl) = yanked_whitelist {
            whitelist = wl.iter().map(|p| p.package_id).collect();
        } else {
            whitelist = Default::default();
        };

        for package in packages {
            let mut source = package
                .package_id
                .source_id()
                .load(&self.config, &whitelist)
                .map_err(|e| e.to_string())?;
            source.block_until_ready().map_err(|e| e.to_string())?;
            map.insert(source)
        }

        let packages: Vec<PackageId> = packages.iter().map(|p| p.package_id).collect();
        let package_set =
            PackageSet::new(&packages, map, &self.config).map_err(|e| e.to_string())?;
        Ok(package_set
            .get_many(package_set.package_ids())
            .map_err(|e| e.to_string())?
            .iter()
            .map(|p| p.root().to_owned())
            .collect())
    }
}

/// Cargo verbosity for use with [`PackageFetcher::with_out`].
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum Verbosity {
    #[default]
    Verbose,
    Normal,
    Quiet,
}

impl From<Verbosity> for cargo::core::Verbosity {
    fn from(value: Verbosity) -> Self {
        match value {
            Verbosity::Verbose => Self::Verbose,
            Verbosity::Normal => Self::Normal,
            Verbosity::Quiet => Self::Quiet,
        }
    }
}

/// Package definition to be fetched by cargo.
///
/// Note that this struct is not an actual package,
/// it only contains the information needed for cargo to fetch the actual package.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Package {
    package_id: PackageId,
}

impl Package {
    /// Constructs a [`Package`], from package name, its [`semver::Version`], and source where to
    /// fetch it from (crates.io, git, ...).
    pub fn new<S: AsRef<str>>(
        name: S,
        version: Version,
        source: &PackageSource,
    ) -> Result<Self, String> {
        Ok(Package {
            package_id: PackageId::new(
                name.as_ref(),
                version,
                source.to_source_id().map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())?,
        })
    }

    /// Same as [`Package::new`], but parses [`semver::Version`] from a [`str`].
    pub fn from_str_ver<S: AsRef<str>, V: AsRef<str>>(
        name: S,
        version: V,
        source: &PackageSource,
    ) -> Result<Self, String> {
        Ok(Package {
            package_id: PackageId::new(
                name.as_ref(),
                Version::from_str(version.as_ref()).map_err(|e| e.to_string())?,
                source.to_source_id().map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())?,
        })
    }
}

/// Git reference for [`PackageSource::Git`]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GitReference {
    DefaultBranch,
    Branch(String),
    Revision(String),
    Tag(String),
}

impl From<GitReference> for cargo::core::GitReference {
    fn from(value: GitReference) -> Self {
        match value {
            GitReference::DefaultBranch => Self::DefaultBranch,
            GitReference::Branch(branch) => Self::Branch(branch),
            GitReference::Revision(rev) => Self::Rev(rev),
            GitReference::Tag(tag) => Self::Tag(tag),
        }
    }
}

/// Defines a source from which a package can be fetched.
///
/// This enum can either be constructed manually, or with associated helper functions on it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackageSource {
    /// Path source:
    /// ```toml
    /// dep = { path = "some/local/dependency" }
    /// ```
    Path(PathBuf),
    /// Git source:
    /// ```toml
    /// regex = { git = "https://github.com/rust-lang/regex", branch = "next" }
    /// ```
    Git { url: Url, git_ref: GitReference },
    /// Remote registry:
    /// ```toml
    /// some-crate = { version = "1.0", registry = "my-registry" }
    /// ```
    RemoteRegistry(Url),
    /// Local registry:
    /// ```toml
    /// some-crate = { version = "1.0", registry = "my-local-registry" }
    /// ```
    LocalRegistry(PathBuf),
    /// `crates.io`:
    /// ```toml
    /// foo = "1.0.0"
    /// ```
    ///
    /// Note that this does *not* respect `.cargo/config.toml`, so if `default-registry` or `crates-io`
    /// are overriden, this would still fetch from `crates.io`
    CratesIo,
}

impl PackageSource {
    /// Constructs a new [`PackageSource::Path`] from path.
    pub fn path<P: Into<PathBuf>>(path: P) -> Self {
        Self::Path(path.into())
    }

    /// Constructs a new [`PackageSource::Git`] from repository url and an optional [`GitReference`], if [`None`] is provided, [`GitReference::DefaultBranch`] will be assumed.
    pub fn git<U: TryInto<Url>>(url: U, git_ref: Option<GitReference>) -> Result<Self, U::Error> {
        Ok(Self::Git {
            url: url.try_into()?,
            git_ref: git_ref.unwrap_or(GitReference::DefaultBranch),
        })
    }

    /// Constructs a new [`PackageSource::RemoteRegistry`] from a registry index url.
    pub fn remote<U: TryInto<Url>>(url: U) -> Result<Self, U::Error> {
        Ok(Self::RemoteRegistry(url.try_into()?))
    }

    /// Constructs a new [`PackageSource::LocalRegistry`] from a registry index path.
    pub fn local<P: Into<PathBuf>>(path: P) -> Self {
        Self::LocalRegistry(path.into())
    }

    /// Returns [`PackageSource::CratesIo`].
    pub fn crates_io() -> Self {
        Self::CratesIo
    }

    fn to_source_id(&self) -> cargo::CargoResult<SourceId> {
        match self {
            PackageSource::Path(path) => SourceId::for_path(path),
            PackageSource::Git { url, git_ref } => SourceId::for_git(url, git_ref.clone().into()),
            PackageSource::RemoteRegistry(url) => SourceId::for_registry(url),
            PackageSource::LocalRegistry(path) => SourceId::for_local_registry(path),
            PackageSource::CratesIo => SourceId::for_registry(&CRATES_IO_INDEX.into_url().unwrap()),
        }
    }
}
