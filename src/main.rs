use cargo_metadata::MetadataCommand;
use rustc_version::{Channel, Version};
use std::path::Path;
use std::{
    env, fmt,
    process::{self, Command, Stdio},
};

#[derive(serde_derive::Deserialize, Default)]
struct CTRConfig {
    name: String,
    author: String,
    description: String,
    icon: String,
}

#[derive(Ord, PartialOrd, PartialEq, Eq, Debug)]
struct CommitDate {
    year: i32,
    month: i32,
    day: i32,
}

impl CommitDate {
    fn parse(date: &str) -> Option<Self> {
        let mut iter = date.split('-');

        let year = iter.next()?.parse().ok()?;
        let month = iter.next()?.parse().ok()?;
        let day = iter.next()?.parse().ok()?;

        Some(Self { year, month, day })
    }
}

impl fmt::Display for CommitDate {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }
}

const MINIMUM_COMMIT_DATE: CommitDate = CommitDate {
    year: 2021,
    month: 10,
    day: 1,
};
const MINIMUM_RUSTC_VERSION: Version = Version::new(1, 56, 0);

fn main() {
    check_rust_version();

    let optimization_level = match env::args().any(|arg| arg == "--release") {
        true => String::from("release"),
        false => String::from("debug"),
    };

    // Skip `cargo 3ds`
    let mut args = env::args().skip(2);

    let command = args.next();
    let must_link = match command {
        None => panic!("No command specified, try with \"build\" or \"link\""),
        Some(s) => match s.as_str() {
            "build" => false,
            "link" => true,
            _ => panic!("Invalid command, try with \"build\" or \"link\""),
        },
    };

    build_elf(args);

    let app_conf = get_metadata();
    build_3dsx(&app_conf, &optimization_level);

    if must_link {
        link(&app_conf.name, &optimization_level);
    }
}

fn check_rust_version() {
    let rustc_version = rustc_version::version_meta().unwrap();

    if rustc_version.channel > Channel::Nightly {
        println!("cargo-3ds requires a nightly rustc version.");
        println!(
            "Please run `rustup override set nightly` to use nightly in the \
            current directory."
        );
        process::exit(1);
    }

    let old_version: bool = MINIMUM_RUSTC_VERSION > rustc_version.semver;

    let old_commit = match rustc_version.commit_date {
        None => false,
        Some(date) => {
            MINIMUM_COMMIT_DATE
                > CommitDate::parse(&date).expect("could not parse `rustc --version` commit date")
        }
    };

    if old_version || old_commit {
        println!(
            "cargo-3ds requires rustc nightly version >= {}",
            MINIMUM_COMMIT_DATE,
        );
        println!("Please run `rustup update nightly` to upgrade your nightly version");

        process::exit(1);
    }
}

fn build_elf(args: std::iter::Skip<env::Args>) {
    let rustflags = format!("-L{}/libctru/lib -lctru ", env::var("DEVKITPRO").unwrap());

    let mut process = Command::new("cargo")
        .arg("build")
        .arg("-Z")
        .arg("build-std")
        .arg("--target")
        .arg("armv6k-nintendo-3ds")
        .args(args)
        .env("RUSTFLAGS", rustflags)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap();

    let status = process.wait().unwrap();

    if !status.success() {
        process::exit(status.code().unwrap_or(1));
    }
}

fn get_metadata() -> CTRConfig {
    let metadata = MetadataCommand::new()
        .exec()
        .expect("Failed to get cargo metadata");

    let root_crate = metadata.root_package().expect("No root crate found");

    let mut icon = String::from("./icon.png");

    if !Path::new(&icon).exists() {
        icon = format!(
            "{}/libctru/default_icon.png",
            env::var("DEVKITPRO").unwrap()
        )
    }

    CTRConfig {
        name: root_crate.name.clone(),
        author: root_crate.authors[0].clone(),
        description: root_crate
            .description
            .clone()
            .unwrap_or_else(|| String::from("Homebrew Application")),
        icon,
    }
}

fn build_3dsx(config: &CTRConfig, opt_lvl: &str) {
    let mut process = Command::new("smdhtool")
        .arg("--create")
        .arg(&config.name)
        .arg(&config.description)
        .arg(&config.author)
        .arg(&config.icon)
        .arg(format!(
            "./target/armv6k-nintendo-3ds/{}/{}.smdh",
            opt_lvl, config.name
        ))
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap();

    let status = process.wait().unwrap();

    if !status.success() {
        process::exit(status.code().unwrap_or(1));
    }

    let mut command = Command::new("3dsxtool");
    let mut process = command
        .arg(format!(
            "./target/armv6k-nintendo-3ds/{}/{}.elf",
            opt_lvl, config.name
        ))
        .arg(format!(
            "./target/armv6k-nintendo-3ds/{}/{}.3dsx",
            opt_lvl, config.name
        ))
        .arg(format!(
            "--smdh=./target/armv6k-nintendo-3ds/{}/{}.smdh",
            opt_lvl, config.name
        ));

    // If romfs directory exists, automatically include it
    if Path::new("./romfs").is_dir() {
        process = process.arg("--romfs=\"./romfs\"");
    }

    let mut process = process
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap();

    let status = process.wait().unwrap();

    if !status.success() {
        process::exit(status.code().unwrap_or(1));
    }
}

fn link(name: &str, opt_lvl: &str) {
    let mut process = Command::new("3dslink")
        .arg(format!(
            "./target/armv6k-nintendo-3ds/{}/{}.3dsx",
            opt_lvl, name
        ))
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap();

    let status = process.wait().unwrap();

    if !status.success() {
        process::exit(status.code().unwrap_or(1));
    }
}