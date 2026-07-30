use reqwest::Client;
use std::fs;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use regex::Regex;

const SPOOL_DIR: &str = "/opt/deployments/woodfine-fleet-deployment/cluster-totebox-personnel/service-email/personnel-maildir/new";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("========================================================");
    println!(" 💥 EWS SOAP HARVESTER (ABSOLUTE PADDING EGRESS)");
    println!("========================================================");

    let env_content = fs::read_to_string("/opt/deployments/woodfine-fleet-deployment/cluster-totebox-personnel/service-email/auth-credentials.env")
        .expect("[FATAL] Missing auth-credentials.env");
        
    let mut tenant = String::new();
    let mut client_id = String::new();
    let mut secret = String::new();
    let mut user = String::new();

    for line in env_content.lines() {
        if line.starts_with("AZURE_TENANT_ID=") { tenant = line.replace("AZURE_TENANT_ID=", "").replace("\"", ""); }
        if line.starts_with("AZURE_CLIENT_ID=") { client_id = line.replace("AZURE_CLIENT_ID=", "").replace("\"", ""); }
        if line.starts_with("AZURE_CLIENT_SECRET=") { secret = line.replace("AZURE_CLIENT_SECRET=", "").replace("\"", ""); }
        if line.starts_with("EXCHANGE_TARGET_USER=") { user = line.replace("EXCHANGE_TARGET_USER=", "").replace("\"", ""); }
    }

    let client = Client::new();
    let token = get_token(&client, &tenant, &client_id, &secret).await?;
    println!("[SYSTEM] EWS OAuth2 Token Negotiated.");

    let folders = vec![
        ("totebox-ingress", "AAMkAGNiMzVmMDMxLTY1OTEtNGQzNC05YzE2LTM2YWMyOWMwOTkyMgAuAAAAAABUZZ+cFXcyR6WM1RpB+73bAQDF6l4UzZZOR6tUM0g2iU/BAAmpzIZlAAA="),
    ];

    fs::create_dir_all(SPOOL_DIR)?;
    let mut grand_total = 0;

    for (folder_name, folder_id) in folders {
        println!("\n[SYSTEM] Probing EWS Folder: {}", folder_name);
        
        let find_xml = format!(
            r#"<?xml version="1.0" encoding="utf-8"?>
            <soap:Envelope xmlns:soap="http://schemas.xmlsoap.org/soap/envelope/" xmlns:t="http://schemas.microsoft.com/exchange/services/2006/types" xmlns:m="http://schemas.microsoft.com/exchange/services/2006/messages">
            <soap:Header><t:RequestServerVersion Version="Exchange2013" /><t:ExchangeImpersonation><t:ConnectingSID><t:PrimarySmtpAddress>{}</t:PrimarySmtpAddress></t:ConnectingSID></t:ExchangeImpersonation></soap:Header>
            <soap:Body><m:FindItem Traversal="Shallow"><m:ItemShape><t:BaseShape>IdOnly</t:BaseShape></m:ItemShape><m:ParentFolderIds><t:FolderId Id="{}"/></m:ParentFolderIds></m:FindItem></soap:Body>
            </soap:Envelope>"#, user, folder_id
        );

        let res = client.post("https://outlook.office365.com/EWS/Exchange.asmx")
            .header("Authorization", format!("Bearer {}", token))
            .header("Content-Type", "text/xml; charset=utf-8")
            .body(find_xml)
            .send().await?.text().await?;

        let re = Regex::new(r#"<t:ItemId Id="([^"]+)""#).unwrap();
        let item_ids: Vec<String> = re.captures_iter(&res).map(|cap| cap[1].to_string()).collect();

        if item_ids.is_empty() {
            println!("  -> [STATUS] Folder is mathematically empty.");
            if res.contains("ResponseClass=\"Error\"") {
                println!("  -> [SOAP FAULT DETECTED]:");
                let fault_re = Regex::new(r#"<m:MessageText>(.*?)</m:MessageText>"#).unwrap();
                if let Some(caps) = fault_re.captures(&res) {
                    println!("     {}", &caps[1]);
                } else {
                    println!("     {}", res);
                }
            }
            continue;
        }

        println!("  -> Extracting {} raw MIME payloads via EWS...", item_ids.len());

        for (idx, item_id) in item_ids.iter().enumerate() {
            let get_xml = format!(
                r#"<?xml version="1.0" encoding="utf-8"?>
                <soap:Envelope xmlns:soap="http://schemas.xmlsoap.org/soap/envelope/" xmlns:t="http://schemas.microsoft.com/exchange/services/2006/types" xmlns:m="http://schemas.microsoft.com/exchange/services/2006/messages">
                <soap:Header><t:RequestServerVersion Version="Exchange2013" /><t:ExchangeImpersonation><t:ConnectingSID><t:PrimarySmtpAddress>{}</t:PrimarySmtpAddress></t:ConnectingSID></t:ExchangeImpersonation></soap:Header>
                <soap:Body><m:GetItem><m:ItemShape><t:BaseShape>IdOnly</t:BaseShape><t:IncludeMimeContent>true</t:IncludeMimeContent></m:ItemShape><m:ItemIds><t:ItemId Id="{}"/></m:ItemIds></m:GetItem></soap:Body>
                </soap:Envelope>"#, user, item_id
            );

            let mime_res = client.post("https://outlook.office365.com/EWS/Exchange.asmx")
                .header("Authorization", format!("Bearer {}", token))
                .header("Content-Type", "text/xml; charset=utf-8")
                .body(get_xml)
                .send().await?.text().await?;

            let mime_re = Regex::new(r"(?s)<t:MimeContent[^>]*>(.*?)</t:MimeContent>").unwrap();
            if let Some(caps) = mime_re.captures(&mime_res) {
                let base64_mime = &caps[1];
                if let Ok(decoded_bytes) = STANDARD.decode(base64_mime) {
                    let local_file = format!("{}/NOSAVE_{}_{}.eml", SPOOL_DIR, folder_name, idx + 1);
                    fs::write(local_file, decoded_bytes)?;
                    grand_total += 1;
                }
            }
            
            let del_xml = format!(
                r#"<?xml version="1.0" encoding="utf-8"?>
                <soap:Envelope xmlns:soap="http://schemas.xmlsoap.org/soap/envelope/" xmlns:t="http://schemas.microsoft.com/exchange/services/2006/types" xmlns:m="http://schemas.microsoft.com/exchange/services/2006/messages">
                <soap:Header><t:RequestServerVersion Version="Exchange2013" /><t:ExchangeImpersonation><t:ConnectingSID><t:PrimarySmtpAddress>{}</t:PrimarySmtpAddress></t:ConnectingSID></t:ExchangeImpersonation></soap:Header>
                <soap:Body><m:DeleteItem DeleteType="HardDelete"><m:ItemIds><t:ItemId Id="{}"/></m:ItemIds></m:DeleteItem></soap:Body>
                </soap:Envelope>"#, user, item_id
            );
            client.post("https://outlook.office365.com/EWS/Exchange.asmx")
                .header("Authorization", format!("Bearer {}", token))
                .header("Content-Type", "text/xml; charset=utf-8")
                .body(del_xml).send().await?;
        }
    }

    println!("\n========================================================");
    println!("[SUCCESS] EWS BURN COMPLETE. Total Assets Extracted: {}", grand_total);
    Ok(())
}

async fn get_token(client: &Client, tenant: &str, client_id: &str, secret: &str) -> Result<String, Box<dyn std::error::Error>> {
    let url = format!("https://login.microsoftonline.com/{}/oauth2/v2.0/token", tenant);
    let params = [("client_id", client_id), ("scope", "https://outlook.office365.com/.default"), ("client_secret", secret), ("grant_type", "client_credentials")];
    let res = client.post(&url).form(&params).send().await?;
    let json: serde_json::Value = res.json().await?;
    Ok(json["access_token"].as_str().unwrap().to_string())
}
