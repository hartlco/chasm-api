use std::env;

use actix_multipart::Multipart;
use actix_web::client::Client;
use actix_web::{web, App, HttpResponse, HttpServer, Result};

use std::str;

use futures::{StreamExt, TryStreamExt};

use std::fs;
use std::fs::File;
use std::io;
use std::io::prelude::*;
use std::path::Path;

mod models;

async fn commit(
    repo: String,
    access_token: String,
    content: models::CommitContent,
) -> std::result::Result<models::CommitResponse, models::ChasmError> {
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
            return Err(models::ChasmError::InvalidCommitRequest);
        }
    }

    successful_response
        .json()
        .await
        .map_err(|_| models::ChasmError::InvalidCommitJSON)
}

async fn commit_image(mut payload: Multipart) -> std::result::Result<models::ImageUploadResponse, models::ChasmError> {
    let mut repo: Option<String> = Option::None;
    let mut access_token: Option<String> = Option::None;
    let mut relative_filename: Option<String> = Option::None;
    let mut postfolder: Option<String> = Option::None;
    let mut image_vec: Option<Vec<u8>> = Option::None;
    let mut local_path: Option<String> = Option::None;

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

            if field_name.get_name() == Some("local_path") {
                local_path = String::from_utf8(vec).ok();
                continue;
            }

            if field_name.get_name() == Some("postfolder") {
                postfolder = String::from_utf8(vec).ok();
                continue;
            }
    
            if field_name.get_name() == Some("file") {
                let filename = field_name.get_filename().ok_or(models::ChasmError::FilenameMissing)?;
                relative_filename = Some(filename.to_string());
                image_vec = Some(vec);
    
                continue;
            }   
        }
    }

    let filename = relative_filename.ok_or(models::ChasmError::FilenameMissing)?.to_string();
    let postfolder = postfolder.ok_or(models::ChasmError::PostfolderMissing)?;
    let filepath = format!("content/{}/{}", postfolder, sanitize_filename::sanitize(&filename));
    let image_vec = image_vec.ok_or(models::ChasmError::ImageDataMissing)?;

    if let Some(local_path) = local_path {
        let full_path = format!("{}/{}", local_path, filepath);
        println!("Local save: {}", full_path.to_string());
        let _ = write_file(&full_path, image_vec);

        let response = models::ImageUploadResponse {
            commit_response: None,
            filename: filename,
        };

        Ok(response)
    } else {
        let content = models::CommitContent::new_from_image(
            "Add image".to_string(),
            image_vec,
            filepath.to_string(),
        );
    
        let repo = repo.ok_or(models::ChasmError::RepoMissing)?;
        let access_token = access_token.ok_or(models::ChasmError::AccessTokenMissing)?;
    
        let response = commit(repo, access_token, content).await?;
    
        let image_upload_response = models::ImageUploadResponse {
            commit_response: Some(response),
            filename: filename,
        };
    
        Ok(image_upload_response)
    }
}

async fn post_content(content: web::Json<models::PostContent>) -> HttpResponse {
    let mut body_string = "".to_string();

    let mut title_string = "".to_string();

    if let Some(title) = &content.title {
        title_string.push_str(&title);
    }

    for content_part in &content.content {
        match content_part {
            models::ContentPart::Header { text }  => {
                let header_string = format!("## {}\n", text);
                body_string.push_str(&header_string);
            }
            models::ContentPart::Paragraph { text } => {
                let paragraph_string = format!("{}\n", text);
                body_string.push_str(&paragraph_string);
            }
            models::ContentPart::Image { filename } => {
                let image_string = format!("![]({})\n", filename);
                body_string.push_str(&image_string);
            }
            models::ContentPart::Link { title, url } => {
                let image_string = format!("[{}]({})\n", title, url);
                body_string.push_str(&image_string);
            }
        }
    }

    let content_string = format!(
        "---\ntitle: \"{}\"\ndate: {}\n---\n{}\n",
        title_string, &content.date.format("%Y-%m-%dT%H:%M:%SZ"), body_string
    );

    let file_path = format!("content/{}/index.md", &content.postfolder);

    match &content.location {
        models::ContentLocation::Github { repo, access_token } => {
            let commit_content = models::CommitContent::new(
                "Add post".to_string(),
                content_string,
                file_path,
            );
        
            let commit_response = commit(
                repo.to_string(),
                access_token.to_string(),
                commit_content,
            )
            .await;
        
            match commit_response {
                Ok(_) => HttpResponse::Ok().json(content.0),
                Err(error) => HttpResponse::from_error(actix_web::error::ErrorBadRequest(error)),
            }
        }
        models::ContentLocation::Local { path } => {
            let full_path = format!("{}/{}", path, file_path);
            println!("Local save: {}", full_path.to_string());
            let _ = write_file(&full_path, content_string.as_bytes().to_vec());
            HttpResponse::Ok().json(content.0)
        }
    }
}

async fn upload_image(payload: Multipart) -> Result<web::Json<models::ImageUploadResponse>> {
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

fn write_file(path_string: &str, content: Vec<u8>) -> Result<(), io::Error> {
    let path = Path::new(path_string);
    let parent = path.parent().unwrap();
    fs::create_dir_all(parent)?;
    let mut file = File::create(&path).unwrap();
    file.write_all(&content)?;

    Ok(())
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
