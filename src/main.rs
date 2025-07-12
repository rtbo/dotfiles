use clap::{Parser, Subcommand};
use std::{fmt, fs, io};
use std::{
    path::{Path, PathBuf},
    process::ExitCode,
};

#[derive(Debug)]
enum Error {
    Clap(clap::error::Error),
    Io(std::io::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Clap(e) => write!(f, "{}", e),
            Error::Io(e) => write!(f, "{}", e),
        }
    }
}

impl From<clap::error::Error> for Error {
    fn from(err: clap::error::Error) -> Self {
        Error::Clap(err)
    }
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Error::Io(err)
    }
}

#[derive(Debug, Parser)]
#[command(name = "dotfiles")]
#[command(about = "A backup utility for config dotfiles")]
struct Cli {
    #[arg(short = 'r')]
    repo: Option<PathBuf>,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Store {
        #[arg(value_name = "PKGS")]
        pkgs: Vec<String>,
    },
    Stage {
        #[arg(value_name = "PKGS")]
        pkgs: Vec<String>,
    },
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("{err}");
            ExitCode::FAILURE
        }
    }
}

fn default_repo(home: &Path) -> PathBuf {
    home.join(".dotfiles")
}

fn run() -> Result<(), Error> {
    let cli = Cli::try_parse()?;

    let home = std::env::var_os("HOME").expect("$HOME should be set");
    let home = Path::new(&home);
    let repo = cli.repo.clone().unwrap_or_else(|| default_repo(home));

    match cli.command {
        Command::Store { pkgs } => {
            if pkgs.is_empty() {
                eprintln!("No packages specified, nothing to do");
            }
            for pkg in pkgs {
                let pkg_path = find_pkg_path(&home, &pkg);
                if pkg_path.is_none() {
                    eprintln!("Could not find config files for {pkg}");
                    continue;
                }
                let pkg_path = pkg_path.unwrap();
                let store_path = repo.join(pkg);
                delete_all(&store_path)?;
                let store_to_pkg = pkg_path.strip_prefix(&home).unwrap();
                let pkg_store = store_path.join(&store_to_pkg);
                copy_all(&pkg_path, &pkg_store)?;
            }
        }
        Command::Stage { pkgs } => {
            if pkgs.is_empty() {
                eprintln!("No packages specified, nothing to do");
            }
            for pkg in pkgs {
                let store_path = repo.join(&pkg);
                if !store_path.exists() {
                    eprintln!("No stored config found for {pkg}, skipping");
                    continue;
                }
                let home_to_pkg = find_pkg_path(&store_path, &pkg)
                    .map(|p| p.strip_prefix(&store_path).unwrap().to_path_buf())
                    .or_else(|| find_pkg_path(&home, &pkg).map(|p| p.strip_prefix(&home).unwrap().to_path_buf()));
                if let Some(home_to_pkg) = home_to_pkg {
                    let home_path = home.join(home_to_pkg);
                    delete_all(&home_path)?;
                }
                copy_all(&store_path, &home)?;
            }
        }
    }

    Ok(())
}

fn delete_all(path: &Path) -> io::Result<()> {
    if path.exists() && path.is_dir() {
        std::fs::remove_dir_all(path)
    } else if path.exists() && path.is_file() {
        std::fs::remove_file(path)
    } else {
        Ok(())
    }
}

fn copy_all(src: &Path, dest: &Path) -> io::Result<()> {
    if dest.exists() {
        panic!("should remove dest before copy");
    }
    if !src.exists() {
        panic!("src should exist");
    }
    if src.is_dir() {
        fs::create_dir_all(dest)?;
        for entry in fs::read_dir(src)? {
            let entry = entry?;
            let src_path = entry.path();
            let dest_path = dest.join(entry.file_name());
            copy_all(&src_path, &dest_path)?;
        }
    } else if src.is_file() {
        fs::create_dir_all(dest.parent().unwrap())?;
        println!("Copying {} to {}", src.display(), dest.display());
        fs::copy(src, dest)?;
    }

    Ok(())
}

fn find_pkg_path(home: &Path, pkg: &str) -> Option<PathBuf> {
    // check in order for:
    //  - ~/.pkg
    //  - ~/.pkg[suffix]
    //  - ~/.config/pkg
    //  - ~/.config/pkg[suffix]
    // [suffix] being one of:
    //   rc, .d, .conf, .conf.d, .toml, .xml, .json, .yml, .lua
    // it stops at the first occurence found

    fn check(path: PathBuf) -> Option<PathBuf> {
        if path.exists() { Some(path) } else { None }
    }

    let suffixes = &[
        "rc", ".d", ".conf", ".conf.d", ".toml", ".xml", ".json", ".yml", ".lua",
    ];
    let dotpkg = format!(".{pkg}");
    if let Some(path) = check(home.join(&dotpkg)) {
        return Some(path);
    }
    for s in suffixes.iter() {
        if let Some(path) = check(home.join(format!("{dotpkg}{s}"))) {
            return Some(path);
        }
    }
    if let Some(path) = check(home.join(".config").join(pkg)) {
        return Some(path);
    }
    for s in suffixes.iter() {
        if let Some(path) = check(home.join(".config").join(format!("{pkg}{s}"))) {
            return Some(path);
        }
    }
    None
}
