mod graph; // Import our custom Microsoft Graph module

use std::fs;
use std::path::Path;
use serde::Deserialize;
use dotenv::dotenv;
use std::env;
use lettre::message::{Message, MultiPart, SinglePart, Attachment, header::ContentType};
use graph::GraphClient;

// -----------------------------------------------------------------------------
// DATA STRUCTURES: The State Ledger
// -----------------------------------------------------------------------------
#[derive(Deserialize, Debug)]
struct ManifestMetadata {
    system: String,
    version: String,
    taxonomy: Vec<String>,
}

#[derive(Deserialize, Debug)]
struct TemplateDef {
    id: String,
    category: String,
    name: String,
    description: String,
    body_file: String,
    assets: Vec<String>,
}

#[derive(Deserialize, Debug)]
struct Manifest {
    metadata: ManifestMetadata,
    templates: Vec<TemplateDef>,
}

// -----------------------------------------------------------------------------
// CORE EXECUTION ENGINE
// -----------------------------------------------------------------------------
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("INITIALIZING: service-email-template");
    println!("VENDOR: PointSav Digital Systems");
    println!("TARGET: Woodfine Management Corp.\n");

    // STEP 1: Load Physical Egress Identities
    dotenv::from_path("Identity.env").expect("FATAL: Failed to locate Identity.env ledger.");
    let target_mailbox = env::var("SYSTEM_TARGET_MAILBOX")
        .expect("FATAL: SYSTEM_TARGET_MAILBOX not defined in Identity.env");
    println!("=> [AUTH] Routing locked to: {}", target_mailbox);

    // STEP 2: Parse the State Ledger
    let manifest_raw = fs::read_to_string("manifest.json").expect("FATAL: Failed to read manifest.json");
    let manifest: Manifest = serde_json::from_str(&manifest_raw).expect("FATAL: Failed to deserialize JSON");
    println!("=> [LEDGER] Loaded {} templates.", manifest.templates.len());

    // STEP 3: Authenticate with Microsoft Graph
    println!("=> [NETWORK] Authenticating with Microsoft Graph API...");
    let graph = GraphClient::authenticate().await?;
    println!("=> [NETWORK] Authentication Successful. Silent Sync engaged.");

    // (Note: In a production run, we would dynamically resolve the M365 Folder IDs here. 
    // For this compiler loop, we assume the GraphClient handles the routing logic.)
    let placeholder_folder_id = "AAMkAGI2..."; 

    // STEP 4: The Compilation & Injection Loop
    for template in manifest.templates {
        // 4a. Generate the Global Unique Search Key
        let search_key = format!("[{}]", template.id.to_uppercase());
        let subject_line = format!("{} - {}", template.category, template.name);
        println!("   -> Compiling: {}", search_key);

        // 4b. Read the raw body and inject the Telemetry Header
        let raw_body = fs::read_to_string(&template.body_file).unwrap_or_else(|_| String::from("[BODY FILE MISSING]"));
        let telemetry_header = format!(
            "===============================\nKEY: {}\nSUBJ: {}\n===============================\n\n",
            search_key, subject_line
        );
        let final_body = format!("{}{}", telemetry_header, raw_body);

        // 4c. Construct the Base MIME Payload
        let mut email_builder = MultiPart::mixed().singlepart(
            SinglePart::builder()
                .header(ContentType::TEXT_PLAIN)
                .body(final_body)
        );

        // 4d. Parse and Attach Binary Assets (PDFs)
        for asset_path in template.assets {
            if let Ok(file_bytes) = fs::read(&asset_path) {
                let filename = Path::new(&asset_path).file_name().unwrap().to_str().unwrap().to_string();
                
                // We default to application/pdf for Woodfine legal/finance documents
                let attachment = Attachment::new(filename)
                    .body(file_bytes, ContentType::parse("application/pdf").unwrap()); 
                
                email_builder = email_builder.singlepart(attachment);
                println!("      + Attached: {}", asset_path);
            } else {
                println!("      ! WARNING: Asset missing at {}", asset_path);
            }
        }

        // 4e. Finalize Email Structure
        let email = Message::builder()
            .subject(&subject_line)
            .multipart(email_builder)
            .expect("FATAL: Failed to compile MIME payload");

        let eml_payload = email.formatted();

        // 4f. Execute the Graph API Silent Sync (Purge & Inject)
        println!("      [SYNC] Purging old versions from M365...");
        graph.purge_old_templates(&target_mailbox, placeholder_folder_id, &search_key).await?;
        
        println!("      [SYNC] Injecting new payload...");
        graph.inject_template(&target_mailbox, placeholder_folder_id, eml_payload).await?;
    }

    println!("\nEXECUTION COMPLETE. Templates deployed to M365.");
    Ok(())
}
