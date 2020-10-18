use std::env;

use actix_multipart::Multipart;
use actix_web::client::Client;
use actix_web::{web, App, HttpResponse, HttpServer, Result};

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
    FilenameMissing,
    PostfolderMissing,
    ImageDataMissing,
    RepoMissing,
    AccessTokenMissing,
}

impl fmt::Display for ChasmError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ChasmError::InvalidCommitRequest => write!(f, "Invalid Commit Request"),
            ChasmError::InvalidCommitJSON => write!(f, "Invalid Commit JSON"),
            ChasmError::FilenameMissing => write!(f, "Filename missing"),
            ChasmError::PostfolderMissing => write!(f, "Postfolder missing"),
            ChasmError::ImageDataMissing => write!(f, "Image Data Missing"),
            ChasmError::RepoMissing => write!(f, "Repo Missng"),
            ChasmError::AccessTokenMissing => write!(f, "Access Token Missing"),
        }
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
    date: DateTime<Utc>,
    postfolder: String,
    title: Option<String>,
    content: Vec<ContentPart>,
}

#[derive(Deserialize, Serialize)]
#[serde(tag = "type")]
enum ContentPart {
    Header {text: String},
    Paragraph {text: String},
    Image {filename: String},
    Link {title: String, url: String},
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
            println!("GitHub Commit Response: {}", response.status());
            successful_response = response;
        }
        Err(error) => {
            println!("{}", error);
            return Err(ChasmError::InvalidCommitRequest);
        }
    }

    successful_response
        .json()
        .await
        .map_err(|_| ChasmError::InvalidCommitJSON)
}

async fn commit_image(mut payload: Multipart) -> std::result::Result<ImageUploadResponse, ChasmError> {
    let mut repo: Option<String> = Option::None;
    let mut access_token: Option<String> = Option::None;
    let mut relative_filename: Option<String> = Option::None;
    let mut postfolder: Option<String> = Option::None;
    let mut image_vec: Option<Vec<u8>> = Option::None;

    while let Ok(Some(field)) = payload.try_next().await {
        let content_disposition = field.content_disposition();

        if let (Some(vec), Some(field_name)) = (vec_from(field).await, content_disposition) {
            if field_name.get_name() == Some("access_token") {
                access_token = String::from_utf8(vec).ok();
                continue;
            }
    
            if field_name.get_name() == Some("repo") {
                repo = String::from_utf8(vec).ok();
                continue;
            }

            if field_name.get_name() == Some("postfolder") {
                postfolder = String::from_utf8(vec).ok();
                continue;
            }
    
            if field_name.get_name() == Some("file") {
                let filename = field_name.get_filename().ok_or(ChasmError::FilenameMissing)?;
                relative_filename = Some(filename.to_string());
                image_vec = Some(vec);
    
                continue;
            }   
        }
    }

    let filename = relative_filename.ok_or(ChasmError::FilenameMissing)?.to_string();
    let postfolder = postfolder.ok_or(ChasmError::PostfolderMissing)?;
    let filepath = format!("content/{}/{}", postfolder, sanitize_filename::sanitize(&filename));
    let image_vec = image_vec.ok_or(ChasmError::ImageDataMissing)?;

    let content = CommitContent::new_from_image(
        "Add image".to_string(),
        image_vec,
        filepath.to_string(),
    );

    let repo = repo.ok_or(ChasmError::RepoMissing)?;
    let access_token = access_token.ok_or(ChasmError::AccessTokenMissing)?;

    let response = commit(repo, access_token, content).await?;

    let image_upload_response = ImageUploadResponse {
        commit_response: response,
        filename: filename,
    };

    Ok(image_upload_response)
}

async fn post_content(content: web::Json<PostContent>) -> HttpResponse {
    let mut body_string = "".to_string();

    let mut title_string = "".to_string();

    if let Some(title) = &content.title {
        title_string.push_str(&title);
    }

    for content_part in &content.content {
        match content_part {
            ContentPart::Header { text }  => {
                let header_string = format!("## {}\n", text);
                body_string.push_str(&header_string);
            }
            ContentPart::Paragraph { text } => {
                let paragraph_string = format!("{}\n", text);
                body_string.push_str(&paragraph_string);
            }
            ContentPart::Image { filename } => {
                let image_string = format!("![]({})\n", filename);
                body_string.push_str(&image_string);
            }
            ContentPart::Link { title, url } => {
                let image_string = format!("[{}]({})\n", title, url);
                body_string.push_str(&image_string);
            }
        }
    }

    let content_string = format!(
        "+++\ntitle = \"{}\"\ndate = {}\n+++\n{}\n\n<!-- more -->",
        title_string, &content.date.format("%Y-%m-%dT%H:%M:%SZ"), body_string
    );

    let commit_content = CommitContent::new(
        "Add post".to_string(),
        content_string,
        format!("content/{}/index.md", &content.postfolder),
    );

    let commit_response = commit(
        content.repo.to_string(),
        content.access_token.to_string(),
        commit_content,
    )
    .await;

    match commit_response {
        Ok(_) => HttpResponse::Ok().json(content.0),
        Err(error) => HttpResponse::from_error(actix_web::error::ErrorBadRequest(error)),
    }
}

async fn upload_image(payload: Multipart) -> Result<web::Json<ImageUploadResponse>> {
    let response = commit_image(payload).await;

    match response {
        Ok(response) => {
            Ok(web::Json(response))
        }
        Err(error) => {
            Err(actix_web::error::ErrorBadRequest(error))
        }
    }
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
    let port = env::var("PORT").unwrap_or("3000".to_string());

    let url = format!("0.0.0.0:{}", port);
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
