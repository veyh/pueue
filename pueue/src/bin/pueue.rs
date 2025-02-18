use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use clap::{CommandFactory, Parser};
use clap_complete::{generate_to, shells};
use log::warn;
use simplelog::{Config, ConfigBuilder, LevelFilter, SimpleLogger};

use pueue_lib::settings::Settings;

use pueue::client::cli::{CliArguments, Shell, SubCommand};
use pueue::client::client::Client;

/// This is the main entry point of the client.
///
/// At first we do some basic setup:
/// - Parse the cli
/// - Initialize logging
/// - Read the config
///
/// Once all this is done, we init the [Client] struct and start the main loop via [Client::start].
#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    // Parse commandline options.
    let opt = CliArguments::parse();

    // In case the user requested the generation of shell completion file, create it and exit.
    if let Some(SubCommand::Completions {
        shell,
        output_directory,
    }) = &opt.cmd
    {
        return create_shell_completion_file(shell, output_directory);
    }

    // Init the logger and set the verbosity level depending on the `-v` flags.
    let level = match opt.verbose {
        0 => LevelFilter::Error,
        1 => LevelFilter::Warn,
        2 => LevelFilter::Info,
        _ => LevelFilter::Debug,
    };

    // Try to initialize the logger with the timezone set to the Local time of the machine.
    let mut builder = ConfigBuilder::new();
    let logger_config = match builder.set_time_offset_to_local() {
        Err(_) => {
            warn!("Failed to determine the local time of this machine. Fallback to UTC.");
            Config::default()
        }
        Ok(builder) => builder.build(),
    };

    SimpleLogger::init(level, logger_config).unwrap();

    // Try to read settings from the configuration file.
    let (mut settings, config_found) =
        Settings::read(&opt.config).context("Failed to read configuration.")?;

    // Load any requested profile.
    if let Some(profile) = &opt.profile {
        settings.load_profile(profile)?;
    }

    // Error if no configuration file can be found, as this is an indicator, that the daemon hasn't
    // been started yet.
    if !config_found {
        bail!("Couldn't find a configuration file. Did you start the daemon yet?");
    }

    // Warn if the deprecated --children option was used
    if let Some(subcommand) = &opt.cmd {
        if matches!(
            subcommand,
            SubCommand::Start { children: true, .. }
                | SubCommand::Pause { children: true, .. }
                | SubCommand::Kill { children: true, .. }
                | SubCommand::Reset { children: true, .. }
        ) {
            println!(concat!(
                "Note: The --children flag is deprecated and will be removed in a future release. ",
                "It no longer has any effect, as this command now always applies to all processes in a task."
            ));
        }
    }

    // Create client to talk with the daemon and connect.
    let mut client = Client::new(settings, opt)
        .await
        .context("Failed to initialize client.")?;
    client.start().await?;

    Ok(())
}

/// [clap] is capable of creating auto-generated shell completion files.
/// This function creates such a file for one of the supported shells and puts it into the
/// specified output directory.
fn create_shell_completion_file(shell: &Shell, output_directory: &PathBuf) -> Result<()> {
    let mut app = CliArguments::command();
    app.set_bin_name("pueue");
    let completion_result = match shell {
        Shell::Bash => generate_to(shells::Bash, &mut app, "pueue", output_directory),
        Shell::Elvish => generate_to(shells::Elvish, &mut app, "pueue", output_directory),
        Shell::Fish => generate_to(shells::Fish, &mut app, "pueue", output_directory),
        Shell::PowerShell => generate_to(shells::PowerShell, &mut app, "pueue", output_directory),
        Shell::Zsh => generate_to(shells::Zsh, &mut app, "pueue", output_directory),
    };
    completion_result.context(format!("Failed to generate completions for {shell:?}"))?;

    Ok(())
}
