#![warn(missing_docs)]
#![warn(clippy::pedantic)]

//! ``PyOCI``

use std::{error::Error, io::Read, str::FromStr};
use tracing::Level;

mod client;
mod package;

#[tracing::instrument(skip(username, password))]
fn list(
    url: &str,
    username: Option<&str>,
    password: Option<&str>,
) -> Result<Vec<String>, Box<dyn Error>> {
    let package = package::Info::from_str(url)?;
    let client = client::Client::new(&package.registry, username, password)?;
    let files = client.list_package_files(&package)?;
    Ok(files)
}

#[tracing::instrument(skip(username, password))]
fn download(
    url: &str,
    username: Option<&str>,
    password: Option<&str>,
) -> Result<(package::Info, impl Read), Box<dyn Error>> {
    let package = package::Info::from_str(url)?;
    let client = client::Client::new(&package.registry, username, password)?;
    let data = client.download_package_file(&package)?;
    Ok((package, data))
}

fn publish(
    url: &str,
    file: &str,
    username: Option<&str>,
    password: Option<&str>,
) -> Result<(), Box<dyn Error>> {
    let package = package::Info::from_str(url)?;
    let client = client::Client::new(&package.registry, username, password)?;
    client.publish_package_file(&package, file)?;
    Ok(())
}

mod cli {

    use std::{
        error::Error,
        fs,
        io::{self, BufReader, BufWriter},
        path::PathBuf,
    };

    use clap::{Parser, Subcommand};

    use crate::{download, list};

    #[derive(Parser)]
    #[command(author, version, about, long_about = None, help_expected = true, arg_required_else_help = true, disable_help_subcommand = true)]
    struct Cli {
        /// Username to authenticate to the OCI registry with
        #[arg(short, long)]
        username: Option<String>,
        /// Password to authenticate to the OCI registry with
        #[arg(short, long)]
        password: Option<String>,

        #[command(subcommand)]
        command: Option<Commands>,
    }

    #[derive(Subcommand)]
    enum Commands {
        /// List a python package in an OCI registry.
        List {
            /// URL of the package to list
            /// in the form `<registry>/<namespace>/<package>`
            url: String,
        },
        /// Publish a python package to an OCI registry.
        Publish {
            /// URL of the namespace to publish the package to in the form `<registry>/<namespace>`.
            url: String,
            /// File to publish, the filename should adhere to the python distribution file name spec.
            ///
            /// Source distribution: <https://packaging.python.org/en/latest/specifications/source-distribution-format/#source-distribution-file-name>
            /// Binary distribution: <https://packaging.python.org/en/latest/specifications/binary-distribution-format/#file-name-convention>
            #[arg(verbatim_doc_comment)]
            file: String,
        },
        /// Download a package from an OCI registry.
        Download {
            /// URL of the file to download in the form `<registry>/<namespace>/<package_name>/<filename>`.
            url: String,
            /// Directory to download the file to.
            out_dir: PathBuf,
        },
    }

    pub fn run() -> Result<(), Box<dyn Error>> {
        let cli = Cli::parse();
        match cli.command {
            Some(Commands::List { url }) => {
                let result = list(&url, cli.username.as_deref(), cli.password.as_deref())?;
                for file in result {
                    println!("{file}");
                }
                Ok(())
            }
            Some(Commands::Publish { .. }) => {
                todo!()
            }
            Some(Commands::Download { url, out_dir }) => {
                let (package, data) =
                    download(&url, cli.username.as_deref(), cli.password.as_deref())?;
                let mut file = fs::File::create(out_dir.join(package.file.to_string()))?;
                let mut reader = BufReader::new(data);
                let mut writer = BufWriter::new(&mut file);
                io::copy(&mut reader, &mut writer)?;
                Ok(())
            }
            None => Ok(()),
        }
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    tracing_subscriber::fmt()
        // all spans/events with a level higher than TRACE (e.g, info, warn, etc.)
        // will be written to stdout.
        .with_max_level(Level::INFO)
        // sets this to be the default, global collector for this application.
        .init();
    cli::run()
}
