// SPDX-License-Identifier: Apache-2.0 OR MIT

//! EWS SOAP client — replaces the former Microsoft Graph REST bridge.
//!
//! Sends SOAP requests to Exchange Web Services using a pre-acquired
//! bearer token. Protocol: Exchange Web Services 2016 over HTTPS.
//! Auth: OAuth 2.0 Bearer (`AZURE_ACCESS_TOKEN`).
//!
//! Every request includes an `ExchangeImpersonation` header per the
//! reference SOAP envelope in
//! `service-email-egress-ews/egress-roster/ews_payload.xml`.
//!
//! Three operations are implemented:
//!   - `find_unread_ids` — FindItem (Shallow, inbox, IsRead=false)
//!   - `get_mime`        — GetItem (IncludeMimeContent=true)
//!   - `mark_read`       — UpdateItem (IsRead=true)

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use reqwest::Client;

pub struct EwsClient {
    client: Client,
    token: String,
    endpoint: String,
    target_user: String,
}

impl EwsClient {
    pub fn new(token: String, endpoint: String, target_user: String) -> Self {
        Self {
            client: Client::new(),
            token,
            endpoint,
            target_user,
        }
    }

    fn soap_envelope(&self, body: &str) -> String {
        format!(
            r#"<?xml version="1.0" encoding="utf-8"?>
<soap:Envelope xmlns:soap="http://schemas.xmlsoap.org/soap/envelope/"
               xmlns:t="http://schemas.microsoft.com/exchange/services/2006/types"
               xmlns:m="http://schemas.microsoft.com/exchange/services/2006/messages">
  <soap:Header>
    <t:RequestServerVersion Version="Exchange2016" />
    <t:ExchangeImpersonation>
      <t:ConnectingSID>
        <t:PrimarySmtpAddress>{}</t:PrimarySmtpAddress>
      </t:ConnectingSID>
    </t:ExchangeImpersonation>
  </soap:Header>
  <soap:Body>
    {}
  </soap:Body>
</soap:Envelope>"#,
            self.target_user, body
        )
    }

    async fn post_soap(
        &self,
        action: &str,
        body: &str,
    ) -> Result<String, Box<dyn std::error::Error>> {
        let envelope = self.soap_envelope(body);
        let res = self
            .client
            .post(&self.endpoint)
            .bearer_auth(&self.token)
            .header("Content-Type", "text/xml; charset=utf-8")
            .header("SOAPAction", action)
            .body(envelope)
            .send()
            .await?;
        let status = res.status();
        let text = res.text().await?;
        if !status.is_success() {
            return Err(format!("EWS {action} HTTP {status}: {text}").into());
        }
        Ok(text)
    }

    /// Returns EWS ItemId strings for all unread inbox messages.
    pub async fn find_unread_ids(&self) -> Result<Vec<String>, Box<dyn std::error::Error>> {
        let body = r#"<m:FindItem Traversal="Shallow">
      <m:ItemShape>
        <t:BaseShape>IdOnly</t:BaseShape>
      </m:ItemShape>
      <m:Restriction>
        <t:IsEqualTo>
          <t:FieldURI FieldURI="message:IsRead" />
          <t:FieldURIOrConstant>
            <t:Constant Value="false" />
          </t:FieldURIOrConstant>
        </t:IsEqualTo>
      </m:Restriction>
      <m:ParentFolderIds>
        <t:DistinguishedFolderId Id="inbox" />
      </m:ParentFolderIds>
    </m:FindItem>"#;

        let response = self
            .post_soap(
                "http://schemas.microsoft.com/exchange/services/2006/messages/FindItem",
                body,
            )
            .await?;
        Ok(extract_item_ids(&response))
    }

    /// Fetches the raw MIME bytes for one item (the `.eml` content).
    pub async fn get_mime(
        &self,
        item_id: &str,
    ) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        let body = format!(
            r#"<m:GetItem>
      <m:ItemShape>
        <t:BaseShape>IdOnly</t:BaseShape>
        <t:IncludeMimeContent>true</t:IncludeMimeContent>
      </m:ItemShape>
      <m:ItemIds>
        <t:ItemId Id="{}" />
      </m:ItemIds>
    </m:GetItem>"#,
            item_id
        );

        let response = self
            .post_soap(
                "http://schemas.microsoft.com/exchange/services/2006/messages/GetItem",
                &body,
            )
            .await?;

        let b64 = extract_mime_content(&response)
            .ok_or("MimeContent element not found in EWS GetItem response")?;
        let bytes = BASE64
            .decode(b64.trim())
            .map_err(|e| format!("MimeContent base64 decode failed: {e}"))?;
        Ok(bytes)
    }

    /// Marks one item as read via EWS UpdateItem.
    pub async fn mark_read(
        &self,
        item_id: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let body = format!(
            r#"<m:UpdateItem MessageDisposition="SaveOnly" ConflictResolution="AlwaysOverwrite">
      <m:ItemChanges>
        <t:ItemChange>
          <t:ItemId Id="{}" />
          <t:Updates>
            <t:SetItemField>
              <t:FieldURI FieldURI="message:IsRead" />
              <t:Message>
                <t:IsRead>true</t:IsRead>
              </t:Message>
            </t:SetItemField>
          </t:Updates>
        </t:ItemChange>
      </m:ItemChanges>
    </m:UpdateItem>"#,
            item_id
        );

        self.post_soap(
            "http://schemas.microsoft.com/exchange/services/2006/messages/UpdateItem",
            &body,
        )
        .await?;
        Ok(())
    }
}

/// Extracts all EWS ItemId `Id` attribute values from a SOAP response.
/// Element form: `<t:ItemId Id="AAAA..." ChangeKey="BBBB..."/>`
fn extract_item_ids(xml: &str) -> Vec<String> {
    let mut ids = Vec::new();
    let marker = r#"<t:ItemId Id=""#;
    let mut rest = xml;
    while let Some(pos) = rest.find(marker) {
        let after = &rest[pos + marker.len()..];
        if let Some(end) = after.find('"') {
            ids.push(after[..end].to_string());
            rest = &after[end + 1..];
        } else {
            break;
        }
    }
    ids
}

/// Returns the text content of the first `<t:MimeContent>` element.
/// EWS encodes the full MIME body as base64 inside this element.
fn extract_mime_content(xml: &str) -> Option<&str> {
    let start_tag = "<t:MimeContent";
    let end_tag = "</t:MimeContent>";
    let tag_start = xml.find(start_tag)?;
    let content_start = xml[tag_start..].find('>')? + tag_start + 1;
    let end = xml.find(end_tag)?;
    if content_start >= end {
        return None;
    }
    Some(&xml[content_start..end])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_item_ids_parses_single_id() {
        let xml = r#"<t:Items><t:Message><t:ItemId Id="AAABBB123" ChangeKey="CK456"/></t:Message></t:Items>"#;
        let ids = extract_item_ids(xml);
        assert_eq!(ids, vec!["AAABBB123"]);
    }

    #[test]
    fn extract_item_ids_parses_multiple_ids() {
        let xml = r#"<t:ItemId Id="ID1" ChangeKey="CK1"/><t:ItemId Id="ID2" ChangeKey="CK2"/>"#;
        let ids = extract_item_ids(xml);
        assert_eq!(ids, vec!["ID1", "ID2"]);
    }

    #[test]
    fn extract_item_ids_empty_on_no_match() {
        assert!(
            extract_item_ids(
                "<soap:Fault><faultstring>Error</faultstring></soap:Fault>"
            )
            .is_empty()
        );
    }

    #[test]
    fn extract_mime_content_returns_base64_body() {
        let b64 = "SGVsbG8gV29ybGQ=";
        let xml = format!(r#"<t:MimeContent CharacterSet="UTF-8">{}</t:MimeContent>"#, b64);
        assert_eq!(extract_mime_content(&xml), Some(b64));
    }

    #[test]
    fn extract_mime_content_none_when_absent() {
        assert!(extract_mime_content("<soap:Body><Fault/></soap:Body>").is_none());
    }

    #[test]
    fn base64_mime_decode_round_trips() {
        let original = b"From: sender@example.com\r\nSubject: Test\r\n\r\nBody.";
        let b64 = BASE64.encode(original);
        let xml = format!("<t:MimeContent>{}</t:MimeContent>", b64);
        let extracted = extract_mime_content(&xml).unwrap();
        let decoded = BASE64.decode(extracted.trim()).unwrap();
        assert_eq!(decoded, original);
    }
}
