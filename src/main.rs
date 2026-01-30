use clap::{Parser, Subcommand};
use colored::Colorize;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::Command;

#[derive(Parser)]
#[command(name = "cf-switch")]
#[command(about = "Cloudflare profile switcher for flarectl", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// List all profiles
    List,
    /// Add a new profile
    Add {
        /// Profile name
        name: String,
        /// Cloudflare account email
        #[arg(short, long)]
        email: String,
        /// API Token (recommended) or API Key
        #[arg(short, long)]
        token: String,
        /// Default zone for this profile (e.g., example.com)
        #[arg(short, long)]
        zone: Option<String>,
    },
    /// Remove a profile
    Remove {
        /// Profile name to remove
        name: String,
    },
    /// Switch to a specific profile
    Use {
        /// Profile name to activate
        name: String,
    },
    /// Show current active profile
    Current,
    /// Print shell hook for automatic sourcing
    Hook,
    /// Purge cache for a zone (uses profile's default zone if not specified)
    Purge {
        /// Zone to purge (e.g., 50bestspa.com) - optional if profile has default zone
        zone: Option<String>,
    },
    /// Add Lamdera app DNS record (CNAME @ -> apps.lamdera.app)
    AddLamderaApp {
        /// Domain to configure (e.g., myapp.com)
        domain: Option<String>,
    },
}

#[derive(Serialize, Deserialize, Default)]
struct Config {
    profiles: HashMap<String, Profile>,
    current: Option<String>,
}

#[derive(Serialize, Deserialize, Clone)]
struct Profile {
    email: String,
    token: String,
    #[serde(default)]
    zone: Option<String>,
}

fn config_path() -> PathBuf {
    dirs::home_dir()
        .expect("Could not find home directory")
        .join(".cf-switch.json")
}

fn env_path() -> PathBuf {
    dirs::home_dir()
        .expect("Could not find home directory")
        .join(".cloudflare.env")
}

fn load_config() -> Config {
    let path = config_path();
    if path.exists() {
        let content = fs::read_to_string(&path).expect("Failed to read config file");
        serde_json::from_str(&content).unwrap_or_default()
    } else {
        Config::default()
    }
}

fn save_config(config: &Config) {
    let path = config_path();
    let content = serde_json::to_string_pretty(config).expect("Failed to serialize config");
    fs::write(&path, content).expect("Failed to write config file");
}

fn write_env_file(profile: &Profile, name: &str) {
    let path = env_path();
    let content = format!(
        "# Cloudflare credentials - profile: {}\nexport CF_API_EMAIL=\"{}\"\nexport CF_API_KEY=\"{}\"\nexport CF_API_TOKEN=\"{}\"\n",
        name, profile.email, profile.token, profile.token
    );
    fs::write(&path, content).expect("Failed to write env file");
}

/// Print to stderr (for user-facing messages)
macro_rules! msg {
    ($($arg:tt)*) => {
        writeln!(io::stderr(), $($arg)*).ok();
    };
}

/// Print to stdout (for shell commands to be eval'd)
fn cmd(s: &str) {
    println!("{}", s);
}

fn detect_shell() -> String {
    std::env::var("SHELL")
        .unwrap_or_default()
        .rsplit('/')
        .next()
        .unwrap_or("bash")
        .to_string()
}

fn output_source_command() {
    let env_file = env_path().display().to_string();
    cmd(&format!("source {}", env_file));
}

fn switch_to_profile(config: &mut Config, name: &str) -> bool {
    if let Some(profile) = config.profiles.get(name).cloned() {
        write_env_file(&profile, name);
        config.current = Some(name.to_string());
        save_config(config);
        msg!("{} {} ({})", "ON".green().bold(), name.cyan().bold(), profile.email);
        output_source_command();
        true
    } else {
        msg!("{} Profile '{}' not found.", "Error:".red().bold(), name);
        false
    }
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        // No command = toggle to next profile
        None => {
            let mut config = load_config();
            if config.profiles.is_empty() {
                msg!("{}", "No profiles configured.".yellow());
                msg!("Add one with: cf-switch add <name> -e <email> -t <token>");
                return;
            }

            // Get sorted profile names for consistent ordering
            let mut names: Vec<_> = config.profiles.keys().cloned().collect();
            names.sort();

            // Find next profile
            let next_name = match &config.current {
                Some(current) => {
                    let current_idx = names.iter().position(|n| n == current).unwrap_or(0);
                    let next_idx = (current_idx + 1) % names.len();
                    names[next_idx].clone()
                }
                None => names[0].clone(),
            };

            switch_to_profile(&mut config, &next_name);
        }

        Some(Commands::List) => {
            let config = load_config();
            if config.profiles.is_empty() {
                msg!("{}", "No profiles configured.".yellow());
                msg!("Add one with: cf-switch add <name> -e <email> -t <token>");
                return;
            }
            msg!("{}", "Cloudflare Profiles:".bold());
            let mut names: Vec<_> = config.profiles.keys().collect();
            names.sort();
            for name in names {
                let profile = &config.profiles[name];
                let marker = if config.current.as_ref() == Some(name) {
                    "ON".green().bold()
                } else {
                    "  ".normal()
                };
                msg!("{} {} ({})", marker, name.cyan(), profile.email);
            }
        }

        Some(Commands::Add { name, email, token, zone }) => {
            let mut config = load_config();
            if config.profiles.contains_key(&name) {
                msg!("{} Profile '{}' already exists.", "Error:".red().bold(), name);
                std::process::exit(1);
            }
            config.profiles.insert(name.clone(), Profile { email, token, zone: zone.clone() });
            save_config(&config);
            if let Some(z) = zone {
                msg!("{} Added profile '{}' with zone '{}'", "✓".green(), name.cyan(), z);
            } else {
                msg!("{} Added profile '{}'", "✓".green(), name.cyan());
            }
        }

        Some(Commands::Remove { name }) => {
            let mut config = load_config();
            if config.profiles.remove(&name).is_some() {
                if config.current.as_ref() == Some(&name) {
                    config.current = None;
                }
                save_config(&config);
                msg!("{} Removed profile '{}'", "✓".green(), name);
            } else {
                msg!("{} Profile '{}' not found.", "Error:".red().bold(), name);
                std::process::exit(1);
            }
        }

        Some(Commands::Use { name }) => {
            let mut config = load_config();
            if !switch_to_profile(&mut config, &name) {
                std::process::exit(1);
            }
        }

        Some(Commands::Current) => {
            let config = load_config();
            match config.current {
                Some(name) => {
                    if let Some(profile) = config.profiles.get(&name) {
                        msg!("{} {} ({})", "ON".green().bold(), name.cyan(), profile.email);
                    } else {
                        msg!("{}", "Current profile no longer exists.".yellow());
                    }
                }
                None => {
                    msg!("{}", "No profile currently active.".yellow());
                }
            }
        }

        Some(Commands::Hook) => {
            let shell = detect_shell();
            msg!("Add this to your shell config:\n");
            match shell.as_str() {
                "fish" => {
                    msg!("# ~/.config/fish/config.fish");
                    msg!("function cfs");
                    msg!("    cf-switch $argv | source");
                    msg!("end");
                }
                _ => {
                    msg!("# ~/.bashrc or ~/.zshrc");
                    msg!("cfs() {{ eval \"$(cf-switch \"$@\")\"; }}");
                }
            }
        }

        Some(Commands::Purge { zone }) => {
            let config = load_config();
            match config.current {
                Some(name) => {
                    if let Some(profile) = config.profiles.get(&name) {
                        // Use provided zone or fall back to profile's default zone
                        let target_zone = zone.or_else(|| profile.zone.clone());

                        match target_zone {
                            Some(z) => {
                                msg!("{} Purging cache for {} using profile '{}'...", "→".cyan(), z.bold(), name.cyan());

                                let output = Command::new("flarectl")
                                    .env("CF_API_EMAIL", &profile.email)
                                    .env("CF_API_TOKEN", &profile.token)
                                    .env("CF_API_KEY", &profile.token)
                                    .args(["zone", "purge", "--zone", &z, "--everything"])
                                    .output();

                                match output {
                                    Ok(result) => {
                                        if result.status.success() {
                                            msg!("{} Cache purged for {}", "✓".green(), z.bold());
                                        } else {
                                            let stderr = String::from_utf8_lossy(&result.stderr);
                                            msg!("{} Failed to purge: {}", "Error:".red().bold(), stderr);
                                            std::process::exit(1);
                                        }
                                    }
                                    Err(e) => {
                                        msg!("{} Failed to run flarectl: {}", "Error:".red().bold(), e);
                                        msg!("Make sure flarectl is installed: brew install cloudflare/cloudflare/flarectl");
                                        std::process::exit(1);
                                    }
                                }
                            }
                            None => {
                                msg!("{} No zone specified and profile '{}' has no default zone.", "Error:".red().bold(), name);
                                msg!("Usage: cfs purge <zone> or set default zone with: cf-switch add <name> -e <email> -t <token> -z <zone>");
                                std::process::exit(1);
                            }
                        }
                    } else {
                        msg!("{}", "Current profile no longer exists.".yellow());
                        std::process::exit(1);
                    }
                }
                None => {
                    msg!("{}", "No profile currently active. Use 'cf-switch use <profile>' first.".yellow());
                    std::process::exit(1);
                }
            }
        }

        Some(Commands::AddLamderaApp { domain }) => {
            let config = load_config();
            match config.current {
                Some(name) => {
                    if let Some(profile) = config.profiles.get(&name) {
                        // Use provided domain or fall back to profile's default zone
                        let target_domain = domain.or_else(|| profile.zone.clone());

                        match target_domain {
                            Some(d) => {
                                msg!("{} Adding Lamdera DNS record for {} using profile '{}'...", "→".cyan(), d.bold(), name.cyan());

                                let output = Command::new("flarectl")
                                    .env("CF_API_EMAIL", &profile.email)
                                    .env("CF_API_TOKEN", &profile.token)
                                    .env("CF_API_KEY", &profile.token)
                                    .args(["dns", "create", "--zone", &d, "--type", "CNAME", "--name", "@", "--content", "apps.lamdera.app", "--proxy"])
                                    .output();

                                match output {
                                    Ok(result) => {
                                        if result.status.success() {
                                            msg!("{} DNS record created: {} -> apps.lamdera.app (proxied)", "✓".green(), d.bold());
                                            msg!("");
                                            msg!("{}", "Next step:".bold());
                                            msg!("DM Lamdera team with: https://{}/ and https://{}.lamdera.app/", d, d.replace('.', "-"));
                                        } else {
                                            let stderr = String::from_utf8_lossy(&result.stderr);
                                            let stdout = String::from_utf8_lossy(&result.stdout);
                                            if stderr.contains("already exists") || stdout.contains("already exists") {
                                                msg!("{} DNS record already exists for {}", "✓".yellow(), d.bold());
                                            } else {
                                                msg!("{} Failed to create DNS record: {}{}", "Error:".red().bold(), stderr, stdout);
                                                std::process::exit(1);
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        msg!("{} Failed to run flarectl: {}", "Error:".red().bold(), e);
                                        msg!("Make sure flarectl is installed: brew install cloudflare/cloudflare/flarectl");
                                        std::process::exit(1);
                                    }
                                }
                            }
                            None => {
                                msg!("{} No domain specified and profile '{}' has no default zone.", "Error:".red().bold(), name);
                                msg!("Usage: cfs add-lamdera-app <domain>");
                                std::process::exit(1);
                            }
                        }
                    } else {
                        msg!("{}", "Current profile no longer exists.".yellow());
                        std::process::exit(1);
                    }
                }
                None => {
                    msg!("{}", "No profile currently active. Use 'cf-switch use <profile>' first.".yellow());
                    std::process::exit(1);
                }
            }
        }
    }
}
