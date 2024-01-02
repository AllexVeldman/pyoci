#![warn(missing_docs)]
#![warn(clippy::pedantic)]

//! ``PyOCI``

use std::{error::Error, str::FromStr};

mod client;
mod package;

fn list(url: &str, username: Option<&str>, password: Option<&str>) -> Result<Vec<String>, Box<dyn Error>> {
    let package = package::Info::from_str(url)?;
    let client = client::Client::new(&package.registry)?.authenticate(username, password)?;
    let files = client.list_package_files(&package)?;
    Ok(files)
}

mod cli {

    use std::error::Error;

    use clap::{Parser, Subcommand};

    use crate::list;

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
            out_dir: String,
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
            Some(Commands::Download { .. }) => {
                todo!()
            }
            None => Ok(()),
        }
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    cli::run()
}
