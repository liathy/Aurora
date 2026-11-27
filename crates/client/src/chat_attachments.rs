//! Steam Chat's begin, cloud upload, and commit flow. The commit delivers the
//! image to the conversation, so it must not be followed by a text message.

use std::io::{Cursor, Read};
use std::path::Path;
use std::time::Duration;

use image::{ImageFormat, ImageReader};
use model::{SteamId, steam_id_to_account};
use proto::{CEconGetTradeOfferAccessTokenRequest, CEconGetTradeOfferAccessTokenResponse};
use reqwest::multipart::Form;
use serde::Deserialize;

use crate::{ChatAttachment, ChatRecipient, market::Web};

pub(crate) const HOST: &str = "steam-chat.com";
const MAX_IMAGE_BYTES: usize = 30 * 1024 * 1024;

pub(crate) struct ChatImage {
    bytes: Vec<u8>,
    name: String,
    mime: &'static str,
    width: u32,
    height: u32,
    file_id: String,
}

pub(crate) fn read_image(
    attachment: ChatAttachment,
    steam_root: Option<&Path>,
    me: SteamId,
) -> Result<ChatImage, String> {
    let path = match attachment {
        ChatAttachment::Image(path) => path,
        ChatAttachment::LastScreenshot => {
            crate::ingame::last_screenshot(steam_root, steam_id_to_account(me))
                .ok_or("No local screenshots found.")?
                .path
        }
        ChatAttachment::TradeOfferUrl => return Err("Expected an image attachment.".into()),
    };
    let mut bytes = Vec::new();
    let file = std::fs::File::open(&path).map_err(|e| e.to_string())?;
    file.take((MAX_IMAGE_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("image");
    prepare_image(bytes, file_name)
}

fn prepare_image(bytes: Vec<u8>, file_name: &str) -> Result<ChatImage, String> {
    if bytes.len() > MAX_IMAGE_BYTES {
        return Err("Steam Chat images can be up to 30 MiB.".into());
    }
    let reader = ImageReader::new(Cursor::new(&bytes))
        .with_guessed_format()
        .map_err(|e| e.to_string())?;
    let mime = match reader.format() {
        Some(ImageFormat::Png) => "image/png",
        Some(ImageFormat::Jpeg) => "image/jpeg",
        Some(ImageFormat::Gif) => "image/gif",
        Some(ImageFormat::WebP) => "image/webp",
        _ => return Err("Choose a PNG, JPEG, GIF, or WebP image.".into()),
    };
    let (width, height) = reader.into_dimensions().map_err(|e| e.to_string())?;
    let file_id = auth::random_hex(20);
    let file_name: String = file_name
        .chars()
        .filter(|c| !c.is_control())
        .take(160)
        .collect();
    Ok(ChatImage {
        bytes,
        name: format!("{file_id}_{file_name}"),
        mime,
        width,
        height,
        file_id,
    })
}

impl ChatImage {
    fn form(&self, web: &Web, language: &str) -> Form {
        Form::new()
            .text("sessionid", web.session_id.clone())
            .text("l", language.to_owned())
            .text("file_name", self.name.clone())
            .text("file_sha", self.file_id.clone())
            .text("file_type", self.mime)
            .text("file_image_width", self.width.to_string())
            .text("file_image_height", self.height.to_string())
    }
}

#[derive(Deserialize)]
struct Begin {
    result: UploadTarget,
    timestamp: serde_json::Value,
    hmac: String,
}

#[derive(Deserialize)]
struct UploadTarget {
    use_https: bool,
    url_host: String,
    url_path: String,
    ugcid: serde_json::Value,
    request_headers: Vec<UploadHeader>,
}

#[derive(Deserialize)]
struct UploadHeader {
    name: String,
    value: String,
}

fn decimal(value: &serde_json::Value) -> Result<String, String> {
    let number = match value {
        serde_json::Value::String(s) => s.parse::<u64>().ok(),
        serde_json::Value::Number(n) => n.as_u64(),
        _ => None,
    };
    number
        .map(|n| n.to_string())
        .ok_or("Invalid upload metadata from Steam.".into())
}

pub(crate) async fn upload(
    web: &Web,
    recipient: ChatRecipient,
    mut image: ChatImage,
) -> Result<(), String> {
    let size = image.bytes.len();
    let begin: Begin = web
        .http
        .post(format!("https://{HOST}/chat/beginfileupload/"))
        .query(&[("l", "english")])
        .multipart(
            image
                .form(web, "english")
                .text("file_size", size.to_string()),
        )
        .send()
        .await
        .map_err(|_| "Couldn't contact Steam Chat.")?
        .error_for_status()
        .map_err(|e| format!("Steam Chat couldn't start the upload ({e})."))?
        .json()
        .await
        .map_err(|_| "Steam Chat didn't return upload details.")?;

    let ugcid = decimal(&begin.result.ugcid)?;
    let timestamp = decimal(&begin.timestamp)?;
    let sent = put_image(&begin.result, std::mem::take(&mut image.bytes)).await;
    let commit = image
        .form(web, "english")
        .text("success", if sent.is_ok() { "1" } else { "0" })
        .text("ugcid", ugcid)
        .text("timestamp", timestamp)
        .text("hmac", begin.hmac)
        .text("spoiler", "0");
    let commit = match recipient {
        ChatRecipient::Friend(to) => commit.text("friend_steamid", to.to_string()),
        ChatRecipient::Group { group_id, chat_id } => commit
            .text("chat_group_id", group_id.to_string())
            .text("chat_id", chat_id.to_string()),
    };
    let response = web
        .http
        .post(format!("https://{HOST}/chat/commitfileupload/"))
        .multipart(commit)
        .send()
        .await;
    sent?;
    let response = response
        .map_err(|_| "Couldn't confirm the attachment with Steam Chat.")?
        .error_for_status()
        .map_err(|e| format!("Steam Chat couldn't deliver the attachment ({e})."))?;
    if let Ok(body) = response.json::<serde_json::Value>().await {
        if matches!(body.get("success"), Some(result) if result == false || result == 0) {
            return Err("Steam Chat rejected the attachment.".into());
        }
    }
    Ok(())
}

async fn put_image(target: &UploadTarget, bytes: Vec<u8>) -> Result<(), String> {
    if !target.use_https
        || target.url_host.contains(['/', '\\', '?', '#', '@'])
        || !target.url_path.starts_with('/')
    {
        return Err("Steam Chat returned an invalid storage URL.".into());
    }
    let url = reqwest::Url::parse(&format!("https://{}{}", target.url_host, target.url_path))
        .map_err(|_| "Steam Chat returned an invalid storage URL.")?;
    // Storage uses its own signed headers. Keep it separate from the Steam login session.
    let storage = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(120))
        .build()
        .map_err(|e| e.to_string())?;
    let mut request = storage.put(url).body(bytes);
    for h in &target.request_headers {
        if !h.name.eq_ignore_ascii_case("host") && !h.name.eq_ignore_ascii_case("content-length") {
            request = request.header(h.name.as_str(), h.value.as_str());
        }
    }
    let response = request
        .send()
        .await
        .map_err(|_| "Couldn't upload the image to Steam storage.")?;
    if !response.status().is_success() {
        return Err(format!(
            "Steam storage rejected the upload ({}).",
            response.status()
        ));
    }
    Ok(())
}

pub(crate) async fn trade_offer_url(conn: &net::Connection, me: SteamId) -> Result<String, String> {
    let response: CEconGetTradeOfferAccessTokenResponse = conn
        .call(
            "Econ.GetTradeOfferAccessToken#1",
            &CEconGetTradeOfferAccessTokenRequest::default(),
        )
        .await
        .map_err(|e| e.to_string())?;
    let token = response.trade_offer_access_token();
    if token.is_empty() {
        return Err("Steam didn't return a trade offer token.".into());
    }
    Ok(format!(
        "https://steamcommunity.com/tradeoffer/new/?partner={}&token={}",
        steam_id_to_account(me),
        crate::market::encode(token),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_large_steam_upload_ids_exact() {
        assert_eq!(
            decimal(&serde_json::json!("18446744073709551615")).unwrap(),
            "18446744073709551615"
        );
        assert_eq!(
            decimal(&serde_json::json!(u64::MAX)).unwrap(),
            "18446744073709551615"
        );
        assert!(decimal(&serde_json::json!(1.5)).is_err());
    }

    #[test]
    fn detects_gif_from_bytes_regardless_of_name() {
        let mut encoded = Vec::new();
        image::codecs::gif::GifEncoder::new(&mut encoded)
            .encode(&[0, 0, 0, 255], 1, 1, image::ExtendedColorType::Rgba8)
            .unwrap();
        let image = prepare_image(encoded.clone(), "test.png").unwrap();
        assert_eq!(image.mime, "image/gif");
        assert_eq!((image.width, image.height), (1, 1));
        assert_eq!(image.bytes, encoded);
        assert!(prepare_image(b"not an image".to_vec(), "test.png").is_err());
    }
}
