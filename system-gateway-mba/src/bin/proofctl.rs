use anyhow::{bail, Result};
use clap::{Parser, Subcommand};
use std::path::PathBuf;

use system_gateway_mba::{
    auth::compute_fingerprint,
    db::{add_user, disable_user, list_users, open_db, rotate_key},
    pairing::normalize,
    pairing_db::{get_by_code, list_pending, set_state, sweep_expired},
};

#[derive(Parser)]
#[command(
    name = "proofctl",
    about = "Manage os-console users and SSH keys",
    version
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    User {
        #[command(subcommand)]
        action: UserAction,
    },
    /// Manage connection requests (zero-jargon pairing)
    Pair {
        #[command(subcommand)]
        action: PairAction,
    },
}

#[derive(Subcommand)]
enum UserAction {
    /// Register a new user with their SSH public key
    Add {
        username: String,
        #[arg(long)]
        tenant: String,
        #[arg(long = "key-file")]
        key_file: PathBuf,
        #[arg(long, default_value = "editor")]
        role: String,
    },
    /// List all registered users
    List,
    /// Disable a user (revoke access)
    Disable { username: String },
    /// Replace a user's registered public key
    RotateKey {
        username: String,
        #[arg(long = "key-file")]
        key_file: PathBuf,
    },
}

#[derive(Subcommand)]
enum PairAction {
    /// Show pending connection requests
    List,
    /// Approve a connection request by its code
    Approve {
        /// The 8-character code shown on the user's screen (e.g. K7Q2-9XMT)
        code: String,
    },
    /// Decline a connection request
    Deny {
        /// The 8-character code
        code: String,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let conn = open_db()?;

    match cli.command {
        Command::User { action } => match action {
            UserAction::Add {
                username,
                tenant,
                key_file,
                role,
            } => {
                if !["pointsav", "woodfine"].contains(&tenant.as_str()) {
                    bail!("tenant must be 'pointsav' or 'woodfine'");
                }
                let key = russh::keys::load_public_key(&key_file)?;
                let fingerprint = compute_fingerprint(&key);
                add_user(&conn, &fingerprint, &username, &tenant, &role)?;
                println!("Added {username}@{tenant}  {fingerprint}");
            }
            UserAction::List => {
                let users = list_users(&conn)?;
                if users.is_empty() {
                    println!("No users registered.");
                } else {
                    println!(
                        "{:<20} {:<10} {:<10} {:<6} FINGERPRINT",
                        "USERNAME", "TENANT", "ROLE", "ACTIVE"
                    );
                    for (fp, name, tenant, role, active) in &users {
                        println!(
                            "{:<20} {:<10} {:<10} {:<6} {}",
                            name,
                            tenant,
                            role,
                            if *active { "yes" } else { "no" },
                            fp
                        );
                    }
                }
            }
            UserAction::Disable { username } => {
                let n = disable_user(&conn, &username)?;
                if n == 0 {
                    bail!("user '{}' not found or already disabled", username);
                }
                println!("Disabled {username}");
            }
            UserAction::RotateKey { username, key_file } => {
                let key = russh::keys::load_public_key(&key_file)?;
                let fingerprint = compute_fingerprint(&key);
                let n = rotate_key(&conn, &username, &fingerprint)?;
                if n == 0 {
                    bail!("user '{}' not found or not active", username);
                }
                println!("Updated key for {username}: {fingerprint}");
            }
        },

        Command::Pair { action } => match action {
            PairAction::List => {
                sweep_expired(&conn)?;
                let pending = list_pending(&conn)?;
                if pending.is_empty() {
                    println!("No pending connection requests.");
                } else {
                    println!("{:<12} {:<20} {:<10} REQUESTED", "CODE", "USER", "TENANT");
                    for (_, code, user, tenant, created) in &pending {
                        println!(
                            "{:<12} {:<20} {:<10} {}",
                            code,
                            user,
                            tenant,
                            &created[..19]
                        );
                    }
                }
            }
            PairAction::Approve { code } => {
                sweep_expired(&conn)?;
                let normalized = normalize(&code);
                match get_by_code(&conn, &normalized)? {
                    Some((request_id, username, tenant, fingerprint, _public_key)) => {
                        add_user(&conn, &fingerprint, &username, &tenant, "editor")?;
                        set_state(&conn, &request_id, "approved")?;
                        println!("Approved — {username}@{tenant} can now connect.");
                        println!("Fingerprint: {fingerprint}");
                    }
                    None => bail!("code '{}' not found or already used", code),
                }
            }
            PairAction::Deny { code } => {
                sweep_expired(&conn)?;
                let normalized = normalize(&code);
                match get_by_code(&conn, &normalized)? {
                    Some((request_id, username, _, _, _)) => {
                        set_state(&conn, &request_id, "denied")?;
                        println!("Declined connection request from {username}.");
                    }
                    None => bail!("code '{}' not found or already used", code),
                }
            }
        },
    }

    Ok(())
}
