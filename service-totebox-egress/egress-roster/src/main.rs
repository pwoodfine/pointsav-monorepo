use std::env;
use std::fs::OpenOptions;
use std::io::Write;
use reqwest::Client;
use serde_json::Value;

fn extract_emails(recipients: &Value) -> String {
    if let Some(arr) = recipients.as_array() {
        let emails: Vec<&str> = arr.iter()
            .filter_map(|r| r["emailAddress"]["address"].as_str())
            .collect();
        emails.join(";")
    } else {
        String::new()
    }
}

fn sanitize_csv(text: &str) -> String {
    text.replace(",", " ").replace("\n", " ").replace("\r", " ")
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("SYSTEM EVENT: Initializing PointSav Maximized Relational Roster (V2 with Folder Tracking).");

    let target_user = env::var("EXCHANGE_TARGET_USER").expect("FATAL: EXCHANGE_TARGET_USER missing.");
    let access_token = env::var("AZURE_ACCESS_TOKEN").expect("FATAL: AZURE_ACCESS_TOKEN missing.");
    let archive_owner = env::var("ARCHIVE_OWNER").expect("FATAL: ARCHIVE_OWNER missing.");
    
    let roster_path = "../data-ledgers/personnel_roster.csv";
    let client = Client::new();
    
    let mut current_url = format!(
        "https://graph.microsoft.com/v1.0/users/{}/mailFolders/ArchiveMsgFolderRoot/messages?$select=id,conversationId,parentFolderId,receivedDateTime,subject,sender,toRecipients,ccRecipients,hasAttachments,importance,isRead&$top=500",
        target_user
    );

    let mut file = OpenOptions::new().create(true).write(true).truncate(true).open(roster_path)?;
    writeln!(file, "ArchiveOwner,FolderID,MessageID,ConversationID,DateReceived,SenderName,SenderEmail,ToEmails,CcEmails,HasAttachments,Importance,IsRead,Subject")?;

    println!("SYSTEM EVENT: Extracting maximized relational metadata from {}'s Archive...", archive_owner);

    let mut extracted_count = 0;

    loop {
        let res = client.get(&current_url).bearer_auth(&access_token).send().await?;
        if !res.status().is_success() {
            eprintln!("SYSTEM ERROR: Failed to fetch archive messages: {}", res.status());
            break;
        }

        let body = res.json::<Value>().await?;
        if let Some(messages) = body["value"].as_array() {
            for msg in messages {
                let folder_id = msg["parentFolderId"].as_str().unwrap_or("UNKNOWN_FOLDER");
                let msg_id = msg["id"].as_str().unwrap_or("UNKNOWN_ID");
                let conv_id = msg["conversationId"].as_str().unwrap_or("UNKNOWN_CONV");
                let date = msg["receivedDateTime"].as_str().unwrap_or("UNKNOWN_DATE");
                let subject = sanitize_csv(msg["subject"].as_str().unwrap_or("NO_SUBJECT"));
                
                let sender_name = sanitize_csv(msg["sender"]["emailAddress"]["name"].as_str().unwrap_or("UNKNOWN_NAME"));
                let sender_email = msg["sender"]["emailAddress"]["address"].as_str().unwrap_or("UNKNOWN_EMAIL");
                
                let to_emails = extract_emails(&msg["toRecipients"]);
                let cc_emails = extract_emails(&msg["ccRecipients"]);
                
                let has_attachments = msg["hasAttachments"].as_bool().unwrap_or(false);
                let importance = msg["importance"].as_str().unwrap_or("normal");
                let is_read = msg["isRead"].as_bool().unwrap_or(true);

                writeln!(
                    file, 
                    "{},{},{},{},{},{},{},{},{},{},{},{},{}", 
                    archive_owner, folder_id, msg_id, conv_id, date, sender_name, sender_email, to_emails, cc_emails, has_attachments, importance, is_read, subject
                )?;
                extracted_count += 1;
            }
            println!("SYSTEM EVENT: Processed batch... Total metadata records extracted: {}", extracted_count);
        }

        if let Some(next_link) = body["@odata.nextLink"].as_str() {
            current_url = next_link.to_string();
        } else {
            break;
        }
    }

    println!("SYSTEM EVENT: Maximized Archive Roster written to {}. Total records: {}", roster_path, extracted_count);
    Ok(())
}
