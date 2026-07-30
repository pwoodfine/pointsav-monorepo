use reqwest::Client;
use serde_json::Value;
use std::env;
use std::fs;
use regex::Regex;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("SYSTEM EVENT: Initializing EWS Archive Rescue Protocol (SAFE MODE)...");

    let tenant_id = env::var("AZURE_TENANT_ID").expect("FATAL: AZURE_TENANT_ID missing.");
    let client_id = env::var("AZURE_CLIENT_ID").expect("FATAL: AZURE_CLIENT_ID missing.");
    let secret = env::var("AZURE_CLIENT_SECRET").expect("FATAL: AZURE_CLIENT_SECRET missing.");
    let target_user = env::var("EXCHANGE_TARGET_USER").expect("FATAL: EXCHANGE_TARGET_USER missing.");

    let vault_dir = "/assets/template-vault";
    fs::create_dir_all(vault_dir).unwrap_or_default();

    let client = Client::new();

    // Negotiate Legacy OAuth Token
    let token_url = format!("https://login.microsoftonline.com/{}/oauth2/v2.0/token", tenant_id);
    let params = [
        ("client_id", client_id.as_str()),
        ("scope", "https://outlook.office365.com/.default"),
        ("client_secret", secret.as_str()),
        ("grant_type", "client_credentials"),
    ];
    
    let res = client.post(&token_url).form(&params).send().await?.json::<Value>().await?;
    let access_token = res["access_token"].as_str().expect("FATAL: Failed to secure EWS token.");

    let ews_url = "https://outlook.office365.com/EWS/Exchange.asmx";

    // STAGE 1: Find Folders
    let find_folder_xml = format!(r#"<?xml version="1.0" encoding="utf-8"?>
<soap:Envelope xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xmlns:m="http://schemas.microsoft.com/exchange/services/2006/messages" xmlns:t="http://schemas.microsoft.com/exchange/services/2006/types" xmlns:soap="http://schemas.xmlsoap.org/soap/envelope/">
  <soap:Header>
    <t:RequestServerVersion Version="Exchange2013" />
    <t:ExchangeImpersonation><t:ConnectingSID><t:PrimarySmtpAddress>{}</t:PrimarySmtpAddress></t:ConnectingSID></t:ExchangeImpersonation>
  </soap:Header>
  <soap:Body>
    <m:FindFolder Traversal="Shallow">
      <m:FolderShape>
        <t:BaseShape>IdOnly</t:BaseShape>
        <t:AdditionalProperties><t:FieldURI FieldURI="folder:DisplayName" /></t:AdditionalProperties>
      </m:FolderShape>
      <m:ParentFolderIds><t:DistinguishedFolderId Id="archivemsgfolderroot" /></m:ParentFolderIds>
    </m:FindFolder>
  </soap:Body>
</soap:Envelope>"#, target_user);

    let folder_res = client.post(ews_url)
        .bearer_auth(access_token)
        .header("Content-Type", "text/xml; charset=utf-8")
        .body(find_folder_xml)
        .send().await?.text().await?;

    let folder_re = Regex::new(r#"(?s)<t:FolderId Id="([^"]+)".*?<t:DisplayName>([^<]+)</t:DisplayName>"#).unwrap();
    let mut folders_processed = 0;

    for cap in folder_re.captures_iter(&folder_res) {
        let folder_id = &cap[1];
        let folder_name = &cap[2];

        // Algorithmic Targeting: Strict focus on Temp* folders only
        if folder_name.starts_with("Temp") {
            folders_processed += 1;
            println!("SYSTEM EVENT: Locked onto Archive Target: {}", folder_name);

            // STAGE 2: Find Items
            let find_item_xml = format!(r#"<?xml version="1.0" encoding="utf-8"?>
<soap:Envelope xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xmlns:m="http://schemas.microsoft.com/exchange/services/2006/messages" xmlns:t="http://schemas.microsoft.com/exchange/services/2006/types" xmlns:soap="http://schemas.xmlsoap.org/soap/envelope/">
  <soap:Header>
    <t:RequestServerVersion Version="Exchange2013" />
    <t:ExchangeImpersonation><t:ConnectingSID><t:PrimarySmtpAddress>{}</t:PrimarySmtpAddress></t:ConnectingSID></t:ExchangeImpersonation>
  </soap:Header>
  <soap:Body>
    <m:FindItem Traversal="Shallow">
      <m:ItemShape><t:BaseShape>IdOnly</t:BaseShape></m:ItemShape>
      <m:ParentFolderIds><t:FolderId Id="{}" /></m:ParentFolderIds>
    </m:FindItem>
  </soap:Body>
</soap:Envelope>"#, target_user, folder_id);

            let item_res = client.post(ews_url)
                .bearer_auth(access_token)
                .header("Content-Type", "text/xml; charset=utf-8")
                .body(find_item_xml)
                .send().await?.text().await?;

            let item_re = Regex::new(r#"<t:ItemId Id="([^"]+)""#).unwrap();
            
            for item_cap in item_re.captures_iter(&item_res) {
                let item_id = &item_cap[1];

                // STAGE 3: Extract Item Details (Force Plain Text)
                let get_item_xml = format!(r#"<?xml version="1.0" encoding="utf-8"?>
<soap:Envelope xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xmlns:m="http://schemas.microsoft.com/exchange/services/2006/messages" xmlns:t="http://schemas.microsoft.com/exchange/services/2006/types" xmlns:soap="http://schemas.xmlsoap.org/soap/envelope/">
  <soap:Header>
    <t:RequestServerVersion Version="Exchange2013" />
    <t:ExchangeImpersonation><t:ConnectingSID><t:PrimarySmtpAddress>{}</t:PrimarySmtpAddress></t:ConnectingSID></t:ExchangeImpersonation>
  </soap:Header>
  <soap:Body>
    <m:GetItem>
      <m:ItemShape>
        <t:BaseShape>Default</t:BaseShape>
        <t:BodyType>Text</t:BodyType>
      </m:ItemShape>
      <m:ItemIds><t:ItemId Id="{}" /></m:ItemIds>
    </m:GetItem>
  </soap:Body>
</soap:Envelope>"#, target_user, item_id);

                let detail_res = client.post(ews_url)
                    .bearer_auth(access_token)
                    .header("Content-Type", "text/xml; charset=utf-8")
                    .body(get_item_xml)
                    .send().await?.text().await?;

                let subj_re = Regex::new(r#"(?s)<t:Subject>(.*?)</t:Subject>"#).unwrap();
                let body_re = Regex::new(r#"(?s)<t:Body BodyType="Text">(.*?)</t:Body>"#).unwrap();

                let subject = subj_re.captures(&detail_res).map_or("UNTITLED", |c| c.get(1).unwrap().as_str());
                let body = body_re.captures(&detail_res).map_or("", |c| c.get(1).unwrap().as_str());

                let safe_subject = subject.replace(|c: char| !c.is_alphanumeric(), "_");
                let file_path = format!("{}/ARCHIVE_{}_{}.txt", vault_dir, folder_name.replace(" ", ""), safe_subject);

                let payload = format!("ARCHIVE_FOLDER: {}\nSUBJECT: {}\n---\n{}", folder_name, subject, body);
                match fs::write(&file_path, payload) {
                    Ok(_) => println!("  -> Extracted: {}", subject),
                    Err(e) => println!("  -> SYSTEM ERROR: Failed to write {}: {}", subject, e),
                }
            }
        }
    }

    println!("SYSTEM EVENT: EWS Archive Rescue Complete. Processed {} Archive Folders.", folders_processed);
    Ok(())
}
