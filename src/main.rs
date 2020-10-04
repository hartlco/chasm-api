use actix_multipart::Multipart;
use actix_web::client::Client;
use actix_web::{get, web, App, HttpResponse, HttpServer, Responder, ResponseError, Result};

use std::fmt;
use std::str;

use futures::{StreamExt, TryStreamExt};

use chrono::{DateTime, Utc};

use serde::{Deserialize, Serialize};

use base64::encode;

#[derive(Debug, Clone)]
enum ChasmError {
    InvalidCommitRequest,
    InvalidCommitJSON,
}

impl fmt::Display for ChasmError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "invalid first item to double")
    }
}

#[derive(Deserialize, Serialize)]
struct CommitContent {
    message: String,
    content: String,
    path: String,
}

impl CommitContent {
    fn new(message: String, content: String, path: String) -> CommitContent {
        CommitContent {
            message: message,
            content: encode(content),
            path: path,
        }
    }

    fn new_from_image(message: String, content: Vec<u8>, path: String) -> CommitContent {
        CommitContent {
            message: message,
            content: encode(content),
            path: path,
        }
    }
}

#[derive(Deserialize, Serialize)]
struct PostContent {
    repo: String,
    access_token: String,
    postfolder: String,
    title: Option<String>,
    content: Vec<ContentPart>,
}

#[derive(Deserialize, Serialize)]
#[serde(tag = "type", content = "value")]
enum ContentPart {
    Header(String),
    Paragraph(String),
    Image(String),
}

#[derive(Deserialize, Serialize)]
struct CommitResponse {
    content: CommitResponseContent,
}

#[derive(Deserialize, Serialize)]
struct CommitResponseContent {
    download_url: String,
}

#[derive(Deserialize, Serialize)]
struct ImageUploadResponse {
    commit_response: CommitResponse,
    filename: String
}

async fn commit(
    repo: String,
    access_token: String,
    content: CommitContent,
) -> std::result::Result<CommitResponse, ChasmError> {
    let repository = repo;

    let post_url = format!(
        "https://api.github.com/repos/{}/contents/{}",
        repository, content.path
    );
    let token = access_token;

    let client = Client::default();

    let response = client
        .put(post_url)
        .header("User-Agent", "actix-web/3.0")
        .header("Authorization", format!("Bearer {}", token))
        .header("Content-Type", "application/json;charset=UTF-8")
        .send_body(serde_json::to_string(&content).unwrap_or("Empty".to_string()))
        .await;

    let mut successful_response;

    match response {
        Ok(response) => {
            successful_response = response;
        }
        Err(_) => {
            return Err(ChasmError::InvalidCommitRequest);
        }
    }

    successful_response
        .json()
        .await
        .map_err(|_| ChasmError::InvalidCommitJSON)
}

async fn post_content(content: web::Json<PostContent>) -> HttpResponse {
    let now: DateTime<Utc> = Utc::now();
    let date = now.format("%Y-%m-%dT%H:%M:%SZ");

    let mut body_string = "".to_string();

    let mut title_string = "".to_string();

    if let Some(title) = &content.title {
        title_string.push_str(&title);
    }

    for content_part in &content.content {
        match content_part {
            ContentPart::Header(text) => {
                let header_string = format!("## {}\n", text);
                body_string.push_str(&header_string);
            }
            ContentPart::Paragraph(text) => {
                let paragraph_string = format!("{}\n", text);
                body_string.push_str(&paragraph_string);
            }
            ContentPart::Image(filename) => {
                let image_string = format!("![]({})\n", filename);
                body_string.push_str(&image_string);
            }
        }
    }

    let content_string = format!(
        "+++\ntitle = \"{}\"\ndate = {}\n+++\n{}\n\n<!-- more -->",
        title_string, date, body_string
    );

    let c2 = CommitContent::new(
        "Add post".to_string(),
        content_string,
        format!("content/{}/index.md", &content.postfolder),
    );

    let commit_content = commit(
        content.repo.to_string(),
        content.access_token.to_string(),
        c2,
    )
    .await;

    match commit_content {
        Ok(_) => HttpResponse::Ok().json(content.0),
        Err(error) => HttpResponse::from_error(actix_web::error::ErrorBadRequest(error)),
    }
}

async fn upload_image(mut payload: Multipart) -> Result<web::Json<ImageUploadResponse>> {
    let mut repo: Option<String> = Option::None;
    let mut access_token: Option<String> = Option::None;
    let mut relative_filename: Option<String> = Option::None;
    let mut postfolder: Option<String> = Option::None;
    let mut image_vec: Option<Vec<u8>> = Option::None;

    while let Ok(Some(field)) = payload.try_next().await {
        let content_disposition = field.content_disposition();

        if let (Some(vec), Some(field_name)) = (vec_from(field).await, content_disposition) {
            if field_name.get_name() == Some("access_token") {
                access_token = Some(str::from_utf8(&vec).unwrap().to_string());
                continue;
            }
    
            if field_name.get_name() == Some("repo") {
                repo = Some(str::from_utf8(&vec).unwrap().to_string());
                continue;
            }

            if field_name.get_name() == Some("postfolder") {
                postfolder = Some(str::from_utf8(&vec).unwrap().to_string());
                continue;
            }
    
            if field_name.get_name() == Some("file") {
                let filename = field_name.get_filename().unwrap();
                relative_filename = Some(filename.to_string());
                image_vec = Some(vec);
    
                continue;
            }   
        }
    }

    // TODO: Fix unwrapping

    let filename = relative_filename.unwrap();
    let n1 = filename.to_string();
    let filepath = format!("content/{}/{}", postfolder.unwrap(), sanitize_filename::sanitize(&n1));

    let content = CommitContent::new_from_image(
        "Add image".to_string(),
        image_vec.unwrap(),
        filepath.to_string(),
    );

    let unwrapped_repo = repo.unwrap();
    let unwrapped_access_token = access_token.unwrap();
    let unwrapped_content = content;

    let response = commit(unwrapped_repo, unwrapped_access_token, unwrapped_content).await;

    let image_upload_response = ImageUploadResponse {
        commit_response: response.unwrap(),
        filename: filename,
    };

    Ok(web::Json(image_upload_response))
}

async fn vec_from(field: actix_multipart::Field) -> Option<Vec<u8>> {
    let mut vec = Vec::new();
    let mut field = field;

    while let Some(chunk) = field.next().await {
        if let Ok(data) = chunk {
            let vec_b = data.to_vec();
            vec.extend(vec_b);
        } else {
            return None
        }
    }

    return Some(vec)
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let url = "127.0.0.1:3000";
    println!("Running on: http://{}", url);

    HttpServer::new(|| {
        App::new()
            .service(web::resource("/post_content").route(web::post().to(post_content)))
            .service(web::resource("/upload_image").route(web::post().to(upload_image)))
    })
    .bind(url)?
    .run()
    .await
}
